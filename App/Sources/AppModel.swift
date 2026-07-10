import Foundation
import SwiftUI
import FamiliarMesh

/// The agent's whole state: enrollment, the signing session, consent, and a small activity log.
/// Deliberately thin — the crypto/wire logic lives in the FamiliarMesh package; sensing lives in
/// SensingCoordinator. This just holds it together for the UI.
@MainActor
final class AppModel: ObservableObject {
    @Published var enrolled = false
    @Published var groupLabel = ""
    @Published var host = ""
    @Published var log: [String] = []
    @Published var sentCount = 0

    // Consent — nothing is gathered until the human turns it on. Persisted.
    @AppStorage("consent.location") var locationEnabled = false
    @AppStorage("consent.motion") var motionEnabled = false

    private let seedAccount = "node.seed"
    private let secretAccount = "group.secret"
    private let defaults = UserDefaults.standard

    private(set) var node: NodeKey
    private var coordinator: SensingCoordinator?

    init() {
        // Restore (or mint) the device node key. The label is what the familiar sees as the peer.
        let label = UIDevice.current.name
        if let seed = KeychainStore.load(account: "node.seed"), let n = try? NodeKey(seed: seed, label: label) {
            node = n
        } else {
            let n = NodeKey(label: label)
            KeychainStore.save(n.seed, account: "node.seed")
            node = n
        }
        host = defaults.string(forKey: "enroll.host") ?? ""
        groupLabel = defaults.string(forKey: "enroll.label") ?? ""
        enrolled = KeychainStore.load(account: secretAccount) != nil && !host.isEmpty
    }

    /// Enroll from a scanned QR / pasted payload: verify the group secret, persist it, and arm.
    func enroll(from json: String) {
        guard let p = EnrollmentPayload.parse(json), let secret = p.secretData else {
            note("✗ could not read that enrollment code")
            return
        }
        // Sanity: the secret must derive the group id it claims.
        guard (try? Cert.groupId(fromSecret: secret)) == p.group else {
            note("✗ enrollment code failed its integrity check")
            return
        }
        KeychainStore.save(secret, account: secretAccount)
        defaults.set(p.host, forKey: "enroll.host")
        defaults.set(String(p.port), forKey: "enroll.port")
        defaults.set(p.label, forKey: "enroll.label")
        defaults.set(p.group, forKey: "enroll.group")
        host = p.host
        groupLabel = p.label
        enrolled = true
        note("✓ enrolled in “\(p.label)” — reaching \(p.host):\(p.port)")
        startSensingIfConsented()
    }

    func unenroll() {
        KeychainStore.delete(account: secretAccount)
        defaults.removeObject(forKey: "enroll.host")
        coordinator?.stop()
        coordinator = nil
        enrolled = false
        note("unenrolled — nothing is sent")
    }

    /// Build the client session (node + freshly-minted cert + endpoint URL), or nil if not ready.
    func makeSession() -> ObservationClient.Session? {
        guard let secret = KeychainStore.load(account: secretAccount),
              let host = defaults.string(forKey: "enroll.host"),
              let port = Int(defaults.string(forKey: "enroll.port") ?? ""),
              let url = URL(string: "http://\(host):\(port)/mesh/observe"),
              let m = try? Cert.mint(groupSecret: secret, node: node.identity,
                                     issued: Int64(Date().timeIntervalSince1970), ttlSecs: defaultCertTTLSecs)
        else { return nil }
        return ObservationClient.Session(node: node, membership: m, url: url)
    }

    func startSensingIfConsented() {
        guard enrolled, locationEnabled || motionEnabled else { return }
        let coord = coordinator ?? SensingCoordinator { [weak self] batch in
            await self?.deliver(batch)
        }
        coordinator = coord
        coord.start(location: locationEnabled, motion: motionEnabled)
        note("sensing armed (location: \(locationEnabled), motion: \(motionEnabled))")
    }

    func setHomeToCurrentLocation() {
        coordinator?.markHomeAtCurrent()
        note("home region set to current location")
    }

    private func deliver(_ batch: [ObsRecord]) async {
        guard let session = makeSession() else { return }
        do {
            let n = try await ObservationClient(session: session).send(batch)
            sentCount += n
            note("→ sent \(n): " + batch.map { $0.object }.joined(separator: ", "))
        } catch {
            note("… send failed: \(error)")
        }
    }

    private func note(_ s: String) {
        log.insert(s, at: 0)
        if log.count > 100 { log.removeLast(log.count - 100) }
    }
}
