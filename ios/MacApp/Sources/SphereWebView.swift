import SwiftUI
import WebKit
import MapKit
import CoreLocation
import FamiliarMesh

// The Metal Sphere console (imported from Claude Design "Familiar Metal Sphere.dc.html"):
// a WKWebView renders the satellite globe + hologram (Resources/sphere/index.html), and the
// street surface is REAL Apple Maps — a native MKMapView under the (transparent) web layer.
// In street mode the web layer stays as the arc overlay: the host projects every node's
// coordinate to screen points ~10×/s and the page draws the electric arcs over the map.
// All daemon I/O is native (loopback /local/worldview → window.sphereUpdate(); answers and
// gate flips back over the script-message bridge). The web layer never fakes a map and
// never touches the network for data.
struct SphereConsole: View {
    @EnvironmentObject var bridge: SphereBridge

    var body: some View {
        ZStack(alignment: .top) {
            MeshMapView(bridge: bridge)
                .ignoresSafeArea()
                .opacity(bridge.mode == .street ? 1 : 0)
                .animation(.easeInOut(duration: 1.6), value: bridge.mode)
            SphereWebView(bridge: bridge)
                .ignoresSafeArea()
                .allowsHitTesting(bridge.mode != .street)
            if bridge.mode == .street {
                // Wordless exit, in the house glyph language — back up to orbit.
                VStack {
                    Spacer()
                    Button(action: { bridge.backToGlobe() }) {
                        OrbitGlyph()
                            .frame(width: 56, height: 56)
                            .background(Color(red: 0.035, green: 0.06, blue: 0.125).opacity(0.55), in: Circle())
                            .overlay(Circle().stroke(Color(red: 0.52, green: 0.81, blue: 1.0).opacity(0.25), lineWidth: 1))
                    }
                    .buttonStyle(.plain)
                    .padding(.bottom, 16)
                }
            }
            // Invisible drag region: the window has no titlebar or visible edge, so the top
            // strip (between the corner controls) moves the window. No chrome appears.
            WindowDragRegion()
                .frame(height: 36)
                .padding(.horizontal, 90)
        }
        .background(Color.black)
    }
}

// The orbit glyph (sphere + meridians + orbiting satellite dot), matching the page's
// animated glyph language — stroked cyan, breathing meridian, dot in continuous orbit.
struct OrbitGlyph: View {
    var body: some View {
        TimelineView(.animation) { tl in
            let t = tl.date.timeIntervalSinceReferenceDate
            Canvas { ctx, size in
                let c = CGPoint(x: size.width / 2, y: size.height / 2)
                let r = min(size.width, size.height) * 0.30
                let ink = Color(red: 0.81, green: 0.88, blue: 1.0)
                var sphere = Path()
                sphere.addEllipse(in: CGRect(x: c.x - r, y: c.y - r, width: 2 * r, height: 2 * r))
                ctx.stroke(sphere, with: .color(ink.opacity(0.8)), lineWidth: 1.5)
                var equator = Path()
                equator.addEllipse(in: CGRect(x: c.x - r, y: c.y - r * 0.36, width: 2 * r, height: 0.72 * r))
                ctx.stroke(equator, with: .color(ink.opacity(0.5)), lineWidth: 1.2)
                let squash = abs(sin(t * 0.63)) * 0.9 + 0.1   // breathing meridian (matches `mer`)
                var meridian = Path()
                meridian.addEllipse(in: CGRect(x: c.x - r * squash, y: c.y - r, width: 2 * r * squash, height: 2 * r))
                ctx.stroke(meridian, with: .color(ink.opacity(0.5)), lineWidth: 1.2)
                let ang = t * (2 * .pi / 6)                    // 6s orbit (matches `orbitspin`)
                let dot = CGPoint(x: c.x + cos(ang) * r * 1.28, y: c.y + sin(ang) * r * 1.28)
                var dp = Path()
                dp.addEllipse(in: CGRect(x: dot.x - 2.6, y: dot.y - 2.6, width: 5.2, height: 5.2))
                ctx.fill(dp, with: .color(ink))
            }
        }
    }
}

// MARK: - the shared bridge (web ↔ native ↔ daemon)

@MainActor
final class SphereBridge: NSObject, ObservableObject, WKScriptMessageHandler, CLLocationManagerDelegate, MKMapViewDelegate {
    enum Mode { case globe, street }
    @Published var mode: Mode = .globe

