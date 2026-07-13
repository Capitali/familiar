import Foundation
import SwiftUI
import FamiliarMesh

/// The agent's whole state: enrollment (via the covenant handshake), the signing session, consent,
/// and a small activity log. Thin — the crypto/wire logic lives in FamiliarMesh; sensing lives in
/// SensingCoordinator. The device holds its own key + a *granted* membership cert; it never holds
/// the group secret.
@MainActor
final class AppModel: ObservableObject {
    @Published var enrolled = false
    @Published var enrolling = false          // a handshake is in flight (waiting for approval)
    @Published var groupLabel = ""
    @Published var host = ""
    @Published var log: [String] = []
    @Published var sentCount = 0

    // Consent — nothing is gathered until the human turns it on. Persisted.
    @AppStorage("consent.location") var locationEnabled = false
    @AppStorage("consent.motion") var motionEnabled = false
    @AppStorage("consent.face") var faceEnabled = false
    @AppStorage("consent.discovery") var discoveryEnabled = false

    private let grantAccount = "grant.json"
    private let defaults = UserDefaults.standard

    private(set) var node: NodeKey
    private var coordinator: SensingCoordinator?
    private var discovery: NetworkDiscovery?

    // The console's answer field (The Glass home screen). The human speaking to the familiar.
    @Published var consoleAnswer = ""

    // The familiar's worldview, as this peer reads it (the iPad Glass console). Polled while shown.
    @Published var worldview: Worldview?
    @Published var worldviewError: String?
    private var worldviewTask: Task<Void, Never>?

