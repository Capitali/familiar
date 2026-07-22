// FamiliarSphere — the Metal Sphere, native.
//
// A SceneKit port of the "Familiar Metal Sphere" design: one blue-marble that morphs across three
// states — Marble (a breathing glass presence), Mesh (the wireframe pipeline over it), and Globe
// (a displaced blue world with the mesh's peers pinned to it). It lives in the shared package so
// EVERY Apple shell renders the same surface: iPhone, iPad, Mac, and — as those targets arrive —
// Apple TV and Vision. SceneKit isn't on watchOS, so the watch shows a lightweight fallback orb.
//
// Gestures are OWNED here (SceneKit's built-in camera control is off), so the interface can
// distinguish one-finger *orbit* from a two-finger *swipe* that the console turns into panel
// navigation — the built-in control would otherwise swallow the two-finger gesture as a camera move.

import SwiftUI
#if !os(watchOS)
import SceneKit
import simd
#endif

#if canImport(UIKit)
import UIKit
private typealias PColor = UIColor
#elseif canImport(AppKit)
import AppKit
private typealias PColor = NSColor
#endif

/// A member of the mesh rendered as a point on the globe. `lat`/`lon` place it geographically when
/// known; otherwise a stable position is derived from the id so a node always sits in the same spot.
public struct SpherePin: Identifiable, Equatable {
    public let id: String
    public let label: String
    public let local: Bool
    public let ai: Bool
    public let lat: Double?
    public let lon: Double?
    public init(id: String, label: String, local: Bool, ai: Bool = false, lat: Double? = nil, lon: Double? = nil) {
        self.id = id; self.label = label; self.local = local; self.ai = ai; self.lat = lat; self.lon = lon
    }
}

/// The three render states of the Metal Sphere.
public enum SphereMode: String, CaseIterable, Identifiable, Sendable {
    case marble = "Marble"
    case mesh = "Mesh"
    case globe = "Globe"
    public var id: String { rawValue }
    public var hint: String {
        switch self { case .marble: return "SMOOTH"; case .mesh: return "WIREFRAME"; case .globe: return "TERRAIN" }
    }
    public var caption: String {
        switch self {
        case .marble: return "The blue marble, breathing — a glass sphere on a 6-second cycle. Presence, not decoration."
        case .mesh: return "The mesh exposed — the same surface as a wireframe. Peers pinned to the collective."
        case .globe: return "The world, and everyone on it — the mesh's nodes placed on a living globe."
        }
    }
}

private enum Sky {
    static let bg = PColor(red: 0x03/255, green: 0x05/255, blue: 0x0a/255, alpha: 1)
    static let deep = PColor(red: 0.043, green: 0.145, blue: 0.42, alpha: 1)
    static let bright = PColor(red: 0x6c/255, green: 0x9b/255, blue: 0xff/255, alpha: 1)
    static let cyan = PColor(red: 0x8f/255, green: 0xd0/255, blue: 0xff/255, alpha: 1)
    static let ice = PColor(red: 0xcf/255, green: 0xe0/255, blue: 0xff/255, alpha: 1)
    static let amber = PColor(red: 0xff/255, green: 0xb1/255, blue: 0x5a/255, alpha: 1)
}

#if os(watchOS)

public struct FamiliarSphereView: View {
    private let mode: SphereMode
    private let pins: [SpherePin]
    @State private var breathe = false
    public init(mode: SphereMode = .globe, pins: [SpherePin] = []) {
        self.mode = mode; self.pins = pins
    }
    public var body: some View {
        ZStack {
            Color(red: 0x03/255, green: 0x05/255, blue: 0x0a/255).ignoresSafeArea()
            Circle()
                .fill(RadialGradient(colors: [Color(red: 0.55, green: 0.72, blue: 1.0),
                                              Color(red: 0.043, green: 0.145, blue: 0.42)],
                                     center: .init(x: 0.36, y: 0.30), startRadius: 2, endRadius: 90))
                .frame(width: 120, height: 120)
                .scaleEffect(breathe ? 1.04 : 0.98)
                .onAppear { withAnimation(.easeInOut(duration: 3).repeatForever(autoreverses: true)) { breathe = true } }
        }
    }
}

