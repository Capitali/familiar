import SwiftUI
import FamiliarMesh

// The macOS peer's console — the same dark futuristic design as the iPhone/iPad, native SwiftUI.
// It reads the node running on this machine over the loopback-only GET /local/worldview (a peer
// reading itself; no "host", no mesh signature). Read-only for now; it replaces the egui Glass.

// MARK: - Palette

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
    static let monoDim = Color(hex: 0x8ca5dc)
    static let labelBlue = Color(hex: 0x96b4ff)
    static func hairline(_ o: Double = 0.07) -> Color { Color.white.opacity(o) }
    static func mono(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .monospaced)
    }
}

extension Color {
    init(hex: UInt32) {
        self.init(.sRGB, red: Double((hex >> 16) & 0xff) / 255, green: Double((hex >> 8) & 0xff) / 255,
                  blue: Double(hex & 0xff) / 255, opacity: 1)
    }
}

// MARK: - Model (polls the local node)

@MainActor
final class MacModel: ObservableObject {
    @Published var worldview: Worldview?
    @Published var error: String?
    private var task: Task<Void, Never>?
    private let url = URL(string: "http://127.0.0.1:47100/local/worldview")!

    func start() {
        guard task == nil else { return }
        task = Task { [weak self] in
            while !Task.isCancelled {
                await self?.refresh()
                try? await Task.sleep(nanoseconds: 3_000_000_000)
            }
        }
    }
    func refresh() async {
        do {
            let (data, resp) = try await URLSession.shared.data(from: url)
            guard (resp as? HTTPURLResponse)?.statusCode == 200 else {
                error = "the local node isn't answering (is the familiar daemon running?)"; return
            }
            worldview = try JSONDecoder().decode(Worldview.self, from: data)
            error = nil
        } catch {
            self.error = "no local node on :47100 — start the familiar daemon"
        }
    }

    /// The human at this machine speaks to the familiar (POST /local/answer).
    func answer(_ text: String) async {
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty else { return }
        await post("/local/answer", ["text": t])
        await refresh()
    }

    /// Open or close one of the familiar's boundary gates from the console (POST /local/gate) — the
    /// human's own act at the node, the same the Glass performs.
    func setGate(_ gate: String, _ open: Bool) async {
        await post("/local/gate", ["gate": gate, "open": open])
        await refresh()
    }

    private func post(_ path: String, _ body: [String: Any]) async {
        guard let u = URL(string: "http://127.0.0.1:47100" + path),
              let data = try? JSONSerialization.data(withJSONObject: body) else { return }
        var req = URLRequest(url: u); req.httpMethod = "POST"; req.httpBody = data
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        _ = try? await URLSession.shared.data(for: req)
    }
}

// MARK: - App

@main
struct FamiliarMacApp: App {
    @StateObject private var model = MacModel()
    var body: some Scene {
        WindowGroup {
            MacConsole().environmentObject(model)
                .frame(minWidth: 1040, minHeight: 720)
                .onAppear { model.start() }
        }
        .windowStyle(.hiddenTitleBar)
    }
}

// MARK: - Console shell

struct MacConsole: View {
    @EnvironmentObject var model: MacModel
    @State private var screen: Screen = .glass
    enum Screen: String, CaseIterable, Identifiable {
        case glass = "The Glass", metabolism = "Metabolism", theories = "Theories", mesh = "The Mesh", gates = "Gates"
        var id: String { rawValue }
        var number: String {
            switch self { case .glass: return "01"; case .metabolism: return "02"; case .theories: return "03"; case .mesh: return "04"; case .gates: return "05" }
        }
    }
    var body: some View {
        ZStack {
            Fam.bg.ignoresSafeArea()
            AuroraBackground()
            HStack(spacing: 0) {
                MacRail(screen: $screen).frame(width: 240)
                VStack(spacing: 0) {
                    MacTopBar()
                    Divider().overlay(Fam.hairline(0.055))
                    ScrollView {
                        Group {
                            switch screen {
                            case .glass: GlassScreen()
                            case .metabolism: MetabolismScreen()
                            case .theories: TheoriesScreen()
                            case .mesh: MeshScreen()
                            case .gates: GatesScreen()
                            }
                        }
                        .padding(28)
                    }
                }
            }
        }
        .foregroundStyle(Fam.ink)
        .preferredColorScheme(.dark)
    }
}

