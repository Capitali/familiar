import Foundation
import SwiftUI
import AVFoundation
import FamiliarMesh
#if os(iOS)
import WatchConnectivity
#endif

/// The agent's whole state: enrollment (via the covenant handshake), the signing session, consent,
/// and a small activity log. Thin — the crypto/wire logic lives in FamiliarMesh; sensing lives in
/// SensingCoordinator. The device holds its own key + a *granted* membership cert; it never holds
/// the group secret.
@MainActor
final class AppModel: ObservableObject {
    /// Where this device stands under the two-filter door (ADR-0026). `enrolled` below stays the
    /// coarse "holds a cert, can read" flag every view already leans on; this is the finer truth:
    /// a guest is a stable, honest state, and `path` is the door's own words for what admission
    /// still needs — shown verbatim, because the refusal text IS the UI copy.
    enum MembershipState: Equatable {
        /// We hold a grant but have not yet heard what the mesh says about our standing.
        /// NOT a visitor — a device that has never been told anything is not thereby a
        /// stranger. Resolves to `.member` on the first recognised read, or to `.guest`
        /// after three unrecognised ones (T-197).
        case unknown
        case none
        case knocking
        case guest(path: String)
        case held(retryIn: Int64)
        case member(handle: String)
    }
    @Published var membership: MembershipState = .none
    /// A join is in flight — the console shows progress instead of the static path card (B6).
    @Published var introducing = false
    var introduceStartedAt: TimeInterval = 0
    /// Why an introduction could not be sent yet, in words for the human. Empty when there is
    /// nothing being held. The console shows this instead of asking again for a name it has
    /// (T-185).
    @Published var introduceHeldReason = ""
    /// The introduction the human made before the door was reachable. Replayed once it is —
    /// an act of identification is not discarded because the handshake was still in flight.
    var pendingIntroduction: (IdentityClaim?, Evidence)?
    /// The door's last verdict on a game act, shown on the games screen (B15). "" when clear.
    @Published var gameNote = ""

    /// The default path-to-admission copy, before the door has said anything more specific.
    static let admissionPath = "Covenant accepted — you're reading as a guest. To be admitted: " +
        "introduce yourself while on the mesh's network, scan an invite from a member, or hand " +
        "off from your old device."

    @Published var enrolled = false
    @Published var enrolling = false          // a handshake is in flight (knock → guest)
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

    // MARK: friendly identification (ADR-0019)

    /// How this device is used — decides whether it has a prior to verify at all. Defaults by
    /// hardware kind and is a human's to change.
    @AppStorage("identity.deviceRole") var deviceRoleRaw = DeviceRole.suggested.rawValue
    /// The bound owner of a personal device. Rung 1 of the ladder, and usually the whole answer
    /// on a phone — no camera, no model, no prompt.
    @AppStorage("identity.deviceOwner") var deviceOwner = ""
    /// The human severed this device (SEVER, twice) — it must not rejoin on its own. Cleared
    /// only by the explicit "Join the mesh" act on the join screen. Persisted: a relaunch
    /// after a severing is still severed.
    @AppStorage("enroll.severedByHuman") var severedByHuman = false

    var deviceRole: DeviceRole { DeviceRole(rawValue: deviceRoleRaw) ?? .shared }

    /// Who the ladder says is here, with a confidence and an expiry.
    @Published var presence: PresenceClaim = .unknown
    /// The human's own answer to the confirm-or-correct prompt — outranks every inference.
    private var answeredClaim: PresenceClaim?
    /// A 1:1 face check that CONFIRMED the prior.
    private var faceClaim: PresenceClaim?
    /// A 1:1 check ran against the prior and disagreed. Demotes the binding rather than letting
    /// the device's guess quietly overrule the camera.
    private var faceContradicted = false

    /// Who this device's reports are attributed to right now: the live claim when there is one,
    /// otherwise the persisted served human. Identification decides addressing, never access.
    var attributedHuman: String { presence.isLive ? presence.handle : servedHuman }

    /// Re-run the ladder and republish. Cheap; safe to call on any signal.
    func refreshPresence() {
        presence = Identification.resolve(
            role: deviceRole, owner: deviceOwner,
            answered: answeredClaim, faceVerified: faceClaim,
            faceContradicted: faceContradicted
        )
        DeviceActor.human = attributedHuman
    }

    /// The human said who they are. Authoritative, and it also settles the binding on a personal
    /// device so the next launch starts at rung 1 instead of asking again.
    ///
    /// On a **guest** device this is also how an E4 introduction begins (ADR-0019 as amended by
    /// ADR-0026): the same act presents the name to the door, so the local ladder and the mesh's
    /// identity filter learn one fact from one answer. The door still decides — provenance is
    /// what IT observed, and its refusal text becomes the guest screen's copy.
    func confirmPresentHuman(_ name: String) {
        markInteraction()
        let handle = Self.slugHandle(name)
        guard !handle.isEmpty else { return }
        answeredClaim = .make(handle: handle, via: .asked)
        faceContradicted = false
        if deviceRole == .personal, deviceOwner.isEmpty { deviceOwner = handle }
        refreshPresence()
        note("identified \(handle) (asked)")
        if case .guest = membership {
            Task { await self.introduceMesh(handle) }
        }
    }

    /// A 1:1 face check finished. `handle` non-nil means it agreed with the prior; nil means it
    /// ran and disagreed — a different fact from never having looked.
    func faceVerification(confirmed handle: String?) {
        if let h = handle, !h.isEmpty {
            faceClaim = .make(handle: h, via: .face)
            faceContradicted = false
        } else {
            faceClaim = nil
            faceContradicted = true
        }
        refreshPresence()
    }

    /// Bind or unbind this device's owner (a human act — a phone changes hands, a shared iPad
    /// becomes someone's).
    func setDeviceBinding(role: DeviceRole, owner: String) {
        deviceRoleRaw = role.rawValue
        deviceOwner = role == .personal ? Self.slugHandle(owner) : ""
        refreshPresence()
        note("device is \(role.rawValue)\(deviceOwner.isEmpty ? "" : " · \(deviceOwner)")")
    }

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

    // MARK: dialogue voice — the loop closes by mouth and ear

    /// The console's push-to-talk is wired and usable (set by the iOS shell when the
    /// recognizer is ready). Read by the sphere console to show its mic control.
    @Published var dialogueVoiceAvailable = false
    /// The ts of a voice-originated turn still awaiting a spoken reply, nil when none. Voice
    /// in → voice out: only a turn that arrived by mouth is answered aloud, so the console
    /// never starts talking at someone who was typing quietly.
    private var awaitingSpokenReplySince: Int64?
    private let replyVoice = AVSpeechSynthesizer()

    /// A dialogue turn that arrived by voice: same pipe as the typed console answer, plus a
    /// marker so the familiar's reply is spoken back when the worldview carries it in.
    func submitVoiceTurn(_ text: String) {
        markInteraction()
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty else { return }
        emit(ObsRecord(actor: servedHuman, action: "told the familiar", object: t,
                       context: "console", confidence: 1.0))
        note("said: \(t)")
        awaitingSpokenReplySince = Int64(Date().timeIntervalSince1970)
    }

    /// Speak the familiar's reply to a pending voice turn, once, when a poll carries it in.
    /// A reply that takes longer than two minutes is stale — the moment has passed, and a
    /// surprise voice an hour later would be worse than silence.
    private func speakReplyIfDue(_ view: Worldview) {
        guard let since = awaitingSpokenReplySince else { return }
        let now = Int64(Date().timeIntervalSince1970)
        if now - since > 120 {
            awaitingSpokenReplySince = nil
            return
        }
        guard let reply = view.recent.first(where: {
            $0.actor == "familiar" && $0.action == "replied" && $0.ts >= since
        }) else { return }
        awaitingSpokenReplySince = nil
        let u = AVSpeechUtterance(string: reply.object)
        u.prefersAssistiveTechnologySettings = true
        replyVoice.speak(u)
    }

    // The familiar's worldview, as this peer reads it (the iPad Glass console). Polled while shown.
    @Published var worldview: Worldview?
    /// The same snapshot as raw JSON, for the Metal Sphere web layer (window.sphereUpdate).
    @Published var worldviewJSON: String?
    /// Last poll cycle, per candidate: "host ✓" / "host ✗ reason" — the Device screen's data.
    @Published var attemptLog: [String] = []
    @Published var worldviewError: String?
    /// Human-addressed partner state from a separate signed read. This is never persisted and
    /// never folded into worldview or diagnostics.
    @Published var partnerInboxJSON: String?
    @Published var partnerInboxError: String?
    /// Which door holds each pending partner item (registration/request/grant/proposal id →
    /// host). Partner state is door-local, so the deciding act must return to the door the
    /// item was read from — not to whichever door the console currently prefers.
    private var partnerItemDoors: [String: String] = [:]
    private var worldviewTask: Task<Void, Never>?

