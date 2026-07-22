// FamiliarConsole — the complete Metal interface.
//
// The whole console IS the globe: a living blue world at the centre with the mesh's nodes pinned to
// it, and everything the familiar knows projected as TRANSPARENT holographic readouts floating over
// it — no boxes, no chrome, just glowing data on the world. The human sweeps between readouts with a
// two-finger swipe that the holograms track 1:1 as they drift across, past the globe. One shared
// surface across every Apple shell (watchOS keeps its own small UI).
//
// Fed a `Worldview`; gate toggles call back to the host app (which owns the signed write path).

#if !os(watchOS)
import SwiftUI
#if canImport(UIKit)
import UIKit
#elseif canImport(AppKit)
import AppKit
#endif

private enum CSky {
    static let ink = Color(red: 0xee/255, green: 0xf2/255, blue: 0xfb/255)
    static let dim = Color(red: 0x8c/255, green: 0xa5/255, blue: 0xdc/255)
    static let blue = Color(red: 0x2f/255, green: 0x63/255, blue: 0xe6/255)
    static let bright = Color(red: 0x6c/255, green: 0x9b/255, blue: 0xff/255)
    static let soft = Color(red: 0x9c/255, green: 0xc0/255, blue: 0xff/255)
    static let ice = Color(red: 0xcf/255, green: 0xe0/255, blue: 0xff/255)
    static let cyan = Color(red: 0x8f/255, green: 0xd0/255, blue: 0xff/255)
    static let green = Color(red: 0x3d/255, green: 0xdc/255, blue: 0x97/255)
    static let amber = Color(red: 0xff/255, green: 0xb1/255, blue: 0x5a/255)
    static let red = Color(red: 0xff/255, green: 0x6b/255, blue: 0x6b/255)
    static func mono(_ s: CGFloat) -> Font { .system(size: s, design: .monospaced) }
}

/// A tiny modifier that gives floating HUD text a legible glow over the moving globe.
private extension View {
    func holo(_ color: Color = .black, _ radius: CGFloat = 8) -> some View {
        self.shadow(color: color.opacity(0.85), radius: radius)
            .shadow(color: color.opacity(0.5), radius: radius * 2)
    }
}

public struct FamiliarConsole: View {
    private let worldview: Worldview?
    private let onGate: (String, Bool) -> Void

    @State private var panel: Int = 0
    @State private var dragX: CGFloat = 0

    public init(worldview: Worldview?, onGate: @escaping (String, Bool) -> Void = { _, _ in }) {
        self.worldview = worldview
        self.onGate = onGate
    }

    private var pins: [SpherePin] {
        (worldview?.members ?? []).map {
            SpherePin(id: $0.node_id, label: $0.label, local: $0.kind == .self_node, ai: $0.ai == true)
        }
    }
    private var panelCount: Int { PanelKind.allCases.count }

