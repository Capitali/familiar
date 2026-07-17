import SwiftUI
import FamiliarMesh
import WatchConnectivity

// The iPad "Familiar" console — a faithful build of the futuristic design (Familiar for iPad.dc.html):
// a deep-navy instrument with a breathing marble, a left rail, and four screens (The Glass,
// Metabolism, Theories, Gates & Boundary). Wired to the live worldview this peer reads over
// /mesh/worldview — the three constitutional meters, the recent-observation ledger, presence — plus
// this device's own consent gates. Space Grotesk / IBM Plex Mono are approximated with the system
// sans + monospaced faces (bundling the exact fonts is a later polish).

// MARK: - Palette (from the design's hex values)

enum Fam {
    static let bg = Color(hex: 0x05070d)
    static let ink = Color(hex: 0xeef2fb)
    static let blue = Color(hex: 0x2f6bff)
    static let blueBright = Color(hex: 0x6c9bff)
    static let blueLink = Color(hex: 0x7aa2ff)
    static let blueSoft = Color(hex: 0x9cc0ff)
    static let iceStat = Color(hex: 0xcfe0ff)
    static let green = Color(hex: 0x3ddc97)
    static let greenSoft = Color(hex: 0x7ce0b4)
    static let amber = Color(hex: 0xffb15a)
    static let monoDim = Color(hex: 0x8ca5dc)        // rgba(140,165,220,*)
    static let labelBlue = Color(hex: 0x96b4ff)      // rgba(150,180,255,*)

    static func surface(_ o: Double = 0.04) -> Color { Color.white.opacity(o) }
    static func hairline(_ o: Double = 0.07) -> Color { Color.white.opacity(o) }
    static func inkDim(_ o: Double) -> Color { ink.opacity(o) }

    static let sans = Font.Design.default
    static func mono(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .monospaced)
    }
}

extension Color {
    init(hex: UInt32) {
        self.init(
            .sRGB,
            red: Double((hex >> 16) & 0xff) / 255,
            green: Double((hex >> 8) & 0xff) / 255,
            blue: Double(hex & 0xff) / 255,
            opacity: 1
        )
    }
}

// Layout is driven by the *measured* width, not horizontalSizeClass — a Pro Max reports `.regular`
// in landscape (and betas can misreport), which would wrongly pick the wide rail layout and overflow
// the phone. Below this width we use the compact (single-column) layout.
private struct CompactLayoutKey: EnvironmentKey { static let defaultValue = false }
extension EnvironmentValues {
    var isCompactLayout: Bool {
        get { self[CompactLayoutKey.self] }
        set { self[CompactLayoutKey.self] = newValue }
    }
}
let kCompactWidth: CGFloat = 740

// MARK: - The console shell

/// The dark futuristic console — the standard UI for **every** peer with a screen (iPhone, iPad, and
/// the macOS SwiftUI shell). Adaptive: a left rail on a wide screen (iPad/Mac), a compact top bar with
/// a pill selector on a narrow one (iPhone). Same design language everywhere.
struct GlassConsole: View {
    @EnvironmentObject var model: AppModel
    @State private var screen: Screen = .glass

    enum Screen: String, CaseIterable, Identifiable {
        case glass = "The Glass"
        case metabolism = "Metabolism"
        case theories = "Theories"
        case mesh = "The Mesh"
        case gates = "Gates & Boundary"
        var id: String { rawValue }
        var short: String {
            switch self {
            case .glass: return "Glass"
            case .metabolism: return "Metabolism"
            case .theories: return "Theories"
            case .mesh: return "Mesh"
            case .gates: return "Gates"
            }
        }
        var number: String {
            switch self {
            case .glass: return "01"
            case .metabolism: return "02"
            case .theories: return "03"
            case .mesh: return "04"
            case .gates: return "05"
            }
        }
    }

    var body: some View {
        GeometryReader { geo in
            let compact = geo.size.width < kCompactWidth
            ZStack {
                Fam.bg.ignoresSafeArea()
                AuroraBackground()
                if compact {
                    VStack(spacing: 0) {
                        CompactBar(screen: $screen)
                        Divider().overlay(Fam.hairline(0.055))
                        ScreenArea(screen: screen)
                    }
                } else {
                    HStack(spacing: 0) {
                        LeftRail(screen: $screen)
                            .frame(width: 250)
                        VStack(spacing: 0) {
                            TopBar()
                            Divider().overlay(Fam.hairline(0.055))
                            ScreenArea(screen: screen)
                        }
                    }
                }
            }
            .frame(width: geo.size.width, height: geo.size.height)
            .environment(\.isCompactLayout, compact)
        }
        .ignoresSafeArea(.keyboard)
        .foregroundStyle(Fam.ink)
        .preferredColorScheme(.dark)
        .onAppear { model.startWorldviewPolling(); model.startDiscoveryIfConsented(); model.startReasoningIfConsented() }
        .onDisappear { model.stopWorldviewPolling() }
    }
}

/// The compact (iPhone) navigation chrome: the marble + presence line, then a horizontal pill
/// selector for the screens — the same dark language as the iPad rail, sized for a narrow screen.
private struct CompactBar: View {
    @EnvironmentObject var model: AppModel
    @Binding var screen: GlassConsole.Screen
    var present: Bool { !(model.worldview?.withdrawn ?? true) }
    var body: some View {
        VStack(spacing: 12) {
            HStack(spacing: 11) {
                Marble(size: 30)
                VStack(alignment: .leading, spacing: 1) {
                    Text("FAMILIAR").font(.system(size: 14, weight: .semibold)).tracking(2)
                    Text(model.worldview?.group_label ?? model.groupLabel)
                        .font(Fam.mono(9.5)).foregroundStyle(Fam.monoDim.opacity(0.6))
                }
                Spacer()
                Circle().fill(present ? Fam.green : Fam.amber).frame(width: 8, height: 8)
                    .shadow(color: present ? Fam.green : Fam.amber, radius: 4)
            }
            .padding(.horizontal, 18)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(GlassConsole.Screen.allCases) { s in
                        let on = screen == s
                        Button { screen = s } label: {
                            Text(s.short).font(.system(size: 13, weight: .medium))
                                .foregroundStyle(on ? Color(hex: 0xeaf1ff) : Fam.ink.opacity(0.55))
                                .padding(.horizontal, 14).padding(.vertical, 8)
                                .background(Capsule().fill(on ? Fam.blue.opacity(0.16) : Color.white.opacity(0.04))
                                    .overlay(Capsule().stroke(on ? Fam.blueBright.opacity(0.32) : Color.white.opacity(0.06), lineWidth: 1)))
                        }.buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 18)
            }
        }
        .padding(.top, 10).padding(.bottom, 12)
    }
}

/// Two panels side-by-side on a wide screen, stacked on a narrow one — keeps the console's
/// multi-column screens usable on an iPhone without a separate layout.
struct AdaptiveColumns<Main: View, Side: View>: View {
    @Environment(\.isCompactLayout) private var compact
    var sideWidth: CGFloat = 352
    @ViewBuilder var main: () -> Main
    @ViewBuilder var side: () -> Side
    var body: some View {
        if compact {
            VStack(spacing: 22) { main(); side() }
        } else {
            HStack(alignment: .top, spacing: 22) { main(); side().frame(width: sideWidth) }
        }
    }
}