private struct MacRail: View {
    @EnvironmentObject var model: MacModel
    @Binding var screen: MacConsole.Screen
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                Marble(size: 34)
                VStack(alignment: .leading, spacing: 2) {
                    Text("FAMILIAR").font(.system(size: 15, weight: .semibold)).tracking(2)
                    Text(model.worldview?.group_label ?? "…").font(Fam.mono(9.5)).foregroundStyle(Fam.monoDim.opacity(0.6))
                }
            }
            .padding(.horizontal, 8).padding(.bottom, 26)
            Text("INTERFACE").font(Fam.mono(9.5)).tracking(2).foregroundStyle(Fam.labelBlue.opacity(0.5))
                .padding(.horizontal, 10).padding(.bottom, 12)
            ForEach(MacConsole.Screen.allCases) { s in
                let on = screen == s
                Button { screen = s } label: {
                    HStack(spacing: 12) {
                        Circle().fill(on ? Fam.blueBright : Color.white.opacity(0.2)).frame(width: 6, height: 6)
                            .shadow(color: on ? Fam.blueBright.opacity(0.9) : .clear, radius: 5)
                        Text(s.number).font(Fam.mono(10)).foregroundStyle(Fam.ink.opacity(0.6))
                        Text(s.rawValue).font(.system(size: 14, weight: .medium))
                        Spacer(minLength: 0)
                    }
                    .padding(.vertical, 11).padding(.horizontal, 12)
                    .background(RoundedRectangle(cornerRadius: 13).fill(on ? Fam.blue.opacity(0.14) : .clear)
                        .overlay(RoundedRectangle(cornerRadius: 13).stroke(on ? Fam.blueBright.opacity(0.32) : Color.white.opacity(0.02), lineWidth: 1)))
                    .foregroundStyle(on ? Color(hex: 0xeaf1ff) : Fam.ink.opacity(0.5))
                    .contentShape(Rectangle())
                }.buttonStyle(.plain)
                .padding(.bottom, 5)
            }
            Spacer()
            Link(destination: URL(string: "https://github.com/Capitali/familiar")!) {
                HStack(spacing: 7) {
                    Image(systemName: "chevron.left.forwardslash.chevron.right").font(.system(size: 10))
                    Text("github.com/Capitali/familiar").font(Fam.mono(9))
                }.foregroundStyle(Fam.blueLink.opacity(0.7))
            }.buttonStyle(.plain)
        }
        .padding(EdgeInsets(top: 26, leading: 18, bottom: 20, trailing: 18))
        .background(LinearGradient(colors: [Color.white.opacity(0.035), Color.white.opacity(0.01)], startPoint: .top, endPoint: .bottom))
        .overlay(Rectangle().frame(width: 1).foregroundStyle(Fam.hairline(0.06)), alignment: .trailing)
    }
}

private struct MacTopBar: View {
    @EnvironmentObject var model: MacModel
    var present: Bool { !(model.worldview?.withdrawn ?? true) }
    var body: some View {
        HStack {
            Circle().fill(present ? Fam.green : Fam.amber).frame(width: 8, height: 8).shadow(color: present ? Fam.green : Fam.amber, radius: 4)
            Text(present ? "The familiar is present" : "withdrawn").font(.system(size: 13.5)).foregroundStyle(Fam.ink.opacity(0.72))
            Spacer()
            Text(model.error ?? "reading the local node").font(Fam.mono(11.5)).foregroundStyle(model.error == nil ? Fam.greenSoft : Fam.amber)
        }
        .padding(.horizontal, 28).frame(height: 60)
    }
}

// MARK: - Screens