#else

/// Builds and owns the SCNScene + node references. Rotation is applied per-frame by the coordinator.
final class SphereScene {
    let scene = SCNScene()
    let camera = SCNNode()
    let group = SCNNode()
    let coreMat = SCNMaterial()
    let wireMat = SCNMaterial()
    let atmoMat = SCNMaterial()
    let pinRoot = SCNNode()
    let arcRoot = SCNNode()
    private var arcs: [Arc] = []
    private var globeTexture: Any?
    private var built = false

    func build() {
        guard !built else { return }; built = true
        scene.background.contents = Sky.bg

        let cam = SCNCamera(); cam.fieldOfView = 40; cam.zNear = 0.1; cam.zFar = 100
        camera.camera = cam; camera.position = SCNVector3(0, 0, 3.05)
        scene.rootNode.addChildNode(camera)

        addLight(.ambient, Sky.deep, 380, nil)
        addLight(.directional, PColor(red: 1, green: 0.96, blue: 0.9, alpha: 1), 1050, SCNVector3(3, 1.4, 3))
        addLight(.directional, Sky.bright, 360, SCNVector3(-3, -1, -2))

        coreMat.lightingModel = .physicallyBased
        coreMat.diffuse.contents = Sky.deep
        coreMat.metalness.contents = 0.35
        coreMat.roughness.contents = 0.38
        coreMat.emission.contents = PColor(red: 0.07, green: 0.16, blue: 0.44, alpha: 1)
        coreMat.emission.intensity = 0.35
        coreMat.reflective.contents = Sky.cyan
        coreMat.fresnelExponent = 2.2
        group.addChildNode(node(1.0, 96, coreMat))

        wireMat.lightingModel = .constant
        wireMat.fillMode = .lines
        wireMat.diffuse.contents = Sky.cyan
        wireMat.emission.contents = Sky.cyan
        wireMat.transparency = 0
        group.addChildNode(node(1.004, 48, wireMat))

        atmoMat.lightingModel = .constant
        atmoMat.diffuse.contents = PColor.clear
        atmoMat.emission.contents = PColor(red: 0.25, green: 0.51, blue: 1.0, alpha: 1)
        atmoMat.blendMode = .add
        atmoMat.cullMode = .front
        atmoMat.writesToDepthBuffer = false
        atmoMat.transparency = 0.55
        group.addChildNode(node(1.28, 48, atmoMat))

        group.addChildNode(pinRoot)
        group.addChildNode(arcRoot)
        scene.rootNode.addChildNode(group)

        let up = SCNAction.scale(to: 1.018, duration: 3); up.timingMode = .easeInEaseOut
        let down = SCNAction.scale(to: 1.0, duration: 3); down.timingMode = .easeInEaseOut
        group.runAction(.repeatForever(.sequence([up, down])))
    }

    func apply(_ mode: SphereMode) {
        globeTextureIfNeeded()
        SCNTransaction.begin(); SCNTransaction.animationDuration = 0.6
        switch mode {
        case .marble:
            coreMat.diffuse.contents = Sky.deep; coreMat.transparency = 1
            wireMat.transparency = 0; atmoMat.transparency = 0.55; pinRoot.opacity = 0
        case .mesh:
            coreMat.diffuse.contents = Sky.deep; coreMat.transparency = 0.5
            wireMat.transparency = 1; atmoMat.transparency = 0.4; pinRoot.opacity = 1
        case .globe:
            coreMat.diffuse.contents = globeTexture ?? Sky.deep; coreMat.transparency = 1
            wireMat.transparency = 0; atmoMat.transparency = 0.4; pinRoot.opacity = 1  // more map, no grid
        }
        SCNTransaction.commit()
    }