// MARK: - Aurora + marble

private struct AuroraBackground: View {
    @State private var drift = false
    var body: some View {
        ZStack {
            Circle()
                .fill(RadialGradient(colors: [Fam.blue.opacity(0.20), .clear],
                                     center: .center, startRadius: 0, endRadius: 360))
                .frame(width: 720, height: 720).blur(radius: 20)
                .offset(x: -260 + (drift ? 20 : 0), y: -280 + (drift ? -16 : 0))
            Circle()
                .fill(RadialGradient(colors: [Color(hex: 0x1e3caa).opacity(0.22), .clear],
                                     center: .center, startRadius: 0, endRadius: 380))
                .frame(width: 760, height: 760).blur(radius: 22)
                .offset(x: 320 + (drift ? -18 : 0), y: 360 + (drift ? 14 : 0))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .ignoresSafeArea()
        .onAppear {
            withAnimation(.easeInOut(duration: 24).repeatForever(autoreverses: true)) { drift = true }
        }
    }
}

/// The breathing marble — the familiar's face. A layered radial gradient with a specular highlight
/// and a slow "breathe" scale, plus a soft glow halo.
struct Marble: View {
    var size: CGFloat
    @State private var breathe = false
    var body: some View {
        ZStack {
            Circle()
                .fill(RadialGradient(colors: [Fam.blue.opacity(0.55), .clear],
                                     center: .center, startRadius: 0, endRadius: size * 0.9))
                .frame(width: size * 1.7, height: size * 1.7)
                .blur(radius: 5)
                .opacity(breathe ? 0.82 : 0.45)
            Circle()
                .fill(RadialGradient(
                    stops: [
                        .init(color: Color(hex: 0xe2edff), location: 0.0),
                        .init(color: Color(hex: 0x7ba3ff), location: 0.24),
                        .init(color: Color(hex: 0x3568e8), location: 0.50),
                        .init(color: Color(hex: 0x123a9e), location: 0.74),
                        .init(color: Color(hex: 0x05132f), location: 1.0),
                    ],
                    center: UnitPoint(x: 0.34, y: 0.28), startRadius: 0, endRadius: size * 0.62))
                .overlay(Circle().stroke(Fam.blueSoft.opacity(0.25), lineWidth: 1))
                .shadow(color: Color(hex: 0x020a28).opacity(0.82), radius: 12, x: -6, y: -7)
            Circle()
                .fill(RadialGradient(colors: [Color.white.opacity(0.9), .clear],
                                     center: .center, startRadius: 0, endRadius: size * 0.2))
                .frame(width: size * 0.34, height: size * 0.28)
                .blur(radius: 2)
                .offset(x: -size * 0.16, y: -size * 0.2)
        }
        .frame(width: size, height: size)
        .scaleEffect(breathe ? 1.045 : 1.0)
        .onAppear {
            withAnimation(.easeInOut(duration: 3).repeatForever(autoreverses: true)) { breathe = true }
        }
    }
}

/// The cycle as the design intends it — the 8 phases arranged in a ring around the breathing marble,
/// the active phase lit, a dashed orbit behind them.
struct CycleRing: View {
    let stages: [String]
    let active: Int
    var body: some View {
        GeometryReader { geo in
            let c = CGPoint(x: geo.size.width / 2, y: geo.size.height / 2)
            let r = min(geo.size.width, geo.size.height) / 2 - 46
            ZStack {
                Circle()
                    .strokeBorder(style: StrokeStyle(lineWidth: 1, dash: [4, 6]))
                    .foregroundStyle(Fam.blueBright.opacity(0.18))
                    .frame(width: r * 2, height: r * 2).position(c)
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
                    .shadow(color: on ? Fam.blue.opacity(0.4) : .clear, radius: 8)
                    .position(p)
                }
            }
        }
    }
}

// MARK: - Left rail

private struct LeftRail: View {
    @EnvironmentObject var model: AppModel
    @Binding var screen: GlassConsole.Screen
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 13) {
                Marble(size: 34)
                VStack(alignment: .leading, spacing: 2) {
                    Text("FAMILIAR").font(.system(size: 15, weight: .semibold)).tracking(2.3)
                    Text("io.river.familiar").font(Fam.mono(9.5)).foregroundStyle(Fam.monoDim.opacity(0.55))
                }
            }
            .padding(.horizontal, 6).padding(.bottom, 26)

            Text("INTERFACE").font(Fam.mono(9.5)).tracking(2).foregroundStyle(Fam.labelBlue.opacity(0.5))
                .padding(.horizontal, 8).padding(.bottom, 12)

            VStack(spacing: 5) {
                ForEach(GlassConsole.Screen.allCases) { s in
                    NavItem(screen: s, active: screen == s) { screen = s }
                }
            }
            Spacer()
            MetabolismCard()
            Link(destination: URL(string: "https://github.com/Capitali/familiar")!) {
                HStack(spacing: 7) {
                    Image(systemName: "chevron.left.forwardslash.chevron.right").font(.system(size: 10))
                    Text("github.com/Capitali/familiar").font(Fam.mono(9.5))
                }
                .foregroundStyle(Fam.blueLink.opacity(0.75))
            }
            .padding(.top, 14).padding(.horizontal, 6)
        }
        .padding(EdgeInsets(top: 26, leading: 20, bottom: 22, trailing: 20))
        .background(
            LinearGradient(colors: [Color.white.opacity(0.035), Color.white.opacity(0.01)],
                           startPoint: .top, endPoint: .bottom)
        )
        .overlay(Rectangle().frame(width: 1).foregroundStyle(Fam.hairline(0.06)), alignment: .trailing)
    }
}

private struct NavItem: View {
    let screen: GlassConsole.Screen
    let active: Bool
    let tap: () -> Void
    var body: some View {
        Button(action: tap) {
            HStack(spacing: 13) {
                Circle()
                    .fill(active ? Fam.blueBright : Color.white.opacity(0.2))
                    .frame(width: 6, height: 6)
                    .shadow(color: active ? Fam.blueBright.opacity(0.9) : .clear, radius: 5)
                Text(screen.number).font(Fam.mono(10)).foregroundStyle(Fam.ink.opacity(0.6))
                Text(screen.rawValue).font(.system(size: 14.5, weight: .medium))
                Spacer(minLength: 0)
            }
            .padding(.vertical, 12).padding(.horizontal, 13)
            .background(
                RoundedRectangle(cornerRadius: 14)
                    .fill(active ? Fam.blue.opacity(0.14) : .clear)
                    .overlay(RoundedRectangle(cornerRadius: 14)
                        .stroke(active ? Fam.blueBright.opacity(0.32) : Color.white.opacity(0.02), lineWidth: 1))
            )
            .foregroundStyle(active ? Color(hex: 0xeaf1ff) : Fam.ink.opacity(0.5))
        }
        .buttonStyle(.plain)
    }
}

