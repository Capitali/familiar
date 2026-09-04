import SwiftUI
import FamiliarSC

/// Screen 2 — one ship's bridge: status and P&L, then the journal as a voiced timeline —
/// one BridgeReport per fold window, the templated floor now, the model's retelling of
/// the latest window when it is ready. Doors to the message window and the dial.
public struct ShipBridgeView: View {
    @Bindable var model: BridgeModel
    let world: String

    public init(model: BridgeModel, world: String) { self.model = model; self.world = world }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                if let e = model.error { Label(e, systemImage: "exclamationmark.triangle").foregroundStyle(SC.red).font(.footnote) }
                if let b = model.book { cards(b, model.summary) }
                if let s = model.summary { statusPanel(s) }
                if let b = model.book, !b.holdings.isEmpty { bookPanel(b) }
                doors
                Text("Today, in \(model.computerName)'s words").font(.caption).foregroundStyle(SC.dim).padding(.top, 4)
                if let spoken = model.spoken { reportCard(spoken.report, title: "\(model.computerName) · \(spoken.lane.rawValue)", note: spoken.note) }
                ForEach(model.reports) { fold in
                    reportCard(fold.report, title: "t\(fold.fromTick)–t\(fold.toTick)", note: nil)
                }
            }
            .padding()
        }
        .background(SC.bg)
        .navigationTitle(model.computerName)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    Task { await model.speakLatest() }
                } label: { Label("Ask her", systemImage: "waveform") }
            }
        }
        .task(id: world) { await model.open(world: world) }
        .refreshable { await model.open(world: world) }
    }

    func statusPanel(_ s: ShipSummary) -> some View {
        Panel {
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Image(systemName: SC.glyph(for: s.mood)).foregroundStyle(SC.color(for: s.mood))
                    Text(s.hull.isEmpty ? s.label : s.hull).font(.headline)
                    Spacer()
                    Chip(text: s.pilotAlive ? "pilot alive" : "NO PILOT", tint: s.pilotAlive ? SC.green : SC.red)
                }
                Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 4) {
                    GridRow { stat("credits", SC.money(s.credits)); stat("debt", SC.money(s.debt)) }
                    GridRow { stat("fuel", s.fuel.map { "\($0)" + (s.fuelCapacity.map { c in "/\(c)" } ?? "") } ?? "—"); stat("wear", s.wearBps.map { "\($0) bps" } ?? "—") }
                    GridRow { stat("where", s.docked ?? s.enRouteTo.map { "→ \($0)" } ?? "under way"); stat("captain", s.captain) }
                }
                Text("automations: " + (s.automations.isEmpty ? "none" : s.automations.joined(separator: ", "))).font(.caption).foregroundStyle(.secondary)
            }
        }
    }

    /// Trades / Freight / Lease — the canvas's three cards.
    func cards(_ b: ShipBook, _ s: ShipSummary?) -> some View {
        HStack(spacing: 10) {
            card("Freight", SC.money(b.freightPaid), "\(b.hauls) haul\(b.hauls == 1 ? "" : "s")")
            card("Positions", b.holdings.isEmpty ? "none" : SC.money(b.inventoryAtCost), b.holdings.isEmpty ? "nothing aboard" : "\(b.holdings.count) at cost")
            if let p = s?.leasePrincipal { card("Lease", SC.money(p), "principal left") } else if let d = s?.debt { card("Debt", SC.money(d), "on the wire") } else { card("Lease", "—", "not on the wire") }
        }
    }

    func card(_ label: String, _ value: String, _ sub: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(.caption2).foregroundStyle(SC.dim)
            Text(value).font(.title3.monospacedDigit().weight(.semibold)).foregroundStyle(SC.ink).lineLimit(1).minimumScaleFactor(0.7)
            Text(sub).font(.caption2).foregroundStyle(.secondary)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(SC.panel, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
    }

    func stat(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(label).font(.caption2).foregroundStyle(SC.dim)
            Text(value).font(.subheadline.monospacedDigit()).foregroundStyle(SC.ink)
        }
    }

    func bookPanel(_ b: ShipBook) -> some View {
        Panel {
            VStack(alignment: .leading, spacing: 6) {
                Text("The book").font(.caption).foregroundStyle(SC.dim)
                Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 4) {
                    GridRow { stat("hauls", "\(b.hauls)"); stat("freight paid", SC.money(b.freightPaid)); stat("booked", SC.money(b.freightBooked)) }
                    GridRow { stat("positions", "\(b.holdings.count)"); stat("inventory at cost", SC.money(b.inventoryAtCost)); Color.clear.gridCellUnsizedAxes([.horizontal, .vertical]) }
                }
                ForEach(b.holdings, id: \.good) { h in
                    Text("\(h.units) \(h.good) at \(h.avgCost), bound for \(h.sellTarget.isEmpty ? "wherever a bid clears" : h.sellTarget); sellable from t\(h.sellableAt)")
                        .font(.footnote.monospacedDigit()).foregroundStyle(SC.ice)
                }
                Text("Deliveries pay a fixed company share; a bought position rides under freight until the exchange's minimum hold passes.")
                    .font(.caption2).foregroundStyle(.secondary)
            }
        }
    }

    var doors: some View {
        HStack(spacing: 10) {
            NavigationLink { MessageWindowView(model: model) } label: {
                Label(model.openProposals > 0 ? "Messages · \(model.openProposals) waiting" : "Messages", systemImage: "text.bubble")
                    .frame(maxWidth: .infinity, minHeight: 44)
            }
            .buttonStyle(.borderedProminent).tint(model.openProposals > 0 ? SC.amber : SC.blue)
            NavigationLink { AutonomyDialView(model: model) } label: {
                Label("Dial", systemImage: "dial.medium").frame(maxWidth: .infinity, minHeight: 44)
            }
            .buttonStyle(.bordered).tint(SC.ice)
        }
    }

    func reportCard(_ r: BridgeReport, title: String, note: String?) -> some View {
        Panel {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Image(systemName: SC.glyph(for: r.mood)).foregroundStyle(SC.color(for: r.mood))
                    Text(title).font(.caption).foregroundStyle(SC.dim)
                    Spacer()
                    Chip(text: r.mood.rawValue, tint: SC.color(for: r.mood))
                }
                Text(r.headline).font(.body.weight(.medium)).foregroundStyle(SC.ink)
                ForEach(Array(r.facts.enumerated()), id: \.offset) { _, f in
                    Text("· " + f).font(.footnote.monospacedDigit()).foregroundStyle(SC.ice)
                }
                Text("→ " + r.nextAct).font(.footnote).foregroundStyle(SC.amber)
                if let n = note { Text(n).font(.caption2).foregroundStyle(.secondary) }
            }
        }
    }
}