    func setPins(_ pins: [SpherePin]) {
        pinRoot.childNodes.forEach { $0.removeFromParentNode() }
        let n = max(1, pins.count)
        _ = n
        var dirs: [simd_float3] = []
        for p in pins {
            let dir = geoDir(p); dirs.append(dir)
            let color: PColor = p.local ? Sky.ice : (p.ai ? Sky.amber : Sky.bright)
            let r: CGFloat = p.local ? 0.032 : 0.022
            let dm = SCNMaterial(); dm.lightingModel = .constant; dm.diffuse.contents = color; dm.emission.contents = color
            let dot = SCNNode(geometry: SCNSphere(radius: r)); dot.geometry?.firstMaterial = dm
            dot.position = scn(dir * 1.03)
            pinRoot.addChildNode(dot)
        }
        buildArcs(pins: pins, dirs: dirs)
    }

    /// Wire the local node to every other node with a living electricity arc (bounded so a big mesh
    /// can't flood the frame). Rebuilt whenever membership changes; animated each frame in `updateArcs`.
    private func buildArcs(pins: [SpherePin], dirs: [simd_float3]) {
        arcRoot.childNodes.forEach { $0.removeFromParentNode() }
        arcs.removeAll()
        guard dirs.count >= 2 else { return }
        let localIdx = pins.firstIndex(where: { $0.local }) ?? 0
        let a = dirs[localIdx]
        var made = 0
        for j in dirs.indices where j != localIdx && made < 12 {
            made += 1
            let arc = Arc(a: a, b: dirs[j], seed: Float(j) * 1.7 + 0.3)
            arcRoot.addChildNode(arc.core); arcRoot.addChildNode(arc.glow)
            arcs.append(arc)
        }
    }

    /// Advance the lightning — layered sine-noise wanders each vertex off the great-circle, pinned at
    /// the endpoints; a crackle flicker modulates opacity. Line geometry is small, so we rebuild it.
    func updateArcs(_ t: Float) {
        for arc in arcs {
            var corePts = [simd_float3](); corePts.reserveCapacity(arc.n + 1)
            var glowPts = [simd_float3](); glowPts.reserveCapacity(arc.n + 1)
            for s in 0...arc.n {
                let tt = Float(s) / Float(arc.n)
                let env = sin(Float.pi * tt)
                let n1 = sin(tt * 17 + t * arc.jrate + arc.seed)
                let n2 = sin(tt * 41 - t * arc.jrate * 1.7 + arc.seed * 2.1)
                let n3 = sin(tt * 7 + t * 2 + arc.seed)
                let off = (n1 * 0.6 + n2 * 0.28 + n3 * 0.5) * 0.028 * env
                let rOff = (sin(tt * 23 + t * 5 + arc.seed) * 0.5) * 0.018 * env
                corePts.append(arc.base[s] + arc.bi[s] * off + arc.rad[s] * rOff)
                let goff = (n1 * 0.6 + n3 * 0.7) * 0.034 * env
                glowPts.append(arc.base[s] + arc.bi[s] * goff + arc.rad[s] * (rOff * 0.6))
            }
            let fl = 0.6 + 0.4 * sin(t * arc.jrate + arc.seed)
            arc.core.geometry = lineGeom(corePts, PColor(red: 0.9, green: 0.97, blue: 1, alpha: 1), CGFloat(max(0.2, 0.7 + 0.3 * fl)))
            arc.glow.geometry = lineGeom(glowPts, Sky.bright, CGFloat(0.32 * (0.6 + 0.4 * fl)))
        }
    }

    private func lineGeom(_ pts: [simd_float3], _ color: PColor, _ opacity: CGFloat) -> SCNGeometry {
        let verts = pts.map { SCNVector3($0.x, $0.y, $0.z) }
        let src = SCNGeometrySource(vertices: verts)
        var idx = [Int32](); idx.reserveCapacity((pts.count - 1) * 2)
        for i in 0..<(pts.count - 1) { idx.append(Int32(i)); idx.append(Int32(i + 1)) }
        let el = SCNGeometryElement(indices: idx, primitiveType: .line)
        let g = SCNGeometry(sources: [src], elements: [el])
        let m = SCNMaterial(); m.lightingModel = .constant
        m.diffuse.contents = color; m.emission.contents = color
        m.blendMode = .add; m.writesToDepthBuffer = false; m.readsFromDepthBuffer = false; m.transparency = opacity
        g.materials = [m]
        return g
    }
    private func scn(_ v: simd_float3) -> SCNVector3 { SCNVector3(v.x, v.y, v.z) }

