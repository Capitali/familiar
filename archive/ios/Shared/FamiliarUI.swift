import SwiftUI
import FamiliarMesh

// The ONE shared design system — the futuristic dark-sphere theme, compiled into every GUI shell
// (iPhone/iPad app, macOS app, and future tvOS). The apps' screens bind to their own models but
// render from THESE atoms, so the theme is guaranteed consistent and can't drift. Pure SwiftUI:
// no UIKit/AppKit-only APIs, so it builds on every Apple platform. Based on the provided iPad design.

// MARK: - Palette

public enum Fam {
    public static let bg = Color(hex: 0x05070d)
    public static let ink = Color(hex: 0xeef2fb)
    public static let blue = Color(hex: 0x2f6bff)
    public static let blueBright = Color(hex: 0x6c9bff)
    public static let blueLink = Color(hex: 0x7aa2ff)
    public static let blueSoft = Color(hex: 0x9cc0ff)
    public static let iceStat = Color(hex: 0xcfe0ff)
    public static let green = Color(hex: 0x3ddc97)
    public static let greenSoft = Color(hex: 0x7ce0b4)
    public static let amber = Color(hex: 0xffb15a)
    public static let monoDim = Color(hex: 0x8ca5dc)
    public static let labelBlue = Color(hex: 0x96b4ff)
    public static func hairline(_ o: Double = 0.07) -> Color { Color.white.opacity(o) }
    public static func mono(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .monospaced)
    }
}

public extension Color {
    init(hex: UInt32) {
        self.init(.sRGB, red: Double((hex >> 16) & 0xff) / 255, green: Double((hex >> 8) & 0xff) / 255,
                  blue: Double(hex & 0xff) / 255, opacity: 1)
    }
}

// MARK: - Responsive layout signal (measured width, not size class)

// Layout is driven by the *measured* width — a Pro Max reports `.regular` in landscape (and betas
// misreport), which would wrongly pick a wide layout and overflow. Any container reads this env.
public struct CompactLayoutKey: EnvironmentKey { public static let defaultValue = false }
public extension EnvironmentValues {
    var isCompactLayout: Bool {
        get { self[CompactLayoutKey.self] }
        set { self[CompactLayoutKey.self] = newValue }
    }
}
public let kCompactWidth: CGFloat = 740

/// Two panels side-by-side on a wide screen, stacked on a narrow one — the responsive primitive for
/// rotation and every screen size.
public struct AdaptiveColumns<Main: View, Side: View>: View {
    @Environment(\.isCompactLayout) private var compact
    var sideWidth: CGFloat
    @ViewBuilder var main: () -> Main
    @ViewBuilder var side: () -> Side
    public init(sideWidth: CGFloat = 352, @ViewBuilder main: @escaping () -> Main, @ViewBuilder side: @escaping () -> Side) {
        self.sideWidth = sideWidth; self.main = main; self.side = side
    }
    public var body: some View {
        if compact { VStack(spacing: 22) { main(); side() } }
        else { HStack(alignment: .top, spacing: 22) { main(); side().frame(width: sideWidth) } }
    }
}

// MARK: - The breathing marble

public struct Marble: View {
    var size: CGFloat
    @State private var breathe = false
    public init(size: CGFloat) { self.size = size }
    public var body: some View {
        ZStack {
            Circle().fill(RadialGradient(colors: [Fam.blue.opacity(0.55), .clear], center: .center, startRadius: 0, endRadius: size * 0.9))
                .frame(width: size * 1.7, height: size * 1.7).blur(radius: 5).opacity(breathe ? 0.82 : 0.45)
            Circle().fill(RadialGradient(stops: [
                .init(color: Color(hex: 0xe2edff), location: 0.0), .init(color: Color(hex: 0x7ba3ff), location: 0.24),
                .init(color: Color(hex: 0x3568e8), location: 0.5), .init(color: Color(hex: 0x123a9e), location: 0.74),
                .init(color: Color(hex: 0x05132f), location: 1.0)], center: UnitPoint(x: 0.34, y: 0.28), startRadius: 0, endRadius: size * 0.62))
                .overlay(Circle().stroke(Fam.blueSoft.opacity(0.25), lineWidth: 1))
                .shadow(color: Color(hex: 0x020a28).opacity(0.82), radius: 12, x: -6, y: -7)
            Circle().fill(RadialGradient(colors: [Color.white.opacity(0.9), .clear], center: .center, startRadius: 0, endRadius: size * 0.2))
                .frame(width: size * 0.34, height: size * 0.28).blur(radius: 2).offset(x: -size * 0.16, y: -size * 0.2)
        }
        .frame(width: size, height: size).scaleEffect(breathe ? 1.045 : 1.0)
        .onAppear { withAnimation(.easeInOut(duration: 3).repeatForever(autoreverses: true)) { breathe = true } }
    }
}

