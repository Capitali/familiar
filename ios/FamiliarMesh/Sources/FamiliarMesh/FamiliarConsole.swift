// FamiliarConsole — the complete Metal interface.
//
// Not a screen among screens: the whole console IS the sphere. A full-bleed glowing orb at the
// centre, and every piece of what the familiar knows floats on a holographic panel beside it —
// panels the human sweeps through horizontally with a two-finger swipe. Marble / Mesh / Globe
// switch the orb's state; the Globe carries the mesh as a living world. One shared surface across
// every Apple shell (watchOS keeps its own small UI and doesn't use this).
//
// Fed a `Worldview`; gate toggles call back out to the host app (which owns the signed write path).

#if !os(watchOS)
import SwiftUI

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

public struct FamiliarConsole: View {
    private let worldview: Worldview?
    private let onGate: (String, Bool) -> Void

    @State private var mode: SphereMode = .marble
    @State private var panel: Int = 0

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
            // The world, full-bleed.
            RadialGradient(colors: [Color(red: 0x0a/255, green: 0x10/255, blue: 0x24/255),
                                    Color(red: 0x03/255, green: 0x05/255, blue: 0x0a/255)],
                           center: .top, startRadius: 40, endRadius: 900).ignoresSafeArea()
            FamiliarSphereView(mode: mode, pins: pins) { dir in
                withAnimation(.spring(response: 0.55, dampingFraction: 0.82)) {
                    panel = (panel + dir + panelCount) % panelCount
                }
            }.ignoresSafeArea()
            // vignette to seat the orb
            RadialGradient(colors: [.clear, Color(red: 0x03/255, green: 0x05/255, blue: 0x0a/255).opacity(0.55)],
                           center: .center, startRadius: 260, endRadius: 720)
                .ignoresSafeArea().allowsHitTesting(false)