    /// A node's position on the globe: its real lat/lon when known, otherwise a stable geographic
    /// spot derived from the id (spread across populated latitudes) so it always sits in one place.
    private func geoDir(_ p: SpherePin) -> simd_float3 {
        if let lat = p.lat, let lon = p.lon { return latLonDir(lat, lon) }
        var hsh: UInt64 = 1469598103934665603
        for byte in p.id.utf8 { hsh = (hsh ^ UInt64(byte)) &* 1099511628211 }
        let lon = Double(hsh & 0xffff) / 65535.0 * 360.0 - 180.0
        let lat = (Double((hsh >> 21) & 0xffff) / 65535.0 - 0.5) * 116.0   // ~ -58…58°, where people are
        return latLonDir(lat, lon)
    }
    private func latLonDir(_ lat: Double, _ lon: Double) -> simd_float3 {
        let phi = (90 - lat) * .pi / 180, theta = (lon + 180) * .pi / 180
        return simd_normalize(simd_float3(Float(-sin(phi) * cos(theta)), Float(cos(phi)), Float(sin(phi) * sin(theta))))
    }

    private func node(_ radius: CGFloat, _ seg: Int, _ mat: SCNMaterial) -> SCNNode {
        let s = SCNSphere(radius: radius); s.segmentCount = seg; s.firstMaterial = mat
        return SCNNode(geometry: s)
    }
    private func addLight(_ t: SCNLight.LightType, _ c: PColor, _ i: CGFloat, _ pos: SCNVector3?) {
        let l = SCNLight(); l.type = t; l.color = c; l.intensity = i
        let nd = SCNNode(); nd.light = l; if let pos = pos { nd.position = pos }
        scene.rootNode.addChildNode(nd)
    }
    private func spiral(_ i: Int, _ n: Int) -> SCNVector3 {
        let gold = Double.pi * (3.0 - (5.0).squareRoot())
        let y = 1.0 - (Double(i) / Double(max(1, n - 1))) * 2.0
        let r = (1.0 - y * y).squareRoot(); let th = gold * Double(i)
        return SCNVector3(Float(cos(th) * r), Float(y), Float(sin(th) * r))
    }
    /// A satellite-style blue-marble skin, generated once (no external asset): fbm value-noise raises
    /// continents out of the ocean — deep blue seas, green→tan land, white ice caps — so the globe
    /// reads as a real world the mesh lives on. Ported from the design's `familiar-sphere.js` maps.
    private func globeTextureIfNeeded() {
        guard globeTexture == nil else { return }
        let w = 512, h = 256
        func hash(_ x: Double, _ y: Double) -> Double { let n = sin(x * 127.1 + y * 311.7) * 43758.5453; return n - floor(n) }
        func smooth(_ t: Double) -> Double { t * t * (3 - 2 * t) }
        func vnoise(_ x: Double, _ y: Double) -> Double {
            let xi = floor(x), yi = floor(y), xf = x - xi, yf = y - yi
            let a = hash(xi, yi), b = hash(xi + 1, yi), c = hash(xi, yi + 1), d = hash(xi + 1, yi + 1)
            let u = smooth(xf), v = smooth(yf)
            return a * (1 - u) * (1 - v) + b * u * (1 - v) + c * (1 - u) * v + d * u * v
        }
        func fbm(_ x: Double, _ y: Double) -> Double {
            var f = 0.0, amp = 0.5, freq = 1.0
            for _ in 0..<6 { f += amp * vnoise(x * freq, y * freq); freq *= 2; amp *= 0.5 }
            return f
        }
        var px = [UInt8](repeating: 0, count: w * h * 4)
        let sea = 0.48
        for y in 0..<h {
            let lat = abs(Double(y) / Double(h) - 0.5) * 2
            let pole = 1 - lat * lat * 0.55
            for x in 0..<w {
                let n = pow(fbm(Double(x) / Double(w) * 7, Double(y) / Double(h) * 7), 1.35)
                let elev = min(1, max(0, n * pole))
                var r = 0.0, g = 0.0, b = 0.0
                if lat > 0.9 {                          // ice caps
                    r = 0.85; g = 0.91; b = 1.0
                } else if elev < sea {                  // ocean
                    let t = elev / sea
                    r = 0.02 + t * 0.04; g = 0.09 + t * 0.20; b = 0.30 + t * 0.34
                } else {                                // land
                    let t = (elev - sea) / (1 - sea)
                    if t < 0.5 { let u = t * 2; r = 0.12 + u * 0.16; g = 0.30 + u * 0.20; b = 0.15 + u * 0.05 }
                    else { let u = (t - 0.5) * 2; r = 0.30 + u * 0.45; g = 0.42 + u * 0.40; b = 0.20 + u * 0.45 }
                }
                let i = (y * w + x) * 4
                px[i] = UInt8(min(255, r * 255)); px[i+1] = UInt8(min(255, g * 255)); px[i+2] = UInt8(min(255, b * 255)); px[i+3] = 255
            }
        }
        let cs = CGColorSpaceCreateDeviceRGB()
        guard let ctx = CGContext(data: &px, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                  space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue),
              let cg = ctx.makeImage() else { return }
        #if canImport(UIKit)
        globeTexture = UIImage(cgImage: cg)
        #else
        globeTexture = NSImage(cgImage: cg, size: CGSize(width: w, height: h))
        #endif
    }
}