    init(model: AppModel) {
        self.model = model
        super.init()
    }

    weak var web: WKWebView?
    weak var map: MKMapView?
    private var timer: Timer?
    private var projectTimer: Timer?
    // The Mac is a peer, not a console bolted to a local daemon: it enrols itself, reads the
    // worldview over the mesh and reports its own observations, exactly as the iOS shells do.
    // `AppModel` is the shared client core (Shared/Sources) that both platforms compile.
    let model: AppModel

    final class NodeAnnotation: MKPointAnnotation {
        var colorHex = "#3ddc97"
        var frontier = false
        var isSelf = false
    }
    private var nodes: [[String: Any]] = []

    // Covenant baseline: this node provides its position too, IF the human has opened
    // allow_location (SPEC.md R9) — macOS CoreLocation (wifi positioning) writes the daemon's
    // mesh/geo.json seam, the same file any shell with a better source (a GPS feed) may own
    // instead. Previously ran unconditionally; now gated like every other platform's location.
    private let locator = CLLocationManager()
    private var locating = false

    // Mic (push-to-talk, allow_microphone) and network discovery (allow_network_discovery) —
    // both gated the same way as location, checked on the same poll tick. Mic is instantiated
    // eagerly (its own `start()` is a no-op unless the human explicitly toggles talking; see
    // `micTapped`) so its permission dialog only ever appears on an explicit human act, never
    // from this reactive gate-check.
    let mic = MacMicrophone()
    let face = MacFaceSensing()
    private let discovery = MacNetworkDiscovery()
    private var discovering = false
    private var watching = false

    func start(web: WKWebView) {
        self.web = web
        mic.onTranscript = { [weak self] text in self?.post("local/answer", ["text": text]) }
        // Both close a follow-up gap: this app has no NodeKey to sign a /mesh/observe push
        // with, so /local/observe (loopback, same trust class as local/answer) is its only
        // path to the daemon's observation/identity registries.
        discovery.onDiscovery = { [weak self] type, name in
            self?.post("local/observe", [
                "actor": "host", "action": "discovered", "object": "service:\(type)", "context": name,
            ])
        }
        face.onIdentityConfirmed = { [weak self] handle in
            self?.post("local/observe", [
                "actor": "host", "action": "recognized", "object": "face:\(handle)",
                "context": "on-device match, confirmed by human", "confidence": 0.95,
            ])
        }
        guard timer == nil else { return }
        locator.delegate = self
        locator.desiredAccuracy = kCLLocationAccuracyHundredMeters
        applyGates()
        timer = Timer.scheduledTimer(withTimeInterval: 3, repeats: true) { [weak self] _ in
            Task { await self?.poll() }
            self?.applyGates()
        }
        Task { await poll() }
    }

    /// A human explicitly asked to talk (e.g. the "Push to Talk" menu command) — checked here,
    /// not in `applyGates`, so the mic permission prompt only ever follows a real human act.
    func micTapped() {
        guard MacBoundary.load().allow_microphone else { return }
        mic.toggle()
    }

    /// Re-check the human-owned boundary and start/stop location + network-discovery to match
    /// — cheap enough to run on the existing 3s poll tick, so toggling a gate in settings takes
    /// effect within a few seconds without any file-watching infrastructure. Mic is excluded —
    /// it's an explicit push-to-talk act (`micTapped`), never auto-started by a gate flip.
    private func applyGates() {
        let gates = MacBoundary.load()
        if gates.allow_location != locating {
            locating = gates.allow_location
            if locating { locator.startUpdatingLocation() } else { locator.stopUpdatingLocation() }
        }
        if gates.allow_network_discovery != discovering {
            discovering = gates.allow_network_discovery
            if discovering { discovery.start() } else { discovery.stop() }
        }
        let wantFace = gates.allow_camera
        if wantFace != watching {
            watching = wantFace
            if watching { face.start(recognize: gates.allow_face_recognition) } else { face.stop() }
        } else if watching {
            face.setRecognition(gates.allow_face_recognition)
        }
    }