            GeometryReader { geo in
                let compact = geo.size.width < 720

                // The floating holograms — a horizontal carousel that sweeps ACROSS, past the orb,
                // when the human two-finger swipes (the orb calls back through onSwipe). Each panel
                // is a free-floating card; only the active one rests in view, the others waiting
                // off to the sides, and the outgoing one drifts across the globe as it leaves.
                panelCarousel(width: geo.size.width, height: geo.size.height, compact: compact)

                // Floating chrome — wordmark + caption (top-left), version pill (top-right).
                VStack { HStack(alignment: .top) {
                    wordmark
                    Spacer()
                    versionPill
                }; Spacer() }
                .padding(compact ? 20 : 30)
                .allowsHitTesting(false)

                // Bottom: panel dots + mode switcher.
                VStack { Spacer(); VStack(spacing: 14) {
                    panelDots
                    modeSwitcher
                }.padding(.bottom, compact ? 26 : 30) }
                .frame(maxWidth: .infinity)
            }
        }
        .foregroundStyle(CSky.ink)
        .preferredColorScheme(.dark)
    }

    // MARK: chrome

    private var wordmark: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 11) {
                Circle().fill(RadialGradient(colors: [CSky.ice, CSky.bright, CSky.blue],
                                             center: .init(x: 0.34, y: 0.28), startRadius: 1, endRadius: 18))
                    .frame(width: 28, height: 28)
                    .shadow(color: CSky.blue.opacity(0.5), radius: 10)
                VStack(alignment: .leading, spacing: 1) {
                    Text("FAMILIAR").font(.system(size: 13, weight: .semibold)).tracking(2.4)
                    Text(worldview?.group_label ?? "metal · unified surface")
                        .font(CSky.mono(8.5)).tracking(1).foregroundStyle(CSky.dim.opacity(0.6))
                }
            }
            Text(mode.rawValue.uppercased() + " · STATE").font(CSky.mono(9.5)).tracking(2.2)
                .foregroundStyle(CSky.soft.opacity(0.62)).padding(.top, 14)
            Text(modeTitle).font(.system(size: 26, weight: .semibold)).tracking(-0.3)
                .shadow(color: .black.opacity(0.6), radius: 18).frame(maxWidth: 320, alignment: .leading)
            Text(mode.caption).font(.system(size: 12.5)).foregroundStyle(CSky.ink.opacity(0.6))
                .frame(maxWidth: 300, alignment: .leading).lineLimit(3)
        }
    }
    private var modeTitle: String {
        switch mode {
        case .marble: return "The blue marble, breathing"
        case .mesh: return "The mesh, exposed"
        case .globe: return "The world, and everyone on it"
        }
    }
    private var versionPill: some View {
        HStack(spacing: 8) {
            Circle().fill(CSky.green).frame(width: 5, height: 5).shadow(color: CSky.green, radius: 5)
            Text("\((worldview?.members ?? []).count) node\((worldview?.members ?? []).count == 1 ? "" : "s") · Metal")
                .font(CSky.mono(10)).foregroundStyle(CSky.ink.opacity(0.72))
        }
        .padding(.horizontal, 13).padding(.vertical, 8)
        .background(Capsule().fill(.ultraThinMaterial).overlay(Capsule().stroke(.white.opacity(0.08), lineWidth: 1)))
    }

    private var panelDots: some View {
        HStack(spacing: 7) {
            ForEach(0..<panelCount, id: \.self) { i in
                Button {
                    withAnimation(.spring(response: 0.5, dampingFraction: 0.85)) { panel = i }
                } label: {
                    Capsule().fill(i == panel ? CSky.cyan : CSky.dim.opacity(0.3))
                        .frame(width: i == panel ? 18 : 6, height: 6)
                }.buttonStyle(.plain)
            }
        }
    }

    private var modeSwitcher: some View {
        HStack(spacing: 5) {
            ForEach(SphereMode.allCases) { m in
                let on = mode == m
                Button { withAnimation(.easeInOut(duration: 0.3)) { mode = m } } label: {
                    VStack(spacing: 2) {
                        Text(m.rawValue).font(.system(size: 13.5, weight: .semibold))
                            .foregroundStyle(on ? Color(red: 0.04, green: 0.075, blue: 0.19) : CSky.ink.opacity(0.72))
                        Text(m.hint).font(CSky.mono(8)).tracking(1)
                            .foregroundStyle(on ? Color(red: 0.04, green: 0.075, blue: 0.19).opacity(0.6) : CSky.dim.opacity(0.5))
                    }
                    .padding(.horizontal, 24).padding(.vertical, 10)
                    .background(RoundedRectangle(cornerRadius: 12)
                        .fill(on ? AnyShapeStyle(LinearGradient(colors: [CSky.soft, CSky.blue], startPoint: .top, endPoint: .bottom))
                                 : AnyShapeStyle(Color.clear)))
                }.buttonStyle(.plain)
            }
        }
        .padding(6)
        .background(Capsule().fill(.ultraThinMaterial).overlay(Capsule().stroke(.white.opacity(0.09), lineWidth: 1)))
    }

    // MARK: holographic panels — a floating carousel that sweeps past the orb

    @ViewBuilder private func panelCarousel(width: CGFloat, height: CGFloat, compact: Bool) -> some View {
        let cardW: CGFloat = compact ? width - 44 : 344
        let restX: CGFloat = compact ? 0 : width * 0.20          // active card rests right-of-centre
        let step: CGFloat = compact ? width * 0.94 : width * 0.60 // neighbours wait off to the sides
        ZStack {
            ForEach(0..<panelCount, id: \.self) { i in
                let d = CGFloat(i - panel)
                holoCard(PanelKind(rawValue: i) ?? .presence)
                    .frame(width: cardW)
                    .scaleEffect(1 - min(0.16, abs(d) * 0.16))
                    .opacity(i == panel ? 1 : max(0, 0.4 - abs(d) * 0.28))
                    .blur(radius: i == panel ? 0 : 3.5)
                    .offset(x: restX + d * step, y: compact ? height * 0.20 : 0)
                    .allowsHitTesting(i == panel)
                    .zIndex(i == panel ? 1 : 0)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
        .animation(.spring(response: 0.55, dampingFraction: 0.82), value: panel)
    }

    private func holoCard(_ kind: PanelKind) -> some View {
        HoloCard {
            VStack(alignment: .leading, spacing: 0) {
                HStack {
                    Text(kind.title).font(CSky.mono(10)).tracking(2).foregroundStyle(CSky.soft.opacity(0.85))
                    Spacer()
                    HStack(spacing: 6) {
                        Circle().fill(CSky.green).frame(width: 5, height: 5).shadow(color: CSky.green, radius: 5)
                        Text("PROJECTING").font(CSky.mono(9)).foregroundStyle(CSky.green)
                    }
                }.padding(.bottom, 16)
                panelBody(kind)
            }
        }
    }

    private enum PanelKind: Int, CaseIterable { case presence, mesh, roadmap, gates, theories
        var title: String {
            switch self {
            case .presence: return "PRESENCE · LAW II"
            case .mesh: return "THE MESH"
            case .roadmap: return "THE ROADMAP"
            case .gates: return "GATES · LAW III"
            case .theories: return "THEORIES"
            }
        }
    }
    private var currentPanel: PanelKind { PanelKind(rawValue: panel) ?? .presence }

    @ViewBuilder private func panelBody(_ kind: PanelKind) -> some View {
        switch kind {
        case .presence:
            signalRow("Service", worldview?.service ?? 0, CSky.blue)
            signalRow("Presence", worldview?.presence ?? 0, CSky.green)
            signalRow("Capacities", worldview?.capacity ?? 0, CSky.amber)
            equalizer.padding(.top, 8)
        case .mesh:
            let members = worldview?.members ?? []
            metricPair("\(members.count)", "NODES", "\(members.filter { $0.online }.count)", "ONLINE")
            ForEach(members.prefix(6)) { m in
                rowLine(m.label, m.kind == .self_node ? "you" : (m.relationship ?? "peer"),
                        dot: m.online ? CSky.green : CSky.dim)
            }
        case .roadmap:
            let goals = worldview?.goals ?? []
            if goals.isEmpty { emptyLine("No goals yet — seed one with `familiar goal add`.") }
            ForEach(goals.prefix(6)) { g in
                rowLine(g.description, g.status.replacingOccurrences(of: "_", with: " "),
                        dot: goalColor(g.status))
            }
        case .gates:
            if let gates = worldview?.gates { gatesGrid(gates) }
            else { emptyLine("Gates unavailable.") }
        case .theories:
            let th = worldview?.theories ?? []
            if th.isEmpty { emptyLine("No theories yet.") }
            ForEach(th.prefix(4)) { t in
                VStack(alignment: .leading, spacing: 3) {
                    Text(t.question).font(.system(size: 12.5, weight: .medium)).lineLimit(2)
                    Text(t.direction).font(CSky.mono(9.5)).foregroundStyle(CSky.dim.opacity(0.7)).lineLimit(2)
                }.padding(.vertical, 5)
            }
        }
    }

    // MARK: panel atoms

    private func signalRow(_ label: String, _ v: Double, _ color: Color) -> some View {
        VStack(spacing: 6) {
            HStack { Text(label).font(.system(size: 13, weight: .medium)); Spacer()
                Text(String(format: "%.2f", v)).font(CSky.mono(12)).foregroundStyle(color) }
            GeometryReader { g in
                ZStack(alignment: .leading) {
                    Capsule().fill(.white.opacity(0.07))
                    Capsule().fill(color).frame(width: max(4, g.size.width * min(1, max(0, v))))
                        .shadow(color: color.opacity(0.7), radius: 8)
                }
            }.frame(height: 6)
        }.padding(.bottom, 13)
    }
    private func metricPair(_ a: String, _ al: String, _ b: String, _ bl: String) -> some View {
        HStack(spacing: 10) {
            metricBox(a, al); metricBox(b, bl)
        }.padding(.bottom, 12)
    }
    private func metricBox(_ v: String, _ l: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(v).font(.system(size: 22, weight: .semibold)).foregroundStyle(CSky.ice).monospacedDigit()
            Text(l).font(CSky.mono(8.5)).tracking(1).foregroundStyle(CSky.dim.opacity(0.55))
        }
        .frame(maxWidth: .infinity, alignment: .leading).padding(12)
        .background(RoundedRectangle(cornerRadius: 12).fill(CSky.bright.opacity(0.06))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(CSky.bright.opacity(0.2), lineWidth: 1)))
    }
    private func rowLine(_ a: String, _ b: String, dot: Color) -> some View {
        HStack(spacing: 8) {
            Circle().fill(dot).frame(width: 6, height: 6)
            Text(a).font(.system(size: 12.5)).foregroundStyle(CSky.ink.opacity(0.85)).lineLimit(1)
            Spacer(minLength: 6)
            Text(b).font(CSky.mono(9.5)).foregroundStyle(CSky.dim.opacity(0.7)).lineLimit(1)
        }
        .padding(.vertical, 7)
        .overlay(Rectangle().fill(.white.opacity(0.06)).frame(height: 1), alignment: .bottom)
    }
    private func emptyLine(_ s: String) -> some View {
        Text(s).font(.system(size: 12.5)).foregroundStyle(CSky.ink.opacity(0.5)).padding(.vertical, 10)
    }
    private func gatesGrid(_ g: GateStates) -> some View {
        let items: [(String, String, Bool)] = [
            ("Network", "allow_network", g.network), ("LLM", "allow_llm", g.llm),
            ("Execute", "allow_execute", g.execute), ("Agent", "allow_agent", g.agent),
            ("Mesh", "allow_mesh", g.mesh), ("Camera", "allow_camera", g.camera),
            ("Tools", "allow_tool_install", g.tool_install)
        ]
        return VStack(spacing: 8) {
            ForEach(items, id: \.1) { item in
                HStack {
                    Text(item.0).font(.system(size: 13, weight: .medium))
                    Spacer()
                    Button { onGate(item.1, !item.2) } label: {
                        Text(item.2 ? "OPEN" : "shut").font(CSky.mono(9.5)).tracking(1)
                            .foregroundStyle(item.2 ? Color(red: 0.04, green: 0.075, blue: 0.19) : CSky.dim)
                            .padding(.horizontal, 12).padding(.vertical, 5)
                            .background(Capsule().fill(item.2 ? AnyShapeStyle(CSky.green) : AnyShapeStyle(.white.opacity(0.06))))
                    }.buttonStyle(.plain)
                }
                .padding(.vertical, 5)
                .overlay(Rectangle().fill(.white.opacity(0.05)).frame(height: 1), alignment: .bottom)
            }
        }
    }
    private var equalizer: some View {
        HStack(alignment: .bottom, spacing: 3) {
            ForEach(0..<7, id: \.self) { i in
                Capsule().fill(LinearGradient(colors: [CSky.cyan, CSky.blue], startPoint: .top, endPoint: .bottom))
                    .frame(maxWidth: .infinity).frame(height: CGFloat([8, 16, 22, 14, 20, 10, 18][i]))
            }
        }.frame(height: 22)
    }
    private func goalColor(_ s: String) -> Color {
        switch s { case "done": return CSky.green; case "failed": return CSky.red
        case "awaiting_human": return CSky.amber; case "in_progress", "claimed": return CSky.bright
        default: return CSky.dim }
    }
}

