import Foundation
import Network
import UIKit
import FamiliarMesh

/// The device's actor namespace as the familiar sees it: `phone:ian` on iPhone, `ipad:ian` on iPad.
/// The observations a device reports are tagged with this so the familiar (and its peers) know which
/// device on the mesh saw what.
enum DeviceActor {
    static var current: String {
        UIDevice.current.userInterfaceIdiom == .pad ? "ipad:ian" : "phone:ian"
    }
}

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
    /// Service types worth surveying. Peers first, then the everyday advertisements a home / boat / RV
    /// network exposes: remote access, media endpoints, printers, smart-home, file shares, brokers.
    /// Extend freely — each entry is just another browse, and each must also appear in
    /// `NSBonjourServices` in Info.plist or iOS silently returns nothing for it.
    static let serviceTypes: [String] = [
        "_familiar-mesh._tcp",                               // other familiars / peers
        "_ssh._tcp", "_sftp-ssh._tcp", "_rfb._tcp",          // remote access (SSH, VNC)
        "_http._tcp", "_https._tcp",                         // web endpoints
        "_airplay._tcp", "_raop._tcp", "_airport._tcp",      // AirPlay / speakers / base stations
        "_googlecast._tcp", "_spotify-connect._tcp",         // cast / audio
        "_ipp._tcp", "_ipps._tcp", "_printer._tcp", "_pdl-datastream._tcp", // printers
        "_homekit._tcp", "_hap._tcp",                        // HomeKit accessories
        "_companion-link._tcp", "_apple-mobdev2._tcp",       // Apple continuity / devices
        "_smb._tcp", "_afpovertcp._tcp",                     // file shares
        "_daap._tcp", "_dacp._tcp",                          // media libraries
        "_mqtt._tcp",                                        // MQTT brokers (the boat/RV runs one)
        "_workstation._tcp", "_device-info._tcp",            // general hosts
    ]

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
        let kind = Self.shortKind(type)
        let actor = DeviceActor.current
        for r in results {
            guard case let .service(name, _, _, _) = r.endpoint else { continue }
            let key = "\(type)|\(name)"
            guard !seen.contains(key) else { continue }
            seen.insert(key)
            batch.append(ObsRecord(
                actor: actor, action: "discovered",
                object: "service:\(kind)", context: name, confidence: 0.9
            ))
        }
        if !batch.isEmpty {
            let out = batch
            Task { await deliver(out) }
        }
    }

    /// "_airplay._tcp" → "airplay".
    private static func shortKind(_ type: String) -> String {
        type.split(separator: ".").first.map { String($0.drop(while: { $0 == "_" })) } ?? type
    }
}
