import Foundation
import SwiftUI
import WatchConnectivity
import WatchKit
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
    /// True right after first enrollment, until the human resolves the consent prompt —
    /// sensing never starts silently on a newly-paired watch. See `consentAsked`.
    @Published var needsConsentPrompt = false
    /// The mesh game's turn has reached this watch's human — "riddle" | "campfire", nil when
    /// not their turn. Drives the full-screen ember and the wrist chime (the law of the fire:
    /// the ember shows on every device of the holder).
    @Published var emberKind: String?

    /// Off by default, matching phone/iPad's posture — a watch left on a wrist unattended
    /// shouldn't silently start reporting health/motion data. `consentAsked` distinguishes
    /// "never asked" from "asked and declined" (a plain bool default can't tell those apart).
    @AppStorage("watch.consent.motion") var motionEnabled = false
    @AppStorage("watch.consent.heart") var heartEnabled = false
    @AppStorage("watch.consent.location") var locationEnabled = false
    @AppStorage("watch.consent.asked") var consentAsked = false

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
    private func onAddress(host: String, port: Int, label: String, human: String) {
        defaults.set(host, forKey: "watch.enroll.host")
        defaults.set(String(port), forKey: "watch.enroll.port")
        defaults.set(label, forKey: "watch.enroll.label")
        // The human the paired phone serves — the watch attributes its reports to the same person
        // (ADR-0016), so `watch:<handle>` matches `phone:<handle>` rather than a baked "ian".
        if !human.isEmpty { defaults.set(human, forKey: "watch.servedHuman") }
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
        guard consentAsked else {
            // First pairing (or a still-unresolved prompt from a prior launch) — ask before
            // sensing anything, rather than defaulting silently the way this used to.
            needsConsentPrompt = true
            return
        }
        let s = sensing ?? WatchSensing { [weak self] batch in await self?.deliver(batch) }
        s.onHeartRate = { [weak self] bpm in Task { @MainActor in self?.lastHeartRate = bpm } }
        s.servedHuman = defaults.string(forKey: "watch.servedHuman") ?? "observer"
        sensing = s
        s.start(motionOn: motionEnabled, heartOn: heartEnabled, locationOn: locationEnabled)
        note("sensing armed")
    }

    /// The human resolved the first-pair consent prompt — record it (so it never asks again
    /// unless the watch is reset) and start sensing with whatever they chose.
    func resolveConsent(motion: Bool, heart: Bool, location: Bool) {
        motionEnabled = motion
        heartEnabled = heart
        locationEnabled = location
        consentAsked = true
        needsConsentPrompt = false
        startSensing()
    }

    private func makeSession() -> ObservationClient.Session? {
        guard let g = storedGrant(),
              let host = defaults.string(forKey: "watch.enroll.host"),
              let port = Int(defaults.string(forKey: "watch.enroll.port") ?? ""),
              // HTTPS — the mesh port is TLS (ADR-0009). This was http, so the watch enrolled but
              // every observation silently failed: it never reached the roster. MeshTLS handles the
              // self-signed cert (accept-any without a pin; the covenant signature is the authenticity floor).
              let url = URL(string: "https://\(host):\(port)/mesh/observe")
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
        await heartbeatStatus()
    }

    private static let lighthouseHost = "134.209.168.50"

    /// Heartbeat the watch's status to the lighthouse (ADR-0017) so it shows on the mesh roster with
    /// its own row and connectivity, attributed to the same human as its paired phone. Piggybacks on
    /// the observation send cadence (when the watch is active); best-effort, lighthouse then the host.
    private func heartbeatStatus() async {
        guard let g = storedGrant(),
              let host = defaults.string(forKey: "watch.enroll.host"),
              let port = Int(defaults.string(forKey: "watch.enroll.port") ?? "")
        else { return }
        let human = defaults.string(forKey: "watch.servedHuman") ?? "observer"
        let status = StatusClient.Member(
            node_id: node.nodeId,
            actor: "watch:\(human)",
            label: "Apple Watch",
            present_human: human,
            connectivity: WatchModel.connectivityMode(host)
        )
        let client = StatusClient(node: node, membership: g.membership, groupPubkey: g.group_pubkey)
        if (try? await client.send(status, host: WatchModel.lighthouseHost, port: port)) != true {
            _ = try? await client.send(status, host: host, port: port)
        }
    }

    private static func connectivityMode(_ host: String) -> String {
        if host == lighthouseHost { return "lighthouse" }
        if host.hasPrefix("100.") { return "tailscale" }   // coarse 100.64/10 tailnet check
        return "local"
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
        handleEmber(info); handleAddress(info)
    }
    // The ember, urgent and transient: sent as a live message when the watch is reachable,
    // queued as userInfo when not. `on: false` clears the flame when the turn moves on.
    nonisolated func session(_ s: WCSession, didReceiveMessage m: [String: Any]) {
        handleEmber(m); handleAddress(m)
    }
    private nonisolated func handleEmber(_ d: [String: Any]) {
        guard let on = d["ember"] as? Bool else { return }
        let kind = d["kind"] as? String ?? "riddle"
        Task { @MainActor in
            if on {
                self.emberKind = kind
                // A chime the wrist can't miss: three rising taps.
                WKInterfaceDevice.current().play(.notification)
                for delay in [0.35, 0.7] {
                    DispatchQueue.main.asyncAfter(deadline: .now() + delay) {
                        WKInterfaceDevice.current().play(.directionUp)
                    }
                }
            } else {
                self.emberKind = nil
            }
        }
    }
    private nonisolated func handleAddress(_ d: [String: Any]) {
        guard let host = d["host"] as? String, let port = d["port"] as? Int else { return }
        let label = d["label"] as? String ?? "familiar"
        let human = d["human"] as? String ?? "observer"
        Task { @MainActor in self.onAddress(host: host, port: port, label: label, human: human) }
    }
}
