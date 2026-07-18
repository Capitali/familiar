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
#endif

#if canImport(UIKit)
import UIKit
private typealias PColor = UIColor
#elseif canImport(AppKit)
import AppKit
private typealias PColor = NSColor
#endif

/// A member of the mesh rendered as a point on the sphere.
public struct SpherePin: Identifiable, Equatable {
    public let id: String
    public let label: String
    public let local: Bool
    public let ai: Bool
    public init(id: String, label: String, local: Bool, ai: Bool = false) {
        self.id = id; self.label = label; self.local = local; self.ai = ai
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
            wireMat.transparency = 0.18; atmoMat.transparency = 0.5; pinRoot.opacity = 1
        }
        SCNTransaction.commit()
    }

    func setPins(_ pins: [SpherePin]) {
        pinRoot.childNodes.forEach { $0.removeFromParentNode() }
        let n = max(1, pins.count)
        for (i, p) in pins.enumerated() {
            let dir = spiral(i, n)
            let color: PColor = p.local ? Sky.ice : (p.ai ? Sky.amber : Sky.bright)
            let r: CGFloat = p.local ? 0.032 : 0.022
            let dm = SCNMaterial(); dm.lightingModel = .constant; dm.diffuse.contents = color; dm.emission.contents = color
            let dot = SCNNode(geometry: SCNSphere(radius: r)); dot.geometry?.firstMaterial = dm
            dot.position = SCNVector3(dir.x * 1.03, dir.y * 1.03, dir.z * 1.03)
            pinRoot.addChildNode(dot)
        }
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
    private func globeTextureIfNeeded() {
        guard globeTexture == nil else { return }
        let w = 256, h = 128
        let stops: [(CGFloat, [CGFloat])] = [
            (0.0, [6/255, 16/255, 46/255]), (0.45, [0.043, 0.145, 0.42]),
            (0.62, [0.18, 0.38, 0.90]), (0.80, [0.42, 0.63, 1.0]), (1.0, [0.70, 0.83, 1.0])]
        func ramp(_ t: CGFloat) -> [CGFloat] {
            for i in 0..<(stops.count - 1) where t <= stops[i + 1].0 {
                let a = stops[i], b = stops[i + 1]; let k = (t - a.0) / max(0.0001, b.0 - a.0)
                return (0..<3).map { a.1[$0] + (b.1[$0] - a.1[$0]) * k }
            }
            return stops.last!.1
        }
        var px = [UInt8](repeating: 0, count: w * h * 4)
        for y in 0..<h {
            let lat = abs(CGFloat(y) / CGFloat(h) - 0.5) * 2
            let band = 0.5 + 0.5 * sin(CGFloat(y) * 0.6)
            let t = max(0, min(1, (1 - lat) * 0.85 + band * 0.12)); let c = ramp(t)
            for x in 0..<w { let i = (y * w + x) * 4
                px[i] = UInt8(c[0]*255); px[i+1] = UInt8(c[1]*255); px[i+2] = UInt8(c[2]*255); px[i+3] = 255 }
        }
        let cs = CGColorSpaceCreateDeviceRGB()
        guard let ctx = CGContext(data: &px, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w*4,
                                  space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue),
              let cg = ctx.makeImage() else { return }
        #if canImport(UIKit)
        globeTexture = UIImage(cgImage: cg)
        #else
        globeTexture = NSImage(cgImage: cg, size: CGSize(width: w, height: h))
        #endif
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