public struct AuroraBackground: View {
    @State private var drift = false
    public init() {}
    public var body: some View {
        ZStack {
            Circle().fill(RadialGradient(colors: [Fam.blue.opacity(0.20), .clear], center: .center, startRadius: 0, endRadius: 360))
                .frame(width: 720, height: 720).blur(radius: 20).offset(x: -260 + (drift ? 20 : 0), y: -280 + (drift ? -16 : 0))
            Circle().fill(RadialGradient(colors: [Color(hex: 0x1e3caa).opacity(0.22), .clear], center: .center, startRadius: 0, endRadius: 380))
                .frame(width: 760, height: 760).blur(radius: 22).offset(x: 320 + (drift ? -18 : 0), y: 360 + (drift ? 14 : 0))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading).ignoresSafeArea()
        .onAppear { withAnimation(.easeInOut(duration: 24).repeatForever(autoreverses: true)) { drift = true } }
    }
}

// MARK: - Panels, labels, meters, headers

public struct Panel<Content: View>: View {
    var radius: CGFloat
    var fill: Double
    @ViewBuilder var content: () -> Content
    public init(radius: CGFloat = 24, fill: Double = 0.03, @ViewBuilder content: @escaping () -> Content) {
        self.radius = radius; self.fill = fill; self.content = content
    }
    public var body: some View {
        content().padding(24).frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: radius).fill(Color.white.opacity(fill))
                .overlay(RoundedRectangle(cornerRadius: radius).stroke(Fam.hairline(0.07), lineWidth: 1)))
    }
}

public struct MonoLabel: View {
    let text: String
    public init(_ t: String) { text = t }
    public init(text: String) { self.text = text }   // both call styles, so shells don't have to change
    public var body: some View { Text(text).font(Fam.mono(10.5)).tracking(1.9).foregroundStyle(Fam.labelBlue.opacity(0.6)) }
}

public struct SignalBar: View {
    let label: String; let value: Double; let color: Color; let note: String
    public init(_ label: String, _ value: Double, _ color: Color, note: String = "") {
        self.label = label; self.value = value; self.color = color; self.note = note
    }
    public init(label: String, value: Double, color: Color, note: String = "") {
        self.label = label; self.value = value; self.color = color; self.note = note
    }
    public var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack { Text(label).font(.system(size: 14, weight: .medium)); Spacer()
                Text(String(format: "%.2f", value)).font(Fam.mono(13)).foregroundStyle(color) }
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule().fill(Color.white.opacity(0.07)).frame(height: 6)
                    Capsule().fill(color).frame(width: max(6, geo.size.width * CGFloat(min(max(value, 0), 1))), height: 6).shadow(color: color.opacity(0.7), radius: 6)
                }
            }.frame(height: 6)
            if !note.isEmpty { Text(note).font(Fam.mono(10)).foregroundStyle(Fam.monoDim.opacity(0.5)) }
        }
    }
}

public struct ScreenHeader: View {
    let number: String, title: String, subtitle: String?
    public init(number: String, title: String, subtitle: String?) { self.number = number; self.title = title; self.subtitle = subtitle }
    public var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            Text(number).font(Fam.mono(11)).tracking(2.4).foregroundStyle(Fam.labelBlue.opacity(0.65))
            Text(title).font(.system(size: 30, weight: .semibold))
            if let s = subtitle { Text(s).font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.5)).fixedSize(horizontal: false, vertical: true) }
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
}

// MARK: - The cycle ring (8 phases orbiting the marble)

public struct CycleRing: View {
    let stages: [String]
    let active: Int
    public init(stages: [String], active: Int) { self.stages = stages; self.active = active }
    public var body: some View {
        GeometryReader { geo in
            let c = CGPoint(x: geo.size.width / 2, y: geo.size.height / 2)
            let r = min(geo.size.width, geo.size.height) / 2 - 46
            ZStack {
                Circle().strokeBorder(style: StrokeStyle(lineWidth: 1, dash: [4, 6]))
                    .foregroundStyle(Fam.blueBright.opacity(0.18)).frame(width: r * 2, height: r * 2).position(c)
                VStack(spacing: 8) {
                    Marble(size: 104)
                    Text(stages[active].uppercased()).font(Fam.mono(11)).tracking(1.4).foregroundStyle(Fam.blueSoft)
                }.position(c)
                ForEach(Array(stages.enumerated()), id: \.offset) { i, s in
                    let a = (Double(i) / Double(stages.count)) * 2 * .pi - .pi / 2
                    let p = CGPoint(x: c.x + r * CGFloat(cos(a)), y: c.y + r * CGFloat(sin(a)))
                    let on = i == active
                    VStack(spacing: 2) {
                        Text(String(format: "%02d", i + 1)).font(Fam.mono(9)).foregroundStyle(on ? Fam.blueSoft : Fam.monoDim.opacity(0.5))
                        Text(s).font(.system(size: 12, weight: .semibold)).foregroundStyle(on ? Fam.ink : Fam.ink.opacity(0.55))
                    }
                    .frame(width: 84, height: 54)
                    .background(RoundedRectangle(cornerRadius: 14).fill(on ? Fam.blue.opacity(0.18) : Color.white.opacity(0.03))
                        .overlay(RoundedRectangle(cornerRadius: 14).stroke(on ? Fam.blueBright.opacity(0.5) : Fam.hairline(0.07), lineWidth: 1)))
                    .shadow(color: on ? Fam.blue.opacity(0.4) : .clear, radius: 8).position(p)
                }
            }
        }
    }
}