/// One living electricity arc between two nodes: a pre-computed great-circle path lifted off the
/// surface, plus the per-vertex frame (bitangent + radial) the lightning wander is applied along.
final class Arc {
    let base: [simd_float3]
    let bi: [simd_float3]
    let rad: [simd_float3]
    let n: Int
    let seed: Float
    let jrate: Float
    let core = SCNNode()
    let glow = SCNNode()

    init(a: simd_float3, b: simd_float3, seed: Float) {
        self.seed = seed
        self.jrate = 7 + abs(sin(seed)) * 4
        let N = 44; self.n = N
        let an = simd_normalize(a), bn = simd_normalize(b)
        let ang = acos(max(-1, min(1, simd_dot(an, bn))))
        let lift: Float = 0.035 + ang * 0.06         // hug the surface — a low, tight arc
        var base = [simd_float3](), biN = [simd_float3](), radN = [simd_float3]()
        for s in 0...N {
            let t = Float(s) / Float(N)
            let v: simd_float3
            if ang < 1e-4 { v = simd_normalize(simd_mix(an, bn, simd_float3(repeating: t))) }
            else { v = an * (sin((1 - t) * ang) / sin(ang)) + bn * (sin(t * ang) / sin(ang)) }
            base.append(simd_normalize(v) * (1.012 + sin(Float.pi * t) * lift))
        }
        for s in 0...N {
            let radial = simd_normalize(base[s])
            let tan = simd_normalize(base[min(N, s + 1)] - base[max(0, s - 1)])
            var bit = simd_cross(tan, radial)
            if simd_length(bit) < 1e-5 { bit = simd_float3(0, 1, 0) }
            biN.append(simd_normalize(bit)); radN.append(radial)
        }
        self.base = base; self.bi = biN; self.rad = radN
    }
}

/// Owns rotation state, and drives the scene each frame (SCNSceneRendererDelegate). One-finger drag
/// orbits the globe; the panel-sweep gesture lives at the console level so it works over the panels
/// too (see `FamiliarConsole`).
final class SphereCoordinator: NSObject, SCNSceneRendererDelegate {
    let world = SphereScene()
    var yaw: CGFloat = 0, pitch: CGFloat = 0.1
    var dragging = false
    private var lastTime: TimeInterval = 0

