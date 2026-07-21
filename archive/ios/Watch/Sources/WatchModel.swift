import Foundation
import SwiftUI
import WatchConnectivity
import FamiliarMesh

/// The watch agent's state: it enrols into the familiar **by covenant** (receiving the familiar's
/// address from the paired iPhone over WatchConnectivity — the watch has no good text entry), then
/// gathers derived heart-rate + motion observations and posts them to `/mesh/observe`. Its own key
/// and cert; it never holds the group secret.
@MainActor
final class WatchModel: NSObject, ObservableObject {
    @Published var enrolled = false
    @Published var enrolling = false
    @Published var groupLabel = ""
    @Published var sentCount = 0
    @Published var lastHeartRate: Int?
    @Published var log: [String] = []

    @AppStorage("watch.consent.motion") var motionEnabled = true
    @AppStorage("watch.consent.heart") var heartEnabled = true

    private let grantAccount = "watch.grant.json"
    private let defaults = UserDefaults.standard
    private var node: NodeKey
    private var sensing: WatchSensing?

    override init() {
        let label = "Apple Watch"
        if let seed = KeychainStore.load(account: "watch.node.seed"), let n = try? NodeKey(seed: seed, label: label) {
            node = n
        } else {
            let n = NodeKey(label: label)
            KeychainStore.save(n.seed, account: "watch.node.seed")
            node = n
        }
        super.init()
        groupLabel = defaults.string(forKey: "watch.enroll.label") ?? ""
        enrolled = storedGrant() != nil
    }

    func start() {
        if WCSession.isSupported() {
            let s = WCSession.default
            s.delegate = self
            s.activate()
        }
        if enrolled { startSensing() }
    }

    /// The paired iPhone handed us the familiar's address → request to join by covenant.
    private func onAddress(host: String, port: Int, label: String) {
        defaults.set(host, forKey: "watch.enroll.host")
        defaults.set(String(port), forKey: "watch.enroll.port")
        defaults.set(label, forKey: "watch.enroll.label")
        groupLabel = label
        guard !enrolled, !enrolling else { return }
        enrolling = true
        note("joining \(label)…")
        let node = self.node
        Task { await self.enroll(host: host, port: port, node: node) }
    }

    private func enroll(host: String, port: Int, node: NodeKey) async {
        let enroller = EnrollmentClient(host: host, port: port)
        do {
            var grant = try await enroller.requestJoin(node: node)
            var tries = 0
            while grant == nil, tries < 100 {
                try await Task.sleep(nanoseconds: 3_000_000_000)
                grant = try await enroller.pollGrant(nodeId: node.nodeId)
                tries += 1
            }
            guard let g = grant else { enrolling = false; note("no approval yet"); return }
            saveGrant(g)
            enrolling = false
            enrolled = true
            note("✓ joined \(g.group_label)")
            startSensing()
        } catch {
            enrolling = false
            note("join failed: \(error)")
        }
    }

    private func startSensing() {
        guard enrolled else { return }
        let s = sensing ?? WatchSensing { [weak self] batch in await self?.deliver(batch) }
        s.onHeartRate = { [weak self] bpm in Task { @MainActor in self?.lastHeartRate = bpm } }
        sensing = s
        s.start(motionOn: motionEnabled, heartOn: heartEnabled)
        note("sensing armed")
    }

    private func makeSession() -> ObservationClient.Session? {
        guard let g = storedGrant(),
              let host = defaults.string(forKey: "watch.enroll.host"),
              let port = Int(defaults.string(forKey: "watch.enroll.port") ?? ""),
              let url = URL(string: "http://\(host):\(port)/mesh/observe")
        else { return nil }
        return ObservationClient.Session(node: node, membership: g.membership, url: url)
    }

    private func deliver(_ batch: [ObsRecord]) async {
        guard let s = makeSession() else { return }
        do {
            let n = try await ObservationClient(session: s).send(batch)
            sentCount += n
            note("→ " + batch.map { $0.object }.joined(separator: ", "))
        } catch {
            note("send failed")
        }
    }

    private func saveGrant(_ g: Grant) {
        if let d = try? JSONEncoder().encode(g) { KeychainStore.save(d, account: grantAccount) }
    }
    private func storedGrant() -> Grant? {
        KeychainStore.load(account: grantAccount).flatMap { try? JSONDecoder().decode(Grant.self, from: $0) }
    }
    private func note(_ s: String) {
        log.insert(s, at: 0)
        if log.count > 20 { log.removeLast(log.count - 20) }
    }
}

extension WatchModel: WCSessionDelegate {
    nonisolated func session(_ s: WCSession, activationDidCompleteWith state: WCSessionActivationState, error: Error?) {}

    nonisolated func session(_ s: WCSession, didReceiveApplicationContext ctx: [String: Any]) {
        handleAddress(ctx)
    }
    // The reliable, queued delivery (the phone also sends the address this way so it lands even if the
    // watch app was closed when the phone enrolled).
    nonisolated func session(_ s: WCSession, didReceiveUserInfo info: [String: Any]) {
        handleAddress(info)
    }
    private nonisolated func handleAddress(_ d: [String: Any]) {
        guard let host = d["host"] as? String, let port = d["port"] as? Int else { return }
        let label = d["label"] as? String ?? "familiar"
        Task { @MainActor in self.onAddress(host: host, port: port, label: label) }
    }
}