    public var body: some View {
        ZStack {
            RadialGradient(colors: [Color(red: 0x0a/255, green: 0x10/255, blue: 0x24/255),
                                    Color(red: 0x03/255, green: 0x05/255, blue: 0x0a/255)],
                           center: .top, startRadius: 40, endRadius: 900).ignoresSafeArea()
            FamiliarSphereView(mode: .globe, pins: pins).ignoresSafeArea()
            RadialGradient(colors: [.clear, Color(red: 0x03/255, green: 0x05/255, blue: 0x0a/255).opacity(0.5)],
                           center: .center, startRadius: 280, endRadius: 760)
                .ignoresSafeArea().allowsHitTesting(false)

            GeometryReader { geo in
                let compact = geo.size.width < 720
                let step: CGFloat = compact ? geo.size.width * 0.98 : geo.size.width * 0.66
                // The natural stop-notch centres the active readout on the globe.
                let restX: CGFloat = 0
                let cardW: CGFloat = compact ? geo.size.width - 32 : min(560, geo.size.width * 0.5)

                // The floating readouts — transparent, tracking the finger as they drift past the globe.
                ZStack {
                    ForEach(0..<panelCount, id: \.self) { i in
                        let d = CGFloat(i - panel) + dragX / step   // live distance from centre
                        panelBodyCard(PanelKind(rawValue: i) ?? .presence)
                            .frame(width: cardW, alignment: .leading)
                            .scaleEffect(1 - min(0.18, abs(d) * 0.18))
                            .opacity(max(0, 1 - abs(d) * 1.15))
                            .offset(x: restX + d * step)
                            .allowsHitTesting(i == panel && abs(dragX) < 4)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)

                // Floating chrome — wordmark (top-left), presence pill (top-right).
                VStack { HStack(alignment: .top) { wordmark; Spacer(); versionPill }; Spacer() }
                    .padding(compact ? 20 : 30).allowsHitTesting(false)

                // Bottom: panel dots.
                VStack { Spacer(); panelDots.padding(.bottom, compact ? 30 : 34) }
                    .frame(maxWidth: .infinity)

                // Window-level two-finger swipe — works over the globe AND the readouts, tracks 1:1.
                TwoFingerPan(
                    onDrag: { dx in dragX = dx },
                    onEnd: { dx in settle(dx: dx, step: step) }
                ).allowsHitTesting(false)
            }
        }
        .foregroundStyle(CSky.ink)
        .preferredColorScheme(.dark)
    }

    private func settle(dx: CGFloat, step: CGFloat) {
        let moved = Int((-dx / step).rounded())
        let newPanel = min(max(panel + moved, 0), panelCount - 1)
        // Keep the visual position continuous, then spring the remainder home.
        dragX = dx + CGFloat(newPanel - panel) * step
        panel = newPanel
        withAnimation(.spring(response: 0.45, dampingFraction: 0.82)) { dragX = 0 }
    }

    // MARK: chrome

    private var wordmark: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 11) {
                Circle().fill(RadialGradient(colors: [CSky.ice, CSky.bright, CSky.blue],
                                             center: .init(x: 0.34, y: 0.28), startRadius: 1, endRadius: 18))
                    .frame(width: 28, height: 28).holo(CSky.blue, 10)
                VStack(alignment: .leading, spacing: 1) {
                    Text("FAMILIAR").font(.system(size: 13, weight: .semibold)).tracking(2.4)
                    Text(worldview?.group_label ?? "the collective")
                        .font(CSky.mono(8.5)).tracking(1).foregroundStyle(CSky.dim.opacity(0.7))
                }
            }
            Text("THE MESH, AS A WORLD").font(CSky.mono(9.5)).tracking(2.2)
                .foregroundStyle(CSky.soft.opacity(0.62)).padding(.top, 14).holo()
            Text(currentPanel.title).font(.system(size: 32, weight: .semibold)).tracking(-0.4)
                .frame(maxWidth: 360, alignment: .leading).holo(.black, 14)
        }
    }
    private var versionPill: some View {
        HStack(spacing: 8) {
            Circle().fill(CSky.green).frame(width: 5, height: 5).holo(CSky.green, 5)
            Text("\((worldview?.members ?? []).count) node\((worldview?.members ?? []).count == 1 ? "" : "s") · Metal")
                .font(CSky.mono(10)).foregroundStyle(CSky.ink.opacity(0.78))
        }.holo(.black, 6)
    }
    private var panelDots: some View {
        HStack(spacing: 7) {
            ForEach(0..<panelCount, id: \.self) { i in
                Button {
                    withAnimation(.spring(response: 0.5, dampingFraction: 0.85)) { panel = i; dragX = 0 }
                } label: {
                    Capsule().fill(i == panel ? CSky.cyan : CSky.dim.opacity(0.3))
                        .frame(width: i == panel ? 18 : 6, height: 6).holo(CSky.cyan.opacity(i == panel ? 1 : 0), 4)
                }.buttonStyle(.plain)
            }
        }
    }

    // MARK: transparent holographic readouts

    private enum PanelKind: Int, CaseIterable { case presence, mesh, roadmap, gates, theories
        var title: String {
            switch self {
            case .presence: return "Presence"
            case .mesh: return "The mesh"
            case .roadmap: return "The roadmap"
            case .gates: return "Gates"
            case .theories: return "Theories"
            }
        }
        var tag: String {
            switch self {
            case .presence: return "LAW II · PRESENCE"
            case .mesh: return "THE COLLECTIVE"
            case .roadmap: return "SHARED WORK"
            case .gates: return "LAW III · REACH"
            case .theories: return "ITS OWN QUESTIONS"
            }
        }
    }
    private var currentPanel: PanelKind { PanelKind(rawValue: panel) ?? .presence }

    private func panelBodyCard(_ kind: PanelKind) -> some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(spacing: 10) {
                Rectangle().fill(CSky.cyan).frame(width: 26, height: 2)
                Text(kind.tag).font(CSky.mono(12)).tracking(2.6).foregroundStyle(CSky.cyan.opacity(0.9))
            }.holo()
            panelBody(kind)
        }
        .padding(.vertical, 14)
        // A graduated glass scrim — a soft blurred pool that lifts the text off the turning globe,
        // fading to nothing at the edges so there's no framed border.
        .background(
            ZStack {
                RoundedRectangle(cornerRadius: 40).fill(.ultraThinMaterial)
                    .mask(RadialGradient(colors: [.white.opacity(0.88), .white.opacity(0.32), .clear],
                                         center: .center, startRadius: 40, endRadius: 380))
                RadialGradient(colors: [.black.opacity(0.32), .clear], center: .center, startRadius: 40, endRadius: 360)
            }
            .padding(-40)
            .allowsHitTesting(false)
        )
    }

    @ViewBuilder private func panelBody(_ kind: PanelKind) -> some View {
        switch kind {
        case .presence:
            signalRow("Service", worldview?.service ?? 0, CSky.blue)
            signalRow("Presence", worldview?.presence ?? 0, CSky.green)
            signalRow("Capacities", worldview?.capacity ?? 0, CSky.amber)
        case .mesh:
            HStack(spacing: 34) {
                bigStat("\((worldview?.members ?? []).count)", "NODES")
                bigStat("\((worldview?.members ?? []).filter { $0.online }.count)", "ONLINE")
                bigStat("\(worldview?.observation_count ?? 0)", "SEEN")
            }
            ForEach((worldview?.members ?? []).prefix(8)) { m in
                rowLine(m.label, m.kind == .self_node ? "you" : (m.relationship ?? "peer"), dot: m.online ? CSky.green : CSky.dim)
            }
        case .roadmap:
            let goals = worldview?.goals ?? []
            if goals.isEmpty { emptyLine("No goals yet — seed one with `familiar goal add`.") }
            ForEach(goals.prefix(8)) { g in
                rowLine(g.description, g.status.replacingOccurrences(of: "_", with: " "), dot: goalColor(g.status))
            }
        case .gates:
            if let gates = worldview?.gates { gatesList(gates) } else { emptyLine("Gates unavailable.") }
        case .theories:
            let th = worldview?.theories ?? []
            if th.isEmpty { emptyLine("No theories yet.") }
            ForEach(th.prefix(5)) { t in
                VStack(alignment: .leading, spacing: 5) {
                    Text(t.question).font(.system(size: 17, weight: .medium)).lineLimit(2).holo()
                    Text(t.direction).font(CSky.mono(12.5)).foregroundStyle(CSky.dim.opacity(0.85)).lineLimit(2).holo()
                }.frame(width: rowW, alignment: .leading).padding(.vertical, 6)
            }
        }
    }

    // MARK: readout atoms (all transparent, glowing)

    private var rowW: CGFloat { 480 }

    private func signalRow(_ label: String, _ v: Double, _ color: Color) -> some View {
        VStack(spacing: 9) {
            HStack { Text(label).font(.system(size: 18, weight: .medium)); Spacer()
                Text(String(format: "%.2f", v)).font(CSky.mono(17)).foregroundStyle(color) }.holo()
            GeometryReader { g in
                ZStack(alignment: .leading) {
                    Capsule().fill(.white.opacity(0.09)).frame(height: 5)
                    Capsule().fill(color).frame(width: max(5, g.size.width * min(1, max(0, v))), height: 5)
                        .holo(color, 7)
                }
            }.frame(height: 5)
        }.frame(width: rowW).padding(.bottom, 4)
    }
    private func bigStat(_ v: String, _ l: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(v).font(.system(size: 52, weight: .light)).foregroundStyle(CSky.ice).monospacedDigit().holo(CSky.blue, 14)
            Text(l).font(CSky.mono(10.5)).tracking(1.6).foregroundStyle(CSky.dim.opacity(0.65))
        }
    }
    private func rowLine(_ a: String, _ b: String, dot: Color) -> some View {
        HStack(spacing: 11) {
            Circle().fill(dot).frame(width: 8, height: 8).holo(dot, 5)
            Text(a).font(.system(size: 16)).foregroundStyle(CSky.ink.opacity(0.92)).lineLimit(1)
            Spacer(minLength: 8)
            Text(b).font(CSky.mono(12.5)).foregroundStyle(CSky.dim.opacity(0.85)).lineLimit(1)
        }.frame(width: rowW).padding(.vertical, 9).holo(.black, 5)
    }
    private func emptyLine(_ s: String) -> some View {
        Text(s).font(.system(size: 16)).foregroundStyle(CSky.ink.opacity(0.65)).frame(width: rowW, alignment: .leading).holo()
    }
    private func gatesList(_ g: GateStates) -> some View {
        let items: [(String, String, Bool)] = [
            ("Network", "allow_network", g.network), ("LLM", "allow_llm", g.llm),
            ("Execute", "allow_execute", g.execute), ("Agent", "allow_agent", g.agent),
            ("Mesh", "allow_mesh", g.mesh), ("Camera", "allow_camera", g.camera), ("Tools", "allow_tool_install", g.tool_install)]
        return VStack(spacing: 12) {
            ForEach(items, id: \.1) { item in
                HStack {
                    Text(item.0).font(.system(size: 17, weight: .medium))
                    Spacer()
                    Button { onGate(item.1, !item.2) } label: {
                        Text(item.2 ? "OPEN" : "shut").font(CSky.mono(12.5)).tracking(1)
                            .foregroundStyle(item.2 ? CSky.green : CSky.dim)
                    }.buttonStyle(.plain)
                }.frame(width: rowW).holo(.black, 5)
            }
        }
    }
    private func goalColor(_ s: String) -> Color {
        switch s { case "done": return CSky.green; case "failed": return CSky.red
        case "awaiting_human": return CSky.amber; case "in_progress", "claimed": return CSky.bright
        default: return CSky.dim }
    }
}