    override init() { super.init() }

    func renderer(_ renderer: SCNSceneRenderer, updateAtTime time: TimeInterval) {
        let dt = lastTime == 0 ? 0 : min(time - lastTime, 0.05); lastTime = time
        if !dragging { yaw += CGFloat(dt) * 0.14 }              // idle autorotate
        world.group.eulerAngles = SCNVector3(Float(pitch), Float(yaw), 0)
        world.updateArcs(Float(time))                           // living electricity
    }
    func orbit(dx: CGFloat, dy: CGFloat) {
        yaw += dx * 0.006
        pitch = max(-1.2, min(1.2, pitch + dy * 0.006))
    }
}

/// The public view — a platform representable that owns the SCNView. The Metal interface uses only
/// the Globe now (Marble/Mesh dropped), but the mode param is retained for callers that still pass it.
public struct FamiliarSphereView: View {
    private let mode: SphereMode
    private let pins: [SpherePin]
    public init(mode: SphereMode = .globe, pins: [SpherePin] = []) {
        self.mode = mode; self.pins = pins
    }
    public var body: some View { SphereRep(mode: mode, pins: pins) }
}

#if canImport(UIKit)
struct SphereRep: UIViewRepresentable {
    let mode: SphereMode; let pins: [SpherePin]
    func makeCoordinator() -> SphereCoordinator { SphereCoordinator() }
    func makeUIView(context: Context) -> SCNView {
        let c = context.coordinator
        c.world.build()
        let v = SCNView()
        v.scene = c.world.scene
        v.pointOfView = c.world.camera
        v.backgroundColor = .clear
        v.antialiasingMode = .multisampling4X
        v.rendersContinuously = true
        v.delegate = c
        v.allowsCameraControl = false
        let one = UIPanGestureRecognizer(target: context.coordinator, action: #selector(SphereCoordinator.oneFinger(_:)))
        one.minimumNumberOfTouches = 1; one.maximumNumberOfTouches = 1
        v.addGestureRecognizer(one)
        return v
    }
    func updateUIView(_ v: SCNView, context: Context) {
        context.coordinator.world.apply(mode)
        context.coordinator.world.setPins(pins)
    }
}
extension SphereCoordinator {
    @objc func oneFinger(_ g: UIPanGestureRecognizer) {
        let t = g.translation(in: g.view); g.setTranslation(.zero, in: g.view)
        if g.state == .began { dragging = true }
        if g.state == .changed { orbit(dx: t.x, dy: t.y) }
        if g.state == .ended || g.state == .cancelled { dragging = false }
    }
}
#else
struct SphereRep: NSViewRepresentable {
    let mode: SphereMode; let pins: [SpherePin]
    func makeCoordinator() -> SphereCoordinator { SphereCoordinator() }
    func makeNSView(context: Context) -> SCNView {
        let c = context.coordinator
        c.world.build()
        let v = OrbitSCNView()
        v.coordinator = c
        v.scene = c.world.scene
        v.pointOfView = c.world.camera
        v.backgroundColor = .clear
        v.antialiasingMode = .multisampling4X
        v.rendersContinuously = true
        v.delegate = c
        v.allowsCameraControl = false
        return v
    }
    func updateNSView(_ v: SCNView, context: Context) {
        context.coordinator.world.apply(mode)
        context.coordinator.world.setPins(pins)
    }
    /// One-finger drag orbits the globe. (The panel sweep is handled at the console level.)
    final class OrbitSCNView: SCNView {
        weak var coordinator: SphereCoordinator?
        override func mouseDown(with e: NSEvent) { coordinator?.dragging = true }
        override func mouseDragged(with e: NSEvent) { coordinator?.orbit(dx: e.deltaX, dy: e.deltaY) }
        override func mouseUp(with e: NSEvent) { coordinator?.dragging = false }
    }
}
#endif

#endif
