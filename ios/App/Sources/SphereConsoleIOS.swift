import SwiftUI
import WebKit
import MapKit
import FamiliarMesh

// The Metal Sphere console on iPhone/iPad — the SAME web bundle the Mac console renders
// (Resources/sphere/index.html: satellite globe, hologram screens, electric arcs), with the
// street surface on REAL Apple Maps (a native MKMapView under the transparent web layer).
// Differences from the Mac host: worldview JSON comes from the app's signed mesh reads
// (AppModel already polls it — devices can't use the loopback seam), answers go through the
// same observation pipe the console has always used, and gate flips are ignored — the
// boundary is a local human act at the familiar itself, never widened from a device.
struct SphereConsoleIOS: View {
    @EnvironmentObject var model: AppModel
    @StateObject private var bridge = SphereBridgeIOS()

    var body: some View {
        ZStack(alignment: .top) {
            MeshMapViewIOS(bridge: bridge)
                .ignoresSafeArea()
            SphereWebViewIOS(bridge: bridge)
                .ignoresSafeArea()
                .allowsHitTesting(bridge.mode != .street)
            if bridge.mode == .street {
                VStack {
                    Spacer()
                    Button(action: { bridge.backToGlobe() }) {
                        OrbitGlyphShared()
                            .frame(width: 56, height: 56)
                            .background(Color(red: 0.035, green: 0.06, blue: 0.125).opacity(0.55), in: Circle())
                            .overlay(Circle().stroke(Color(red: 0.52, green: 0.81, blue: 1.0).opacity(0.25), lineWidth: 1))
                    }
                    .buttonStyle(.plain)
                    .padding(.bottom, 16)
                }
            }
        }
        .background(Color.black.ignoresSafeArea())
        .preferredColorScheme(.dark)
        .onAppear {
            model.startWorldviewPolling()
            bridge.onAnswer = { [weak model] text in
                model?.consoleAnswer = text
                model?.submitConsoleAnswer()
            }
            bridge.onConsent = { [weak model] key, on in
                model?.setConsent(key, on)
                model.map { bridge.pushDevice($0.deviceStateJSON()) }
            }
            bridge.onUnenroll = { [weak model] in model?.unenroll() }
            bridge.pushDevice(model.deviceStateJSON())
        }
        .onReceive(model.$worldviewJSON) { json in
            if let json {
                bridge.push(worldviewJSON: json)
                bridge.pushDevice(model.deviceStateJSON())
            }
        }
        .onReceive(model.$worldviewError) { err in
            if let err { bridge.pushLinkDown(err) }
        }
    }
}

/// The orbit glyph in the house language (shared shape with the Mac console's exit control).
struct OrbitGlyphShared: View {
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
                let squash = abs(sin(t * 0.63)) * 0.9 + 0.1
                var meridian = Path()
                meridian.addEllipse(in: CGRect(x: c.x - r * squash, y: c.y - r, width: 2 * r * squash, height: 2 * r))
                ctx.stroke(meridian, with: .color(ink.opacity(0.5)), lineWidth: 1.2)
                let ang = t * (2 * .pi / 6)
                let dot = CGPoint(x: c.x + cos(ang) * r * 1.28, y: c.y + sin(ang) * r * 1.28)
                var dp = Path()
                dp.addEllipse(in: CGRect(x: dot.x - 2.6, y: dot.y - 2.6, width: 5.2, height: 5.2))
                ctx.fill(dp, with: .color(ink))
            }
        }
    }
}

// MARK: - bridge (web ↔ native; data pushed in by AppModel)

@MainActor
final class SphereBridgeIOS: NSObject, ObservableObject, WKScriptMessageHandler, MKMapViewDelegate, WKNavigationDelegate {
    enum Mode { case globe, street }
    @Published var mode: Mode = .globe

    weak var web: WKWebView?
    weak var map: MKMapView?
    var onAnswer: ((String) -> Void)?
    var onConsent: ((String, Bool) -> Void)?
    var onUnenroll: (() -> Void)?
    private var projectTimer: Timer?
    private var nodes: [[String: Any]] = []

    final class NodeAnnotation: MKPointAnnotation {
        var colorHex = "#3ddc97"
        var frontier = false
        var isSelf = false
    }

    private var lastJSON: String?

    func push(worldviewJSON: String) {
        lastJSON = worldviewJSON
        web?.evaluateJavaScript("window.sphereUpdate(\(worldviewJSON))", completionHandler: nil)
    }