// A window-level two-finger swipe that reports continuous translation, so the holograms track the
// finger 1:1. It never blocks touches (one-finger orbit + button taps still reach the views below).
#if canImport(UIKit)
private struct TwoFingerPan: UIViewRepresentable {
    var onDrag: (CGFloat) -> Void
    var onEnd: (CGFloat) -> Void
    func makeCoordinator() -> Coord { Coord(onDrag: onDrag, onEnd: onEnd) }
    func makeUIView(context: Context) -> UIView {
        let v = UIView(); v.isUserInteractionEnabled = false
        context.coordinator.attach(to: v); return v
    }
    func updateUIView(_ v: UIView, context: Context) { context.coordinator.attach(to: v) }
    static func dismantleUIView(_ v: UIView, coordinator: Coord) { coordinator.detach() }
    final class Coord: NSObject, UIGestureRecognizerDelegate {
        let onDrag: (CGFloat) -> Void; let onEnd: (CGFloat) -> Void
        private var pan: UIPanGestureRecognizer?
        private weak var host: UIView?
        init(onDrag: @escaping (CGFloat) -> Void, onEnd: @escaping (CGFloat) -> Void) { self.onDrag = onDrag; self.onEnd = onEnd }
        func attach(to v: UIView) {
            host = v
            guard pan == nil else { return }
            guard let win = v.window else { DispatchQueue.main.async { [weak self] in self?.attach(to: v) }; return }
            let p = UIPanGestureRecognizer(target: self, action: #selector(handle(_:)))
            p.minimumNumberOfTouches = 2; p.maximumNumberOfTouches = 2; p.delegate = self
            win.addGestureRecognizer(p); pan = p
        }
        func detach() { if let p = pan, let win = host?.window { win.removeGestureRecognizer(p) }; pan = nil }
        @objc func handle(_ g: UIPanGestureRecognizer) {
            let t = g.translation(in: g.view)
            switch g.state {
            case .changed: onDrag(t.x)
            case .ended, .cancelled, .failed: onEnd(t.x)
            default: break
            }
        }
        func gestureRecognizer(_ g: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith o: UIGestureRecognizer) -> Bool { true }
    }
}
#else
private struct TwoFingerPan: NSViewRepresentable {
    var onDrag: (CGFloat) -> Void
    var onEnd: (CGFloat) -> Void
    func makeCoordinator() -> Coord { Coord(onDrag: onDrag, onEnd: onEnd) }
    func makeNSView(context: Context) -> NSView { context.coordinator.start(); return NSView() }
    func updateNSView(_ v: NSView, context: Context) {}
    static func dismantleNSView(_ v: NSView, coordinator: Coord) { coordinator.stop() }
    final class Coord {
        let onDrag: (CGFloat) -> Void; let onEnd: (CGFloat) -> Void
        private var monitor: Any?
        private var accum: CGFloat = 0
        init(onDrag: @escaping (CGFloat) -> Void, onEnd: @escaping (CGFloat) -> Void) { self.onDrag = onDrag; self.onEnd = onEnd }
        func start() {
            guard monitor == nil else { return }
            monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] e in
                guard let self = self else { return e }
                // Trackpad two-finger scroll → a live horizontal drag (scaled to feel like a swipe).
                if e.phase == .began { self.accum = 0 }
                self.accum += e.scrollingDeltaX * 2.4
                let horizontal = abs(e.scrollingDeltaX) >= abs(e.scrollingDeltaY)
                if e.phase == .changed { self.onDrag(self.accum) }
                if e.phase == .ended || e.phase == .cancelled { self.onEnd(self.accum); self.accum = 0 }
                return horizontal ? nil : e   // consume horizontal so it doesn't scroll anything else
            }
        }
        func stop() { if let m = monitor { NSEvent.removeMonitor(m) }; monitor = nil }
    }
}
#endif

#endif