    /// Push this device's own state — consents, role, and the presence claim the console renders
    /// as PRESENT (ADR-0019). The Mac never did this at all: `window.sphereDevice` existed and only
    /// the iOS shell called it, so the Mac's Device screen had no claim to show and said "no one
    /// identified" no matter who answered.
    func pushDevice() {
        let json = model.deviceStateJSON()
        web?.evaluateJavaScript("window.sphereDevice && window.sphereDevice(\(json))",
                                completionHandler: nil)
    }

    func poll() async {
        // The core owns the read (host ordering, TLS pinning, failover, status heartbeat and the
        // consult service); the console just renders whatever it last saw.
        await model.refreshWorldview()
        pushDevice()
        if let json = model.worldviewJSON {
            web?.evaluateJavaScript("window.sphereUpdate(\(json))", completionHandler: nil)
        } else {
            web?.evaluateJavaScript("window.sphereLinkDown && window.sphereLinkDown()", completionHandler: nil)
        }
    }

    // ---- nodes on the real map ----

    private func setNodes(_ list: [[String: Any]]) {
        nodes = list
        guard let map else { return }
        map.removeAnnotations(map.annotations)
        for n in nodes {
            let a = NodeAnnotation()
            a.coordinate = CLLocationCoordinate2D(latitude: n["lat"] as? Double ?? 0,
                                                  longitude: n["lon"] as? Double ?? 0)
            a.title = n["label"] as? String
            a.subtitle = n["sublabel"] as? String
            a.colorHex = n["color"] as? String ?? "#3ddc97"
            a.frontier = n["frontier"] as? Bool ?? false
            a.isSelf = n["self"] as? Bool ?? false
            map.addAnnotation(a)
        }
    }

    nonisolated func mapView(_ mapView: MKMapView, viewFor annotation: MKAnnotation) -> MKAnnotationView? {
        guard let node = annotation as? NodeAnnotation else { return nil }
        let id = node.frontier ? "frontier" : "member"
        let v = mapView.dequeueReusableAnnotationView(withIdentifier: id) as? MKMarkerAnnotationView
            ?? MKMarkerAnnotationView(annotation: node, reuseIdentifier: id)
        v.annotation = node
        v.markerTintColor = NSColor(hex: node.colorHex)
        v.displayPriority = node.frontier ? .defaultLow : .required
        v.alphaValue = node.frontier ? 0.45 : 1.0
        v.titleVisibility = .visible
        v.subtitleVisibility = .adaptive
        return v
    }

    // A tap on a map node re-centers and dives the camera to it (same act as a roster dive).
    nonisolated func mapView(_ mapView: MKMapView, didSelect view: MKAnnotationView) {
        guard let node = view.annotation as? NodeAnnotation else { return }
        let target = node.coordinate
        Task { @MainActor in
            let close = MKMapCamera(lookingAtCenter: target, fromDistance: 900, pitch: 35, heading: 0)
            NSAnimationContext.runAnimationGroup { ctx in
                ctx.duration = 1.4
                ctx.allowsImplicitAnimation = true
                self.map?.camera = close
            }
        }
    }