    // A push that raced the page load is replayed once the page is ready.
    nonisolated func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        Task { @MainActor in
            if let json = self.lastJSON {
                webView.evaluateJavaScript("window.sphereUpdate(\(json))", completionHandler: nil)
            }
        }
    }
    func pushLinkDown(_ message: String = "") {
        // JSON-encode so quotes/newlines in error text can't break the injection.
        let quoted = (try? JSONEncoder().encode(message)).flatMap { String(data: $0, encoding: .utf8) } ?? "\"link down\""
        web?.evaluateJavaScript("window.sphereLinkDown && window.sphereLinkDown(\(quoted))", completionHandler: nil)
    }

    func pushDevice(_ json: String) {
        web?.evaluateJavaScript("window.sphereDevice && window.sphereDevice(\(json))", completionHandler: nil)
    }

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
        v.markerTintColor = UIColor(hexString: node.colorHex)
        v.displayPriority = node.frontier ? .defaultLow : .required
        v.alpha = node.frontier ? 0.45 : 1.0
        v.titleVisibility = node.frontier ? .hidden : .visible
        return v
    }

    nonisolated func mapView(_ mapView: MKMapView, didSelect view: MKAnnotationView) {
        guard let node = view.annotation as? NodeAnnotation else { return }
        let target = node.coordinate
        Task { @MainActor in
            let close = MKMapCamera(lookingAtCenter: target, fromDistance: 900, pitch: 35, heading: 0)
            UIView.animate(withDuration: 1.4) { self.map?.camera = close }
        }
    }

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

    // Same choreography as the Mac console: surface at matched altitude, descend with the
    // crossfade; the ascent mirrors it (25% street out, crossfade, satellite out).
    func surfaceStreet(lat: Double, lon: Double) {
        mode = .street
        guard let map else { return }
        let target = CLLocationCoordinate2D(latitude: lat, longitude: lon)
        map.camera = MKMapCamera(lookingAtCenter: target, fromDistance: 220_000, pitch: 0, heading: 0)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
            let close = MKMapCamera(lookingAtCenter: target, fromDistance: 900, pitch: 35, heading: 0)
            UIView.animate(withDuration: 2.4) { self?.map?.camera = close }
        }
        startProjecting()
    }

    func backToGlobe() {
        guard let map else { mode = .globe; return }
        let up = MKMapCamera(lookingAtCenter: map.centerCoordinate, fromDistance: 220_000, pitch: 0, heading: 0)
        UIView.animate(withDuration: 2.4) { map.camera = up }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) { [weak self] in
            guard let self else { return }
            self.mode = .globe
            self.stopProjecting()
            self.web?.evaluateJavaScript("window.sphereBackToGlobe && window.sphereBackToGlobe()", completionHandler: nil)
        }
    }

    nonisolated func userContentController(_ ucc: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let body = message.body as? [String: Any], let kind = body["kind"] as? String else { return }
        Task { @MainActor in
            switch kind {
            case "answer":
                if let text = body["text"] as? String, !text.isEmpty { self.onAnswer?(text) }
            case "street":
                self.setNodes(body["nodes"] as? [[String: Any]] ?? [])
                self.surfaceStreet(lat: body["lat"] as? Double ?? 0,
                                   lon: body["lon"] as? Double ?? 0)
            case "nodes":
                self.setNodes(body["nodes"] as? [[String: Any]] ?? [])
            case "surface":
                if (body["to"] as? String) == "globe" { self.backToGlobe() }
            case "consent":
                if let key = body["key"] as? String { self.onConsent?(key, body["on"] as? Bool ?? false) }
            case "unenroll":
                self.onUnenroll?()
            case "gate":
                break   // boundary writes are a local human act at the familiar, never from a device
            default: break
            }
        }
    }
}

extension UIColor {
    convenience init(hexString: String) {
        var h = hexString.trimmingCharacters(in: CharacterSet(charactersIn: "#"))
        if h.count == 3 { h = h.map { "\($0)\($0)" }.joined() }
        let v = UInt64(h, radix: 16) ?? 0x3ddc97
        self.init(red: CGFloat((v >> 16) & 0xff) / 255,
                  green: CGFloat((v >> 8) & 0xff) / 255,
                  blue: CGFloat(v & 0xff) / 255, alpha: 1)
    }
}

// MARK: - web + map layers

struct SphereWebViewIOS: UIViewRepresentable {
    let bridge: SphereBridgeIOS

    func makeUIView(context: Context) -> WKWebView {
        let cfg = WKWebViewConfiguration()
        cfg.userContentController.add(bridge, name: "sphere")
        let web = WKWebView(frame: .zero, configuration: cfg)
        web.isOpaque = false
        web.backgroundColor = .clear
        web.scrollView.isScrollEnabled = false
        web.scrollView.contentInsetAdjustmentBehavior = .never
        web.scrollView.bounces = false
        web.navigationDelegate = bridge
        bridge.web = web
        if let url = Bundle.main.url(forResource: "index", withExtension: "html", subdirectory: "sphere") {
            web.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        }
        return web
    }

    func updateUIView(_ view: WKWebView, context: Context) {}
}

struct MeshMapViewIOS: UIViewRepresentable {
    let bridge: SphereBridgeIOS

    func makeUIView(context: Context) -> MKMapView {
        let map = MKMapView()
        map.overrideUserInterfaceStyle = .dark
        map.mapType = .mutedStandard
        map.pointOfInterestFilter = .includingAll
        map.delegate = bridge
        bridge.map = map
        return map
    }

    func updateUIView(_ view: MKMapView, context: Context) {}
}