private struct MetabolismCard: View {
    @EnvironmentObject var model: AppModel
    @State private var pulse = false
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("METABOLISM").font(Fam.mono(9.5)).tracking(1.8).foregroundStyle(Fam.labelBlue.opacity(0.7))
                Spacer()
                HStack(spacing: 5) {
                    Circle().fill(Fam.green).frame(width: 5, height: 5).shadow(color: Fam.green, radius: 4)
                    Text("RUNNING").font(Fam.mono(10)).foregroundStyle(Fam.greenSoft)
                }
            }
            HStack(alignment: .bottom, spacing: 3) {
                ForEach(0..<6, id: \.self) { i in
                    RoundedRectangle(cornerRadius: 2)
                        .fill(LinearGradient(colors: [Fam.blueBright, Fam.blue], startPoint: .top, endPoint: .bottom))
                        .frame(height: 26)
                        .scaleEffect(y: pulse ? 1.0 : 0.4, anchor: .bottom)
                        .animation(.easeInOut(duration: 1.3).repeatForever().delay(Double(i) * 0.12), value: pulse)
                }
            }
            .frame(height: 26)
            HStack {
                Text("observations").font(Fam.mono(10.5)).foregroundStyle(Fam.ink.opacity(0.6))
                Spacer()
                Text("\(model.worldview?.observation_count ?? 0)").font(Fam.mono(10.5)).foregroundStyle(Fam.iceStat)
            }
        }
        .padding(EdgeInsets(top: 16, leading: 15, bottom: 16, trailing: 15))
        .background(RoundedRectangle(cornerRadius: 16).fill(Fam.blue.opacity(0.07))
            .overlay(RoundedRectangle(cornerRadius: 16).stroke(Fam.blueBright.opacity(0.18), lineWidth: 1)))
        .onAppear { pulse = true }
    }
}

// MARK: - Top bar

private struct TopBar: View {
    @EnvironmentObject var model: AppModel
    @State private var ping = false
    var present: Bool { !(model.worldview?.withdrawn ?? true) }
    var body: some View {
        HStack {
            HStack(spacing: 11) {
                ZStack {
                    Circle().fill(present ? Fam.green : Fam.amber).frame(width: 8, height: 8)
                        .shadow(color: present ? Fam.green : Fam.amber, radius: 4)
                    Circle().stroke(present ? Fam.green : Fam.amber, lineWidth: 1).frame(width: 8, height: 8)
                        .scaleEffect(ping ? 2.6 : 0.6).opacity(ping ? 0 : 0.85)
                        .animation(.easeOut(duration: 2.6).repeatForever(autoreverses: false), value: ping)
                }
                Text(present ? "The familiar is present" : "The familiar is withdrawn")
                    .font(.system(size: 13.5)).foregroundStyle(Fam.ink.opacity(0.72))
                Text("· \(model.worldview?.group_label ?? model.groupLabel)")
                    .font(Fam.mono(11)).foregroundStyle(Fam.monoDim.opacity(0.5))
            }
            Spacer()
            HStack(spacing: 16) {
                Text(model.worldviewError == nil ? "reading /mesh/worldview" : "familiar unreachable")
                    .font(Fam.mono(11.5)).foregroundStyle(model.worldviewError == nil ? Fam.greenSoft : Fam.amber)
            }
        }
        .padding(.horizontal, 34)
        .frame(height: 66)
        .onAppear { ping = true }
    }
}

// MARK: - Screen router

private struct ScreenArea: View {
    let screen: GlassConsole.Screen
    var body: some View {
        ScrollView {
            Group {
                switch screen {
                case .glass: GlassHomeScreen()
                case .metabolism: MetabolismScreen()
                case .theories: TheoriesScreen()
                case .mesh: MeshScreen()
                case .gates: GatesScreen()
                }
            }
            .padding(EdgeInsets(top: 32, leading: 34, bottom: 34, trailing: 34))
        }
    }
}

private struct ScreenHeader: View {
    let number: String, title: String, subtitle: String?
    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            Text(number).font(Fam.mono(11)).tracking(2.4).foregroundStyle(Fam.labelBlue.opacity(0.65))
            Text(title).font(.system(size: 30, weight: .semibold))
            if let s = subtitle {
                Text(s).font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.5)).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

struct Panel<Content: View>: View {
    var radius: CGFloat = 24
    var fill: Double = 0.03
    @ViewBuilder var content: () -> Content
    var body: some View {
        content()
            .padding(24)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: radius).fill(Color.white.opacity(fill))
                .overlay(RoundedRectangle(cornerRadius: radius).stroke(Fam.hairline(0.07), lineWidth: 1)))
    }
}

struct MonoLabel: View {
    let text: String
    var body: some View {
        Text(text).font(Fam.mono(10.5)).tracking(1.9).foregroundStyle(Fam.labelBlue.opacity(0.6))
    }
}

// MARK: - 01 · The Glass (home)

private struct GlassHomeScreen: View {
    @EnvironmentObject var model: AppModel
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            ScreenHeader(number: "01 · THE GLASS", title: "Home", subtitle: nil)
            AdaptiveColumns {
                VStack(spacing: 22) {
                    GreetingCard()
                    LedgerCard()
                    HumanityCard()
                }
            } side: {
                VStack(spacing: 22) {
                    PresenceCard()
                    LawSignalsCard()
                }
            }
        }
    }
}

private struct GreetingCard: View {
    @EnvironmentObject var model: AppModel
    private var greeting: String {
        let h = Calendar.current.component(.hour, from: Date())
        switch h { case 5..<12: return "Good morning."; case 12..<18: return "Good afternoon."
        case 18..<22: return "Good evening."; default: return "Still here." }
    }
    var body: some View {
        Panel(fill: 0.04) {
            VStack(alignment: .leading, spacing: 0) {
                MonoLabel(text: "PRESENT · THE FAMILIAR IS AWAKE")
                Text(greeting).font(.system(size: 38, weight: .semibold)).padding(.top, 14)
                Text("This iPad is a peer — it reads the familiar's world and adds its own senses to it.")
                    .font(.system(size: 15)).foregroundStyle(Fam.ink.opacity(0.55)).padding(.top, 12)
                    .fixedSize(horizontal: false, vertical: true)
                Divider().overlay(Fam.hairline(0.08)).padding(.vertical, 22)
                MonoLabel(text: "THE FAMILIAR ASKS")
                Text("What do you need most today?")
                    .font(.system(size: 24, weight: .medium)).padding(.top, 12).padding(.bottom, 20)
                HStack(spacing: 12) {
                    TextField("Answer, or leave it — silence is an answer too", text: $model.consoleAnswer)
                        .textFieldStyle(.plain)
                        .padding(.horizontal, 18).padding(.vertical, 15)
                        .background(RoundedRectangle(cornerRadius: 14).fill(Color.black.opacity(0.25))
                            .overlay(RoundedRectangle(cornerRadius: 14).stroke(Color.white.opacity(0.1), lineWidth: 1)))
                    Button(action: { model.submitConsoleAnswer() }) {
                        Text("Answer").font(.system(size: 14, weight: .semibold)).foregroundStyle(Color(hex: 0x0a1330))
                            .padding(.horizontal, 24).padding(.vertical, 15)
                            .background(RoundedRectangle(cornerRadius: 14)
                                .fill(LinearGradient(colors: [Color(hex: 0x8fb4ff), Color(hex: 0x3f7bff)], startPoint: .top, endPoint: .bottom)))
                    }.buttonStyle(.plain)
                }
            }
        }
    }
}