    /// While the map is up, feed the arc overlay the nodes' projected screen positions —
    /// the page draws the electric arcs; this side only does geometry.
    private func startProjecting() {
        stopProjecting()
        projectTimer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.pushProjectedPoints() }
        }
    }
    private func stopProjecting() {
        projectTimer?.invalidate()
        projectTimer = nil
    }
    private func pushProjectedPoints() {
        guard mode == .street, let map, let web else { return }
        var pts: [[String: Any]] = []
        for n in nodes {
            let coord = CLLocationCoordinate2D(latitude: n["lat"] as? Double ?? 0,
                                               longitude: n["lon"] as? Double ?? 0)
            let p = map.convert(coord, toPointTo: map)
            pts.append(["x": p.x, "y": p.y,
                        "label": n["label"] as? String ?? "",
                        "frontier": n["frontier"] as? Bool ?? false,
                        "self": n["self"] as? Bool ?? false])
        }
        if let data = try? JSONSerialization.data(withJSONObject: pts),
           let json = String(data: data, encoding: .utf8) {
            web.evaluateJavaScript("window.streetArcPoints(\(json))", completionHandler: nil)
        }
    }

    // ---- surface transitions ----

    // The globe reached the crossfade point — surface Apple Maps at matching altitude and
    // descend in step with the fade, ending in pure-street close detail.
    func surfaceStreet(lat: Double, lon: Double) {
        mode = .street
        guard let map else { return }
        let target = CLLocationCoordinate2D(latitude: lat, longitude: lon)
        map.camera = MKMapCamera(lookingAtCenter: target, fromDistance: 220_000, pitch: 0, heading: 0)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak map] in
            let close = MKMapCamera(lookingAtCenter: target, fromDistance: 900, pitch: 35, heading: 0)
            NSAnimationContext.runAnimationGroup { ctx in
                ctx.duration = 2.4
                ctx.allowsImplicitAnimation = true
                map?.camera = close
            }
        }
        startProjecting()
    }

    // The reverse pattern, mirrored: 25% pure street zoom-out first, then at the crossfade
    // point the satellite fades back in (the page runs its globe ascent while this map
    // keeps rising underneath), last 25% pure satellite zoom-out.
    func backToGlobe() {
        guard let map else { mode = .globe; return }
        let center = map.centerCoordinate
        let up = MKMapCamera(lookingAtCenter: center, fromDistance: 220_000, pitch: 0, heading: 0)
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 2.4
            ctx.allowsImplicitAnimation = true
            map.camera = up
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) { [weak self] in
            guard let self else { return }
            self.mode = .globe
            self.stopProjecting()
            self.web?.evaluateJavaScript("window.sphereBackToGlobe && window.sphereBackToGlobe()", completionHandler: nil)
        }
    }

    // ---- web → native acts ----

    nonisolated func userContentController(_ ucc: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let body = message.body as? [String: Any], let kind = body["kind"] as? String else { return }
        Task { @MainActor in
            switch kind {
            case "answer":
                if let text = body["text"] as? String, !text.isEmpty { self.post("local/answer", ["text": text]) }
            case "gate":
                if let gate = body["gate"] as? String {
                    self.post("local/gate", ["gate": gate, "open": body["open"] as? Bool ?? false])
                }
            case "standing":
                // Recognise a guest, or hold them off (ADR-0020). Any active member may decide —
                // this Mac is one, so the decision is taken here and written locally. NOTE: the
                // standing roll is still per-node, so this applies where it is made until the roll
                // federates from the minting door; the console reads its own node's roll back on
                // the next poll, which is why the welcome list updates but a sibling's may not.
                if let act = body["act"] as? String, let node = body["node_id"] as? String,
                   !node.isEmpty {
                    self.post("local/standing", ["act": act, "node_id": node])
                }
            case "setHuman":
                // The human at this Mac says who they are — rung 3 of the identification ladder
                // (ADR-0019), which is the AUTHORITATIVE rung. It sets the presence claim the
                // console actually renders, and settles the device binding so the next launch
                // starts at rung 1 instead of asking again.
                //
                // It used to write the daemon's observer.txt directly. That could never work: this
                // app is sandboxed, so the write landed in the container and the console went on
                // saying "no one identified" because nothing had touched the claim it displays.
                if let name = body["name"] as? String, !name.isEmpty {
                    self.model.confirmPresentHuman(name)
                    self.model.setServedHuman(name)
                    self.pushDevice()
                }
            case "street":
                self.setNodes(body["nodes"] as? [[String: Any]] ?? [])
                self.surfaceStreet(lat: body["lat"] as? Double ?? 0,
                                   lon: body["lon"] as? Double ?? 0)
            case "nodes":
                self.setNodes(body["nodes"] as? [[String: Any]] ?? [])
            case "surface":
                if (body["to"] as? String) == "globe" { self.backToGlobe() }
            case "invite":
                self.fetchInvite()
            case "answerThread":
                if let id = body["id"] as? String, let text = body["text"] as? String, !text.isEmpty {
                    self.post("local/answer", ["text": text, "thread": id])
                }
            case "consent":
                // The device screen's five status buttons (position / motion / face / survey /
                // reason). This case did not exist — the exact mirror of the iOS bridge's old
                // missing "standing" case — so every click here went nowhere. Two halves, both
                // needed: setConsent flips the state the screen renders back, and the matching
                // MacBoundary gate is what actually arms this shell's sensing (applyGates).
                if let key = body["key"] as? String {
                    let on = body["on"] as? Bool ?? false
                    self.model.setConsent(key, on)
                    MacBoundary.set { g in
                        switch key {
                        case "location": g.allow_location = on
                        case "motion": g.allow_motion = on
                        case "face": g.allow_camera = on
                        case "discovery": g.allow_network_discovery = on
                        default: break   // "reasoning" has no shell gate; setConsent runs it
                        }
                    }
                    self.applyGates()   // arm now, not on the next 3s tick
                    self.pushDevice()
                }
            case "unenroll":
                // The SEVER button (armed + confirm in the web layer) — also previously dead.
                self.model.unenroll()
                self.pushDevice()
            case "game":
                // A move in the mesh game — signed by this console's own key; the door judges.
                // The game's kind travels as "game_kind": the message-routing key is "kind",
                // and a same-named payload field would overwrite it in toApp's spread.
                if let act = body["act"] as? String {
                    let kind = body["game_kind"] as? String
                    let text = body["text"] as? String ?? ""
                    let to = body["to"] as? String ?? ""
                    Task {
                        await self.model.gameAct(act, kind: kind, text: text, to: to)
                        await self.poll()
                    }
                }
            case "deviceRole":
                // Whose hands hold this machine — decides whether identity survives relaunch.
                if let roleRaw = body["role"] as? String,
                   let role = DeviceRole(rawValue: roleRaw) {
                    self.model.setDeviceBinding(role: role, owner: body["owner"] as? String ?? "")
                    self.pushDevice()
                }
            case "sponsor":
                // A member welcomes a NEW name in — the sponsor's half of vouching.
                if let nodeId = body["node_id"] as? String,
                   let handle = body["handle"] as? String {
                    Task {
                        _ = await self.model.sponsorFor(nodeId: nodeId, handle: handle)
                        await self.poll()
                    }
                }
            case "vouch":
                // The second person is this human: their own device confirms the claim and the
                // rules engine admits (ADR-0026 E2 over the mesh — no invite paste, no QR).
                if let nodeId = body["node_id"] as? String,
                   let pubkey = body["pubkey"] as? String,
                   let handle = body["handle"] as? String {
                    Task {
                        _ = await self.model.vouchFor(nodeId: nodeId, pubkey: pubkey, handle: handle)
                        await self.poll()
                    }
                }
            default: break
            }
        }
    }

    nonisolated func locationManager(_ m: CLLocationManager, didUpdateLocations locs: [CLLocation]) {
        guard let loc = locs.last else { return }
        let geo = ["lat": loc.coordinate.latitude, "lon": loc.coordinate.longitude]
        Task { @MainActor in
            let dir = FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent("Library/Application Support/Familiar/data/mesh")
            try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            if let data = try? JSONSerialization.data(withJSONObject: geo) {
                try? data.write(to: dir.appendingPathComponent("geo.json"))
            }
        }
    }

    nonisolated func locationManager(_ m: CLLocationManager, didFailWithError error: Error) {}

    /// What this peer can offer a device that wants to join: its *address*, not a secret.
    ///
    /// This used to pull a group-secret invite off the local daemon's loopback seam, which only
    /// worked because the Mac was the node holding the group key. A peer holds no group key — it
    /// carries a granted membership cert and nothing that could admit anyone — so the honest
    /// payload is where the mesh can be reached. Admission still happens at a mint-capable door
    /// (the lighthouse, ADR-0012), which is where it belongs.
    private func fetchInvite() {
        Task { @MainActor in
            // A member's payload carries a fresh single-use invite token (ADR-0026), so the
            // scanning device is admitted in one motion; a guest's is the plain address.
            guard let payload = model.invitePayload(handoff: false),
                  let quoted = (try? JSONEncoder().encode(payload)).flatMap({ String(data: $0, encoding: .utf8) })
            else { return }
            self.web?.evaluateJavaScript("window.sphereInvite(\(quoted))", completionHandler: nil)
        }
    }

    /// The console's acts, in the peer's own voice. What used to be a loopback POST to a daemon
    /// on this machine is now a signed act by this node against the mesh — an answer to a thread,
    /// or an observation this Mac made and is reporting like any other member.
    private func post(_ path: String, _ payload: [String: Any]) {
        switch path {
        case "local/answer":
            let text = payload["text"] as? String ?? ""
            guard !text.isEmpty else { return }
            if let thread = payload["thread"] as? String {
                model.answerThread(thread, text)
            } else {
                model.consoleAnswer = text
                model.submitConsoleAnswer()
            }
        case "local/observe":
            model.emit(ObsRecord(
                actor: payload["actor"] as? String ?? DeviceActor.current,
                action: payload["action"] as? String ?? "noticed",
                object: payload["object"] as? String ?? "",
                context: payload["context"] as? String ?? "",
                confidence: payload["confidence"] as? Double ?? 0.9))
        case "local/standing":
            // Recognise a guest, or hold them off (ADR-0020). Written straight to the daemon's
            // roll, exactly like a gate flip — this used to fall through to `default: break`,
            // so the welcome buttons called into nothing.
            if let act = payload["act"] as? String, let node = payload["node_id"] as? String {
                // Over the mesh ONLY. The local mirror used to live here and was worse than
                // useless: this app is sandboxed, so `homeDirectoryForCurrentUser` resolves to
                // ~/Library/Containers/io.river.familiar.mac/Data — a private copy the daemon
                // never reads. Clicking "recognise" wrote a file nobody would ever see and
                // reported success. A sandboxed console cannot own the daemon's data dir, and
                // should not: the serving node is the authority (ADR-0020).
                Task { await model.decideStanding(node, act: act) }
            }
        case "local/gate":
            // The boundary is this machine's own, and stays a local file — a gate governs what
            // this shell may sense, and is nobody else's to set.
            if let gate = payload["gate"] as? String {
                let open = payload["open"] as? Bool ?? false
                MacBoundary.set { g in
                    switch gate {
                    case "allow_camera": g.allow_camera = open
                    case "allow_microphone": g.allow_microphone = open
                    case "allow_location": g.allow_location = open
                    case "allow_motion": g.allow_motion = open
                    case "allow_network_discovery": g.allow_network_discovery = open
                    case "allow_face_recognition": g.allow_face_recognition = open
                    default: break
                    }
                }
            }
        default:
            break
        }
        Task { await poll() }   // reflect the act right away
    }
}

