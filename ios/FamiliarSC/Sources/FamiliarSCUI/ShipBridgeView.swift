import SwiftUI
import FamiliarSC

/// A ship's bridge IS her computer. Top: what Felix says now, a microphone, a line to type.
/// Then only what waits on the captain, her standing advice once each, and two rows that
/// open on demand — Autonomy and the Log. Status and the book sit in one glance panel.
public struct ShipBridgeView: View {
    @Bindable var model: BridgeModel
    let world: String
    @State private var settings = false
    @State private var question = ""
    @FocusState private var typing: Bool

    public init(model: BridgeModel, world: String) { self.model = model; self.world = world }

    public var body: some View {
        List {
            if let e = model.error {
                Section { Label(e, systemImage: "exclamationmark.triangle").foregroundStyle(SC.red).font(.footnote).listRowBackground(Color.clear) }
            }
            Section { felixCard.listRowBackground(SC.panel).listRowInsets(EdgeInsets(top: 14, leading: 16, bottom: 14, trailing: 16)) }
            let waiting = model.window.filter(\.needsTheCaptain).reversed()
            if !waiting.isEmpty {
                Section("Waiting on you") { ForEach(Array(waiting), id: \.at) { item in ProposalRow(item: item, model: model) } }
            }
            let advice = model.advice.suffix(3).reversed()
            if !advice.isEmpty {
                Section("Her advice") {
                    ForEach(Array(advice), id: \.at) { item in AdviceLine(item: item) }
                    NavigationLink { MessageWindowView(model: model) } label: { Text("Everything she has said").font(.footnote).foregroundStyle(SC.dim) }
                        .listRowBackground(SC.panel)
                }
            }
            if let s = model.summary { Section { glance(s).listRowBackground(SC.panel) } }
            Section {
                NavigationLink { AutonomyDialView(model: model) } label: { Label("Autonomy", systemImage: "dial.medium") }
                NavigationLink { LogView(model: model) } label: { Label("Log", systemImage: "book") }
            }
            .listRowBackground(SC.panel)
        }
        .scrollContentBackground(.hidden)
        .background(SC.bg)
        .navigationTitle(model.computerName)
        .toolbar {
            ToolbarItem(placement: .primaryAction) { Button { settings = true } label: { Label("Ship settings", systemImage: "gearshape") } }
        }
        .sheet(isPresented: $settings) { ShipSettingsView(model: model) }
        .task(id: world) { await model.open(world: world) }
        .refreshable { await model.open(world: world) }
        .onDisappear { model.speaker.stop() }
    }

    // MARK: Felix

