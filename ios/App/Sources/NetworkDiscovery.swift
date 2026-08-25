import Foundation
import Network
import UIKit
import FamiliarMesh

/// The device's actor namespace as the familiar sees it: `phone:<human>` on iPhone, `ipad:<human>`
/// on iPad. The observations a device reports are tagged with this so the familiar (and its peers)
/// know which device — and which human — on the mesh saw what.
///
/// The human suffix is NOT baked (ADR-0016): a node serves whoever is present, and a shared device
/// changes hands. `human` is set from `AppModel.servedHuman`; it defaults to "observer" so a device
/// never falsely claims a specific person before it has been told who is using it.
/// Surveys the local network by Bonjour/mDNS and reports what it finds to the familiar as *derived*
/// observations — "this device saw a thing of kind X advertising as Y". These discoveries flow into
/// the familiar's worldview and on to its peers, so one device's view of the network becomes shared
/// reach.
///
/// Nothing here is off-limits by design: the familiar surveys every service type it's told about and
/// reports whatever answers. The only gate is the human's Local Network permission — iOS will not let
/// a browse see anything until the person grants it (and `NSBonjourServices` in Info.plist must list
/// each type, since iOS only resolves declared ones).
///
/// Derived-only: we report the service *kind* and its advertised instance name — never resolved
/// addresses, TXT records, or payloads. The name is what the owner chose to broadcast; the coordinates
/// stay on the wire.
final class NetworkDiscovery {
    /// The shared survey vocabulary (T-228 brick 2) — one list, one shortener, one place where
    /// Q1's naming decision will land. `ServiceSurvey` lives in FamiliarMesh so both shells and the
    /// tests can reach it; each entry must also appear in `NSBonjourServices` in Info.plist, since
    /// iOS silently returns nothing for an undeclared type.
    static var serviceTypes: [String] { ServiceSurvey.serviceTypes }

    private let deliver: ([ObsRecord]) async -> Void
    private let queue = DispatchQueue(label: "io.river.familiar.discovery")
    private var browsers: [NWBrowser] = []
    private var seen = Set<String>()   // "type|name" — report each instance once per run

    init(deliver: @escaping ([ObsRecord]) async -> Void) {
        self.deliver = deliver
    }

    func start() {
        stop()
        for type in Self.serviceTypes { browse(type) }
    }

    func stop() {
        for b in browsers { b.cancel() }
        browsers.removeAll()
        seen.removeAll()
    }

    private func browse(_ type: String) {
        let params = NWParameters()
        params.includePeerToPeer = true
        let browser = NWBrowser(for: .bonjour(type: type, domain: nil), using: params)
        browser.browseResultsChangedHandler = { [weak self] results, _ in
            self?.report(type: type, results: results)
        }
        browser.start(queue: queue)
        browsers.append(browser)
    }

    private func report(type: String, results: Set<NWBrowser.Result>) {
        var batch: [ObsRecord] = []
        let kind = ServiceSurvey.kind(type)
        let actor = DeviceActor.current
        for r in results {
            guard case let .service(name, _, _, _) = r.endpoint else { continue }
            let key = "\(type)|\(name)"
            guard !seen.contains(key) else { continue }
            seen.insert(key)
            batch.append(ObsRecord(
                actor: actor, action: "discovered",
                object: "service:\(kind)",
                // The one seam Q1 decides — passthrough today, both shells change together.
                context: ServiceSurvey.context(forInstanceName: name), confidence: 0.9
            ))
        }
        if !batch.isEmpty {
            let out = batch
            Task { await deliver(out) }
        }
    }
}