extension NSColor {
    convenience init(hex: String) {
        var h = hex.trimmingCharacters(in: CharacterSet(charactersIn: "#"))
        if h.count == 3 { h = h.map { "\($0)\($0)" }.joined() }
        let v = UInt64(h, radix: 16) ?? 0x3ddc97
        self.init(red: CGFloat((v >> 16) & 0xff) / 255,
                  green: CGFloat((v >> 8) & 0xff) / 255,
                  blue: CGFloat(v & 0xff) / 255, alpha: 1)
    }
}

// MARK: - the web layer (globe + holograms + street arc overlay)

struct SphereWebView: NSViewRepresentable {
    let bridge: SphereBridge

    func makeNSView(context: Context) -> WKWebView {
        let cfg = WKWebViewConfiguration()
        cfg.userContentController.add(bridge, name: "sphere")
        // Serve the bundle over a custom scheme rather than file:// — see SphereScheme. Must be
        // set on the configuration BEFORE the web view is created; WKWebView copies it at init.
        SphereScheme.register(on: cfg)
        let web = WKWebView(frame: .zero, configuration: cfg)
        web.setValue(false, forKey: "drawsBackground")   // transparent over the native map
        bridge.start(web: web)
        web.load(URLRequest(url: SphereScheme.indexURL))
        return web
    }

    func updateNSView(_ view: WKWebView, context: Context) {}
}

// MARK: - the street layer: REAL Apple Maps

struct MeshMapView: NSViewRepresentable {
    let bridge: SphereBridge

    func makeNSView(context: Context) -> MKMapView {
        let map = MKMapView()
        map.appearance = NSAppearance(named: .darkAqua)
        map.mapType = .mutedStandard
        map.showsCompass = false
        map.showsZoomControls = false
        map.pointOfInterestFilter = .includingAll
        map.delegate = bridge
        bridge.map = map
        return map
    }

    func updateNSView(_ view: MKMapView, context: Context) {}
}

// MARK: - invisible window drag (no titlebar, no visible edge)

struct WindowDragRegion: NSViewRepresentable {
    final class DragView: NSView {
        override func mouseDown(with event: NSEvent) {
            window?.performDrag(with: event)
        }
    }
    func makeNSView(context: Context) -> NSView { DragView() }
    func updateNSView(_ view: NSView, context: Context) {}
}