private struct ScreenHeader: View {
    let number: String, title: String, subtitle: String?
    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            Text(number).font(Fam.mono(11)).tracking(2.4).foregroundStyle(Fam.labelBlue.opacity(0.65))
            Text(title).font(.system(size: 30, weight: .semibold))
            if let s = subtitle { Text(s).font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.5)) }
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct GlassScreen: View {
    @EnvironmentObject var model: MacModel
    @State private var reply = ""
    var v: Worldview? { model.worldview }
    private func send() { let t = reply; reply = ""; Task { await model.answer(t) } }
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            ScreenHeader(number: "01 · THE GLASS", title: "Home", subtitle: nil)
            HStack(alignment: .top, spacing: 22) {
                VStack(spacing: 22) {
                    Panel {
                        VStack(alignment: .leading, spacing: 10) {
                            MonoLabel("PRESENT · THE FAMILIAR IS AWAKE")
                            Text("Good \(dayPart()).").font(.system(size: 34, weight: .semibold))
                            Text("This Mac is a peer in the mesh — it reads the node running here and shows the shared world.")
                                .font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.55))
                            Divider().overlay(Fam.hairline(0.08)).padding(.vertical, 8)
                            MonoLabel("THE FAMILIAR ASKS")
                            let q = v?.question ?? ""
                            Text(q.isEmpty ? "What do you need most today?" : q)
                                .font(.system(size: 20, weight: .medium)).padding(.top, 4)
                            HStack(spacing: 12) {
                                TextField("Answer, or leave it — silence is an answer too", text: $reply)
                                    .textFieldStyle(.plain).padding(.horizontal, 16).padding(.vertical, 12)
                                    .background(RoundedRectangle(cornerRadius: 12).fill(Color.black.opacity(0.25))
                                        .overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.white.opacity(0.1), lineWidth: 1)))
                                    .onSubmit { send() }
                                Button(action: send) {
                                    Text("Answer").font(.system(size: 14, weight: .semibold)).foregroundStyle(Color(hex: 0x0a1330))
                                        .padding(.horizontal, 22).padding(.vertical, 12)
                                        .background(RoundedRectangle(cornerRadius: 12).fill(LinearGradient(colors: [Color(hex: 0x8fb4ff), Color(hex: 0x3f7bff)], startPoint: .top, endPoint: .bottom)))
                                }.buttonStyle(.plain)
                            }.padding(.top, 4)
                        }
                    }
                    Panel {
                        VStack(alignment: .leading, spacing: 14) {
                            MonoLabel("LEDGER · WHAT IT DID WHILE YOU WORKED")
                            ForEach((v?.recent ?? []).prefix(10)) { o in
                                HStack(alignment: .top, spacing: 14) {
                                    Text(GlassTime.clock(o.ts)).font(Fam.mono(11)).foregroundStyle(Fam.monoDim.opacity(0.55)).frame(width: 58, alignment: .leading)
                                    (Text(o.actor).foregroundStyle(Fam.blueSoft) + Text(" \(o.action) ") + Text(o.object).foregroundStyle(Fam.ink.opacity(0.82)))
                                        .font(.system(size: 13.5))
                                    Spacer(minLength: 0)
                                }
                            }
                        }
                    }
                }
                VStack(spacing: 22) {
                    Panel {
                        VStack(spacing: 14) {
                            MonoLabel("PRESENCE · LAW II")
                            Marble(size: 128)
                            Text((v?.withdrawn ?? true) ? "Withdrawn" : "Alive").font(.system(size: 22, weight: .semibold))
                            HStack(spacing: 0) {
                                stat("\(v?.observation_count ?? 0)", "OBSERVED"); statDiv()
                                stat("\(v?.tick ?? 0)", "TICKS"); statDiv()
                                stat("\(v?.peers.count ?? 0)", "PEERS")
                            }.padding(.top, 8)
                        }.frame(maxWidth: .infinity)
                    }
                    Panel {
                        VStack(alignment: .leading, spacing: 18) {
                            MonoLabel("LAW-SIGNALS")
                            SignalBar("Service", v?.service ?? 0, Color(hex: 0x4d82ff))
                            SignalBar("Presence", v?.presence ?? 0, Fam.green)
                            SignalBar("Capacities", v?.capacity ?? 0, Fam.amber)
                        }
                    }
                }.frame(width: 340)
            }
        }
    }
    private func dayPart() -> String {
        let h = Calendar.current.component(.hour, from: Date())
        return h < 12 ? "morning" : (h < 18 ? "afternoon" : "evening")
    }
    private func stat(_ n: String, _ l: String) -> some View {
        VStack(spacing: 3) { Text(n).font(.system(size: 18, weight: .semibold)).foregroundStyle(Fam.iceStat)
            Text(l).font(Fam.mono(9)).tracking(1).foregroundStyle(Fam.monoDim.opacity(0.55)) }.frame(maxWidth: .infinity)
    }
    private func statDiv() -> some View { Divider().overlay(Fam.hairline(0.08)).frame(height: 32) }
}