    // The iPad as a thinking-peer: on-device Apple Intelligence reasoning under the Three Laws.
    let reasoner = LocalReasoner()
    @AppStorage("consent.reasoning") var reasoningEnabled = false
    /// May THIS device choose Private Cloud Compute for a cloud-cleared consult (ADR-0038)?
    /// Stacks with the hub's cloud_ok — both must hold; default off. (Toggle UI: sphere, next brick.)
    @AppStorage("consent.pcc") var pccEnabled = false
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
        // A personal device that already knows who it serves is already bound — seed rung 1 from
        // it rather than making the human state the same fact twice. ("observer" is the
        // *absence* of an answer, so it never becomes a binding.)
        if deviceRole == .personal, deviceOwner.isEmpty, servedHuman != "observer" {
            deviceOwner = servedHuman
        }
        refreshPresence()
        enrolled = storedGrant() != nil && !host.isEmpty
        // Fine-grained truth arrives with the first worldview read. Until then this device's
        // standing is UNKNOWN — it is not a visitor (T-197). Assuming guest here meant every
        // member's console flashed "VISITOR — TAP FOR YOUR PATH TO MEMBERSHIP" at them for the
        // seconds before the first read landed, which is the mesh calling its own people
        // strangers because it had not finished reading yet. Absence is not a negative.
        membership = enrolled ? .unknown : .none
        #if os(iOS)
        voice = VoiceSensing { [weak self] obs in self?.emit(obs) }
        face = FaceSensing { [weak self] obs in self?.emit(obs) }
        // Rung 2 reports back: agreed with the prior, or ran and disagreed (ADR-0019).
        face.onVerification = { [weak self] handle in self?.faceVerification(confirmed: handle) }
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
        // The two-filter state, in words the console shows verbatim (ADR-0026): which state,
        // and — for a guest — the door's own path-to-admission copy.
        let membershipDict: [String: Any]
        switch membership {
        case .unknown: membershipDict = ["state": "unknown", "path": ""]
        case .none: membershipDict = ["state": "none", "path": ""]
        case .knocking: membershipDict = ["state": "knocking", "path": ""]
        case .guest(let path): membershipDict = ["state": "guest", "path": path]
        case .held(let s): membershipDict = ["state": "held", "path": "held — try again in \(s)s"]
        case .member(let handle): membershipDict = ["state": "member", "path": "", "handle": handle]
        }
        // The door's DeviceRecord name leads when the latest worldview carries this device;
        // PlatformDevice.name remains the honest fallback before the first read or on an older
        // familiar. The editable field writes the record through the signed console-act seam.
        let selfRow = worldview?.members?.first(where: { $0.node_id == node.nodeId })
        let deviceName = selfRow?.label ?? PlatformDevice.name
        // What the MESH says this device serves, as against what this app believes locally
        // (T-199). The app has never read this, so `Mine — Ian's` in the console and
        // `established='MacOnStick'` in the record could not disagree in any way the system was
        // able to notice — there was no comparison to fail. Two truths that cannot conflict in
        // public will conflict in private forever.
        let establishedHandle = selfRow?.human ?? ""
        // ADR-0039's unfinished half shows up as a MACHINE name sitting in the human slot. Name
        // that specifically rather than reporting a vague mismatch: it is a known, diagnosed
        // condition with a known migration, not a mystery.
        let looksLikeMachine = !establishedHandle.isEmpty
            && establishedHandle.lowercased() == deviceName.lowercased().replacingOccurrences(of: " console", with: "")
        // T-171 is deliberately phone-local: the shared Device screen may render what the
        // iPhone already knows about its paired watch, but no observation/worldview field is
        // created here. T-169 owns any later, retention-governed mesh report.
        let watchDict: [String: Any]
        #if os(iOS)
        let watch = PhoneWatchLink.shared
        watchDict = [
            "supported": WCSession.isSupported(),
            "paired": watch.paired,
            "appInstalled": watch.appInstalled,
            "lastSent": watch.lastSent ?? "",
        ]
        #else
        watchDict = ["supported": false]
        #endif
        let d: [String: Any] = [
            "label": PlatformDevice.name,
            "deviceName": deviceName,
            // Which shell is rendering — the console HTML is ONE file shared by the Mac app and
            // the iOS WKWebView, so a screen that belongs only on a Mac needs this to know
            // (T-183). "mac" | "phone" | "ipad".
            "kind": PlatformDevice.kind,
            // This device's own node id — the console needs it to know "is it MY turn",
            // "is this claim for MY human", distinct from the daemon's id in the worldview.
            "node_id": node.nodeId,
            "build": Self.appBuild,
            "host": host,
            "hosts": hosts,
            "membership": membershipDict,
            "attempts": attemptLog,
            // The join story as machine state (T-120): the console shows what is being TRIED
            // while the link is still forming, instead of wearing the failure mark.
            "join": [
                "stage": joinProgress.stage.rawValue,
                "detail": joinProgress.detail,
                "host": joinProgress.host,
                "tries": joinProgress.tries,
                "elapsed": Int(Date().timeIntervalSince(joinProgress.startedAt)),
                "causes": joinProgress.causes,
            ] as [String: Any],
            "servedHuman": servedHuman,
            // T-199: the mesh's own answer, and whether it names a machine instead of a person.
            "establishedHandle": establishedHandle,
            "establishedIsMachine": looksLikeMachine,
            // Join in flight (B6): the console shows a progress indicator instead of snapping
            // back to the static path card while the introduction round-trips.
            "introducing": introducing,
            // Why an introduction is held or was refused — the console shows this instead of
            // nudging for a name the human already gave (T-185).
            "introduceHeld": introduceHeldReason,
            "introduceElapsed": introducing ? Int(Date().timeIntervalSince1970 - introduceStartedAt) : 0,
            // The door's last verdict on a game act (B15) — surfaced on the games screen so a
            // refused BEGIN shows its reason instead of silently bouncing to the finished game.
            "gameNote": gameNote,
            // Claims waiting on this device's human (B7) — the welcome glyph flashes on it.
            "pendingClaims": pendingClaimCount,
            // What the ladder currently believes, so the console can SHOW the belief instead of
            // asking (ADR-0019). An expired claim reports nobody rather than the last person seen.
            "presence": [
                "handle": presence.isLive ? presence.handle : "",
                "via": presence.isLive ? presence.via.rawValue : "",
                "confidence": presence.isLive ? presence.confidence : 0,
                "since": presence.isLive ? Int(presence.since.timeIntervalSince1970) : 0,
            ],
            "deviceRole": deviceRole.rawValue,
            "deviceOwner": deviceOwner,
            "oracle": ConsultRunner.state,
            // Why this device is or is not surveying, in the same spirit as "oracle" above:
            // the console shows the state rather than inferring it from the toggle, because
            // the toggle no longer decides it on its own (T-228 Q2).
            "discovery_state": discoveryState,
            // Push-to-talk is wired on this shell — the dialogue screen shows its mic.
            "voice": dialogueVoiceAvailable,
            "watch": watchDict,
            // The app's recent working notes — the door's verbatim replies to this device's own
            // acts (game moves, vouches, invites). The console shows the newest one; without
            // this, a refused BEGIN looked like a dead button (the door's words landed in a
            // log no screen ever read).
            "notes": Array(log.prefix(5)),
            "consents": [
                "location": locationEnabled, "motion": motionEnabled, "face": faceEnabled,
                "faceRecognition": faceRecognitionEnabled,
                "discovery": discoveryEnabled, "reasoning": reasoningEnabled,
                // ADR-0038: the device-side half of the cloud gate. The hub's allow_llm_cloud
                // stays file-only by boundary doctrine — permission does not compose.
                "pcc": pccEnabled,
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
        // No sensing to start/stop: ConsultRunner reads the flag at each consult
        // (ADR-0038 — it gates where a thought may run, not whether one runs).
        case "pcc": pccEnabled = on
        default: return
        }
        startSensingIfConsented()
        startDiscoveryIfAuthorized()
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

    /// Disable one rule shown by the current worldview. This only narrows standing authority;
    /// the door still verifies full standing, signature, freshness, and the rule id.
    func disableRule(_ id: String) async {
        let ruleId = id.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !ruleId.isEmpty else { return }
        await sendConsoleAct(.disableRule(ruleId), label: "disable rule")
    }

    /// Give this certified device its deliberate DeviceRecord name. The wire act carries no
    /// target id: the door always names the node key that signed it.
    func setDeviceName(_ value: String) async {
        let name = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else {
            note("device name needs at least one visible character")
            return
        }
        await sendConsoleAct(.nameDevice(name), label: "name device")
    }

    @discardableResult
    func decidePartnerGrant(
        requestId: String,
        surface: String,
        allowedOperations: PartnerOperationBounds,
        expiresAt: Int64
    ) async -> PartnerActOutcome {
        await sendConsoleAct(
            .decideGrant(
                requestId: requestId,
                surface: surface,
                allowedOperations: allowedOperations,
                expiresAt: expiresAt
            ),
            label: "grant partner request",
            item: requestId
        )
    }

    /// The explicit result of one console act, for the deciding surface itself. Feedback that
    /// lives only in the notes feed leaves the button sitting there inviting more clicks —
    /// the first ceremony proved a human will keep tapping until SOMETHING answers.
    struct PartnerActOutcome {
        let ok: Bool
        let message: String
    }

    @discardableResult
    func registerPartner(_ registrationId: String) async -> PartnerActOutcome {
        await sendConsoleAct(
            .registerPartner(registrationId),
            label: "register partner",
            item: registrationId
        )
    }

    @discardableResult
    func declinePartnerGrant(_ requestId: String) async -> PartnerActOutcome {
        await sendConsoleAct(
            .declineGrant(requestId), label: "decline partner request", item: requestId)
    }

    @discardableResult
    func revokePartnerGrant(_ grantId: String) async -> PartnerActOutcome {
        await sendConsoleAct(.revokeGrant(grantId), label: "revoke partner grant", item: grantId)
    }

    @discardableResult
    func refusePartnerProposal(_ proposalId: String) async -> PartnerActOutcome {
        await sendConsoleAct(
            .refuseProposal(proposalId), label: "refuse partner proposal", item: proposalId)
    }

    @discardableResult
    private func sendConsoleAct(
        _ act: ConsoleAct, label: String, item: String? = nil
    ) async -> PartnerActOutcome {
        // The act must land on the door that holds the item — partner state is door-local,
        // so a card read from the lighthouse is decided AT the lighthouse even while the
        // console's preferred door is its LAN hub.
        let door = item.flatMap { partnerItemDoors[$0] } ?? host
        guard let session = consoleActSession(door: door) else {
            note("\(label) not sent — no enrolled door")
            return PartnerActOutcome(ok: false, message: "\(label) not sent — no enrolled door")
        }
        let isPartnerDecision: Bool
        switch act {
        case .registerPartner, .decideGrant, .declineGrant, .revokeGrant, .refuseProposal:
            isPartnerDecision = true
        case .disableRule, .nameDevice:
            isPartnerDecision = false
        }
        do {
            let reply = try await ConsoleActClient(session: session).send(act)
            if isPartnerDecision {
                guard let data = reply.data(using: .utf8),
                      (try? JSONDecoder().decode(PartnerInbox.self, from: data)) != nil
                else {
                    partnerInboxJSON = nil
                    partnerInboxError = "decision response was not a valid projection"
                    note("\(label) not confirmed — refresh before another decision")
                    return PartnerActOutcome(
                        ok: false,
                        message: "\(label) not confirmed — the door's reply was not a valid "
                            + "projection; refresh before deciding again")
                }
                // The reply proves the act landed, but it is ONE door's post-append projection —
                // rebuild the merged view across every door before showing decision state again.
                note("✓ \(label) recorded")
                await refreshPartnerInbox()
                await refreshWorldview()
                return PartnerActOutcome(ok: true, message: "✓ \(label) recorded at \(door)")
            }
            note("✓ \(reply)")
            await refreshWorldview()
            return PartnerActOutcome(ok: true, message: "✓ \(reply)")
        } catch ConsoleActClient.ActError.http(let status, let body) {
            note("\(label) refused (\(status)): \(body.prefix(80))")
            // A competing device may have decided first. Read the ledger's actual state back;
            // never leave a stale actionable card after an honest transition conflict.
            await refreshPartnerInbox()
            return PartnerActOutcome(
                ok: false, message: "\(label) refused (\(status)): \(body.prefix(120))")
        } catch ConsoleActClient.ActError.transport(let message) {
            note("\(label) could not reach \(door): \(message.prefix(80))")
            return PartnerActOutcome(
                ok: false, message: "\(label) could not reach \(door): \(message.prefix(120))")
        } catch {
            note("\(label) failed: \(error.localizedDescription)")
            return PartnerActOutcome(
                ok: false, message: "\(label) failed: \(error.localizedDescription)")
        }
    }

    /// A single derived observation from any sensor → the /mesh/observe pipe.
    func emit(_ obs: ObsRecord) {
        Task { await deliver([obs]) }
    }

    /// The human answered the familiar's question in the console — a served-facing observation, so
    /// presence and service register that a person is here and spoke.
    func submitConsoleAnswer() {
        markInteraction()
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
        // Hand the sensor the prior to CHECK. A shared device has none, so it asks instead of
        // searching every known face (ADR-0019 rungs 2–3).
        face.prior = deviceRole == .personal && !deviceOwner.isEmpty ? deviceOwner : nil
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

    /// What the join machinery is DOING right now (T-120) — a state the screens can branch on,
    /// so a slow first join reads as live progress instead of silence resolving to a red mark.
    /// Progress and failure are different facts and must read differently.
    enum JoinStage: String {
        case idle                // nothing in flight
        case reaching            // enrolled: walking the candidate doors for a read
        case seekingDirectory    // asking the lighthouse which meshes are reachable
        case knocking            // presenting the covenant at a door
        case awaitingAdmission   // the door heard the knock; the mesh has not admitted yet
        case admitted            // grant in hand; first worldview read under way
        case joined              // linked — the worldview is flowing
        case unreachable         // terminal this round (retryable): nothing answered / no admission
        case declined            // terminal: the mesh said no
    }
    struct JoinProgress: Equatable {
        var stage: JoinStage = .idle
        var detail = ""            // one human sentence: what is being tried, or what failed
        var host = ""              // the address in hand
        var tries = 0              // admission polls answered "pending" so far
        var startedAt = Date()     // when this stage began (screens derive elapsed)
        var causes: [String] = []  // terminal states: per-address causes, human-readable
    }
    @Published var joinProgress = JoinProgress()

    /// Enter a stage: the clock resets on every transition; in-stage ticks mutate fields instead.
    private func joinStage(_ stage: JoinStage, _ detail: String, host: String = "", causes: [String] = []) {
        joinProgress = JoinProgress(stage: stage, detail: detail, host: host, tries: 0, startedAt: Date(), causes: causes)
    }

    /// **Auto-enroll (ADR-0012).** On first run, ask the baked rendezvous (the lighthouse) what
    /// meshes are reachable and start joining the one it finds — no QR needed. The device attests
    /// the Three Laws and shows its confirmation code; the human approves at the familiar. Falls
    /// back to the QR/paste path when the rendezvous is unreachable or lists nothing.
    func autoEnroll() {
        guard !severedByHuman else { autoEnrollTried = true; return }
        guard !enrolled, !enrolling, !autoEnrollTried else { return }
        autoEnrollTried = true
        let node = self.node
        Task {
            let port = enrollPort > 0 ? enrollPort : 47100
            // Same rule as the status heartbeat: name the failure. `try?` here turned an ATS
            // refusal into "no mesh found", which sent us hunting a directory that was up and
            // answering the whole time. An empty list and an unreachable lighthouse are
            // different facts and must read differently.
            var doors: [MeshDoor] = []
            self.joinStage(.seekingDirectory, "asking the lighthouse which meshes are reachable…",
                           host: Self.rendezvousHost)
            do {
                doors = try await RendezvousClient.directory(host: Self.rendezvousHost, port: port)
            } catch {
                self.joinStage(.unreachable, "couldn't reach the lighthouse — it may be down, or this device offline",
                               causes: ["\(Self.rendezvousHost): \(Self.brief(error))"])
                note("✗ couldn't reach the lighthouse: \(Self.brief(error))")
                return
            }
            guard let door = doors.first else {
                self.joinStage(.unreachable, "the lighthouse answered, but lists no reachable mesh — use an invite")
                note("the lighthouse lists no reachable mesh — use an invite instead")
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

    /// An invite token that arrived with the payload this device scanned — held until it is
    /// spent by a successful introduction (a refusal does not spend it), so an unnamed token
    /// can wait for the human to say who they are.
    private var pendingInvite: InviteToken?

    func requestJoin(from json: String) {
        guard let p = EnrollmentPayload.parse(json) else {
            note("✗ could not read that invite")
            return
        }
        hosts = p.candidateHosts
        host = hosts[0]
        enrollPort = p.port
        groupLabel = p.label
        tlsPin = p.tlspin
        tlsPins = p.pins ?? (p.tlspin.map { [$0] } ?? [])   // seed the group's pin set
        pendingInvite = p.invite   // E3 evidence, if the payload carried one (ADR-0026)
        ensureRendezvous()   // now that we're pinning, trust the lighthouse too (same session)
        saveEnrollment()   // Keychain — durable across reinstalls (UserDefaults is wiped on reinstall)
        enrolling = true
        membership = .knocking
        joinStage(.knocking, "joining “\(p.label)” — presenting the covenant…", host: hosts[0])
        note("joining “\(p.label)” — accepting the Three Laws…")
        let node = self.node
        Task { await self.runHandshake(candidates: self.hosts, port: p.port, node: node) }
    }

    private func runHandshake(candidates: [String], port: Int, node: NodeKey) async {
        // Walk the candidate addresses until one answers — the payload lists them most-universal
        // first, but only the device knows which are reachable from where it is right now.
        var lastError: Error?
        var causes: [String] = []
        for host in candidates {
            let enroller = EnrollmentClient(host: host, port: port)
            joinStage(.knocking, "presenting the covenant at \(host)…", host: host)
            do {
                // Under the two-filter door (ADR-0026) a knock lands a guest cert immediately.
                // The polling loop stays for one release: an OLD familiar still pends, and its
                // poll seam upgrades the pending to a guest grant the moment it redeploys.
                var grant = try await enroller.requestJoin(node: node)
                promoteHost(host)
                if grant == nil {
                    joinStage(.awaitingAdmission,
                              "the door heard the knock — waiting for the mesh to admit this device",
                              host: host)
                    note("waiting for the mesh to answer the knock…")
                }
                var tries = 0
                while grant == nil, tries < 150 {                          // ~5 min of polling
                    try await Task.sleep(nanoseconds: 2_000_000_000)
                    grant = try await enroller.pollGrant(nodeId: node.nodeId)
                    tries += 1
                    joinProgress.tries = tries   // the screens read a live count, not silence
                }
                guard let g = grant else {
                    enrolling = false; membership = .none
                    joinStage(.unreachable,
                              "the door heard the knock, but no one admitted this device in five minutes — try again",
                              host: host)
                    note("… no answer yet — tap to retry"); return
                }
                saveGrant(g)
                enrolling = false
                enrolled = true
                membership = .guest(path: Self.admissionPath)
                joinStage(.admitted, "admitted — reading “\(g.group_label)” as a guest…", host: host)
                note("✓ the covenant is in force — reading “\(g.group_label)” as a guest")
                // Hand the paired Apple Watch this familiar's address so it can enrol itself by
                // covenant (address only — the watch mints its own key + gets its own grant).
                #if os(iOS)
                PhoneWatchLink.shared.sendAddress(host: host, port: port, label: g.group_label, human: servedHuman)
                #endif
                startFixBaseline()
                startSensingIfConsented()
                startDiscoveryIfAuthorized()
                // The payload carried an invite (E3): identity filter, same motion. A NAMED
                // token admits outright; an unnamed one asks the human to say who they are
                // first, and the door's refusal text becomes the guest screen's copy.
                if let tok = pendingInvite {
                    let claim: IdentityClaim? = servedHuman == "observer" ? nil : IdentityClaim(handle: servedHuman)
                    _ = await introduce(claim: claim, evidence: .invite(tok))
                } else if let (claim, evidence) = pendingIntroduction {
                    // The human named themselves before the handshake finished (T-185). Their act
                    // waited; now the door is reachable, so it goes through without their having
                    // to say it a second time.
                    pendingIntroduction = nil
                    note("sending the name you gave earlier…")
                    _ = await introduce(claim: claim, evidence: evidence)
                }
                return
            } catch EnrollmentClient.EnrollError.denied {
                enrolling = false
                membership = .none
                joinStage(.declined, "the mesh declined this device")
                note("✗ the mesh declined this device")
                return
            } catch {
                lastError = error      // unreachable on this path — try the next address
                causes.append("\(host): \(Self.brief(error))")
            }
        }
        enrolling = false
        membership = .none
        joinStage(.unreachable, "couldn't reach the mesh at any address — try again, or use an invite",
                  causes: causes)
        note("… couldn't reach the mesh at any address: \(lastError.map { "\($0)" } ?? "no candidates")")
    }

    // MARK: the identity filter (ADR-0026)

    /// Present evidence at `POST /mesh/introduce`. On yes the device is a member and both sides
    /// hear it; on not-yet the door's words become the guest screen's path-to-admission copy.
    /// One move in the mesh game (begin / guess / line / pass / close), signed and sent to
    /// the door. The judge's reply lands in the activity feed verbatim.
    // ---- APNs (the ember reaches a locked phone) ------------------------------------------
    /// The OS-issued device token, hex — held until the device is enrolled with a door.
    private var apnsToken: String?

    /// The app delegate got a token from the OS. Keep it and hand it to the door.
    func apnsTokenArrived(_ hex: String) {
        apnsToken = hex
        Task { await sendApnsToken() }
    }

    /// Post the token to this device's door (idempotent — the door keeps one row per node).
    /// Called on token arrival and safe to call again after enrollment or a door change.
    func sendApnsToken() async {
        guard enrolled, !host.isEmpty, let tok = apnsToken else { return }
        do {
            let said = try await PushTokenClient(node: node)
                .register(token: tok, host: host, port: enrollPort)
            note("push: \(said)")
        } catch {
            note("push registration failed at door \(host): \(error.localizedDescription)")
        }
    }

    func gameAct(_ act: String, kind: String? = nil, text: String = "", to: String = "",
                 solo: Bool = false) async {
        markInteraction()
        // Never bail silently: a dead-looking button is worse than an error. The note surfaces
        // on the games screen (deviceStateJSON.notes), door named, so a refusal is legible.
        guard !host.isEmpty else {
            note("\(act): no door to act through — this device has no enrolled host")
            return
        }
        do {
            switch try await GameClient(node: node).act(act, kind: kind, text: text, to: to,
                                                        solo: solo,
                                                        host: host, port: enrollPort) {
            case .said(let words):
                gameNote = ""
                note(words.isEmpty ? "the move landed" : words)
            case .refused(let why):
                // Surface the refusal ON THE GAMES SCREEN (B15): a begin that the door refuses
                // (no players present, members only, an already-burning game) used to vanish
                // into a device-screen note while the games view silently bounced to the last
                // finished game after its 12-second grace — reading as "the game won't start".
                gameNote = why
                note("door \(host) refused \(act): \(why)")
            case .error(let e):
                gameNote = e
                note("\(act) failed at door \(host): \(e)")
            }
        } catch {
            gameNote = Self.brief(error)
            note("\(act) failed at door \(host): \(error.localizedDescription)")
        }
        await refreshWorldview()
    }

    /// An enrolled visitor redeeming a pasted invite (the visitor path card's REDEEM box).
    /// Accepts the full enrollment payload OR a bare invite token (the CLI prints the latter).
    func redeemInvite(_ text: String) async {
        guard let data = text.data(using: .utf8) else { return }
        var token: InviteToken?
        if let payload = try? JSONDecoder().decode(EnrollmentPayload.self, from: data) {
            token = payload.invite
        }
        if token == nil {
            token = try? JSONDecoder().decode(InviteToken.self, from: data)
        }
        guard let tok = token else {
            note("that didn't read as an invite — paste exactly what the member sent")
            return
        }
        pendingInvite = tok
        let name = attributedHuman != "observer" ? attributedHuman : tok.expected_handle
        note("redeeming the invite…")
        await introduceMesh(name)
    }

    /// A member welcoming a NEW human in by name — the sponsor's half of vouchFor.
    func sponsorFor(nodeId: String, handle: String) async -> String? {
        guard !host.isEmpty else { return "no host" }
        do {
            switch try await SponsorClient(node: node).sponsor(subject: nodeId, handle: handle,
                                                               host: host, port: enrollPort) {
            case .welcomed(let h):
                note("✓ welcomed \(h) into the mesh")
                await refreshWorldview()
                return nil
            case .refused(let why):
                note("welcome refused: \(why)")
                return why
            case .error(let e):
                note("welcome failed: \(e)")
                return e
            }
        } catch {
            note("welcome failed: \(error.localizedDescription)")
            return error.localizedDescription
        }
    }

    /// One tap on the claimed human's own device (ADR-0026 E2 over the mesh): mint a voucher
    /// for the waiting device's key and deliver it to the door. The rules engine does the rest —
    /// the new device's next poll finds itself a member. Returns the door's words on refusal.
    func vouchFor(nodeId: String, pubkey: String, handle: String) async -> String? {
        guard !host.isEmpty else { return "no host" }
        do {
            let voucher = try DeviceVoucher.mint(node: node, handle: handle, subjectPubkey: pubkey)
            switch try await VouchClient(node: node).vouch(voucher, host: host, port: enrollPort) {
            case .admitted(let h):
                note("✓ vouched — their device is now \(h.isEmpty ? "a member" : h)'s")
                await refreshWorldview()
                return nil
            case .refused(let why):
                note("vouch refused: \(why)")
                return why
            case .error(let e):
                note("vouch failed: \(e)")
                return e
            }
        } catch {
            note("vouch failed: \(error.localizedDescription)")
            return error.localizedDescription
        }
    }

    @discardableResult
    private func introduce(claim: IdentityClaim?, evidence: Evidence) async -> Bool {
        // A human just told the familiar who they are. That is the single most important thing
        // anybody says to it, and it must never be dropped on the floor (T-185).
        //
        // This guard used to be `guard storedGrant() != nil, !host.isEmpty else { return false }`
        // — silent. If the covenant handshake had not landed a grant yet, or no door address was
        // in hand, the name went nowhere and NOTHING was said: the console kept the name (it is
        // local state), membership stayed `guest`, and the nudge went on asking for a name that
        // had already been given. Ian hit exactly this adding an iPad, 2026-08-15.
        //
        // So: say why, and REMEMBER the intent. The introduction is replayed the moment the
        // grant and address exist, because the human's act does not expire just because the
        // handshake had not finished when they made it.
        guard storedGrant() != nil, !host.isEmpty else {
            pendingIntroduction = (claim, evidence)
            let why = storedGrant() == nil
                ? "the covenant handshake hasn't finished — your name is held and will be sent the moment it does"
                : "no door address in hand yet — your name is held and will be sent when one answers"
            introduceHeldReason = why
            note("holding “\(claim?.handle ?? "")” — \(why)")
            return false
        }
        introduceHeldReason = ""
        // Show the join is IN FLIGHT (B6): a name entered kicks off a round trip (evidence,
        // maybe a vouch, a sync) that could take seconds; without this the console snapped back
        // to the static path card and looked like nothing happened.
        introducing = true
        introduceStartedAt = Date().timeIntervalSince1970
        defer { introducing = false }
        let client = AdmissionClient(node: node)
        do {
            switch try await client.introduce(claim: claim, evidence: evidence, host: host, port: enrollPort) {
            case .member(let handle):
                pendingInvite = nil
                pendingIntroduction = nil
                introduceHeldReason = ""
                membership = .member(handle: handle)
                wasRecognised = true          // the worldview edge must not chime twice
                Chime.accepted()
                note(handle.isEmpty ? "✓ admitted to the mesh" : "✓ admitted — established as \(handle)")
                await refreshWorldview()
                return true
            case .notYet(let path):
                membership = .guest(path: path)
                note("… still a guest — \(path)")
            case .held(let s):
                membership = .held(retryIn: s)
                note("… held — try again in \(s)s")
            case .error(let m):
                // The door's own words are the path forward ("that handle already exists here —
                // ask for an invite naming it, or hand off from one of their devices"). Shown
                // verbatim rather than summarised into a shrug.
                introduceHeldReason = m
                note("✗ introduce failed: \(m)")
            }
        } catch {
            note("✗ introduce failed: \(Self.brief(error))")
        }
        return false
    }

    /// The wire half of an introduction: the E4 interaction, or the claim a held unnamed invite
    /// was waiting for. Fired by `confirmPresentHuman` on a guest; callable directly by a join
    /// screen that carries the human's own words.
    func introduceMesh(_ handle: String, statement: String = "") async {
        let claim = IdentityClaim(handle: handle)
        let evidence: Evidence
        if let tok = pendingInvite {
            evidence = .invite(tok)
        } else {
            let words = statement.isEmpty ? "introduced from \(PlatformDevice.name)" : statement
            evidence = .introduction(handle: handle, statement: words,
                                     ts: Int64(Date().timeIntervalSince1970))
        }
        _ = await introduce(claim: claim, evidence: evidence)
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
    var addressPayload: String? { payload(invite: nil) }

    /// The QR an admitted member renders (ADR-0026): the address plus a fresh ten-minute,
    /// single-use, member-signed invite token — so the scanning device knocks and is admitted in
    /// one motion, no third person, no waiting. `handoff: true` names this device's own human
    /// (old user / new device — the deliberate act is this render+scan); `false` leaves the
    /// token unnamed, and the newcomer introduces themselves. A device that cannot mint yet (a
    /// guest) falls back to the plain address payload, which still lands the scanner as a guest.
    func invitePayload(handoff: Bool) -> String? {
        var token: InviteToken?
        if case .member = membership, let g = storedGrant() {
            let mine = attributedHuman
            let named = handoff && mine != "observer" ? mine : ""
            token = try? InviteToken.mint(node: node, membership: g.membership, expectedHandle: named)
        }
        return payload(invite: token)
    }

    private func payload(invite: InviteToken?) -> String? {
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
                                  tlspin: tlsPin, pins: pinSet.isEmpty ? nil : pinSet,
                                  invite: invite)
        guard let data = try? JSONEncoder().encode(p) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    func unenroll() {
        // Tell the mesh this identity is RELEASED before forgetting how to reach it — the
        // record travels as a self-Disestablish correction, so the roster stops naming who
        // left and the next human on this hardware introduces themselves fresh. Best-effort:
        // leaving must never wait on the network.
        if !host.isEmpty {
            let releaseHost = host, releasePort = enrollPort, releaseNode = node
            Task.detached {
                await ReleaseClient(node: releaseNode).release(host: releaseHost, port: releasePort)
            }
        }
        KeychainStore.delete(account: grantAccount)
        KeychainStore.delete(account: enrollAccount)
        // A severed device holds no cached claim about the familiar it left — the App
        // Intents projection dies with the enrollment (codex's brick-1 return).
        IntentProjection.clear()
        host = ""
        hosts = []
        #if os(iOS)
        coordinator?.stop()
        coordinator = nil
        discovery?.stop()
        discovery = nil
        #endif
        enrolled = false
        membership = .none
        pendingInvite = nil
        // A severing is a human's deliberate act — the device must NOT quietly rejoin the
        // moment the join screen appears (it did: auto-enroll fired instantly and the mesh,
        // still holding this key's record, handed the old identity straight back — there was
        // no way to leave, and no way to test arriving). Severed stays severed until the
        // human explicitly asks to join again.
        severedByHuman = true
        autoEnrollTried = true   // the join screen shows the explicit button, not the spinner
        // And a severed device forgets whom it served: the serving relationship ended with
        // the membership. Without this, "MINE — IAN'S" pre-filled from the old life before
        // the new one had said a single name.
        servedHuman = "observer"
        deviceOwner = ""
        deviceRoleRaw = DeviceRole.suggested.rawValue
        answeredClaim = nil
        faceClaim = nil
        refreshPresence()
        note("severed by your hand — this device will not rejoin until you ask it to")
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
            if let answer = await ConsultRunner.answer(p.prompt, kind: p.kind, cloudOK: p.cloud_ok ?? false) {
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
        // Same DOOR only: each door serves its own worldview (its theories, its feed), so a
        // tailnet address that answers as a DIFFERENT node is a different door, not a better
        // path — promoting it swapped the whole console between doors every probe cycle (the
        // theories screen flickered between wildhorse's and the lighthouse's). A path upgrade
        // must keep the node identity fixed.
        guard let reading = worldview?.node_id else { return }
        if let heard = await Self.helloNodeId(host: tailnet, port: enrollPort), heard == reading {
            promoteHost(tailnet)   // data now flows peer-to-peer over Tailscale; the badge flips
            note("↔ Tailscale confirmed — data over \(tailnet)")
        }
    }

    /// Who answers at a candidate path — GET /mesh/hello's node_id, or nil if unreachable.
    static func helloNodeId(host: String, port: Int) async -> String? {
        guard let url = URL(string: "https://\(host):\(port)/mesh/hello") else { return nil }
        var r = URLRequest(url: url)
        r.timeoutInterval = 4
        guard let (data, resp) = try? await MeshTLS.session.data(for: r),
              ((resp as? HTTPURLResponse)?.statusCode ?? 0) == 200,
              let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else {
            return nil
        }
        return obj["node_id"] as? String
    }


    /// Heartbeat this device's status to the lighthouse (ADR-0017) — status flows through the always-
    /// on hub so the mesh sees this device whatever path it's on. The connectivity mode is classified
    /// from the host it actually read its worldview from just now.
    func heartbeatStatus(readHost: String) async {
        guard let g = storedGrant() else { return }
        // The presence CLAIM, not the persisted default: what the ladder concluded, how, how sure,
        // and since when (ADR-0019). An expired claim reports nobody rather than the last person
        // we saw — "Jeff is here" and "Jeff was here an hour ago" must not arrive as the same fact.
        let claim = presence
        var live = claim.isLive
        var presentHuman = live ? claim.handle : ""
        var presentVia = live ? claim.via.rawValue : ""
        var presentSince = live ? Int64(claim.since.timeIntervalSince1970) : 0
        var presentConfidence = live ? claim.confidence : 0
        // Active use of the console IS presence (B17): if the ladder holds no live claim but the
        // human is right here — served handle known, and they've acted within the last ten
        // minutes — say so plainly ("I'm typing, I'm present") rather than reporting "unknown".
        // A real face/asked claim, being stronger, always wins the branch above.
        if !live, !servedHuman.isEmpty, servedHuman != "observer",
           Date().timeIntervalSince1970 - lastInteractionAt < 600 {
            live = true
            presentHuman = servedHuman
            presentVia = "interaction"
            presentSince = Int64(lastInteractionAt)
            presentConfidence = 0.5
        }
        let status = StatusClient.Member(
            node_id: node.nodeId,
            actor: DeviceActor.current,
            label: PlatformDevice.name,
            present_human: presentHuman,
            connectivity: Self.connectivityMode(readHost),
            present_via: presentVia,
            present_since: presentSince,
            present_confidence: presentConfidence,
            lat: myLat,
            lon: myLon
        )
        let client = StatusClient(node: node, membership: g.membership, groupPubkey: g.group_pubkey)
        // Heartbeat to the lighthouse (the mesh's meeting point) AND to the door we just read
        // (B11): presence/session are computed from local evidence, so a device whose status
        // only ever reached the lighthouse read "unknown / no GPS / — session" on every other
        // door. Sending to the read door too lets that door's roster see us present. Deduped so
        // we never send twice to the same address.
        var doors = [Self.rendezvousHost]
        if !readHost.isEmpty, readHost != Self.rendezvousHost { doors.append(readHost) }
        var anyOK = false
        var lastErr: Error?
        for h in doors {
            do {
                if try await client.send(status, host: h, port: enrollPort) { anyOK = true }
            } catch {
                lastErr = error
            }
        }
        if anyOK != (lastStatusOK ?? false) {
            note(anyOK ? "↑ status reaching the mesh" : "✗ the mesh rejected this device's status")
        }
        if !anyOK, let e = lastErr, lastStatusOK ?? true {
            note("✗ status heartbeat failed: \(Self.brief(e))")
        }
        lastStatusOK = anyOK
    }

    /// The wall-clock of the last deliberate human act at this console — a typed answer, a name,
    /// a game move, a consent toggle. Feeds the interaction-presence signal (B17).
    var lastInteractionAt: TimeInterval = 0
    func markInteraction() { lastInteractionAt = Date().timeIntervalSince1970 }

    /// This device's own last GPS fix, relayed in the status heartbeat so it shows at its TRUE
    /// location on every door (B11/B19). Set by the platform shells (iOS CoreMotion coordinator,
    /// Mac CoreLocation). 0,0 = no fix / location not consented.
    var myLat = 0.0
    var myLon = 0.0
    func setMyFix(lat: Double, lon: Double) {
        guard lat != 0 || lon != 0 else { return }
        myLat = lat
        myLon = lon
    }

    /// Guests waiting as of the previous successful read — nil until the first one, so launch is
    /// silent. The chime is an arrival, not a standing condition. (Fallback for old familiars;
    /// the arrivals list below is the real signal.)
    private var lastGuestsWaiting: Int?
    /// Arrival ids as of the previous read — nil until the first, so launch greets nobody twice.
    private var knownArrivalIds: Set<String>?
    /// Claim edge keys, "node_id:since" — a fresh claim.ts re-triggers even for a device whose
    /// node id we already knew (a rejoining phone). nil until the first read (launch-silent).
    private var knownClaimKeys: Set<String>?
    /// Live count of claims waiting on THIS device's human — the welcome glyph flashes on it,
    /// so a waiting acceptance is visible even on the first read (B7).
    @Published var pendingClaimCount = 0
    private var wasMyTurn = false
    /// Whether the current finished game's win was already celebrated — reset when a game is
    /// open or absent, so the fanfare rings once per win (B13).
    private var wonGameShown = false
    private var preferredReadFails = 0
    /// Consecutive reads that served this device the guest projection. Demotion waits for
    /// three — a single projected read (a fallback door's stale roll, a mid-merge worldview)
    /// must not flap the welcome screen between member and visitor.
    private var unrecognisedReads = 0
    /// Whether this device stood at full standing as of the previous read. nil until the first,
    /// so launching already-recognised is silent.
    private var wasRecognised: Bool?

    /// Whether the last status heartbeat landed — nil until the first attempt. Only used to log
    /// transitions, so a persistent failure doesn't flood the activity log every 5 seconds.
    private var lastStatusOK: Bool?

    /// A short, human-legible cause from an arbitrary error — URLError codes are what actually
    /// show up here (ATS refusal, timeout, no route), and their raw descriptions are long.
    static func brief(_ error: Error) -> String {
        if let u = error as? URLError { return "\(u.code.rawValue) \(u.localizedDescription)" }
        return (error as NSError).localizedDescription
    }

    /// Classify a worldview-read host into a connectivity mode for the roster badge (ADR-0017):
    /// the always-on lighthouse, a Tailscale (100.64/10) path, or a local/LAN path.
    static func connectivityMode(_ host: String) -> String {
        if host == rendezvousHost { return "lighthouse" }
        if isTailnet(host) { return "tailscale" }
        return "local"
    }

    /// True when a label is only the node id wearing a haircut — the doors' own fallback is
    /// `node_id[..8]` when a device has no name — so it must never lead anything said to the
    /// human; the id stays small print.
    static func idLed(_ label: String, nodeId: String) -> Bool {
        let l = label.trimmingCharacters(in: .whitespaces).lowercased()
        let n = nodeId.trimmingCharacters(in: .whitespaces).lowercased()
        if l.isEmpty { return true }
        return !n.isEmpty && (n.hasPrefix(l) || l.hasPrefix(n))
    }

    /// The name a node leads with in anything said to the human — its record's established
    /// handle (ADR-0027: never a cached brief's word), else the device's own label, else what
    /// the mesh honestly knows ("an unnamed device"), with the id demoted to a parenthesis.
    func displayName(for nodeId: String) -> String {
        let m = worldview?.members?.first { $0.node_id == nodeId }
        if let h = m?.human, !h.isEmpty { return h }
        if let l = m?.label, !Self.idLed(l, nodeId: nodeId) { return l }
        return "an unnamed device (\(nodeId.prefix(8)))"
    }

    /// A member's deliberate act about another device (ADR-0026 §5): corrections — sever,
    /// disestablish ("that's not Betty"), hold, restore — signed and sent to the node this
    /// device reads from; the record travels from there. Approval is gone: admission is
    /// rules-based, so there is nothing to grant. The old console acts still arrive here for
    /// one release and are translated honestly: "deny" is a hold; "grant" falls through to the
    /// legacy standing alias, which an old familiar still honors.
    func decideStanding(_ subject: String, act: String) async {
        guard let g = storedGrant(), !host.isEmpty else {
            note("✗ not enrolled yet")
            return
        }
        let mapped: String?
        switch act {
        case "deny": mapped = "hold"
        case "sever", "disestablish", "hold", "restore": mapped = act
        default: mapped = nil
        }
        if let mact = mapped {
            let client = CorrectionClient(node: node, membership: g.membership, groupPubkey: g.group_pubkey)
            do {
                switch try await client.correct(subject: subject, act: mact,
                                                reason: "from the console",
                                                host: host, port: enrollPort) {
                case .applied(let state):
                    note("✓ \(displayName(for: subject)) — \(mact) (now \(state))")
                case .refused(let why):
                    note("✗ \(mact) refused: \(why)")
                }
            } catch {
                note("✗ correction failed: \(Self.brief(error))")
            }
        } else {
            // Legacy "grant" against an old familiar — the one-release alias.
            let client = StandingClient(node: node, membership: g.membership, groupPubkey: g.group_pubkey)
            do {
                switch try await client.cast(subject: subject, act: act, host: host, port: enrollPort) {
                case .decided(let said):
                    if act == "grant" { Chime.accepted() }
                    note("✓ \(displayName(for: subject)) — \(said)")
                case .alreadyDecided(let said):
                    note("· \(displayName(for: subject)) — someone already decided (\(said))")
                case .refused(let why):
                    note("✗ standing refused: \(why)")
                }
            } catch {
                note("✗ standing vote failed: \(Self.brief(error))")
            }
        }
        await refreshWorldview()   // reflect the record immediately
    }

    /// A member's federation tap (ADR-0033): welcome a pending sibling mesh, or sever a
    /// standing one. Travels signed to the door, like a standing decision.
    func federateAct(_ act: String, subjectGroupId: String, reason: String = "") async {
        guard let g = storedGrant(), !host.isEmpty else {
            note("✗ federation: not enrolled or no host")
            return
        }
        let client = FederateClient(node: node, membership: g.membership, groupPubkey: g.group_pubkey)
        do {
            switch try await client.cast(subjectGroupId: subjectGroupId, act: act,
                                         reason: reason, host: host, port: enrollPort) {
            case .done(let said):
                if act == "welcome" { Chime.accepted() }
                note("✓ federation — \(said)")
            case .refused(let why):
                note("✗ federation \(act) refused: \(why)")
            }
        } catch {
            note("✗ federation \(act) failed: \(Self.brief(error))")
        }
        await refreshWorldview()
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

    /// A signing session pointed at one door's narrow console write seam.
    func consoleActSession(door: String) -> ObservationClient.Session? {
        guard let g = storedGrant(), !door.isEmpty,
              let url = ConsoleActClient.consoleActURL(host: door, port: enrollPort)
        else { return nil }
        return ObservationClient.Session(node: node, membership: g.membership, url: url)
    }

    /// A signing session pointed at one door's private partner projection.
    func partnerInboxSession(door: String) -> ObservationClient.Session? {
        guard let g = storedGrant(), !door.isEmpty,
              let url = PartnerInboxClient.inboxURL(host: door, port: enrollPort)
        else { return nil }
        return ObservationClient.Session(node: node, membership: g.membership, url: url)
    }

    /// One human-addressed view across EVERY candidate door, not just the promoted one.
    /// Partner state is door-local: a registration staged at the public lighthouse used to be
    /// invisible to a console sitting next to its LAN hub (the first ceremony stalled on
    /// exactly this). Each door's read stays fail-closed on its own state; a door that cannot
    /// answer becomes a warning line instead of blanking the doors that can.
    func refreshPartnerInbox() async {
        let doors = hosts.filter { !$0.isEmpty }
        guard storedGrant() != nil, !doors.isEmpty else {
            partnerInboxJSON = nil
            partnerInboxError = "not available — no enrolled door"
            return
        }
        var raws: [(door: String, raw: Data)] = []
        var troubles: [String] = []
        await withTaskGroup(of: (String, Result<Data, PartnerInboxClient.ReadError>).self) { group in
            for door in doors {
                guard let session = partnerInboxSession(door: door) else { continue }
                group.addTask {
                    do {
                        let (_, raw) = try await PartnerInboxClient(session: session).fetchWithRaw()
                        return (door, .success(raw))
                    } catch let error as PartnerInboxClient.ReadError {
                        return (door, .failure(error))
                    } catch {
                        return (door, .failure(.transport(error.localizedDescription)))
                    }
                }
            }
            for await (door, result) in group {
                switch result {
                case .success(let raw): raws.append((door, raw))
                case .failure(.http(let status, let body)):
                    troubles.append("door \(door) refused (\(status)): \(body.prefix(60))")
                case .failure(.transport(let message)):
                    troubles.append("door \(door) unreachable: \(message.prefix(60))")
                case .failure:
                    troubles.append("door \(door) sent an unreadable reply")
                }
            }
        }
        guard !raws.isEmpty else {
            partnerInboxJSON = nil
            partnerInboxError = troubles.sorted().joined(separator: " · ")
            return
        }
        // Merge in door-preference order so an item federated to two doors keeps its
        // most-preferred provenance; the act for an item must return to the door that holds it.
        raws.sort { (doors.firstIndex(of: $0.door) ?? .max) < (doors.firstIndex(of: $1.door) ?? .max) }
        let sections = [
            ("pending_registrations", "registration_id"),
            ("pending_requests", "request_id"),
            ("active_grants", "grant_id"),
            ("pending_proposals", "proposal_id"),
        ]
        var doorByItem: [String: String] = [:]
        var merged: [String: [Any]] = ["warnings": []]
        for (section, _) in sections { merged[section] = [] }
        for (door, raw) in raws {
            guard let view = (try? JSONSerialization.jsonObject(with: raw)) as? [String: Any] else {
                troubles.append("door \(door) sent an unreadable reply")
                continue
            }
            for (section, idKey) in sections {
                for item in view[section] as? [[String: Any]] ?? [] {
                    guard let id = item[idKey] as? String, doorByItem[id] == nil else { continue }
                    doorByItem[id] = door
                    var tagged = item
                    tagged["door"] = door
                    merged[section]?.append(tagged)
                }
            }
            for warning in view["warnings"] as? [String] ?? [] {
                merged["warnings"]?.append("\(door): \(warning)")
            }
        }
        for trouble in troubles.sorted() { merged["warnings"]?.append(trouble) }
        guard let body = try? JSONSerialization.data(withJSONObject: merged),
              let json = String(data: body, encoding: .utf8)
        else {
            partnerInboxJSON = nil
            partnerInboxError = "could not merge the door views"
            return
        }
        partnerItemDoors = doorByItem
        partnerInboxJSON = json
        partnerInboxError = nil
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
        // Every round RE-STARTS at the preferred door: a fallback read is data, not a
        // defection. Each door serves its own worldview, so swapping doors on every hiccup
        // flapped the whole console (roster nesting, theories) between two houses' truths.
        if let preferred = hosts.first { host = preferred }
        let preferred = host
        var attempts: [String] = []   // per-host diagnostic, surfaced if every candidate fails
        // Walk a SNAPSHOT of the candidates. Fallback used to rotate-and-persist the
        // preference order itself, so ONE dropped read re-homed the console to another door
        // for good — and each door serves its own truth, so the welcome screen flapped
        // member/visitor. Trying the next door is for this round only; only the five-miss
        // hysteresis below may change the preference.
        let candidates = hosts.isEmpty ? [host] : hosts
        // The SECOND journey narrates too (T-132). An enrolled console at launch is not
        // joining — it is walking its doors for a first read, and that walk can take a
        // minute across timeouts. T-120 taught the join to speak; silence here resolved
        // into the same red mark it was meant to retire. Progress and failure remain
        // different facts: only a walk that exhausts every candidate is a failure.
        if joinProgress.stage != .joined {
            joinStage(.reaching, "reaching the mesh — trying \(candidates.first ?? host)…",
                      host: candidates.first ?? host)
        }
        for candidate in candidates {
            host = candidate
            let tried = candidate
            if joinProgress.stage == .reaching {
                joinProgress.host = candidate
                joinProgress.detail = "reaching the mesh — asking \(candidate)…"
                joinProgress.tries += 1
            }
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
                // Remember our own fix so the status heartbeat can relay it to every door
                // (B11/B19) — otherwise a device seen only via the lighthouse reads unlocated,
                // or scatters onto whoever is looking (Leif saw wildhorse "in Phoenix").
                if let f = fix { setMyFix(lat: f.lat, lon: f.lon) }
                let (view, raw) = try await WorldviewClient(session: session)
                    .fetchWithRaw(clientVersion: Self.appBuild, osVersion: Self.osRelease,
                                  lat: fix?.lat ?? 0, lon: fix?.lon ?? 0)
                // Someone new has JOINED (ADR-0026: the welcome is a greeting, not a gate).
                // Edge-triggered on arrival ids, and deliberately silent on the FIRST read after
                // launch — otherwise every launch announces yesterday's arrivals. Falls back to
                // the old guests-waiting edge against a familiar that predates arrivals.
                if let arr = view.arrivals {
                    let ids = Set(arr.map { $0.node_id })
                    if let known = knownArrivalIds {
                        let fresh = arr.filter { !known.contains($0.node_id) && $0.node_id != node.nodeId }
                        if !fresh.isEmpty {
                            Chime.guestWaiting()
                            // The established handle leads the greeting; a device with no name
                            // is greeted as what it is, never as a bare hex id.
                            let names = fresh.map { a in
                                !a.handle.isEmpty ? a.handle
                                    : !Self.idLed(a.label, nodeId: a.node_id) ? a.label
                                    : "an unnamed device (\(a.node_id.prefix(8)))"
                            }
                            note("welcome \(names.joined(separator: ", ")) — new to the mesh")
                        }
                        // ACCUMULATE, never replace: a read that momentarily lost an arrival
                        // (door failover, a freshness-boundary flicker) made the same visitor
                        // read as "new" again on the next poll — the join chime rang on loop
                        // for one static guest, live, 2026-08-08. Once greeted, greeted.
                        knownArrivalIds = known.union(ids)
                    } else {
                        knownArrivalIds = ids
                    }
                } else {
                    let waiting = view.guests_waiting ?? 0
                    if let before = lastGuestsWaiting, waiting > before {
                        Chime.guestWaiting()
                        note("someone new is at the door")
                    }
                    lastGuestsWaiting = waiting
                }

                // Someone's new device is claiming THIS device's human (E2 over the mesh) —
                // the second person is us. A waiting acceptance must announce itself (B7): the
                // old edge stayed silent for a REJOINING device (its node id was already known,
                // so a fresh claim.ts moving forward was invisible) and only chimed while THIS
                // console still read as a member. Now: key on (node_id, since) so a fresh claim
                // re-triggers, drop the member guard, and keep a live count for the glyph to
                // flash on. First read is still silent (seeded), but pendingClaimCount shows it.
                if let claims = view.claims_waiting {
                    let mine = claims.filter {
                        $0.handle.caseInsensitiveCompare(attributedHuman) == .orderedSame
                    }
                    pendingClaimCount = mine.count
                    let keys = Set(mine.map { "\($0.node_id):\($0.since)" })
                    if let known = knownClaimKeys {
                        let fresh = mine.filter { !known.contains("\($0.node_id):\($0.since)") }
                        if !fresh.isEmpty {
                            Chime.guestWaiting()
                            let names = fresh.map { c in
                                Self.idLed(c.label, nodeId: c.node_id)
                                    ? "an unnamed device (\(c.node_id.prefix(8)))" : c.label
                            }.joined(separator: ", ")
                            note("\(names) says it is yours — confirm on the welcome screen")
                        }
                    }
                    knownClaimKeys = keys
                }

                // The ember reached this device (the mesh games): edge-triggered chime, so a
                // player who wandered off hears their turn arrive.
                // The turn belongs to the HUMAN: chime when the holder handle is this
                // device's human — whichever of their devices they're nearest.
                let myHandle = attributedHuman.lowercased()
                let myTurn = view.game.map {
                    $0.status == "open" && myHandle != "observer" && !myHandle.isEmpty
                        && $0.holder.lowercased() == myHandle
                } ?? false
                if myTurn && !wasMyTurn {
                    Chime.guestWaiting()
                    switch view.game?.kind {
                    case "campfire":
                        note("🔥 the ember has reached you — add your line")
                    case "changeling":
                        note(view.game?.phase == "voting"
                             ? "🎭 three lines, one human truth — come vote"
                             : "🎭 your round to witness — one true line")
                    case "pact":
                        note(view.game?.phase == "gambit"
                             ? "⚖️ your temptation — write the request"
                             : "⚖️ the constitution has dealt — come rule")
                    default:
                        note("🧩 your turn — the riddle waits on you")
                    }
                }
                #if os(iOS)
                // The wrist is a device of the holder too (the law of the fire): flame on the
                // rising edge, cleared on the falling one.
                if myTurn != wasMyTurn {
                    PhoneWatchLink.shared.sendEmber(myTurn, kind: view.game?.kind ?? "riddle")
                }
                #endif
                wasMyTurn = myTurn

                // A riddle just SOLVED (B13): ring the fanfare once, on the win edge. Reset when
                // a game is open or absent so a finished game doesn't re-ring every poll.
                let riddleWon = view.game.map {
                    $0.status == "done" && !($0.winner ?? "").isEmpty && $0.kind == "riddle"
                } ?? false
                if riddleWon && !wonGameShown {
                    wonGameShown = true
                    Chime.fanfare()
                } else if !(view.game.map { $0.status == "done" } ?? false) {
                    wonGameShown = false
                }

                // Were WE just admitted? The moment this device's own id appears on the roll it
                // had been absent from — an admission completed elsewhere (another door, a
                // correction restored). Being accepted should be felt on the accepted device.
                // Edge-triggered and silent on the first read; an introduce() on THIS device
                // already chimed and pre-set the edge.
                let recognisedNow = (view.standing_full ?? []).contains(node.nodeId)
                if let was = wasRecognised, !was, recognisedNow {
                    Chime.accepted()
                    note("✓ admitted — reading the mesh in full")
                }
                wasRecognised = recognisedNow
                // Keep the fine state honest against what the mesh actually serves: a device
                // the roll knows is a member; one it doesn't reads projected, whatever we
                // believed. The door's copy is kept when we already have specific words.
                if recognisedNow {
                    unrecognisedReads = 0
                    if case .member = membership {} else { membership = .member(handle: "") }
                } else if enrolled {
                    unrecognisedReads += 1
                    if unrecognisedReads >= 3 {
                        if case .guest = membership {} else if case .held = membership {} else {
                            membership = .guest(path: Self.admissionPath)
                        }
                    }
                }
                worldview = view
                worldviewJSON = String(data: raw, encoding: .utf8)
                worldviewError = nil
                // Refresh the external-indexed projection App Intents serve (T-227 Q2):
                // kind-only by construction — the scrub happens at projection time, so an
                // intent can never reach for more than this deliberately narrow cache.
                IntentProjection.project(
                    from: view,
                    oracleLine: ConsultRunner.state,
                    now: Int64(Date().timeIntervalSince1970)
                ).store()
                // The boundary arrived (or changed) with this read — a gate the human just shut must
                // stand the survey down, and one just opened must arm it, without waiting for the
                // person to touch a toggle. Cheap and idempotent: it returns immediately when the
                // authorization is unchanged.
                startDiscoveryIfAuthorized()
                attemptLog = []
                // This is a separate fresh signed request to the exact door that answered.
                // It never becomes part of the worldview snapshot or its diagnostics.
                await refreshPartnerInbox()
                // The first successful read closes the join story (T-120) — but only on a
                // transition, so the every-few-seconds poll doesn't churn the stage clock.
                if joinProgress.stage != .joined {
                    joinStage(.joined, "linked — the worldview is flowing", host: host)
                }
                // A voice turn is answered by voice — speak the reply this read carried in.
                speakReplyIfDue(view)
                // Loyalty with hysteresis: only a preferred door that keeps failing loses its
                // place. Five consecutive misses ≈ 15s of silence — a real outage, not a hiccup.
                if tried == preferred {
                    preferredReadFails = 0
                    promoteHost(host)
                } else {
                    preferredReadFails += 1
                    if preferredReadFails >= 5 {
                        preferredReadFails = 0
                        promoteHost(host)
                        note("↪ reading from \(host) — \(preferred) stopped answering")
                    }
                }
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
            }
        }
        // Every candidate failed — surface the full picture so the cause is diagnosable at a glance:
        // trusted-pin counts (enrolled + baked) and each host's error code. The per-attempt lines
        // also land in attemptLog, which the Device screen already renders (T-120 — the field
        // existed with a render path but was never written).
        attemptLog = attempts
        worldviewError = "pins \(MeshTLS.pins.count)+\(MeshTLS.alwaysTrust.count) · " + attempts.joined(separator: " ")
        // Every door refused or timed out — NOW it is a failure, and it says which and why
        // (T-132). A link that had been joined and dropped narrates the same way.
        if joinProgress.stage == .reaching || joinProgress.stage == .joined {
            joinStage(.unreachable,
                      "no door answered — retrying every few seconds",
                      causes: attempts)
        }
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

    /// The household boundary as this device last read it — the human's authorization, which is the
    /// authority every client follows (Ian, 2026-08-24: "The clients are authorized by the user, so
    /// thats the authority that they both should follow"). Nil until the first worldview read lands,
    /// and nil reads as shut everywhere it is consulted.
    var boundaryGates: GateStates? { worldview?.gates }

    /// Why this device is or is not surveying, in words a console can show — **iOS only**; empty on
    /// macOS, where the Mac's own sensing owns this state. Four refusals that look alike from
    /// outside are kept apart deliberately: a boundary we have not heard yet, a door too
    /// old to report one, a gate the human shut, and a device that declined locally are different
    /// facts, and collapsing them is how a system starts substituting a plausible surface for a
    /// missing capability. Not modelled here: whether iOS itself has granted Local Network
    /// permission — there is no API to ask, so this string must never imply it (that honesty is
    /// T-228 Q5's, not this brick's).
    var discoveryState: String {
        #if os(iOS)
        guard enrolled else { return "not enrolled" }
        guard let gates = boundaryGates else { return "waiting to read the boundary" }
        guard gates.reportsSensorGates else { return "this door does not report the boundary" }
        guard gates.networkDiscoveryOpen else { return "closed by the household boundary" }
        guard discoveryEnabled else { return "declined on this device" }
        return discovery == nil ? "authorized" : "surveying \(NetworkDiscovery.serviceTypes.count) service kinds"
        #else
        // The Mac surveys too, but through its own path (`MacSensing`, gated on `MacBoundary`'s
        // read of boundary.json on the local disk — platform-appropriate enforcement of the same
        // authorization). This shared model cannot see that state and must not guess at it:
        // "unsupported" would be a plain lie, and any confident string would be a fabricated
        // surface for a capability this type does not own. Empty means "not reported here", the
        // same convention `presence.handle` already uses for nobody.
        return ""
        #endif
    }

    /// Survey the local network by Bonjour and report what's out there — the device's view of the
    /// mesh's surroundings becomes the familiar's (and its peers').
    ///
    /// **Authorization, not consent** (Ian, 2026-08-24, T-228 Q2). Two things must hold and they are
    /// the same human speaking in two places: the household boundary's `network_discovery` gate, and
    /// this device's own preference. The boundary is the authority; the local preference may only
    /// narrow it — it can decline, never permit. Before this brick, iOS armed on the local toggle
    /// alone and never read the boundary at all (it could not: the field was missing from the Swift
    /// mirror of `GateStates`), while macOS gated the same act on the boundary. One act, two
    /// authorities; this is the half that was wrong.
    func startDiscoveryIfAuthorized() {
        #if os(iOS)
        guard enrolled,
              let gates = boundaryGates, gates.networkDiscoveryOpen,
              discoveryEnabled
        else {
            if discovery != nil { note("network discovery stood down — \(discoveryState)") }
            discovery?.stop()
            discovery = nil
            return
        }
        // Already surveying: leave it alone. `NetworkDiscovery.start()` calls `stop()` first, which
        // clears the per-run `seen` set — so re-arming on every worldview read would re-report every
        // service on the network at the poll interval. This guard is what makes it safe to call this
        // from the read path at all.
        guard discovery == nil else { return }
        let d = NetworkDiscovery { [weak self] batch in await self?.deliver(batch) }
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
        for candidate in (hosts.isEmpty ? [host] : hosts) {
            host = candidate
            guard let session = makeSession() else { return }
            do {
                let n = try await ObservationClient(session: session).send(batch)
                sentCount += n
                promoteHost(host)
                note("→ sent \(n): " + batch.map { $0.object }.joined(separator: ", "))
                return
            } catch {
                note("… send failed via \(candidate): \(error)")
            }
        }
    }

    private func note(_ s: String) {
        log.insert(s, at: 0)
        if log.count > 100 { log.removeLast(log.count - 100) }
    }
}
