import SwiftUI
import WebKit
import MapKit
import CoreLocation

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

    weak var web: WKWebView?
    weak var map: MKMapView?
    private var timer: Timer?
    private var projectTimer: Timer?
    // The daemon's local seams: plain HTTP, loopback-only, one port above the TLS mesh port.
    private let base = URL(string: "http://127.0.0.1:47101")!

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
    private let discovery = MacNetworkDiscovery()
    private var discovering = false

    func start(web: WKWebView) {
        self.web = web
        mic.onTranscript = { [weak self] text in self?.post("local/answer", ["text": text]) }
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
    }

    func poll() async {
        do {
            let (data, resp) = try await URLSession.shared.data(from: base.appendingPathComponent("local/worldview"))
            guard (resp as? HTTPURLResponse)?.statusCode == 200,
                  let json = String(data: data, encoding: .utf8) else { throw URLError(.badServerResponse) }
            web?.evaluateJavaScript("window.sphereUpdate(\(json))", completionHandler: nil)
        } catch {
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
        v.titleVisibility = node.frontier ? .hidden : .visible
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

    /// The enrollment payload for a new device (group secret — trusted screen only): fetch
    /// from the loopback seam and hand it to the page to render as a QR.
    private func fetchInvite() {
        Task { @MainActor in
            guard let (data, resp) = try? await URLSession.shared.data(from: base.appendingPathComponent("local/invite")),
                  (resp as? HTTPURLResponse)?.statusCode == 200,
                  let payload = String(data: data, encoding: .utf8),
                  let quoted = (try? JSONEncoder().encode(payload)).flatMap({ String(data: $0, encoding: .utf8) })
            else { return }
            self.web?.evaluateJavaScript("window.sphereInvite(\(quoted))", completionHandler: nil)
        }
    }

    private func post(_ path: String, _ payload: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: payload) else { return }
        var req = URLRequest(url: base.appendingPathComponent(path))
        req.httpMethod = "POST"
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = data
        URLSession.shared.dataTask(with: req) { [weak self] _, _, _ in
            Task { await self?.poll() }   // reflect the act right away
        }.resume()
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
        let web = WKWebView(frame: .zero, configuration: cfg)
        web.setValue(false, forKey: "drawsBackground")   // transparent over the native map
        bridge.start(web: web)
        if let url = Bundle.main.url(forResource: "index", withExtension: "html", subdirectory: "sphere") {
            web.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        }
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
