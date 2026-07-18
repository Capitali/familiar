// FamiliarSphere — the Metal Sphere, native.
//
// A SceneKit port of the "Familiar Metal Sphere" design: one blue-marble that morphs across three
// states — Marble (a breathing glass presence), Mesh (the wireframe pipeline over it), and Globe
// (a displaced blue world with the mesh's peers pinned to it). It lives in the shared package so
// EVERY Apple shell renders the same surface: iPhone, iPad, Mac, and — as those targets arrive —
// Apple TV and Vision. SceneKit's `SceneView` isn't on watchOS, so the watch shows a lightweight
// SwiftUI fallback orb; the concept still travels there.
//
// Fed live: pass the mesh's members as `SpherePin`s (the local node highlighted). No geolocation is
// assumed — pins are distributed deterministically by id on a golden-angle spiral, so the same node
// always lands in the same place, and the sphere reads as "the collective," not a literal map.

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
    /// The local node — brighter, larger, ice-white.
    public let local: Bool
    /// This node has direct/context AI (badged with a warmer tint).
    public let ai: Bool
    public init(id: String, label: String, local: Bool, ai: Bool = false) {
        self.id = id
        self.label = label
        self.local = local
        self.ai = ai
    }
}

/// The three render states of the Metal Sphere.
public enum SphereMode: String, CaseIterable, Identifiable, Sendable {
    case marble = "Marble"
    case mesh = "Mesh"
    case globe = "Globe"
    public var id: String { rawValue }
    /// The mono sub-label shown under the mode name (mirrors the design).
    public var hint: String {
        switch self {
        case .marble: return "SMOOTH"
        case .mesh: return "WIREFRAME"
        case .globe: return "TERRAIN"
        }
    }
    public var caption: String {
        switch self {
        case .marble: return "The blue marble, breathing — a glass sphere on a 6-second cycle. Presence, not decoration."
        case .mesh: return "The mesh exposed — the same surface as a wireframe. Peers pinned to the collective."
        case .globe: return "The world, and everyone on it — the mesh's nodes placed on a living globe."
        }
    }
}

// MARK: - Palette (self-contained; mirrors the design so the package needs no UI dependency)

private enum Sky {
    static let bg = PColor(red: 0x03/255, green: 0x05/255, blue: 0x0a/255, alpha: 1)
    static let deep = PColor(red: 0.043, green: 0.145, blue: 0.42, alpha: 1)
    static let blue = PColor(red: 0x2f/255, green: 0x63/255, blue: 0xe6/255, alpha: 1)
    static let bright = PColor(red: 0x6c/255, green: 0x9b/255, blue: 0xff/255, alpha: 1)
    static let cyan = PColor(red: 0x8f/255, green: 0xd0/255, blue: 0xff/255, alpha: 1)
    static let ice = PColor(red: 0xcf/255, green: 0xe0/255, blue: 0xff/255, alpha: 1)
    static let amber = PColor(red: 0xff/255, green: 0xb1/255, blue: 0x5a/255, alpha: 1)
}

#if os(watchOS)

/// watchOS has no `SceneView`; show a simple breathing orb so the concept still travels to the wrist.
public struct FamiliarSphereView: View {
    private let mode: SphereMode
    private let pins: [SpherePin]
    @State private var breathe = false
    public init(mode: SphereMode = .marble, pins: [SpherePin] = []) {
        self.mode = mode
        self.pins = pins
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
                .shadow(color: Color(red: 0x2f/255, green: 0x63/255, blue: 0xe6/255).opacity(0.6), radius: 24)
                .onAppear { withAnimation(.easeInOut(duration: 3).repeatForever(autoreverses: true)) { breathe = true } }
        }
    }
}

#else

/// Builds and owns the SCNScene; the SwiftUI view drives it and animates state transitions.
public final class SphereController: ObservableObject {
    public let scene = SCNScene()
    public let cameraNode = SCNNode()

    private let group = SCNNode()          // spun + breathed; holds every sphere layer + pins
    private let coreMat = SCNMaterial()    // the marble/globe surface
    private let wireMat = SCNMaterial()    // the mesh overlay
    private let atmoMat = SCNMaterial()    // the halo
    private let pinRoot = SCNNode()
    private var built = false
    private var globeTexture: Any?

    public init() {}

