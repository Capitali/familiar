import SwiftUI
import WebKit
import MapKit

// The Metal Sphere console (imported from Claude Design "Familiar Metal Sphere.dc.html"):
// a WKWebView renders the satellite globe + hologram (Resources/sphere/index.html), and the
// street surface is REAL Apple Maps — a native MKMapView this host surfaces when the globe
// dive reaches map altitude, continuing the same zoom down to street detail. The host does
// all daemon I/O natively (loopback /local/worldview → window.sphereUpdate(); answers and
// gate flips back over the script-message bridge). The web layer never fakes a map and
// never touches the network for data.
struct SphereConsole: View {
    @StateObject private var bridge = SphereBridge()

    var body: some View {
        ZStack(alignment: .top) {
            MeshMapView(bridge: bridge)
                .ignoresSafeArea()
            SphereWebView(bridge: bridge)
                .ignoresSafeArea()
                .opacity(bridge.mode == .street ? 0 : 1)
                .allowsHitTesting(bridge.mode != .street)
                .animation(.easeInOut(duration: 1.6), value: bridge.mode)
            if bridge.mode == .street {
                // Wordless exit: the orbit glyph, bottom-center — back up to the globe.
                VStack {
                    Spacer()
                    Button(action: { bridge.backToGlobe() }) {
                        Image(systemName: "globe.americas.fill")
                            .font(.system(size: 22))
                            .foregroundStyle(Color(red: 0.81, green: 0.88, blue: 1.0))
                            .frame(width: 52, height: 52)
                            .background(.black.opacity(0.55), in: Circle())
                            .overlay(Circle().stroke(.white.opacity(0.18), lineWidth: 1))
                    }
                    .buttonStyle(.plain)
                    .padding(.bottom, 18)
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

// MARK: - the shared bridge (web ↔ native ↔ daemon)

@MainActor
final class SphereBridge: NSObject, ObservableObject, WKScriptMessageHandler {
    enum Mode { case globe, street }
    @Published var mode: Mode = .globe
    @Published var streetTarget: (lat: Double, lon: Double, label: String)?
    @Published var members: [MapNode] = []

    weak var web: WKWebView?
    weak var map: MKMapView?
    private var timer: Timer?
    private let base = URL(string: "http://127.0.0.1:47100")!

    struct MapNode: Identifiable {
        let id: String, label: String, lat: Double, lon: Double
        let online: Bool, frontier: Bool
    }

    func start(web: WKWebView) {
        self.web = web
        guard timer == nil else { return }
        timer = Timer.scheduledTimer(withTimeInterval: 3, repeats: true) { [weak self] _ in
            Task { await self?.poll() }
        }
        Task { await poll() }
    }

    func poll() async {
        do {
            let (data, resp) = try await URLSession.shared.data(from: base.appendingPathComponent("local/worldview"))
            guard (resp as? HTTPURLResponse)?.statusCode == 200,
                  let json = String(data: data, encoding: .utf8) else { throw URLError(.badServerResponse) }
            web?.evaluateJavaScript("window.sphereUpdate(\(json))", completionHandler: nil)
            updateMapNodes(from: data)
        } catch {
            web?.evaluateJavaScript("window.sphereLinkDown && window.sphereLinkDown()", completionHandler: nil)
        }
    }

    /// Members + frontier with real coordinates → native map annotations.
    private func updateMapNodes(from data: Data) {
        guard let v = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }
        var nodes: [MapNode] = []
        for m in v["members"] as? [[String: Any]] ?? [] {
            let lat = m["lat"] as? Double ?? 0, lon = m["lon"] as? Double ?? 0
            guard lat != 0 || lon != 0 else { continue }
            nodes.append(MapNode(id: m["node_id"] as? String ?? UUID().uuidString,
                                 label: m["label"] as? String ?? "?",
                                 lat: lat, lon: lon,
                                 online: (m["status"] as? String ?? "") == "online" || (m["online"] as? Bool ?? false),
                                 frontier: false))
        }
        members = nodes
        map.map { syncAnnotations(on: $0) }
    }

    func syncAnnotations(on map: MKMapView) {
        map.removeAnnotations(map.annotations)
        for n in members {
            let a = MKPointAnnotation()
            a.coordinate = CLLocationCoordinate2D(latitude: n.lat, longitude: n.lon)
            a.title = n.label
            map.addAnnotation(a)
        }
    }

    // The globe reached handoff altitude — surface Apple Maps at the matching height and
    // fly the rest of the way down. Satellite→street continuity: the camera starts high
    // (the globe's visual altitude), then descends to close detail in one native flight.
    func surfaceStreet(lat: Double, lon: Double, label: String) {
        streetTarget = (lat, lon, label)
        mode = .street
        guard let map else { return }
        syncAnnotations(on: map)
        let target = CLLocationCoordinate2D(latitude: lat, longitude: lon)
        // Surfaced at the globe's altitude; descend in step with the crossfade, ending in
        // pure-street close detail — one continuous flight, no jump in the zoom range.
        map.camera = MKMapCamera(lookingAtCenter: target, fromDistance: 220_000, pitch: 0, heading: 0)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak map] in
            let close = MKMapCamera(lookingAtCenter: target, fromDistance: 900, pitch: 35, heading: 0)
            NSAnimationContext.runAnimationGroup { ctx in
                ctx.duration = 2.4
                ctx.allowsImplicitAnimation = true
                map?.camera = close
            }
        }
    }

    func backToGlobe() {
        mode = .globe
        web?.evaluateJavaScript("window.sphereBackToGlobe && window.sphereBackToGlobe()", completionHandler: nil)
    }

    // web → native acts
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
                self.surfaceStreet(lat: body["lat"] as? Double ?? 0,
                                   lon: body["lon"] as? Double ?? 0,
                                   label: body["label"] as? String ?? "")
            case "surface":
                if (body["to"] as? String) == "globe" { self.backToGlobe() }
            default: break
            }
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

// MARK: - the web layer (globe + holograms)

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
        bridge.map = map
        bridge.syncAnnotations(on: map)
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