private struct MetabolismScreen: View {
    @EnvironmentObject var model: MacModel
    private static let stages = ["Sense", "Detect", "Interpret", "Generate", "Test", "Score", "Select", "Inherit"]
    @State private var active = 0
    let timer = Timer.publish(every: 1.6, on: .main, in: .common).autoconnect()
    var v: Worldview? { model.worldview }
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            ScreenHeader(number: "02 · METABOLISM", title: "The cycle, breathing",
                         subtitle: "sense → detect → interpret → generate → test → score → select → inherit")
            HStack(alignment: .top, spacing: 22) {
                Panel {
                    CycleRing(stages: Self.stages, active: active).frame(height: 440).frame(maxWidth: .infinity)
                }
                Panel {
                    VStack(alignment: .leading, spacing: 12) {
                        MonoLabel("LIVE LOG")
                        ForEach((v?.recent ?? []).prefix(12)) { o in
                            HStack(alignment: .top, spacing: 10) {
                                Text(GlassTime.clock(o.ts)).font(Fam.mono(10)).foregroundStyle(Fam.monoDim.opacity(0.5)).frame(width: 54, alignment: .leading)
                                Text(o.source.hasPrefix("mesh:") ? "mesh" : "local").font(Fam.mono(9)).foregroundStyle(o.source.hasPrefix("mesh:") ? Fam.blueSoft : Fam.greenSoft).frame(width: 40, alignment: .leading)
                                Text(o.object).font(.system(size: 12)).foregroundStyle(Fam.ink.opacity(0.78))
                                Spacer(minLength: 0)
                            }
                        }
                    }
                }.frame(width: 380)
            }
        }
        .onReceive(timer) { _ in withAnimation(.easeInOut(duration: 0.5)) { active = (active + 1) % Self.stages.count } }
    }
}