    // Richer iPad sensors (voice is push-to-talk; face is a toggle). Created after node so their
    // closures can capture a fully-initialised self.
    private(set) var voice: VoiceSensing!
    private(set) var face: FaceSensing!

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
        enrolled = storedGrant() != nil && !host.isEmpty
        voice = VoiceSensing { [weak self] obs in self?.emit(obs) }
        face = FaceSensing { [weak self] obs in self?.emit(obs) }
    }

    /// A single derived observation from any sensor → the /mesh/observe pipe.
    func emit(_ obs: ObsRecord) {
        Task { await deliver([obs]) }
    }

    /// The human answered the familiar's question in the console — a served-facing observation, so
    /// presence and service register that a person is here and spoke.
    func submitConsoleAnswer() {
        let t = consoleAnswer.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty else { return }
        emit(ObsRecord(actor: "ian", action: "told the familiar", object: t, context: "console", confidence: 1.0))
        note("answered: \(t)")
        consoleAnswer = ""
    }

    /// Start/stop on-device facial analysis per consent (heavier than location/motion, so its own
    /// toggle). Only while enrolled.
    func startFaceIfConsented() {
        if enrolled, faceEnabled { face.start() } else { face.stop() }
    }

    /// Request to join from a scanned QR / pasted address payload: attest the Three Laws, ask the
    /// familiar, and wait for its human to approve. The group secret never touches this device.
    func requestJoin(from json: String) {
        guard let p = EnrollmentPayload.parse(json) else {
            note("✗ could not read that address")
            return
        }
        host = p.host
        groupLabel = p.label
        defaults.set(p.host, forKey: "enroll.host")
        defaults.set(String(p.port), forKey: "enroll.port")
        defaults.set(p.label, forKey: "enroll.label")
        enrolling = true
        note("requesting to join “\(p.label)” — accepting the Three Laws…")
        let node = self.node
        Task { await self.runHandshake(host: p.host, port: p.port, node: node) }
    }

    private func runHandshake(host: String, port: Int, node: NodeKey) async {
        let enroller = EnrollmentClient(host: host, port: port)
        do {
            var grant = try await enroller.requestJoin(node: node)     // non-nil if auto-approved
            if grant == nil { note("waiting for the familiar to approve this device…") }
            var tries = 0
            while grant == nil, tries < 150 {                          // ~5 min of polling
                try await Task.sleep(nanoseconds: 2_000_000_000)
                grant = try await enroller.pollGrant(nodeId: node.nodeId)
                tries += 1
            }
            guard let g = grant else { enrolling = false; note("… no approval yet — tap to retry"); return }
            saveGrant(g)
            enrolling = false
            enrolled = true
            note("✓ admitted to “\(g.group_label)” — the covenant is in force")
            // Hand the paired Apple Watch this familiar's address so it can enrol itself by
            // covenant (address only — the watch mints its own key + gets its own grant).
            PhoneWatchLink.shared.sendAddress(host: host, port: port, label: g.group_label)
            startSensingIfConsented()
            startDiscoveryIfConsented()
        } catch EnrollmentClient.EnrollError.denied {
            enrolling = false
            note("✗ the familiar declined this device")
        } catch {
            enrolling = false
            note("… couldn't reach the familiar: \(error)")
        }
    }

    /// Activate the watch link and, if we're enrolled, (re)hand the watch our address — so a watch
    /// that connects *after* the phone enrolled still gets linked. Safe to call every launch.
    func syncWatch() {
        let link = PhoneWatchLink.shared // touch = activate the WCSession
        if enrolled,
           let host = defaults.string(forKey: "enroll.host"),
           let port = Int(defaults.string(forKey: "enroll.port") ?? "") {
            link.sendAddress(host: host, port: port, label: groupLabel)
        }
    }

    /// The address payload this device enrolled with — an *address*, not a secret. An enrolled
    /// member shows this as a QR so a new device can scan it and join the same familiar.
    var addressPayload: String? {
        guard let host = defaults.string(forKey: "enroll.host"),
              let port = defaults.string(forKey: "enroll.port"), !host.isEmpty
        else { return nil }
        return "{\"v\":1,\"host\":\"\(host)\",\"port\":\(port),\"label\":\"\(groupLabel)\"}"
    }

    func unenroll() {
        KeychainStore.delete(account: grantAccount)
        defaults.removeObject(forKey: "enroll.host")
        coordinator?.stop()
        coordinator = nil
        discovery?.stop()
        discovery = nil
        enrolled = false
        note("unenrolled — nothing is sent")
    }

    /// Build the client session from the *granted* cert (not from any secret), or nil if not ready.
    func makeSession() -> ObservationClient.Session? {
        guard let g = storedGrant(),
              let host = defaults.string(forKey: "enroll.host"),
              let port = Int(defaults.string(forKey: "enroll.port") ?? ""),
              let url = URL(string: "http://\(host):\(port)/mesh/observe")
        else { return nil }
        return ObservationClient.Session(node: node, membership: g.membership, url: url)
    }

    /// A signing session pointed at the familiar's `/mesh/worldview` (the read seam).
    func worldviewSession() -> ObservationClient.Session? {
        guard let g = storedGrant(),
              let host = defaults.string(forKey: "enroll.host"),
              let port = Int(defaults.string(forKey: "enroll.port") ?? ""),
              let url = WorldviewClient.worldviewURL(host: host, port: port)
        else { return nil }
        return ObservationClient.Session(node: node, membership: g.membership, url: url)
    }

    /// Poll the familiar's worldview so the iPad Glass shows a live console. Idempotent; cancelled by
    /// `stopWorldviewPolling`. A peer *reads* the familiar's snapshot — it never sees the data dir.
    func startWorldviewPolling() {
        guard enrolled, worldviewTask == nil else { return }
        worldviewTask = Task { [weak self] in
            while !Task.isCancelled {
                await self?.refreshWorldview()
                try? await Task.sleep(nanoseconds: 5_000_000_000)
            }
        }
    }

    func stopWorldviewPolling() {
        worldviewTask?.cancel()
        worldviewTask = nil
    }

    func refreshWorldview() async {
        guard let session = worldviewSession() else { return }
        do {
            let view = try await WorldviewClient(session: session).fetch()
            worldview = view
            worldviewError = nil
        } catch {
            worldviewError = "\(error)"
        }
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

    /// Survey the local network by Bonjour and report what's out there — the device's view of the
    /// mesh's surroundings becomes the familiar's (and its peers'). Consent-gated; only while enrolled.
    func startDiscoveryIfConsented() {
        guard enrolled, discoveryEnabled else { discovery?.stop(); return }
        let d = discovery ?? NetworkDiscovery { [weak self] batch in await self?.deliver(batch) }
        discovery = d
        d.start()
        note("network discovery armed — surveying \(NetworkDiscovery.serviceTypes.count) service kinds")
    }

    // MARK: grant persistence (the cert is public — Keychain just keeps it tidy with the key)

    private func saveGrant(_ g: Grant) {
        if let data = try? JSONEncoder().encode(g) { KeychainStore.save(data, account: grantAccount) }
    }

    private func storedGrant() -> Grant? {
        guard let data = KeychainStore.load(account: grantAccount) else { return nil }
        return try? JSONDecoder().decode(Grant.self, from: data)
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
