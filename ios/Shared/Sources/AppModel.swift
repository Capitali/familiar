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
    /// Recognition (matching a face to a known identity) is "strongly sensitive" per
    /// docs/design-orientation-and-mesh.md — its own opt-in above plain presence (SPEC.md R10).
    @AppStorage("consent.faceRecognition") var faceRecognitionEnabled = false
    @AppStorage("consent.discovery") var discoveryEnabled = false

    /// Who this device is currently serving (ADR-0016) — the human its observations are attributed
    /// to. A node serves many people and devices are shared, so this is NOT a baked creator: it
    /// defaults to "observer" and is set by the human (Device menu), later by facial recognition.
    @AppStorage("identity.servedHuman") var servedHuman = "observer"

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

    /// The built-in rendezvous (ADR-0012): the lighthouse's public address and TLS pin, shipped as
    /// a default so a device off-LAN is never stranded — even a stale enrollment that predates the
    /// mesh learning about the lighthouse can still reach it on cellular. Learned hosts/pins from
    /// worldview reads take precedence; this is the floor, always a candidate and always trusted.
    /// (A device that runs its own mesh can override this in a later settings pass.)
    static let rendezvousHost = "134.209.168.50"
    static let rendezvousPin = "46b43ebf7111a6c17e91577397143b834f1bac8598879ab7e5e83fbf91796a6a"

    /// Guarantee the rendezvous host is a candidate (appended after learned hosts — the LAN/tailnet
    /// paths are faster when they work). The lighthouse PIN is trusted unconditionally as a baked
    /// `alwaysTrust` (set once at init), so this never has to touch the enrolled pin set — no risk
    /// of flipping a pinless device into strict mode, and the fallback works whenever pinning is on.
    private func ensureRendezvous() {
        var h = hosts
        if !h.contains(Self.rendezvousHost) { h.append(Self.rendezvousHost) }
        // Read preference: the nearest peer first (fresh + local), the lighthouse as fallback,
        // tailnet last. The enrollment handshake still knocks on the lighthouse first
        // (orderedCandidates); this only orders the ONGOING worldview reads so an on-network member
        // keeps that peer's roster current instead of routing every read through the lighthouse.
        // Preference is latency, never authority — a nearby peer earns no standing (ADR-0018).
        let ordered = Self.readOrderedCandidates(h)
        if ordered != hosts {
            hosts = ordered
            if host.isEmpty || !hosts.contains(host) { host = hosts.first ?? Self.rendezvousHost }
            saveEnrollment()
        }
    }

    /// The familiar's TLS key pin from enrollment (nil on older enrollments).
    var tlsPin: String? {
        didSet { MeshTLS.pin = tlsPin }
    }

    /// The group's trusted TLS pin set — the enrolling node's plus every sibling it vouches for
    /// (the lighthouse, peers). A device accepts any of these, so failover to a reachable member
    /// doesn't hit a pin mismatch (ADR-0012). Learned from worldview reads; persisted.
    var tlsPins: [String] = [] {
        didSet { MeshTLS.trust(tlsPins) }
    }

    private func saveEnrollment() {
        var d: [String: Any] = ["host": host, "hosts": hosts, "port": enrollPort, "label": groupLabel]
        if let pin = tlsPin { d["tlspin"] = pin }
        if !tlsPins.isEmpty { d["pins"] = tlsPins }
        if let data = try? JSONSerialization.data(withJSONObject: d) { KeychainStore.save(data, account: enrollAccount) }
    }
    private func loadEnrollment() -> (host: String, hosts: [String], port: Int, label: String)? {
        // Keychain first (durable); fall back to the old UserDefaults keys once, to migrate.
        if let data = KeychainStore.load(account: enrollAccount),
           let d = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let h = d["host"] as? String, !h.isEmpty {
            let list = (d["hosts"] as? [String] ?? []).filter { !$0.isEmpty }
            tlsPin = d["tlspin"] as? String
            tlsPins = (d["pins"] as? [String] ?? []).filter { !$0.isEmpty }
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

    /// A Tailscale (tailnet) address — the 100.64.0.0/10 CGNAT range Tailscale assigns. Per the
    /// join doctrine (ADR-0012): a device establishes a NON-Tailscale path first; a tailnet path is
    /// used only afterward, if reachable — so tailnet candidates always sort LAST.
    static func isTailnet(_ h: String) -> Bool {
        let bare = h.replacingOccurrences(of: "[", with: "").replacingOccurrences(of: "]", with: "")
            .split(separator: ":").first.map(String.init) ?? h
        let parts = bare.split(separator: ".")
        guard parts.count == 4, let a = Int(parts[0]), let b = Int(parts[1]) else { return false }
        return a == 100 && (64...127).contains(b)
    }

    /// The join/connect candidate order the doctrine wants: the always-on public lighthouse (the
    /// PRIMARY door) first, then any other non-Tailscale address, then Tailscale addresses LAST.
    /// Deduped and validity-filtered. `promoteHost` may later move whatever actually answered to the
    /// front — so a tailnet path is still used once it's proven, just never tried before a
    /// non-Tailscale one.
    static func orderedCandidates(_ raw: [String]) -> [String] {
        var seen = Set<String>()
        let valid = raw.filter { isValidHost($0) && seen.insert($0).inserted }
        let lighthouse = valid.filter { $0 == rendezvousHost }
        let nonTail = valid.filter { $0 != rendezvousHost && !isTailnet($0) }
        let tail = valid.filter { $0 != rendezvousHost && isTailnet($0) }
        return lighthouse + nonTail + tail
    }

    /// The ONGOING read preference (distinct from the enrollment door order above): the nearest
    /// peer first, the lighthouse second, Tailscale last. A member reads its worldview from a peer
    /// it shares a network with — lower latency, and it keeps that peer's roster fresh about this
    /// device (a read updates last_seen there). The lighthouse stays the always-reachable fallback
    /// for when no peer is on the same network; tailnet is still the post-establishment path.
    static func readOrderedCandidates(_ raw: [String]) -> [String] {
        var seen = Set<String>()
        let valid = raw.filter { isValidHost($0) && seen.insert($0).inserted }
        let lan = valid.filter { $0 != rendezvousHost && !isTailnet($0) }
        let lighthouse = valid.filter { $0 == rendezvousHost }
        let tail = valid.filter { $0 != rendezvousHost && isTailnet($0) }
        return lan + lighthouse + tail
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
        // Learning a peer's LAN address (advertised in the worldview) lets a running device
        // switch its reads to it without a relaunch — re-sort to the read preference (home → lighthouse
        // → tailnet) so the freshest, most-local path wins.
        hosts.append(contentsOf: fresh)
        hosts = Self.readOrderedCandidates(hosts)
        if !hosts.contains(host), let first = hosts.first { host = first }
        saveEnrollment()
        note("learned address\(fresh.count > 1 ? "es" : ""): \(fresh.joined(separator: ", "))")
    }

    /// The familiar told us the group's trusted TLS pins — adopt any we don't hold, so a later
    /// failover to a sibling (the lighthouse) passes the pin check (ADR-0012). This is how a
    /// device that pinned one node on the LAN comes to accept the lighthouse's cert on cellular.
    private func learnPins(_ advertised: [String]?) {
        let fresh = (advertised ?? []).filter { !$0.isEmpty && !tlsPins.contains($0) }
        guard !fresh.isEmpty else { return }
        tlsPins.append(contentsOf: fresh)   // didSet trusts them in MeshTLS
        saveEnrollment()
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
    // Device sensing is platform-shaped: CoreMotion and the Bonjour survey are iOS-only, and the
    // Mac shell runs its own (MacSensing/MacFaceSensing/MacNetworkDiscovery) and feeds this core
    // through `emit`. The mesh-peer role below — enrol, read, heartbeat, consult — is shared.
    #if os(iOS)
    private var coordinator: SensingCoordinator?
    private var discovery: NetworkDiscovery?
    #endif

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
    #if os(iOS)
    private(set) var voice: VoiceSensing!
    private(set) var face: FaceSensing!
    #endif

    init() {
        // The lighthouse's cert is always acceptable — a baked fallback pin that lets a device
        // reach the mesh off-LAN whatever its enrollment carried, without weakening pinning (ADR-0012).
        if !Self.rendezvousPin.isEmpty { MeshTLS.alwaysTrust.insert(Self.rendezvousPin) }
        // Restore (or mint) the device node key. The label is what the familiar sees as the peer.
        let label = PlatformDevice.name
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
        ensureRendezvous()   // the public failover is always a candidate — never strand off-LAN
        // Attribute this device's reports to the human it serves, not a baked creator (ADR-0016).
        DeviceActor.human = servedHuman
        enrolled = storedGrant() != nil && !host.isEmpty
        #if os(iOS)
        voice = VoiceSensing { [weak self] obs in self?.emit(obs) }
        face = FaceSensing { [weak self] obs in self?.emit(obs) }
        #endif
        // Migrate an existing UserDefaults-only enrollment into the Keychain so it stops evaporating.
        if enrolled { saveEnrollment() }
        // Covenant baseline: an enrolled device with GPS provides its position to the mesh.
        if enrolled { startFixBaseline() }
    }

    /// Position reporting is part of the covenant — hold a fix whenever enrolled, without
    /// turning on the richer derived sensing (that stays behind its own toggles).
    private func startFixBaseline() {
        #if os(iOS)
        let coord = coordinator ?? SensingCoordinator { [weak self] batch in
            await self?.deliver(batch)
        }
        coordinator = coord
        coord.startFixBaseline()
        #endif
    }

    /// The sphere's device screen state — consents + identity, as JSON for the web layer.
    func deviceStateJSON() -> String {
        let d: [String: Any] = [
            "label": PlatformDevice.name,
            "build": Self.appBuild,
            "host": host,
            "hosts": hosts,
            "attempts": attemptLog,
            "servedHuman": servedHuman,
            "oracle": ConsultRunner.state,
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

    /// Set who this device is serving (ADR-0016). The name is slugged to a stable handle (matching
    /// the daemon's `identity::slug`), so this device's reports thereafter tag `phone:<handle>` and
    /// attribute to the right person — and the paired watch is handed the same handle.
    func setServedHuman(_ name: String) {
        let handle = Self.slugHandle(name)
        guard !handle.isEmpty else { return }
        servedHuman = handle
        DeviceActor.human = handle
        note("serving \(handle)")
        syncWatch()   // re-hand address + human to the watch
    }

    /// A lowercase, dash-separated handle from a display name — "Betty Jo" -> "betty-jo". Mirrors
    /// `familiar_kernel::identity::slug` so the phone and daemon agree on the handle.
    static func slugHandle(_ name: String) -> String {
        let mapped = name.lowercased().map { $0.isLetter || $0.isNumber ? $0 : "-" }
        let collapsed = String(mapped).split(separator: "-").joined(separator: "-")
        return collapsed
    }

    /// The human answered a specific theory's question — the answer attaches to that
    /// thread on the familiar (context "thread:<id>") and travels with its pursuit.
    func answerThread(_ id: String, _ text: String) {
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty else { return }
        emit(ObsRecord(actor: servedHuman, action: "answered", object: t, context: "thread:\(id)", confidence: 1.0))
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
        emit(ObsRecord(actor: servedHuman, action: "told the familiar", object: t, context: "console", confidence: 1.0))
        note("answered: \(t)")
        consoleAnswer = ""
    }

    /// Start/stop on-device facial analysis per consent (heavier than location/motion, so its own
    /// toggle). Only while enrolled. Recognition is a further, separately-consented layer on top
    /// of plain presence — faceEnabled alone never triggers identity matching.
    func startFaceIfConsented() {
        #if os(iOS)
        if enrolled, faceEnabled { face.start(recognize: faceRecognitionEnabled) } else { face.stop() }
        #endif
    }

    /// Request to join from a scanned QR / pasted address payload: attest the Three Laws, ask the
    /// familiar, and wait for its human to approve. The group secret never touches this device.
    /// The short code the human matches when approving this device on the familiar — the first 6
    /// of this device's node fingerprint, exactly what `mesh pending` / the console shows. It stands
    /// in for the QR's proof-of-possession: the code on this screen must equal the one by the pending
    /// request. Derived, never a secret (ADR-0012).
    var confirmationCode: String { String(node.nodeId.prefix(6)) }

    /// Whether a rendezvous auto-enroll attempt has run this launch (so the view shows a spinner
    /// first, then the QR fallback only if discovery turned up nothing).
    @Published var autoEnrollTried = false

    /// **Auto-enroll (ADR-0012).** On first run, ask the baked rendezvous (the lighthouse) what
    /// meshes are reachable and start joining the one it finds — no QR needed. The device attests
    /// the Three Laws and shows its confirmation code; the human approves at the familiar. Falls
    /// back to the QR/paste path when the rendezvous is unreachable or lists nothing.
    func autoEnroll() {
        guard !enrolled, !enrolling, !autoEnrollTried else { return }
        autoEnrollTried = true
        let node = self.node
        Task {
            let port = enrollPort > 0 ? enrollPort : 47100
            let doors = (try? await RendezvousClient.directory(host: Self.rendezvousHost, port: port)) ?? []
            guard let door = doors.first else {
                note("no familiar found automatically — scan a QR or paste an address")
                return
            }
            // The lighthouse (always-on public door) is knocked on FIRST, then any other
            // non-Tailscale address the directory named, then tailnet paths last (ADR-0012 doctrine).
            let cand = Self.orderedCandidates(door.hosts + [Self.rendezvousHost])
            hosts = cand
            host = cand.first ?? Self.rendezvousHost
            enrollPort = door.port
            groupLabel = door.group_label
            // Trust the door's cert the rendezvous vouched for, so the covenant handshake's TLS can
            // complete on first contact with a familiar this device has never met (ADR-0012). Without
            // this the pin-checked session would reject the door and auto-enroll could never finish.
            if let p = door.pins, !p.isEmpty { tlsPins = p }
            ensureRendezvous()
            saveEnrollment()
            enrolling = true
            note("found “\(door.group_label)” — asking to join (code \(confirmationCode))…")
            await self.runHandshake(candidates: cand, port: door.port, node: node)
        }
    }

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
        tlsPins = p.pins ?? (p.tlspin.map { [$0] } ?? [])   // seed the group's pin set
        ensureRendezvous()   // now that we're pinning, trust the lighthouse too (same session)
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
                #if os(iOS)
                PhoneWatchLink.shared.sendAddress(host: host, port: port, label: g.group_label, human: servedHuman)
                #endif
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
        #if os(iOS)
        let link = PhoneWatchLink.shared // touch = activate the WCSession
        if enrolled, !host.isEmpty {
            link.sendAddress(host: host, port: enrollPort, label: groupLabel, human: servedHuman)
        }
        #endif
    }

    /// The address payload this device enrolled with — an *address*, not a secret. An enrolled
    /// member shows this as a QR so a new device can scan it and join the same familiar.
    var addressPayload: String? {
        guard !host.isEmpty else { return nil }
        // Carry the group's TLS pins too, so a device enrolling from THIS device's invite trusts
        // every member's cert (the lighthouse included) and can fail over off-LAN (ADR-0012).
        var pinSet = tlsPins
        if !Self.rendezvousPin.isEmpty && !pinSet.contains(Self.rendezvousPin) {
            pinSet.append(Self.rendezvousPin)
        }
        let hostList = hosts.contains(Self.rendezvousHost) ? hosts : hosts + [Self.rendezvousHost]
        let p = EnrollmentPayload(label: groupLabel, host: host, port: enrollPort,
                                  hosts: hostList.isEmpty ? nil : hostList,
                                  tlspin: tlsPin, pins: pinSet.isEmpty ? nil : pinSet)
        guard let data = try? JSONEncoder().encode(p) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    func unenroll() {
        KeychainStore.delete(account: grantAccount)
        KeychainStore.delete(account: enrollAccount)
        host = ""
        hosts = []
        #if os(iOS)
        coordinator?.stop()
        coordinator = nil
        discovery?.stop()
        discovery = nil
        #endif
        enrolled = false
        note("unenrolled — nothing is sent")
    }

    private var lastTailnetProbe: Date?
    private var servicingConsults = false

    /// Device oracle (ADR-0014): pull the prompts the familiar has queued for this device, answer
    /// each with the on-device model, and push the answers back. Only devices with Apple Intelligence
    /// serve consults; others skip silently and the familiar's provider chain rolls on. Serialized so
    /// a slow generation never overlaps the next read cycle. Pulls from the host we just read from —
    /// the muse queues on whichever peer is serving, so an on-network device picks them up.
    func serviceConsults(host readHost: String) async {
        guard ConsultRunner.available, !servicingConsults else { return }
        guard let g = storedGrant(), !readHost.isEmpty else { return }
        servicingConsults = true
        defer { servicingConsults = false }
        let client = ConsultClient(node: node, membership: g.membership, groupPubkey: g.group_pubkey,
                                   host: readHost, port: enrollPort)
        for p in await client.pull() {
            if let answer = await ConsultRunner.answer(p.prompt, kind: p.kind) {
                await client.push(id: p.id, json: answer)
            }
        }
    }

    /// Prefer Tailscale for data once it's confirmed working (ADR-0017 Phase C). Only after a
    /// non-Tailscale connection is established (the caller just read from one): probe a known tailnet
    /// address, and if it answers, promote it so the next reads take the direct peer-to-peer path.
    /// Throttled so we don't churn. Fallback is automatic — a failed read on the tailnet host fails
    /// over to a non-Tailscale candidate (readOrderedCandidates keeps tailnet last), and the reported
    /// mode reverts with it. So Tailscale being disabled or dropping self-heals to lighthouse/LAN.
    func maybeProbeTailnet() async {
        guard !Self.isTailnet(host) else { return }                      // already on Tailscale
        guard let tailnet = hosts.first(where: { Self.isTailnet($0) }) else { return }  // none known
        if let last = lastTailnetProbe, Date().timeIntervalSince(last) < 30 { return }
        lastTailnetProbe = Date()
        if await Self.probeHello(host: tailnet, port: enrollPort) {
            promoteHost(tailnet)   // data now flows peer-to-peer over Tailscale; the badge flips
            note("↔ Tailscale confirmed — data over \(tailnet)")
        }
    }

    /// A cheap liveness probe of a candidate path — GET /mesh/hello. True iff it answers 200.
    static func probeHello(host: String, port: Int) async -> Bool {
        guard let url = URL(string: "https://\(host):\(port)/mesh/hello") else { return false }
        var r = URLRequest(url: url)
        r.timeoutInterval = 4
        guard let (_, resp) = try? await MeshTLS.session.data(for: r) else { return false }
        return ((resp as? HTTPURLResponse)?.statusCode ?? 0) == 200
    }

    /// Heartbeat this device's status to the lighthouse (ADR-0017) — status flows through the always-
    /// on hub so the mesh sees this device whatever path it's on. The connectivity mode is classified
    /// from the host it actually read its worldview from just now.
    func heartbeatStatus(readHost: String) async {
        guard let g = storedGrant() else { return }
        let status = StatusClient.Member(
            node_id: node.nodeId,
            actor: DeviceActor.current,
            label: PlatformDevice.name,
            present_human: servedHuman,
            connectivity: Self.connectivityMode(readHost)
        )
        let client = StatusClient(node: node, membership: g.membership, groupPubkey: g.group_pubkey)
        _ = try? await client.send(status, host: Self.rendezvousHost, port: enrollPort)
    }

    /// Classify a worldview-read host into a connectivity mode for the roster badge (ADR-0017):
    /// the always-on lighthouse, a Tailscale (100.64/10) path, or a local/LAN path.
    static func connectivityMode(_ host: String) -> String {
        if host == rendezvousHost { return "lighthouse" }
        if isTailnet(host) { return "tailscale" }
        return "local"
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
        var attempts: [String] = []   // per-host diagnostic, surfaced if every candidate fails
        for _ in 0..<max(1, hosts.count) {
            let tried = host
            guard let session = worldviewSession() else {
                worldviewError = "no session: grant=\(storedGrant() != nil) host=\(host.isEmpty ? "empty" : host)"
                return
            }
            do {
                #if os(iOS)
                let fix = coordinator?.lastCoordinate
                #else
                // The Mac shell owns its own CoreLocation (MacSensing) and writes position through
                // `emit`; the worldview read carries no fix of its own here.
                let fix: (lat: Double, lon: Double)? = nil
                #endif
                let (view, raw) = try await WorldviewClient(session: session)
                    .fetchWithRaw(clientVersion: Self.appBuild, osVersion: Self.osRelease,
                                  lat: fix?.lat ?? 0, lon: fix?.lon ?? 0)
                worldview = view
                worldviewJSON = String(data: raw, encoding: .utf8)
                worldviewError = nil
                promoteHost(host)
                learnPins(view.pins)     // trust the group's pins before learning new hosts
                learnHosts(view.hosts)
                let readHost = host
                Task { await self.heartbeatStatus(readHost: readHost) }
                // Doctrine (ADR-0017 Phase C): a non-Tailscale path is now established — probe the
                // tailnet, and if it answers, prefer Tailscale for data. On tailnet already, a failed
                // read above would have failed us over to a non-Tailscale path (the fallback).
                if !Self.isTailnet(readHost) { await maybeProbeTailnet() }
                Task { await self.serviceConsults(host: readHost) }
                return
            } catch {
                // Compact, legible per-host cause. A ReadError names WHAT failed and — for an HTTP
                // rejection — the server's status + message, so the reason is on screen, not guessed.
                let cause: String
                switch error {
                case WorldviewClient.ReadError.http(let s, let b):
                    cause = "h\(s):\(b.prefix(40))"
                case WorldviewClient.ReadError.transport(let m):
                    cause = "t:\(m.prefix(30))"
                case WorldviewClient.ReadError.encoding: cause = "enc"
                case WorldviewClient.ReadError.decoding: cause = "dec"
                default: cause = "\((error as NSError).code)"
                }
                attempts.append("\(tried)→\(cause)")
                if failoverHost() == nil { break }
            }
        }
        // Every candidate failed — surface the full picture so the cause is diagnosable at a glance:
        // trusted-pin counts (enrolled + baked) and each host's error code.
        worldviewError = "pins \(MeshTLS.pins.count)+\(MeshTLS.alwaysTrust.count) · " + attempts.joined(separator: " ")
    }

    /// This app's build number ("16") — reported to the familiar so it shows in the roster.
    static let appBuild: String = (Bundle.main.infoDictionary?["CFBundleVersion"] as? String) ?? ""
    /// This device's OS release ("iPadOS 26.1") — reported to the familiar for the roster.
    static let osRelease: String = PlatformDevice.systemDescription

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
        #if os(iOS)
        guard enrolled, locationEnabled || motionEnabled else { return }
        let coord = coordinator ?? SensingCoordinator { [weak self] batch in
            await self?.deliver(batch)
        }
        coordinator = coord
        coord.start(location: locationEnabled, motion: motionEnabled)
        note("sensing armed (location: \(locationEnabled), motion: \(motionEnabled))")
        #endif
    }

    func setHomeToCurrentLocation() {
        #if os(iOS)
        coordinator?.markHomeAtCurrent()
        note("home region set to current location")
        #endif
    }

    /// Survey the local network by Bonjour and report what's out there — the device's view of the
    /// mesh's surroundings becomes the familiar's (and its peers'). Consent-gated; only while enrolled.
    func startDiscoveryIfConsented() {
        #if os(iOS)
        guard enrolled, discoveryEnabled else { discovery?.stop(); return }
        let d = discovery ?? NetworkDiscovery { [weak self] batch in await self?.deliver(batch) }
        discovery = d
        d.start()
        note("network discovery armed — surveying \(NetworkDiscovery.serviceTypes.count) service kinds")
        #endif
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