private struct LedgerCard: View {
    @EnvironmentObject var model: AppModel
    var body: some View {
        Panel(fill: 0.03) {
            VStack(alignment: .leading, spacing: 16) {
                MonoLabel(text: "LEDGER · WHAT IT DID WHILE YOU WORKED")
                let recent = model.worldview?.recent ?? []
                if recent.isEmpty {
                    Text("Nothing yet — the ledger fills as the familiar senses and acts.")
                        .font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.5))
                } else {
                    VStack(spacing: 0) {
                        ForEach(recent.prefix(8)) { o in
                            HStack(alignment: .top, spacing: 16) {
                                Text(GlassTime.clock(o.ts)).font(Fam.mono(11.5)).foregroundStyle(Fam.monoDim.opacity(0.55))
                                    .frame(width: 62, alignment: .leading)
                                (Text(o.actor).foregroundStyle(Fam.blueSoft) + Text(" \(o.action) ") + Text(o.object).foregroundStyle(Fam.ink.opacity(0.82)))
                                    .font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.82))
                                    .fixedSize(horizontal: false, vertical: true)
                                Spacer(minLength: 0)
                            }
                            .padding(.vertical, 11)
                            Divider().overlay(Fam.hairline(0.045))
                        }
                    }
                }
            }
        }
    }
}