    public func build() {
        guard !built else { return }
        built = true
        scene.background.contents = Sky.bg

        // Camera — SceneView drives orbit via allowsCameraControl; we set a sensible start.
        let cam = SCNCamera()
        cam.fieldOfView = 40
        cam.zNear = 0.1
        cam.zFar = 100
        cameraNode.camera = cam
        cameraNode.position = SCNVector3(0, 0, 3.05)
        scene.rootNode.addChildNode(cameraNode)

        // Lights — a warm key, cool rim, soft ambient (matching the design's sun-lit marble).
        addLight(.ambient, color: PColor(red: 0.10, green: 0.14, blue: 0.25, alpha: 1), intensity: 380, at: nil)
        addLight(.directional, color: PColor(red: 1.0, green: 0.96, blue: 0.90, alpha: 1), intensity: 1050,
                 at: SCNVector3(3, 1.4, 3))
        addLight(.directional, color: Sky.bright, intensity: 360, at: SCNVector3(-3, -1, -2))

        // ---- Core: the marble/globe surface ----
        coreMat.lightingModel = .physicallyBased
        coreMat.diffuse.contents = Sky.deep
        coreMat.metalness.contents = 0.35
        coreMat.roughness.contents = 0.38
        coreMat.emission.contents = PColor(red: 0.07, green: 0.16, blue: 0.44, alpha: 1)
        coreMat.emission.intensity = 0.35
        coreMat.reflective.contents = Sky.cyan     // built-in fresnel rim — no custom shader needed
        coreMat.fresnelExponent = 2.2
        let core = SCNNode(geometry: sphere(radius: 1.0, seg: 96, material: coreMat))
        group.addChildNode(core)

        // ---- Mesh: the wireframe pipeline over the surface ----
        wireMat.lightingModel = .constant
        wireMat.fillMode = .lines
        wireMat.diffuse.contents = Sky.cyan
        wireMat.emission.contents = Sky.cyan
        wireMat.transparency = 0            // faded in for .mesh / .globe
        let wire = SCNNode(geometry: sphere(radius: 1.004, seg: 48, material: wireMat))
        group.addChildNode(wire)

        // ---- Atmosphere: a soft additive halo ----
        atmoMat.lightingModel = .constant
        atmoMat.diffuse.contents = PColor.clear
        atmoMat.emission.contents = PColor(red: 0.25, green: 0.51, blue: 1.0, alpha: 1)
        atmoMat.blendMode = .add
        atmoMat.cullMode = .front            // render the far side → reads as a rim glow
        atmoMat.isDoubleSided = false
        atmoMat.writesToDepthBuffer = false
        atmoMat.transparency = 0.55
        let atmo = SCNNode(geometry: sphere(radius: 1.28, seg: 48, material: atmoMat))
        group.addChildNode(atmo)

        // ---- Pins live under the group so they spin/breathe with the sphere ----
        group.addChildNode(pinRoot)

        scene.rootNode.addChildNode(group)

        // Breathing (6s) + a slow autorotate, both on the group so the camera stays free to orbit.
        let up = SCNAction.scale(to: 1.018, duration: 3)
        up.timingMode = .easeInEaseOut
        let down = SCNAction.scale(to: 1.0, duration: 3)
        down.timingMode = .easeInEaseOut
        group.runAction(.repeatForever(.sequence([up, down])))
        group.runAction(.repeatForever(.rotateBy(x: 0, y: .pi * 2, z: 0, duration: 46)))
    }

    /// Cross-fade the layers to a mode. Called on appear and on every mode change.
    public func apply(_ mode: SphereMode) {
        globeTextureIfNeeded()
        SCNTransaction.begin()
        SCNTransaction.animationDuration = 0.6
        switch mode {
        case .marble:
            coreMat.diffuse.contents = Sky.deep
            coreMat.transparency = 1
            wireMat.transparency = 0
            atmoMat.transparency = 0.55
            pinRoot.opacity = 0
        case .mesh:
            coreMat.diffuse.contents = Sky.deep
            coreMat.transparency = 0.5
            wireMat.transparency = 1
            atmoMat.transparency = 0.4
            pinRoot.opacity = 1
        case .globe:
            coreMat.diffuse.contents = globeTexture ?? Sky.deep
            coreMat.transparency = 1
            wireMat.transparency = 0.18
            atmoMat.transparency = 0.5
            pinRoot.opacity = 1
        }
        SCNTransaction.commit()
    }

    /// Rebuild the pins from the current mesh membership.
    public func setPins(_ pins: [SpherePin]) {
        pinRoot.childNodes.forEach { $0.removeFromParentNode() }
        let n = max(1, pins.count)
        for (i, p) in pins.enumerated() {
            let dir = spiralPoint(index: i, count: n)
            let color: PColor = p.local ? Sky.ice : (p.ai ? Sky.amber : Sky.bright)
            let r: CGFloat = p.local ? 0.032 : 0.022

            let dotMat = SCNMaterial()
            dotMat.lightingModel = .constant
            dotMat.diffuse.contents = color
            dotMat.emission.contents = color
            let dot = SCNNode(geometry: SCNSphere(radius: r))
            dot.geometry?.firstMaterial = dotMat
            dot.position = SCNVector3(dir.x * 1.03, dir.y * 1.03, dir.z * 1.03)

            // a faint halo ring around each pin (a flat disc, billboarded to the camera)
            let halo = SCNNode(geometry: SCNPlane(width: r * 6, height: r * 6))
            let haloMat = SCNMaterial()
            haloMat.lightingModel = .constant
            haloMat.diffuse.contents = radialSprite(color)
            haloMat.blendMode = .add
            haloMat.writesToDepthBuffer = false
            haloMat.isDoubleSided = true
            halo.geometry?.firstMaterial = haloMat
            halo.position = dot.position
            halo.constraints = [SCNBillboardConstraint()]

            pinRoot.addChildNode(halo)
            pinRoot.addChildNode(dot)
        }
    }

    // MARK: helpers