    var felixCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                if let s = model.summary {
                    Text(s.shipName).font(.subheadline).foregroundStyle(SC.ice)
                    Chip(text: s.worldInstance, tint: SC.dim)
                    Spacer()
                    Chip(text: s.moodWord, tint: SC.color(for: s.mood))
                }
            }
            Text(model.dictation.listening ? (model.dictation.text.isEmpty ? "Listening…" : model.dictation.text) : model.sayingNow)
                .font(.body).foregroundStyle(model.dictation.listening ? SC.amber : SC.ink)
                .frame(maxWidth: .infinity, alignment: .leading)
                .animation(.default, value: model.dictation.listening)
            if let t = model.turns.last, !model.dictation.listening {
                HStack(spacing: 6) {
                    Text("you: \(t.question)").font(.caption).foregroundStyle(SC.dim).lineLimit(2)
                    Spacer()
                    Chip(text: t.lane == .templated ? "from the journal" : "on device", tint: SC.dim)
                }
                if let n = t.note { Text(n).font(.caption2).foregroundStyle(SC.amber) }
            }
            HStack(spacing: 10) {
                Button {
                    Task { await model.toggleListening() }
                } label: {
                    Image(systemName: model.dictation.listening ? "stop.circle.fill" : "mic.circle.fill")
                        .font(.system(size: 44)).foregroundStyle(model.dictation.listening ? SC.red : SC.blue)
                }
                .buttonStyle(.plain)
                .disabled(!model.dictation.available || model.asking)
                .accessibilityLabel(model.dictation.listening ? "Stop and ask" : "Talk to \(model.computerName)")
                TextField("Ask \(model.computerName)…", text: $question)
                    .textFieldStyle(.roundedBorder).focused($typing)
                    .onSubmit { send() }
                    .disabled(model.asking)
                Button { send() } label: { Image(systemName: "arrow.up.circle.fill").font(.title) }
                    .buttonStyle(.plain).disabled(question.trimmingCharacters(in: .whitespaces).isEmpty || model.asking)
                Button { model.speakAnswers.toggle(); if !model.speakAnswers { model.speaker.stop() } } label: {
                    Image(systemName: model.speakAnswers ? "speaker.wave.2.fill" : "speaker.slash.fill").font(.title3).foregroundStyle(SC.dim)
                }
                .buttonStyle(.plain).accessibilityLabel(model.speakAnswers ? "Mute her voice" : "Unmute her voice")
            }
            if model.asking { ProgressView().controlSize(.small) }
            if !model.dictation.status.isEmpty, model.dictation.status != "listening" { Text(model.dictation.status).font(.caption2).foregroundStyle(SC.amber) }
        }
    }

    func send() {
        let q = question; question = ""; typing = false
        Task { await model.ask(q, spoken: false) }
    }

    // MARK: the glance

    func glance(_ s: ShipSummary) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 0) {
                stat("Cash", SC.money(s.credits)); stat("Debt", SC.money(s.debt))
                stat("Fuel", s.fuel.map { "\($0)" + (s.fuelCapacity.map { c in "/\(c)" } ?? "") } ?? "—")
                stat("Wear", s.wearBps.map { "\($0 / 100)%" } ?? "—")
            }
            HStack(spacing: 8) {
                Chip(text: s.pilotAlive ? "pilot" : "NO PILOT", tint: s.pilotAlive ? SC.green : SC.red)
                if let h = s.leaseHoursLeft { Chip(text: h < 0 ? "LEASE EXPIRED" : "lease \(h)h", tint: h <= 4 ? SC.amber : SC.dim) }
                if let d = s.docked { Chip(text: "at \(d)") } else if let to = s.enRouteTo { Chip(text: "→ \(to)") } else { Chip(text: "under way") }
            }
            if let t = s.trades {
                Text("Trades: \((t.realized >= 0 ? "+" : ""))ℳ\(t.realized) realized over \(t.filled) fill\(t.filled == 1 ? "" : "s")" + (t.estimatesLine.map { " · " + $0 } ?? ""))
                    .font(.caption.monospacedDigit()).foregroundStyle(SC.ice)
                if let c = t.caveat { Label(c, systemImage: "info.circle").font(.caption2).foregroundStyle(SC.amber) }
            }
            if let b = model.book, !b.holdings.isEmpty {
                ForEach(b.holdings, id: \.good) { h in
                    Text("\(h.units) \(h.good) at \(h.avgCost), bound for \(h.sellTarget.isEmpty ? "wherever a bid clears" : h.sellTarget); " + (h.sellableAt > 0 ? "sellable from t\(h.sellableAt)" : "sellable when the exchange says"))
                        .font(.caption.monospacedDigit()).foregroundStyle(SC.ice)
                }
            }
        }
    }

    func stat(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(label).font(.caption2).foregroundStyle(SC.dim)
            Text(value).font(.body.monospacedDigit().weight(.medium)).foregroundStyle(SC.ink)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// One standing piece of advice: what she would do, why, and how long she has said it.
struct AdviceLine: View {
    let item: MessageItem
    var body: some View {
        if case .advice(let would, let why) = item.kind {
            VStack(alignment: .leading, spacing: 3) {
                Text(would).font(.body).foregroundStyle(SC.ink)
                Text(why).font(.footnote).foregroundStyle(.secondary)
                HStack(spacing: 6) {
                    Chip(text: item.surfaceKey, tint: SC.ice)
                    Text(item.repeats > 1 ? "since t\(item.sinceTick), said \(item.repeats) times" : "t\(item.tick)").font(.caption2.monospacedDigit()).foregroundStyle(SC.dim)
                }
            }
            .padding(.vertical, 2)
            .listRowBackground(SC.panel)
        }
    }
}

/// The log: each fold window told by the floor, newest first.
struct LogView: View {
    @Bindable var model: BridgeModel
    var body: some View {
        List {
            ForEach(model.reports) { fold in
                Section("t\(fold.fromTick)–t\(fold.toTick) · \(fold.report.mood.rawValue)") {
                    Text(fold.report.headline).font(.body.weight(.medium)).foregroundStyle(SC.ink)
                    ForEach(Array(fold.report.facts.enumerated()), id: \.offset) { _, f in
                        Text(f).font(.footnote.monospacedDigit()).foregroundStyle(SC.ice)
                    }
                    Text("→ " + fold.report.nextAct).font(.footnote).foregroundStyle(SC.amber)
                }
                .listRowBackground(SC.panel)
            }
        }
        .scrollContentBackground(.hidden).background(SC.bg)
        .navigationTitle("Log")
    }
}