// MARK: - Mesh constellation (self-contained: colors/icons by kind)

public enum MeshStyle {
    public static func color(_ k: Member.Kind) -> Color {
        switch k { case .self_node: return Fam.iceStat; case .gossip_peer: return Fam.blueBright
        case .device_peer: return Fam.green; case .device_agent: return Fam.amber }
    }
    public static func icon(_ m: Member) -> String {
        switch m.kind {
        case .self_node: return "house.fill"; case .gossip_peer: return "cpu"
        case .device_peer where m.actor.hasPrefix("ipad"): return "ipad"
        case .device_peer where m.actor.hasPrefix("watch"): return "applewatch"
        case .device_peer: return "iphone"
        case .device_agent where m.actor.hasPrefix("watch"): return "applewatch"
        case .device_agent: return "iphone" }
    }
    public static func kindLabel(_ k: Member.Kind) -> String {
        switch k { case .self_node: return "this node"; case .gossip_peer: return "mesh peer"
        case .device_peer: return "device peer"; case .device_agent: return "device agent" }
    }
    public static func rank(_ k: Member.Kind) -> Int {
        switch k { case .self_node: return 0; case .gossip_peer: return 1; case .device_peer: return 2; case .device_agent: return 3 }
    }
}

public struct MeshConstellation: View {
    let members: [Member]
    public init(members: [Member]) { self.members = members }
    public var body: some View {
        GeometryReader { geo in
            let center = CGPoint(x: geo.size.width / 2, y: geo.size.height / 2)
            let radius = min(geo.size.width, geo.size.height) / 2 - 52
            let selfNode = members.first { $0.kind == .self_node }
            let others = members.filter { $0.kind != .self_node }
            ZStack {
                ForEach(Array(others.enumerated()), id: \.element.id) { i, m in
                    let p = point(center, radius, i, others.count)
                    Path { path in path.move(to: center); path.addLine(to: p) }
                        .stroke(MeshStyle.color(m.kind).opacity(m.online ? 0.35 : 0.12), lineWidth: 1)
                }
                node(selfNode ?? members.first, center, true)
                ForEach(Array(others.enumerated()), id: \.element.id) { i, m in node(m, point(center, radius, i, others.count), false) }
            }
        }
    }
    private func point(_ c: CGPoint, _ r: CGFloat, _ i: Int, _ n: Int) -> CGPoint {
        guard n > 0 else { return c }
        let a = (Double(i) / Double(n)) * 2 * .pi - .pi / 2
        return CGPoint(x: c.x + r * CGFloat(cos(a)), y: c.y + r * CGFloat(sin(a)))
    }
    @ViewBuilder private func node(_ m: Member?, _ p: CGPoint, _ big: Bool) -> some View {
        if let m = m {
            let c = MeshStyle.color(m.kind)
            VStack(spacing: 4) {
                ZStack {
                    Circle().fill(c.opacity(m.online ? 0.22 : 0.08)).frame(width: big ? 58 : 42, height: big ? 58 : 42)
                    Circle().stroke(c.opacity(m.online ? 0.9 : 0.4), lineWidth: 1.5).frame(width: big ? 58 : 42, height: big ? 58 : 42)
                    Image(systemName: MeshStyle.icon(m)).font(.system(size: big ? 20 : 15)).foregroundStyle(c)
                }.shadow(color: m.online ? c.opacity(0.5) : .clear, radius: 8)
                Text(m.label.isEmpty ? String(m.node_id.prefix(6)) : m.label).font(Fam.mono(9.5)).foregroundStyle(Fam.ink.opacity(0.8)).lineLimit(1).frame(maxWidth: 90)
            }.position(x: p.x, y: p.y)
        }
    }
}

// MARK: - Time helpers

public enum GlassTime {
    public static func clock(_ ts: Int64) -> String {
        let f = DateFormatter(); f.dateFormat = "HH:mm"; return f.string(from: Date(timeIntervalSince1970: TimeInterval(ts)))
    }
    public static func ago(_ ts: Int64) -> String {
        let s = Int64(Date().timeIntervalSince1970) - ts
        if s < 5 { return "just now" }; if s < 60 { return "\(s)s" }; if s < 3600 { return "\(s / 60)m" }
        if s < 86400 { return "\(s / 3600)h" }; return "\(s / 86400)d"
    }
}
