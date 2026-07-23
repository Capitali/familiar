import Foundation
import SwiftUI
import UIKit
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
    /// Recognition (matching a face to a known identity) is "strongly sensitive" per
    /// docs/design-orientation-and-mesh.md — its own opt-in above plain presence (SPEC.md R10).
    @AppStorage("consent.faceRecognition") var faceRecognitionEnabled = false
    @AppStorage("consent.discovery") var discoveryEnabled = false

    private let grantAccount = "grant.json"
    private let enrollAccount = "enroll.info"   // {host,port,label} in the Keychain — survives reinstall
    private let defaults = UserDefaults.standard

    // The enrollment address, held in the model and persisted in the KEYCHAIN (not UserDefaults, which
    // is wiped on reinstall — the cause of the app dropping back to the join screen after a TestFlight
    // update). Loaded on init, saved on join, cleared on unenroll.
    var enrollPort: Int = 47100

    // Every address the familiar can be reached at, preferred first. `host` is always the current
    // preference (hosts.first); on any send/read failure the model rotates to the next candidate,
    // so whichever interface the device is on (wifi, cellular, tailnet VPN) it finds a path that
    // answers instead of pinning to the one that worked at enrollment.
    var hosts: [String] = []

    /// The familiar's TLS key pin from enrollment (nil on older enrollments).
    var tlsPin: String? {
        didSet { MeshTLS.pin = tlsPin }
    }

    private func saveEnrollment() {
        var d: [String: Any] = ["host": host, "hosts": hosts, "port": enrollPort, "label": groupLabel]
        if let pin = tlsPin { d["tlspin"] = pin }
        if let data = try? JSONSerialization.data(withJSONObject: d) { KeychainStore.save(data, account: enrollAccount) }
    }
    private func loadEnrollment() -> (host: String, hosts: [String], port: Int, label: String)? {
        // Keychain first (durable); fall back to the old UserDefaults keys once, to migrate.
        if let data = KeychainStore.load(account: enrollAccount),
           let d = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let h = d["host"] as? String, !h.isEmpty {
            let list = (d["hosts"] as? [String] ?? []).filter { !$0.isEmpty }
            tlsPin = d["tlspin"] as? String
            return (h, list.isEmpty ? [h] : list, (d["port"] as? Int) ?? 47100, (d["label"] as? String) ?? "")
        }
        if let h = defaults.string(forKey: "enroll.host"), !h.isEmpty {
            return (h, [h], Int(defaults.string(forKey: "enroll.port") ?? "") ?? 47100, defaults.string(forKey: "enroll.label") ?? "")
        }
        return nil
    }

    /// A plausible network address: hostname/IPv4/IPv6, optional :port — never prose. A
    /// poisoned advertisement once put an error SENTENCE here; nothing that can't be part
    /// of a URL authority is allowed into the candidate list (or kept in it).
    static func isValidHost(_ h: String) -> Bool {
        guard !h.isEmpty, h.count <= 253, !h.contains(" "), !h.contains("\n") else { return false }
        let allowed = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-:[]%")
        guard h.unicodeScalars.allSatisfy({ allowed.contains($0) }) else { return false }
        return URL(string: "http://\(h)/") != nil
    }

    /// Drop any invalid candidates (self-heal a poisoned stored list) and keep `host` valid.
    private func sanitizeHosts() {
        let before = hosts
        hosts = hosts.filter { Self.isValidHost($0) }
        if !Self.isValidHost(host) { host = hosts.first ?? "" }
        if hosts != before { saveEnrollment() }
    }

    /// `h` answered — make it the standing preference (front of the candidate list).
    private func promoteHost(_ h: String) {
        guard host != h || hosts.first != h else { return }
        hosts.removeAll { $0 == h }
        hosts.insert(h, at: 0)
        host = h
        saveEnrollment()
    }

    /// The familiar told us every address it answers at (in a worldview read) — adopt the ones we
    /// don't hold yet, after the current preference. This is how a device that enrolled on the LAN
    /// learns the tailnet path and can reach the mesh from cellular without re-enrolling.
    private func learnHosts(_ advertised: [String]?) {
        let fresh = (advertised ?? []).filter { Self.isValidHost($0) && !hosts.contains($0) }
        guard !fresh.isEmpty else { return }
        hosts.append(contentsOf: fresh)
        saveEnrollment()
        note("learned address\(fresh.count > 1 ? "es" : ""): \(fresh.joined(separator: ", "))")
    }

    /// The current host went quiet — rotate to the next candidate. Returns the new preference,
    /// or nil when there is nowhere else to try.
    private func failoverHost() -> String? {
        guard hosts.count > 1 else { return nil }
        let tired = hosts.removeFirst()
        hosts.append(tired)
        host = hosts[0]
        saveEnrollment()
        note("… \(tired) unreachable — trying \(host)")
        return host
    }

    private(set) var node: NodeKey
    private var coordinator: SensingCoordinator?
    private var discovery: NetworkDiscovery?

    // The console's answer field (The Glass home screen). The human speaking to the familiar.
    @Published var consoleAnswer = ""

    // The familiar's worldview, as this peer reads it (the iPad Glass console). Polled while shown.
    @Published var worldview: Worldview?
    /// The same snapshot as raw JSON, for the Metal Sphere web layer (window.sphereUpdate).
    @Published var worldviewJSON: String?
    /// Last poll cycle, per candidate: "host ✓" / "host ✗ reason" — the Device screen's data.
    @Published var attemptLog: [String] = []
    @Published var worldviewError: String?
    private var worldviewTask: Task<Void, Never>?

    // The iPad as a thinking-peer: on-device Apple Intelligence reasoning under the Three Laws.
    let reasoner = LocalReasoner()
    @AppStorage("consent.reasoning") var reasoningEnabled = false
    private var reasoningTask: Task<Void, Never>?
    private var lastReasonedAt: Date?

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
        if let e = loadEnrollment() {
            host = e.host; hosts = e.hosts; enrollPort = e.port; groupLabel = e.label
            sanitizeHosts()
        }
        enrolled = storedGrant() != nil && !host.isEmpty
        voice = VoiceSensing { [weak self] obs in self?.emit(obs) }
        face = FaceSensing { [weak self] obs in self?.emit(obs) }
        // Migrate an existing UserDefaults-only enrollment into the Keychain so it stops evaporating.
        if enrolled { saveEnrollment() }
        // Covenant baseline: an enrolled device with GPS provides its position to the mesh.
        if enrolled { startFixBaseline() }
    }

    /// Position reporting is part of the covenant — hold a fix whenever enrolled, without
    /// turning on the richer derived sensing (that stays behind its own toggles).
    private func startFixBaseline() {
        let coord = coordinator ?? SensingCoordinator { [weak self] batch in
            await self?.deliver(batch)
        }
        coordinator = coord
        coord.startFixBaseline()
    }

    /// The sphere's device screen state — consents + identity, as JSON for the web layer.
    func deviceStateJSON() -> String {
        let d: [String: Any] = [
            "label": UIDevice.current.name,
            "build": Self.appBuild,
            "host": host,
            "hosts": hosts,
            "attempts": attemptLog,
            "consents": [
                "location": locationEnabled, "motion": motionEnabled, "face": faceEnabled,
                "faceRecognition": faceRecognitionEnabled,
                "discovery": discoveryEnabled, "reasoning": reasoningEnabled,
            ],
        ]
        return (try? JSONSerialization.data(withJSONObject: d)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
    }

    /// A consent flipped on the sphere's device screen — apply it and start/stop the sensing.
    func setConsent(_ key: String, _ on: Bool) {
        switch key {
        case "location": locationEnabled = on
        case "motion": motionEnabled = on
        case "face": faceEnabled = on
        case "faceRecognition": faceRecognitionEnabled = on
        case "discovery": discoveryEnabled = on
        case "reasoning": reasoningEnabled = on
        default: return
        }
        startSensingIfConsented()
        startDiscoveryIfConsented()
        startFaceIfConsented()
        startReasoningIfConsented()
    }

    /// The human answered a specific theory's question — the answer attaches to that
    /// thread on the familiar (context "thread:<id>") and travels with its pursuit.
    func answerThread(_ id: String, _ text: String) {
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty else { return }
        emit(ObsRecord(actor: "ian", action: "answered", object: t, context: "thread:\(id)", confidence: 1.0))
        note("answered theory: \(t)")
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
    /// toggle). Only while enrolled. Recognition is a further, separately-consented layer on top
    /// of plain presence — faceEnabled alone never triggers identity matching.
    func startFaceIfConsented() {
        if enrolled, faceEnabled { face.start(recognize: faceRecognitionEnabled) } else { face.stop() }
    }

    /// Request to join from a scanned QR / pasted address payload: attest the Three Laws, ask the
    /// familiar, and wait for its human to approve. The group secret never touches this device.
    func requestJoin(from json: String) {
        guard let p = EnrollmentPayload.parse(json) else {
            note("✗ could not read that address")
            return
        }
        hosts = p.candidateHosts
        host = hosts[0]
        enrollPort = p.port
        groupLabel = p.label
        tlsPin = p.tlspin
        saveEnrollment()   // Keychain — durable across reinstalls (UserDefaults is wiped on reinstall)
        enrolling = true
        note("requesting to join “\(p.label)” — accepting the Three Laws…")
        let node = self.node
        Task { await self.runHandshake(candidates: self.hosts, port: p.port, node: node) }
    }

    private func runHandshake(candidates: [String], port: Int, node: NodeKey) async {
        // Walk the candidate addresses until one answers — the payload lists them most-universal
        // first, but only the device knows which are reachable from where it is right now.
        var lastError: Error?
        for host in candidates {
            let enroller = EnrollmentClient(host: host, port: port)
            do {
                var grant = try await enroller.requestJoin(node: node)     // non-nil if auto-approved
                promoteHost(host)
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
                startFixBaseline()
                startSensingIfConsented()
                startDiscoveryIfConsented()
                return
            } catch EnrollmentClient.EnrollError.denied {
                enrolling = false
                note("✗ the familiar declined this device")
                return
            } catch {
                lastError = error      // unreachable on this path — try the next address
            }
        }
        enrolling = false
        note("… couldn't reach the familiar at any address: \(lastError.map { "\($0)" } ?? "no candidates")")
    }

    /// Activate the watch link and, if we're enrolled, (re)hand the watch our address — so a watch
    /// that connects *after* the phone enrolled still gets linked. Safe to call every launch.
    func syncWatch() {
        let link = PhoneWatchLink.shared // touch = activate the WCSession
        if enrolled, !host.isEmpty {
            link.sendAddress(host: host, port: enrollPort, label: groupLabel)
        }
    }

    /// The address payload this device enrolled with — an *address*, not a secret. An enrolled
    /// member shows this as a QR so a new device can scan it and join the same familiar.
    var addressPayload: String? {
        guard !host.isEmpty else { return nil }
        let p = EnrollmentPayload(label: groupLabel, host: host, port: enrollPort,
                                  hosts: hosts.isEmpty ? nil : hosts)
        guard let data = try? JSONEncoder().encode(p) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    func unenroll() {
        KeychainStore.delete(account: grantAccount)
        KeychainStore.delete(account: enrollAccount)
        host = ""
        hosts = []
        coordinator?.stop()
        coordinator = nil
        discovery?.stop()
        discovery = nil
        enrolled = false
        note("unenrolled — nothing is sent")
    }

    /// Build the client session from the *granted* cert (not from any secret), or nil if not ready.
    func makeSession() -> ObservationClient.Session? {
        guard let g = storedGrant(), !host.isEmpty,
              let url = URL(string: "https://\(host):\(enrollPort)/mesh/observe")
        else { return nil }
        return ObservationClient.Session(node: node, membership: g.membership, url: url)
    }

    /// A signing session pointed at the familiar's `/mesh/worldview` (the read seam).
    func worldviewSession() -> ObservationClient.Session? {
        guard let g = storedGrant(), !host.isEmpty,
              let url = WorldviewClient.worldviewURL(host: host, port: enrollPort)
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
        // One read per candidate address at most — the preferred host first, failing over to the
        // others so a device off-LAN (cellular + tailnet) still reads the worldview.
        for _ in 0..<max(1, hosts.count) {
            guard let session = worldviewSession() else {
                worldviewError = "no session: grant=\(storedGrant() != nil) host=\(host.isEmpty ? "empty" : host)"
                return
            }
            do {
                let fix = coordinator?.lastCoordinate
                let (view, raw) = try await WorldviewClient(session: session)
                    .fetchWithRaw(clientVersion: Self.appBuild, osVersion: Self.osRelease,
                                  lat: fix?.lat ?? 0, lon: fix?.lon ?? 0)
                worldview = view
                worldviewJSON = String(data: raw, encoding: .utf8)
                worldviewError = nil
                promoteHost(host)
                learnHosts(view.hosts)
                return
            } catch {
                worldviewError = "\(error)"
                if failoverHost() == nil { return }
            }
        }
    }

    /// This app's build number ("16") — reported to the familiar so it shows in the roster.
    static let appBuild: String = (Bundle.main.infoDictionary?["CFBundleVersion"] as? String) ?? ""
    /// This device's OS release ("iPadOS 26.1") — reported to the familiar for the roster.
    static let osRelease: String = {
        let d = UIDevice.current
        return "\(d.systemName) \(d.systemVersion)"
    }()

    /// The iPad reasons over the familiar's recent observations with on-device Apple Intelligence
    /// (under the Three Laws) and submits a proposed theory to the mesh as a `theorizes` observation,
    /// where the familiar adopts it and an executor peer tests it. Consent-gated, paced (≤ every
    /// ~20 min), only while enrolled and only where the model is available.
    func startReasoningIfConsented() {
        guard enrolled, reasoningEnabled, reasoner.available, reasoningTask == nil else {
            if !reasoningEnabled { reasoningTask?.cancel(); reasoningTask = nil }
            return
        }
        reasoningTask = Task { [weak self] in
            while !Task.isCancelled {
                await self?.reasonOnce()
                try? await Task.sleep(nanoseconds: 20 * 60 * 1_000_000_000)
            }
        }
    }

    func stopReasoning() { reasoningTask?.cancel(); reasoningTask = nil }

    func reasonOnce() async {
        guard reasoningEnabled, let recent = worldview?.recent, !recent.isEmpty else { return }
        guard let proposal = await reasoner.reason(over: recent) else { return }
        // Submit the theory as a derived observation; the familiar turns it into a testable thread.
        emit(ObsRecord(actor: DeviceActor.current, action: "theorizes",
                       object: proposal.direction, context: proposal.question, confidence: 0.8))
        note("reasoned a theory: \(proposal.direction)")
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
        // Same failover walk as the worldview read: an observation should reach the familiar by
        // any address that answers, not only the one that worked at enrollment.
        for _ in 0..<max(1, hosts.count) {
            guard let session = makeSession() else { return }
            do {
                let n = try await ObservationClient(session: session).send(batch)
                sentCount += n
                promoteHost(host)
                note("→ sent \(n): " + batch.map { $0.object }.joined(separator: ", "))
                return
            } catch {
                note("… send failed: \(error)")
                if failoverHost() == nil { return }
            }
        }
    }

    private func note(_ s: String) {
        log.insert(s, at: 0)
        if log.count > 100 { log.removeLast(log.count - 100) }
    }
}