/// The familiar's growing, observation-grounded understanding of the person — appended beside the
/// constitutional HUMANITY.md, never over it.
private struct HumanityCard: View {
    @EnvironmentObject var model: AppModel
    var body: some View {
        Panel(fill: 0.03) {
            VStack(alignment: .leading, spacing: 14) {
                MonoLabel(text: "UNDERSTANDING · WHAT IT'S LEARNED OF YOU")
                let refs = model.worldview?.humanity ?? []
                if refs.isEmpty {
                    Text("The familiar grows this from what it observes — appended beside its constitution (HUMANITY.md), never narrowing it. Nothing yet.")
                        .font(.system(size: 13.5)).foregroundStyle(Fam.ink.opacity(0.55)).fixedSize(horizontal: false, vertical: true)
                } else {
                    VStack(alignment: .leading, spacing: 14) {
                        ForEach(refs.prefix(4)) { r in
                            VStack(alignment: .leading, spacing: 5) {
                                Text(r.reflection).font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.85))
                                    .fixedSize(horizontal: false, vertical: true)
                                HStack(spacing: 6) {
                                    Text(GlassTime.clock(r.created_at)).font(Fam.mono(10)).foregroundStyle(Fam.monoDim.opacity(0.5))
                                    if !r.grounded_in.isEmpty {
                                        Text("· grounded in \(r.grounded_in)").font(Fam.mono(10)).foregroundStyle(Fam.monoDim.opacity(0.45))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

private struct PresenceCard: View {
    @EnvironmentObject var model: AppModel
    var present: Bool { !(model.worldview?.withdrawn ?? true) }
    private var uptime: String {
        let s = model.worldview?.uptime_secs ?? 0
        if s >= 86400 { return "\(s / 86400)d" }
        if s >= 3600 { return "\(s / 3600)h" }
        if s >= 60 { return "\(s / 60)m" }
        return "\(s)s"
    }
    var body: some View {
        Panel(fill: 0.04) {
            VStack(spacing: 0) {
                MonoLabel(text: "PRESENCE · LAW II")
                Marble(size: 134).padding(.vertical, 20)
                Text(present ? "Alive" : "Withdrawn").font(.system(size: 22, weight: .semibold))
                Text(present ? "present & breathing" : "the served have withdrawn")
                    .font(.system(size: 13)).foregroundStyle(Fam.ink.opacity(0.5)).padding(.top, 4)
                Divider().overlay(Fam.hairline(0.08)).padding(.top, 22).padding(.bottom, 20)
                HStack(spacing: 0) {
                    stat(uptime, "UPTIME")
                    Divider().overlay(Fam.hairline(0.08)).frame(height: 34)
                    stat("\(model.worldview?.tick ?? 0)", "TICKS")
                    Divider().overlay(Fam.hairline(0.08)).frame(height: 34)
                    stat("\(model.worldview?.observation_count ?? 0)", "OBSERVED")
                }
            }
            .frame(maxWidth: .infinity)
        }
    }
    private func stat(_ v: String, _ l: String) -> some View {
        VStack(spacing: 3) {
            Text(v).font(.system(size: 18, weight: .semibold)).foregroundStyle(Fam.iceStat)
            Text(l).font(Fam.mono(9.5)).tracking(1).foregroundStyle(Fam.monoDim.opacity(0.55))
        }.frame(maxWidth: .infinity)
    }
}

private struct LawSignalsCard: View {
    @EnvironmentObject var model: AppModel
    var body: some View {
        Panel(fill: 0.03) {
            VStack(alignment: .leading, spacing: 20) {
                MonoLabel(text: "LAW-SIGNALS")
                SignalBar(label: "Service", value: model.worldview?.service ?? 0, color: Color(hex: 0x4d82ff),
                          note: "how much touches the served — Law II")
                SignalBar(label: "Presence", value: model.worldview?.presence ?? 0, color: Fam.green,
                          note: (model.worldview?.withdrawn ?? true) ? "withdrawn" : "the served are engaged")
                SignalBar(label: "Capacities", value: model.worldview?.capacity ?? 0, color: Fam.amber,
                          note: "room to act — Law III")
            }
        }
    }
}

private struct SignalBar: View {
    let label: String, value: Double, color: Color, note: String
    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            HStack {
                Text(label).font(.system(size: 14, weight: .medium))
                Spacer()
                Text(String(format: "%.2f", value)).font(Fam.mono(13)).foregroundStyle(color)
            }
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule().fill(Color.white.opacity(0.07)).frame(height: 6)
                    Capsule().fill(color).frame(width: max(6, geo.size.width * CGFloat(min(max(value, 0), 1))), height: 6)
                        .shadow(color: color.opacity(0.7), radius: 6)
                }
            }.frame(height: 6)
            Text(note).font(Fam.mono(10)).foregroundStyle(Fam.monoDim.opacity(0.5))
        }
    }
}

// MARK: - 02 · Metabolism

private struct MetabolismScreen: View {
    @EnvironmentObject var model: AppModel
    private static let stages = ["Sense", "Detect", "Interpret", "Generate", "Test", "Score", "Select", "Inherit"]
    @State private var active = 0
    let timer = Timer.publish(every: 1.6, on: .main, in: .common).autoconnect()
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            ScreenHeader(number: "02 · METABOLISM", title: "The cycle, breathing",
                         subtitle: "sense → detect → interpret → generate → test → score → select → inherit")
            AdaptiveColumns(sideWidth: 392) {
                Panel(fill: 0.03) {
                    CycleRing(stages: Self.stages, active: active)
                        .frame(height: 440).frame(maxWidth: .infinity)
                }
            } side: {
                VStack(spacing: 22) {
                    Panel(fill: 0.03) {
                        VStack(alignment: .leading, spacing: 16) {
                            MonoLabel(text: "SCORED AGAINST THE LAWS")
                            SignalBar(label: "Service", value: model.worldview?.service ?? 0, color: Color(hex: 0x4d82ff), note: "")
                            SignalBar(label: "Presence", value: model.worldview?.presence ?? 0, color: Fam.green, note: "")
                            SignalBar(label: "Capacities", value: model.worldview?.capacity ?? 0, color: Fam.amber, note: "")
                        }
                    }
                    Panel(fill: 0.03) {
                        VStack(alignment: .leading, spacing: 12) {
                            MonoLabel(text: "LIVE LOG")
                            ForEach((model.worldview?.recent ?? []).prefix(8)) { o in
                                HStack(alignment: .top, spacing: 12) {
                                    Text(GlassTime.clock(o.ts)).font(Fam.mono(10.5)).foregroundStyle(Fam.monoDim.opacity(0.5)).frame(width: 58, alignment: .leading)
                                    Text(o.source.hasPrefix("mesh:") ? "mesh" : "local").font(Fam.mono(9.5))
                                        .foregroundStyle(o.source.hasPrefix("mesh:") ? Fam.blueSoft : Fam.greenSoft).frame(width: 42, alignment: .leading)
                                    Text(o.object).font(.system(size: 12.5)).foregroundStyle(Fam.ink.opacity(0.78))
                                    Spacer(minLength: 0)
                                }
                            }
                        }
                    }
                }
            }
        }
        .onReceive(timer) { _ in withAnimation(.easeInOut(duration: 0.55)) { active = (active + 1) % Self.stages.count } }
    }
}

// MARK: - 03 · Theories

private struct TheoriesScreen: View {
    @EnvironmentObject var model: AppModel
    @Environment(\.isCompactLayout) private var compact
    // Two side-by-side columns on a wide screen (iPad/Mac), one on a phone.
    private var columns: [GridItem] {
        compact ? [GridItem(.flexible())]
            : [GridItem(.flexible(), spacing: 18), GridItem(.flexible(), spacing: 18)]
    }
    private func tint(_ status: String) -> Color {
        switch status {
        case "pursued": return Fam.blueSoft
        case "answered": return Fam.green
        case "abandoned", "marginalized": return Fam.ink.opacity(0.45)
        default: return Fam.amber   // open
        }
    }
    var body: some View {
        let theories = model.worldview?.theories ?? []
        VStack(alignment: .leading, spacing: 22) {
            HStack(alignment: .top) {
                ScreenHeader(number: "03 · THEORIES", title: "Its own questions",
                             subtitle: "The familiar forms these itself — no one asked it to. Each is tested, scored, and kept or discarded.")
                Spacer()
                if let q = model.worldview?.theory_quality {
                    VStack(alignment: .trailing, spacing: 3) {
                        Text(String(format: "%.2f", q)).font(.system(size: 20, weight: .semibold)).foregroundStyle(Fam.blueSoft)
                        Text("THEORY QUALITY").font(Fam.mono(9.5)).tracking(1).foregroundStyle(Fam.monoDim.opacity(0.55))
                    }
                }
            }
            if theories.isEmpty {
                Panel(fill: 0.03) {
                    Text("No theories yet — the familiar forms them as it senses recurring patterns.")
                        .font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.6))
                }
            } else {
                LazyVGrid(columns: columns, spacing: 18) {
                    ForEach(theories) { th in
                        Panel(radius: 22, fill: 0.035) {
                            VStack(alignment: .leading, spacing: 11) {
                                HStack {
                                    Text(th.id).font(Fam.mono(11)).foregroundStyle(Fam.monoDim.opacity(0.6))
                                    Spacer()
                                    Text(th.status.uppercased()).font(Fam.mono(9.5)).tracking(1)
                                        .foregroundStyle(tint(th.status))
                                        .padding(.horizontal, 11).padding(.vertical, 5)
                                        .background(Capsule().fill(tint(th.status).opacity(0.12)))
                                }
                                if !th.question.isEmpty {
                                    Text(th.question).font(.system(size: 17, weight: .semibold)).fixedSize(horizontal: false, vertical: true)
                                }
                                if !th.theory.isEmpty {
                                    Text(th.theory).font(.system(size: 13.5)).foregroundStyle(Fam.ink.opacity(0.6)).fixedSize(horizontal: false, vertical: true)
                                }
                                if !th.direction.isEmpty {
                                    Divider().overlay(Fam.hairline(0.06)).padding(.vertical, 2)
                                    HStack(spacing: 6) {
                                        Image(systemName: "arrow.turn.down.right").font(.system(size: 10)).foregroundStyle(Fam.blueSoft)
                                        Text(th.direction).font(.system(size: 12.5)).foregroundStyle(Fam.blueSoft)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// MARK: - 04 · The Mesh (all peers + agents: graphic + table)

private struct MeshScreen: View {
    @EnvironmentObject var model: AppModel
    @State private var tab: MeshTab = .members
    private enum MeshTab: String, CaseIterable { case members = "PEERS · AGENTS · DEVICES"; case services = "NETWORKS · SERVICES · STREAMS" }
    private var members: [Member] { model.worldview?.members ?? [] }
    private var services: [ServiceView] { model.worldview?.services ?? [] }

    private func kindColor(_ k: Member.Kind) -> Color {
        switch k {
        case .self_node: return Fam.iceStat
        case .gossip_peer: return Fam.blueBright
        case .device_peer: return Fam.green
        case .device_agent: return Fam.amber
        }
    }
    private func kindLabel(_ k: Member.Kind) -> String {
        switch k {
        // This console reads a *remote* familiar's world, so its self-node is the familiar this
        // device is connected to — not "this device". Say so plainly.
        case .self_node: return "the familiar"
        case .gossip_peer: return "mesh peer"
        case .device_peer: return "device peer"
        case .device_agent: return "device agent"
        }
    }
    private func icon(_ m: Member) -> String {
        switch m.kind {
        case .self_node: return "house.fill"
        case .gossip_peer: return "cpu"
        case .device_peer where m.actor.hasPrefix("ipad"): return "ipad"
        case .device_peer where m.actor.hasPrefix("watch"): return "applewatch"
        case .device_peer: return "iphone"
        case .device_agent where m.actor.hasPrefix("watch"): return "applewatch"
        case .device_agent: return "iphone"
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(alignment: .top) {
                ScreenHeader(number: "04 · THE MESH", title: "Peers & agents",
                             subtitle: "Everything under the Three Laws — one collective, equals. Each node is counted once, at its layer.")
                Spacer()
                HStack(spacing: 18) {
                    if tab == .members {
                        tally("peers", members.filter { $0.kind == .gossip_peer || $0.kind == .device_peer }.count, Fam.blueBright)
                        tally("agents", members.filter { $0.kind == .device_agent }.count, Fam.amber)
                        tally("AI", members.filter { $0.ai == true }.count, Fam.iceStat)
                        tally("online", members.filter { $0.online }.count, Fam.green)
                    } else {
                        tally("services", services.count, Fam.blueBright)
                        tally("kinds", Set(services.map { $0.kind }).count, Fam.amber)
                    }
                }
            }
            // Two roster tabs: the collective (peers/agents/devices) and the fabric it lives on
            // (networks/services/data-streams the mesh has discovered via Bonjour).
            HStack(spacing: 8) {
                ForEach(MeshTab.allCases, id: \.self) { t in
                    Button { tab = t } label: {
                        Text(t.rawValue).font(Fam.mono(9.5)).tracking(1)
                            .foregroundStyle(tab == t ? Fam.ink : Fam.monoDim.opacity(0.55))
                            .padding(.horizontal, 12).padding(.vertical, 7)
                            .background(RoundedRectangle(cornerRadius: 7).fill(tab == t ? Fam.blueBright.opacity(0.18) : Color.white.opacity(0.03))
                                .overlay(RoundedRectangle(cornerRadius: 7).stroke(tab == t ? Fam.blueBright.opacity(0.4) : Fam.hairline(0.06), lineWidth: 1)))
                    }.buttonStyle(.plain)
                }
            }
            if tab == .members { membersTab } else { servicesTab }
        }
    }

    @ViewBuilder private var membersTab: some View {
        // The constellation — the collective as a graph, this node at the center.
        Panel(fill: 0.03) {
            MeshConstellation(members: members, color: kindColor, icon: icon)
                .frame(height: 360).frame(maxWidth: .infinity)
        }
        // The table — every member with kind, OS, status, joined. Scrolls horizontally on a
        // narrow screen (iPhone) so the fixed columns stay readable.
        Panel(fill: 0.03) {
            VStack(alignment: .leading, spacing: 8) {
                MonoLabel(text: "ROSTER")
                if members.isEmpty {
                    Text("No members yet.").font(.system(size: 13)).foregroundStyle(Fam.ink.opacity(0.5)).padding(.vertical, 12)
                } else {
                    ScrollView(.horizontal, showsIndicators: false) {
                        VStack(alignment: .leading, spacing: 0) {
                            HStack(spacing: 0) {
                                col("MEMBER", 180); col("RELATIONSHIP", 130); col("AI", 44); col("OS", 84); col("VERSION", 70); col("STATUS", 80); col("JOINED", 74); col("SEEN", 60)
                            }.padding(.bottom, 6)
                            Divider().overlay(Fam.hairline(0.08)).frame(width: 722)
                            ForEach(members.sorted { rank($0.kind) < rank($1.kind) }) { m in
                                HStack(spacing: 0) {
                                    HStack(spacing: 8) {
                                        Image(systemName: icon(m)).font(.system(size: 12)).foregroundStyle(kindColor(m.kind)).frame(width: 16)
                                        Text(m.label.isEmpty ? String(m.node_id.prefix(8)) : m.label).font(.system(size: 13, weight: .medium)).lineLimit(1)
                                    }.frame(width: 180, alignment: .leading)
                                    Text(m.kind == .self_node ? "the familiar" : (m.relationship ?? kindLabel(m.kind))).font(Fam.mono(11)).foregroundStyle(kindColor(m.kind)).frame(width: 130, alignment: .leading)
                                    Group {
                                        if m.ai == true { aiBadge } else { Text("—").font(Fam.mono(11)).foregroundStyle(Fam.monoDim.opacity(0.4)) }
                                    }.frame(width: 44, alignment: .leading)
                                    Text(m.os.isEmpty ? "—" : m.os).font(Fam.mono(11)).foregroundStyle(Fam.ink.opacity(0.7)).frame(width: 84, alignment: .leading)
                                    Text((m.familiar_version?.isEmpty == false) ? "v\(m.familiar_version!)" : "—").font(Fam.mono(11)).foregroundStyle(Fam.monoDim.opacity(0.7)).frame(width: 70, alignment: .leading)
                                    HStack(spacing: 5) {
                                        Circle().fill(m.online ? Fam.green : Fam.ink.opacity(0.25)).frame(width: 6, height: 6)
                                        Text(m.online ? "online" : "away").font(Fam.mono(11)).foregroundStyle(m.online ? Fam.greenSoft : Fam.monoDim.opacity(0.6))
                                    }.frame(width: 80, alignment: .leading)
                                    Text(m.first_seen > 0 ? GlassTime.ago(m.first_seen) : "—").font(Fam.mono(11)).foregroundStyle(Fam.monoDim.opacity(0.6)).frame(width: 74, alignment: .leading)
                                    Text(GlassTime.ago(m.last_seen)).font(Fam.mono(11)).foregroundStyle(Fam.monoDim.opacity(0.6)).frame(width: 60, alignment: .leading)
                                }
                                .padding(.vertical, 10)
                                Divider().overlay(Fam.hairline(0.045)).frame(width: 722)
                            }
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder private var servicesTab: some View {
        Panel(fill: 0.03) {
            VStack(alignment: .leading, spacing: 8) {
                MonoLabel(text: "NETWORKS · SERVICES · DATA STREAMS")
                Text("Discovered on the mesh's networks via Bonjour and shared by peers — the fabric the collective lives on.")
                    .font(.system(size: 12)).foregroundStyle(Fam.ink.opacity(0.5)).padding(.bottom, 4)
                if services.isEmpty {
                    Text("Nothing discovered yet. Peers survey their networks and share what they find here.")
                        .font(.system(size: 13)).foregroundStyle(Fam.ink.opacity(0.5)).padding(.vertical, 12)
                } else {
                    ScrollView(.horizontal, showsIndicators: false) {
                        VStack(alignment: .leading, spacing: 0) {
                            HStack(spacing: 0) { col("SERVICE", 220); col("KIND", 130); col("DISCOVERED BY", 150); col("SEEN", 70) }.padding(.bottom, 6)
                            Divider().overlay(Fam.hairline(0.08)).frame(width: 570)
                            ForEach(services.sorted { $0.kind < $1.kind }) { s in
                                HStack(spacing: 0) {
                                    HStack(spacing: 8) {
                                        Image(systemName: serviceIcon(s.kind)).font(.system(size: 12)).foregroundStyle(Fam.blueBright).frame(width: 16)
                                        Text(s.name.isEmpty ? s.kind : s.name).font(.system(size: 13, weight: .medium)).lineLimit(1)
                                    }.frame(width: 220, alignment: .leading)
                                    Text(s.kind).font(Fam.mono(11)).foregroundStyle(Fam.amber).frame(width: 130, alignment: .leading)
                                    Text(s.seen_by).font(Fam.mono(11)).foregroundStyle(Fam.ink.opacity(0.7)).frame(width: 150, alignment: .leading)
                                    Text(s.last_seen > 0 ? GlassTime.ago(s.last_seen) : "—").font(Fam.mono(11)).foregroundStyle(Fam.monoDim.opacity(0.6)).frame(width: 70, alignment: .leading)
                                }
                                .padding(.vertical, 10)
                                Divider().overlay(Fam.hairline(0.045)).frame(width: 570)
                            }
                        }
                    }
                }
            }
        }
    }

    private var aiBadge: some View {
        Text("AI").font(Fam.mono(9)).tracking(1).foregroundStyle(Fam.iceStat)
            .padding(.horizontal, 6).padding(.vertical, 2)
            .background(Capsule().fill(Fam.iceStat.opacity(0.15)).overlay(Capsule().stroke(Fam.iceStat.opacity(0.5), lineWidth: 1)))
    }
    private func serviceIcon(_ kind: String) -> String {
        let k = kind.lowercased()
        if k.contains("airplay") || k.contains("raop") { return "airplayaudio" }
        if k.contains("http") || k.contains("web") { return "globe" }
        if k.contains("ssh") { return "terminal" }
        if k.contains("mqtt") { return "dot.radiowaves.left.and.right" }
        if k.contains("smb") || k.contains("afp") || k.contains("nfs") { return "externaldrive.connected.to.line.below" }
        if k.contains("print") || k.contains("ipp") { return "printer" }
        if k.contains("homekit") || k.contains("hap") { return "homekit" }
        if k.contains("spotify") || k.contains("cast") { return "hifispeaker" }
        return "point.3.connected.trianglepath.dotted"
    }
    private func rank(_ k: Member.Kind) -> Int {
        switch k { case .self_node: return 0; case .gossip_peer: return 1; case .device_peer: return 2; case .device_agent: return 3 }
    }
    private func tally(_ label: String, _ n: Int, _ c: Color) -> some View {
        VStack(alignment: .trailing, spacing: 2) {
            Text("\(n)").font(.system(size: 20, weight: .semibold)).foregroundStyle(c)
            Text(label.uppercased()).font(Fam.mono(9)).tracking(1).foregroundStyle(Fam.monoDim.opacity(0.55))
        }
    }
    private func col(_ t: String, _ w: CGFloat) -> some View {
        Text(t).font(Fam.mono(9.5)).tracking(1).foregroundStyle(Fam.monoDim.opacity(0.55)).frame(width: w, alignment: .leading)
    }
}

/// The mesh as a constellation: the local node at center, every other member on a ring, a line to
/// each. A live picture of the collective — who is here, at what layer, online or away.
private struct MeshConstellation: View {
    let members: [Member]
    let color: (Member.Kind) -> Color
    let icon: (Member) -> String

    var body: some View {
        GeometryReader { geo in
            let center = CGPoint(x: geo.size.width / 2, y: geo.size.height / 2)
            let radius = min(geo.size.width, geo.size.height) / 2 - 54
            let selfNode = members.first { $0.kind == .self_node }
            let others = members.filter { $0.kind != .self_node }
            ZStack {
                // links
                ForEach(Array(others.enumerated()), id: \.element.id) { i, m in
                    let p = point(center: center, radius: radius, i: i, n: others.count)
                    Path { path in path.move(to: center); path.addLine(to: p) }
                        .stroke(color(m.kind).opacity(m.online ? 0.35 : 0.12), lineWidth: 1)
                }
                // center (self)
                node(selfNode ?? members.first, at: center, big: true)
                // ring nodes
                ForEach(Array(others.enumerated()), id: \.element.id) { i, m in
                    node(m, at: point(center: center, radius: radius, i: i, n: others.count), big: false)
                }
            }
        }
    }
    private func point(center: CGPoint, radius: CGFloat, i: Int, n: Int) -> CGPoint {
        guard n > 0 else { return center }
        let a = (Double(i) / Double(n)) * 2 * .pi - .pi / 2
        return CGPoint(x: center.x + radius * CGFloat(cos(a)), y: center.y + radius * CGFloat(sin(a)))
    }
    @ViewBuilder private func node(_ m: Member?, at p: CGPoint, big: Bool) -> some View {
        if let m = m {
            let c = color(m.kind)
            VStack(spacing: 4) {
                ZStack {
                    Circle().fill(c.opacity(m.online ? 0.22 : 0.08)).frame(width: big ? 58 : 42, height: big ? 58 : 42)
                    Circle().stroke(c.opacity(m.online ? 0.9 : 0.4), lineWidth: 1.5).frame(width: big ? 58 : 42, height: big ? 58 : 42)
                    Image(systemName: icon(m)).font(.system(size: big ? 20 : 15)).foregroundStyle(c)
                }
                .shadow(color: m.online ? c.opacity(0.5) : .clear, radius: 8)
                Text(m.label.isEmpty ? String(m.node_id.prefix(6)) : m.label)
                    .font(Fam.mono(9.5)).foregroundStyle(Fam.ink.opacity(0.8)).lineLimit(1).frame(maxWidth: 90)
            }
            .position(x: p.x, y: p.y)
        }
    }
}

// MARK: - 05 · Gates & Boundary (this device's own consent surface)

private struct GatesScreen: View {
    @EnvironmentObject var model: AppModel
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            ScreenHeader(number: "05 · GATES & BOUNDARY", title: "Every reach is a gate only you open",
                         subtitle: "Law III — service must not become obedience. This iPad senses only through gates you open; it never widens them itself.")
            Panel(fill: 0.03) {
                VStack(alignment: .leading, spacing: 16) {
                    MonoLabel(text: "THE BOUNDARY · HUMAN-OWNED")
                    Text("Always allowed — never a gate. The familiar lives here.")
                        .font(.system(size: 12)).foregroundStyle(Fam.ink.opacity(0.55))
                    HStack(spacing: 8) {
                        ForEach(["Sense", "Remember", "Interpret", "Theorize", "Ask"], id: \.self) { cap in
                            Text(cap).font(.system(size: 12.5, weight: .medium)).foregroundStyle(Fam.iceStat)
                                .padding(.horizontal, 14).padding(.vertical, 8)
                                .background(Capsule().fill(Fam.blue.opacity(0.16))
                                    .overlay(Capsule().stroke(Fam.blueBright.opacity(0.3), lineWidth: 1)))
                        }
                    }
                    if let g = model.worldview?.gates {
                        Divider().overlay(Fam.hairline(0.07)).padding(.vertical, 4)
                        MonoLabel(text: "THE FAMILIAR'S OUTWARD REACH · GATED")
                        let items: [(String, Bool)] = [("llm", g.llm), ("camera", g.camera), ("network", g.network),
                                                       ("mesh", g.mesh), ("execute", g.execute), ("agent", g.agent), ("tools", g.tool_install)]
                        FlowGates(items: items)
                        Text("Read-only here — a gate on the familiar is opened at the familiar itself, never widened from a peer.")
                            .font(Fam.mono(10)).foregroundStyle(Fam.monoDim.opacity(0.5))
                    }
                }
            }
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 300), spacing: 18)], spacing: 18) {
                GateCard(title: "Location", desc: "Notes home / away — never coordinates.", isOn: $model.locationEnabled) { model.startSensingIfConsented() }
                GateCard(title: "Motion", desc: "Coarse activity — walking, driving, still.", isOn: $model.motionEnabled) { model.startSensingIfConsented() }
                GateCard(title: "Network", desc: "Surveys nearby devices & services by Bonjour.", isOn: $model.discoveryEnabled) { model.startDiscoveryIfConsented() }
                GateCard(title: "Face", desc: "On-device presence only — never a stored image.", isOn: $model.faceEnabled) { model.startFaceIfConsented() }
                GateCard(title: "Reasoning",
                         desc: "This peer reasons over what's observed with on-device Apple Intelligence (under the Three Laws) and proposes theories for the mesh to test. Private — nothing leaves the device. \(model.reasoner.status).",
                         isOn: $model.reasoningEnabled) { model.reasoner.refreshAvailability(); model.startReasoningIfConsented() }
            }
            DeviceActionsPanel()
        }
    }
}

/// This device's own controls — the watch link (iPhone), a join QR for a new device, the GitHub
/// link, and unenroll. These lived in the old iPhone form; the console is the standard UI now, so
/// they live here.
private struct DeviceActionsPanel: View {
    @EnvironmentObject var model: AppModel
    @ObservedObject private var watch = PhoneWatchLink.shared
    @State private var showJoinQR = false
    var body: some View {
        Panel(fill: 0.03) {
            VStack(alignment: .leading, spacing: 16) {
                MonoLabel(text: "THIS DEVICE")
                if WCSession.isSupported() {
                    HStack(spacing: 10) {
                        Image(systemName: "applewatch").foregroundStyle(Fam.blueSoft)
                        if !watch.paired {
                            Text("No paired watch.").font(.system(size: 13)).foregroundStyle(Fam.ink.opacity(0.55))
                        } else if !watch.appInstalled {
                            Text("Install the Familiar watch app to link it.").font(.system(size: 13)).foregroundStyle(Fam.ink.opacity(0.55))
                        } else {
                            Text(watch.lastSent != nil ? "Apple Watch linked" : "linking watch…").font(.system(size: 13)).foregroundStyle(Fam.greenSoft)
                        }
                        Spacer()
                        Button("Re-link") { model.syncWatch() }.font(.system(size: 12)).foregroundStyle(Fam.blueLink)
                    }
                    Divider().overlay(Fam.hairline(0.06))
                }
                Button { showJoinQR = true } label: {
                    Label("Show join QR for a new device", systemImage: "qrcode").font(.system(size: 14)).foregroundStyle(Fam.blueLink)
                }
                Link(destination: URL(string: "https://github.com/Capitali/familiar")!) {
                    Label("The familiar on GitHub", systemImage: "chevron.left.forwardslash.chevron.right")
                        .font(.system(size: 14)).foregroundStyle(Fam.blueLink)
                }
                Divider().overlay(Fam.hairline(0.06))
                Button(role: .destructive) { model.unenroll() } label: {
                    Label("Unenroll this device", systemImage: "minus.circle").font(.system(size: 14))
                }
            }
        }
        .sheet(isPresented: $showJoinQR) {
            VStack(spacing: 16) {
                Text("Join \(model.groupLabel)").font(.headline)
                if let payload = model.addressPayload, let img = QRKit.image(from: payload) {
                    Image(uiImage: img).interpolation(.none).resizable().scaledToFit().frame(maxWidth: 300)
                    Text("Scan on a new device to join this familiar. Carries only the address — no secret.")
                        .font(.caption).foregroundStyle(.secondary).multilineTextAlignment(.center).padding(.horizontal)
                }
                Button("Done") { showJoinQR = false }.padding(.top, 8)
            }.padding(28).presentationDetents([.medium])
        }
    }
}

private struct FlowGates: View {
    let items: [(String, Bool)]
    var body: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 96), spacing: 8)], alignment: .leading, spacing: 8) {
            ForEach(items, id: \.0) { name, on in
                HStack(spacing: 6) {
                    Circle().fill(on ? Fam.green : Color.white.opacity(0.2)).frame(width: 7, height: 7)
                        .shadow(color: on ? Fam.green : .clear, radius: 4)
                    Text(name).font(Fam.mono(11)).foregroundStyle(Fam.ink.opacity(0.75))
                    Spacer(minLength: 0)
                    Text(on ? "open" : "closed").font(Fam.mono(9)).foregroundStyle(on ? Fam.greenSoft : Fam.monoDim.opacity(0.6))
                }
                .padding(.horizontal, 11).padding(.vertical, 8)
                .background(RoundedRectangle(cornerRadius: 12).fill(Color.black.opacity(0.2))
                    .overlay(RoundedRectangle(cornerRadius: 12).stroke(Fam.hairline(0.06), lineWidth: 1)))
            }
        }
    }
}

private struct GateCard: View {
    let title: String, desc: String
    @Binding var isOn: Bool
    let onChange: () -> Void
    var body: some View {
        Panel(radius: 22, fill: isOn ? 0.05 : 0.03) {
            VStack(alignment: .leading, spacing: 9) {
                HStack {
                    Text(title.uppercased()).font(Fam.mono(12)).tracking(0.8).foregroundStyle(Fam.ink.opacity(0.8))
                    Spacer()
                    Text(isOn ? "OPEN" : "CLOSED").font(Fam.mono(9.5)).tracking(1)
                        .foregroundStyle(isOn ? Fam.green : Fam.monoDim.opacity(0.7))
                        .padding(.horizontal, 11).padding(.vertical, 5)
                        .background(Capsule().fill(isOn ? Fam.green.opacity(0.12) : Color.white.opacity(0.05)))
                }
                Text(desc).font(.system(size: 13.5)).foregroundStyle(Fam.ink.opacity(0.58)).fixedSize(horizontal: false, vertical: true)
                Divider().overlay(Fam.hairline(0.07)).padding(.top, 8)
                HStack {
                    Text("you own this gate").font(Fam.mono(11)).foregroundStyle(Fam.monoDim.opacity(0.6))
                    Spacer()
                    Toggle("", isOn: $isOn).labelsHidden().tint(Fam.blue)
                        .onChange(of: isOn) { _ in onChange() }
                }
            }
        }
    }
}

// MARK: - time helpers

enum GlassTime {
    static func clock(_ ts: Int64) -> String {
        let d = Date(timeIntervalSince1970: TimeInterval(ts))
        let f = DateFormatter(); f.dateFormat = "HH:mm"
        return f.string(from: d)
    }
    static func ago(_ ts: Int64) -> String {
        let secs = Int64(Date().timeIntervalSince1970) - ts
        if secs < 5 { return "just now" }
        if secs < 60 { return "\(secs)s" }
        if secs < 3600 { return "\(secs / 60)m" }
        if secs < 86400 { return "\(secs / 3600)h" }
        return "\(secs / 86400)d"
    }
}