/// The glassmorphic holographic card — blur, a hair-thin blue frame, and corner brackets.
private struct HoloCard<Content: View>: View {
    @ViewBuilder var content: () -> Content
    var body: some View {
        content()
            .padding(20)
            .background(
                RoundedRectangle(cornerRadius: 18).fill(.ultraThinMaterial)
                    .overlay(RoundedRectangle(cornerRadius: 18).stroke(CSky.cyan.opacity(0.5), lineWidth: 1))
                    .overlay(RoundedRectangle(cornerRadius: 18).stroke(CSky.cyan.opacity(0.12), lineWidth: 6).blur(radius: 6))
            )
            .overlay(bracket(.topLeading)).overlay(bracket(.topTrailing))
            .overlay(bracket(.bottomLeading)).overlay(bracket(.bottomTrailing))
            .shadow(color: CSky.blue.opacity(0.3), radius: 30)
    }
    private func bracket(_ corner: Alignment) -> some View {
        let h = corner == .topLeading || corner == .bottomLeading
        let v = corner == .topLeading || corner == .topTrailing
        return Path { p in
            p.move(to: CGPoint(x: h ? 0 : 13, y: v ? 0 : 13))
            p.addLine(to: CGPoint(x: h ? 13 : 0, y: v ? 0 : 13))
            p.move(to: CGPoint(x: h ? 0 : 13, y: v ? 0 : 13))
            p.addLine(to: CGPoint(x: h ? 0 : 13, y: v ? 13 : 0))
        }
        .stroke(CSky.cyan.opacity(0.85), lineWidth: 1.5)
        .frame(width: 13, height: 13).padding(9)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: corner)
    }
}

#endif
