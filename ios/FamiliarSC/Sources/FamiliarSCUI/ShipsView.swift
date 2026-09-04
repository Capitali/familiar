import SwiftUI
import FamiliarSC

/// Screen 1 — the captain's fleet: each row the computer's OWN name, her mood, and the hull
/// glance (what `fleet status` prints). Tap a row for her bridge.
public struct ShipsView: View {
    @Bindable var model: BridgeModel
    let scanner: PairingScanner?
    @State private var pairing = false

    public init(model: BridgeModel, scanner: PairingScanner? = nil) {
        self.model = model; self.scanner = scanner
    }

    public var body: some View {
        List {
            if let e = model.error {
                Section { Label(e, systemImage: "exclamationmark.triangle").foregroundStyle(SC.red).font(.footnote) }
            }
            if model.ships.isEmpty && !model.loading {
                Section {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("No ship paired.").font(.headline)
                        Text("Pair a hull with its exchange key and her computer is born — Purr, until you name her.")
                            .font(.footnote).foregroundStyle(.secondary)
                    }
                }
            }
            ForEach(model.ships) { ship in
                NavigationLink(value: ship.world) { ShipRow(ship: ship) }
                    .listRowBackground(SC.panel)
            }
        }
        .scrollContentBackground(.hidden)
        .background(SC.bg)
        .navigationTitle("Your ships")
        .navigationDestination(for: String.self) { world in ShipBridgeView(model: model, world: world) }
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button { pairing = true } label: { Label("Pair a ship", systemImage: "plus") }
            }
        }
        .sheet(isPresented: $pairing) { PairingView(model: model, scanner: scanner) }
        .refreshable { await model.refreshShips() }
        .task { await model.refreshShips() }
    }
}

struct ShipRow: View {
    let ship: ShipSummary
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(ship.computer).font(.title3.weight(.semibold)).foregroundStyle(ship.named ? SC.ink : SC.dim)
                Spacer()
                Chip(text: ship.moodWord, tint: SC.color(for: ship.mood))
            }
            HStack(spacing: 8) {
                Text(ship.shipName).font(.subheadline).foregroundStyle(SC.ice)
                Chip(text: ship.worldInstance, tint: SC.dim)
            }
            if !ship.sentence.isEmpty {
                Text(ship.sentence).font(.footnote).foregroundStyle(SC.ink.opacity(0.85)).lineLimit(3)
            }
            HStack(spacing: 0) {
                stat("Cash", ship.credits.map { "\($0)" } ?? "—")
                stat("Fuel", ship.fuel.map { "\($0)" } ?? "—", suffix: ship.fuelCapacity.map { "/\($0)" })
                stat("Wear", ship.wearBps.map { "\($0 / 100)%" } ?? "—")
                stat("Waiting", "\(ship.openProposals)", tint: ship.openProposals > 0 ? SC.amber : nil)
            }
            HStack(spacing: 8) {
                Chip(text: ship.pilotAlive ? "pilot" : "NO PILOT", tint: ship.pilotAlive ? SC.green : SC.red)
                if let h = ship.leaseHoursLeft { Chip(text: h < 0 ? "LEASE EXPIRED" : "lease \(h)h", tint: h <= 4 ? SC.amber : SC.dim) }
                if let d = ship.docked { Chip(text: "at \(d)") } else if let to = ship.enRouteTo { Chip(text: "→ \(to)") } else { Chip(text: "under way") }
            }
        }
        .padding(.vertical, 6)
        .accessibilityElement(children: .combine)
    }

    func stat(_ label: String, _ value: String, suffix: String? = nil, tint: Color? = nil) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(label).font(.caption2).foregroundStyle(SC.dim)
            HStack(alignment: .firstTextBaseline, spacing: 0) {
                Text(value).font(.body.monospacedDigit().weight(.medium)).foregroundStyle(tint ?? SC.ink)
                if let s = suffix { Text(s).font(.caption.monospacedDigit()).foregroundStyle(SC.dim) }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