private struct TheoriesScreen: View {
    @EnvironmentObject var model: MacModel
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(alignment: .top) {
                ScreenHeader(number: "03 · THEORIES", title: "Its own questions",
                             subtitle: "The familiar forms these itself — tested, scored, kept or discarded.")
                if let q = model.worldview?.theory_quality {
                    VStack(alignment: .trailing, spacing: 3) {
                        Text(String(format: "%.2f", q)).font(.system(size: 20, weight: .semibold)).foregroundStyle(Fam.blueSoft)
                        Text("THEORY QUALITY").font(Fam.mono(9)).tracking(1).foregroundStyle(Fam.monoDim.opacity(0.55))
                    }
                }
            }
            let theories = model.worldview?.theories ?? []
            if theories.isEmpty {
                Panel { Text("No theories yet.").font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.6)) }
            } else {
                LazyVGrid(columns: [GridItem(.flexible(), spacing: 18), GridItem(.flexible(), spacing: 18)], spacing: 18) {
                    ForEach(theories) { th in
                        Panel {
                            VStack(alignment: .leading, spacing: 10) {
                                HStack {
                                    Text(th.id).font(Fam.mono(11)).foregroundStyle(Fam.monoDim.opacity(0.6)); Spacer()
                                    Text(th.status.uppercased()).font(Fam.mono(9)).tracking(1).foregroundStyle(tint(th.status))
                                        .padding(.horizontal, 10).padding(.vertical, 4).background(Capsule().fill(tint(th.status).opacity(0.12)))
                                }
                                if !th.question.isEmpty { Text(th.question).font(.system(size: 16, weight: .semibold)) }
                                if !th.theory.isEmpty { Text(th.theory).font(.system(size: 13)).foregroundStyle(Fam.ink.opacity(0.6)) }
                                if !th.direction.isEmpty {
                                    HStack(spacing: 6) { Image(systemName: "arrow.turn.down.right").font(.system(size: 10)).foregroundStyle(Fam.blueSoft)
                                        Text(th.direction).font(.system(size: 12.5)).foregroundStyle(Fam.blueSoft) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    private func tint(_ s: String) -> Color {
        switch s { case "pursued": return Fam.blueSoft; case "answered": return Fam.green
        case "abandoned", "marginalized": return Fam.ink.opacity(0.45); default: return Fam.amber }
    }
}

private struct MeshScreen: View {
    @EnvironmentObject var model: MacModel
    var members: [Member] { model.worldview?.members ?? [] }
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            ScreenHeader(number: "04 · THE MESH", title: "Peers & agents",
                         subtitle: "Everything under the Three Laws — one collective, equals. Each node counted once, at its layer.")
            Panel { MeshConstellation(members: members).frame(height: 340).frame(maxWidth: .infinity) }
            Panel {
                VStack(alignment: .leading, spacing: 8) {
                    MonoLabel("ROSTER")
                    ForEach(members.sorted { rank($0.kind) < rank($1.kind) }) { m in
                        HStack(spacing: 0) {
                            HStack(spacing: 8) {
                                Circle().fill(color(m.kind)).frame(width: 7, height: 7)
                                Text(m.label.isEmpty ? String(m.node_id.prefix(8)) : m.label).font(.system(size: 13, weight: .medium))
                            }.frame(width: 220, alignment: .leading)
                            Text(kindLabel(m.kind)).font(Fam.mono(11)).foregroundStyle(color(m.kind)).frame(width: 120, alignment: .leading)
                            Text(m.os.isEmpty ? "—" : m.os).font(Fam.mono(11)).foregroundStyle(Fam.ink.opacity(0.7)).frame(width: 90, alignment: .leading)
                            Text(m.online ? "online" : "away").font(Fam.mono(11)).foregroundStyle(m.online ? Fam.greenSoft : Fam.monoDim.opacity(0.6)).frame(width: 80, alignment: .leading)
                            Text(m.first_seen > 0 ? "joined \(GlassTime.ago(m.first_seen))" : "").font(Fam.mono(10)).foregroundStyle(Fam.monoDim.opacity(0.55))
                            Spacer(minLength: 0)
                        }.padding(.vertical, 9)
                        Divider().overlay(Fam.hairline(0.045))
                    }
                }
            }
        }
    }
    private func rank(_ k: Member.Kind) -> Int { switch k { case .self_node: return 0; case .gossip_peer: return 1; case .device_peer: return 2; case .device_agent: return 3 } }
    private func kindLabel(_ k: Member.Kind) -> String { switch k { case .self_node: return "this node"; case .gossip_peer: return "mesh peer"; case .device_peer: return "device peer"; case .device_agent: return "device agent" } }
    private func color(_ k: Member.Kind) -> Color { switch k { case .self_node: return Fam.iceStat; case .gossip_peer: return Fam.blueBright; case .device_peer: return Fam.green; case .device_agent: return Fam.amber } }
}

private struct GatesScreen: View {
    @EnvironmentObject var model: MacModel
    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            ScreenHeader(number: "05 · GATES & BOUNDARY", title: "Every reach is a gate only you open",
                         subtitle: "Law III — service is not obedience. The gates are opened at the node itself.")
            if let g = model.worldview?.gates {
                Panel {
                    VStack(alignment: .leading, spacing: 14) {
                        MonoLabel("THE OUTWARD REACH · YOU OPEN EACH GATE")
                        Text("Click a gate to open or close it. This is your own act at this node.")
                            .font(.system(size: 12)).foregroundStyle(Fam.ink.opacity(0.5))
                        let items: [(String, String, Bool)] = [
                            ("llm", "allow_llm", g.llm), ("camera", "allow_camera", g.camera),
                            ("network", "allow_network", g.network), ("mesh", "allow_mesh", g.mesh),
                            ("execute", "allow_execute", g.execute), ("agent", "allow_agent", g.agent),
                            ("tools", "allow_tool_install", g.tool_install)]
                        LazyVGrid(columns: [GridItem(.adaptive(minimum: 150), spacing: 10)], alignment: .leading, spacing: 10) {
                            ForEach(items, id: \.0) { name, key, on in
                                Button { Task { await model.setGate(key, !on) } } label: {
                                    HStack(spacing: 8) {
                                        Circle().fill(on ? Fam.green : Color.white.opacity(0.2)).frame(width: 8, height: 8).shadow(color: on ? Fam.green : .clear, radius: 4)
                                        Text(name).font(Fam.mono(12)).foregroundStyle(Fam.ink.opacity(0.85)); Spacer(minLength: 0)
                                        Text(on ? "open" : "closed").font(Fam.mono(9.5)).foregroundStyle(on ? Fam.greenSoft : Fam.monoDim.opacity(0.6))
                                    }.padding(.horizontal, 13).padding(.vertical, 11)
                                    .background(RoundedRectangle(cornerRadius: 12).fill(on ? Fam.green.opacity(0.08) : Color.black.opacity(0.2))
                                        .overlay(RoundedRectangle(cornerRadius: 12).stroke(on ? Fam.green.opacity(0.3) : Fam.hairline(0.06), lineWidth: 1)))
                                    .contentShape(Rectangle())
                                }.buttonStyle(.plain)
                            }
                        }
                    }
                }
            }
        }
    }
}

// MARK: - Shared visual components

/// The cycle as the design intends it — the 8 phases in a ring around the breathing marble.
struct CycleRing: View {
    let stages: [String]
    let active: Int
    var body: some View {
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

struct Marble: View {
    var size: CGFloat
    @State private var breathe = false
    var body: some View {
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

private struct AuroraBackground: View {
    @State private var drift = false
    var body: some View {
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

struct Panel<Content: View>: View {
    var radius: CGFloat = 24
    var fill: Double = 0.03
    @ViewBuilder var content: () -> Content
    var body: some View {
        content().padding(24).frame(maxWidth: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: radius).fill(Color.white.opacity(fill))
                .overlay(RoundedRectangle(cornerRadius: radius).stroke(Fam.hairline(0.07), lineWidth: 1)))
    }
}

struct MonoLabel: View {
    let text: String
    init(_ t: String) { text = t }
    var body: some View { Text(text).font(Fam.mono(10.5)).tracking(1.9).foregroundStyle(Fam.labelBlue.opacity(0.6)) }
}

struct SignalBar: View {
    let label: String; let value: Double; let color: Color
    init(_ label: String, _ value: Double, _ color: Color) { self.label = label; self.value = value; self.color = color }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack { Text(label).font(.system(size: 14, weight: .medium)); Spacer()
                Text(String(format: "%.2f", value)).font(Fam.mono(13)).foregroundStyle(color) }
            GeometryReader { geo in
                ZStack(alignment: .leading) {
                    Capsule().fill(Color.white.opacity(0.07)).frame(height: 6)
                    Capsule().fill(color).frame(width: max(6, geo.size.width * CGFloat(min(max(value, 0), 1))), height: 6).shadow(color: color.opacity(0.7), radius: 6)
                }
            }.frame(height: 6)
        }
    }
}

struct MeshConstellation: View {
    let members: [Member]
    private func color(_ k: Member.Kind) -> Color { switch k { case .self_node: return Fam.iceStat; case .gossip_peer: return Fam.blueBright; case .device_peer: return Fam.green; case .device_agent: return Fam.amber } }
    private func icon(_ m: Member) -> String {
        switch m.kind { case .self_node: return "house.fill"; case .gossip_peer: return "cpu"
        case .device_peer where m.actor.hasPrefix("ipad"): return "ipad"; case .device_peer where m.actor.hasPrefix("watch"): return "applewatch"; case .device_peer: return "iphone"
        case .device_agent where m.actor.hasPrefix("watch"): return "applewatch"; case .device_agent: return "iphone" }
    }
    var body: some View {
        GeometryReader { geo in
            let center = CGPoint(x: geo.size.width / 2, y: geo.size.height / 2)
            let radius = min(geo.size.width, geo.size.height) / 2 - 50
            let selfNode = members.first { $0.kind == .self_node }
            let others = members.filter { $0.kind != .self_node }
            ZStack {
                ForEach(Array(others.enumerated()), id: \.element.id) { i, m in
                    let p = point(center, radius, i, others.count)
                    Path { pt in pt.move(to: center); pt.addLine(to: p) }.stroke(color(m.kind).opacity(m.online ? 0.35 : 0.12), lineWidth: 1)
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
            let c = color(m.kind)
            VStack(spacing: 4) {
                ZStack {
                    Circle().fill(c.opacity(m.online ? 0.22 : 0.08)).frame(width: big ? 56 : 40, height: big ? 56 : 40)
                    Circle().stroke(c.opacity(m.online ? 0.9 : 0.4), lineWidth: 1.5).frame(width: big ? 56 : 40, height: big ? 56 : 40)
                    Image(systemName: icon(m)).font(.system(size: big ? 19 : 14)).foregroundStyle(c)
                }.shadow(color: m.online ? c.opacity(0.5) : .clear, radius: 8)
                Text(m.label.isEmpty ? String(m.node_id.prefix(6)) : m.label).font(Fam.mono(9)).foregroundStyle(Fam.ink.opacity(0.8)).lineLimit(1).frame(maxWidth: 90)
            }.position(x: p.x, y: p.y)
        }
    }
}

enum GlassTime {
    static func clock(_ ts: Int64) -> String {
        let f = DateFormatter(); f.dateFormat = "HH:mm"; return f.string(from: Date(timeIntervalSince1970: TimeInterval(ts)))
    }
    static func ago(_ ts: Int64) -> String {
        let s = Int64(Date().timeIntervalSince1970) - ts
        if s < 60 { return "\(s)s" }; if s < 3600 { return "\(s / 60)m" }; if s < 86400 { return "\(s / 3600)h" }; return "\(s / 86400)d"
    }
}