    private func sphere(radius: CGFloat, seg: Int, material: SCNMaterial) -> SCNSphere {
        let s = SCNSphere(radius: radius)
        s.segmentCount = seg
        s.firstMaterial = material
        return s
    }

    private func addLight(_ type: SCNLight.LightType, color: PColor, intensity: CGFloat, at pos: SCNVector3?) {
        let l = SCNLight()
        l.type = type
        l.color = color
        l.intensity = intensity
        let node = SCNNode()
        node.light = l
        if let pos = pos { node.position = pos }
        scene.rootNode.addChildNode(node)
    }

    /// Even distribution on a sphere by the golden angle — stable per index.
    private func spiralPoint(index i: Int, count n: Int) -> SCNVector3 {
        let gold = Double.pi * (3.0 - (5.0).squareRoot())
        let y = 1.0 - (Double(i) / Double(max(1, n - 1))) * 2.0     // 1 … -1
        let r = (1.0 - y * y).squareRoot()
        let theta = gold * Double(i)
        return SCNVector3(Float(cos(theta) * r), Float(y), Float(sin(theta) * r))
    }

    /// A soft round sprite for pin halos / the globe — a radial gradient CGImage.
    private func radialSprite(_ color: PColor) -> Any {
        let size = 64
        #if canImport(UIKit)
        let renderer = UIGraphicsImageRenderer(size: CGSize(width: size, height: size))
        return renderer.image { ctx in
            let cg = ctx.cgContext
            let c = color.cgColor.components ?? [1, 1, 1, 1]
            let colors = [PColor(red: c[0], green: c[1], blue: c[2], alpha: 1).cgColor,
                          PColor(red: c[0], green: c[1], blue: c[2], alpha: 0).cgColor] as CFArray
            if let grad = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(), colors: colors, locations: [0, 1]) {
                cg.drawRadialGradient(grad, startCenter: CGPoint(x: size/2, y: size/2), startRadius: 0,
                                      endCenter: CGPoint(x: size/2, y: size/2), endRadius: CGFloat(size/2), options: [])
            }
        }
        #else
        let img = NSImage(size: CGSize(width: size, height: size))
        img.lockFocus()
        let c = color.usingColorSpace(.deviceRGB) ?? color
        let grad = NSGradient(colors: [c.withAlphaComponent(1), c.withAlphaComponent(0)])
        grad?.draw(fromCenter: CGPoint(x: size/2, y: size/2), radius: 0,
                   toCenter: CGPoint(x: size/2, y: size/2), radius: CGFloat(size/2), options: [])
        img.unlockFocus()
        return img
        #endif
    }

    /// A banded blue "blue-marble" gradient generated once for the Globe mode (no external texture).
    private func globeTextureIfNeeded() {
        guard globeTexture == nil else { return }
        let w = 256, h = 128
        let stops: [(CGFloat, [CGFloat])] = [
            (0.0, [6/255, 16/255, 46/255]), (0.45, [0.043, 0.145, 0.42]),
            (0.62, [0.18, 0.38, 0.90]), (0.80, [0.42, 0.63, 1.0]), (1.0, [0.70, 0.83, 1.0])
        ]
        func ramp(_ t: CGFloat) -> [CGFloat] {
            for i in 0..<(stops.count - 1) where t <= stops[i + 1].0 {
                let a = stops[i], b = stops[i + 1]
                let k = (t - a.0) / max(0.0001, b.0 - a.0)
                return (0..<3).map { a.1[$0] + (b.1[$0] - a.1[$0]) * k }
            }
            return stops.last!.1
        }
        var px = [UInt8](repeating: 0, count: w * h * 4)
        for y in 0..<h {
            let lat = abs(CGFloat(y) / CGFloat(h) - 0.5) * 2
            let band = 0.5 + 0.5 * sin(CGFloat(y) * 0.6)       // faint latitude banding
            let t = max(0, min(1, (1 - lat) * 0.85 + band * 0.12))
            let c = ramp(t)
            for x in 0..<w {
                let i = (y * w + x) * 4
                px[i] = UInt8(c[0] * 255); px[i+1] = UInt8(c[1] * 255); px[i+2] = UInt8(c[2] * 255); px[i+3] = 255
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

/// The Metal Sphere as a SwiftUI view — drop it anywhere and drive it with `mode` + `pins`.
public struct FamiliarSphereView: View {
    @StateObject private var ctrl = SphereController()
    private let mode: SphereMode
    private let pins: [SpherePin]

    public init(mode: SphereMode = .marble, pins: [SpherePin] = []) {
        self.mode = mode
        self.pins = pins
    }

    public var body: some View {
        SceneView(scene: ctrl.scene, pointOfView: ctrl.cameraNode,
                  options: [.allowsCameraControl])
            .background(Color(red: 0x03/255, green: 0x05/255, blue: 0x0a/255))
            .onAppear {
                ctrl.build()
                ctrl.setPins(pins)
                ctrl.apply(mode)
            }
            .onChange(of: mode) { _, newValue in ctrl.apply(newValue) }
            .onChange(of: pins) { _, newValue in ctrl.setPins(newValue) }
    }
}

#endif
