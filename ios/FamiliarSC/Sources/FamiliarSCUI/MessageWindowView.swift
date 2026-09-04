import SwiftUI
import FamiliarSC

/// Screen 3 — the message window, the product's centre: the open proposals first (Approve /
/// Deny, each ≥44pt), then the advice, then the record. Every tap is the captain's act; the
/// screen re-reads the store after it and shows what the store says.
public struct MessageWindowView: View {
    @Bindable var model: BridgeModel

    public init(model: BridgeModel) { self.model = model }

    var open: [MessageItem] { model.window.filter(\.needsTheCaptain).reversed() }
    var advice: [MessageItem] { model.window.filter { if case .advice = $0.kind { return true }; return false }.reversed() }
    var record: [MessageItem] { model.window.filter { if case .proposal(_, _, _, _, let st) = $0.kind { return st != .open }; return false }.reversed() }

    public var body: some View {
        List {
            if model.window.isEmpty {
                Section { Text("Nothing to say yet. Advice and proposals appear here as the pilot writes them.").font(.footnote).foregroundStyle(.secondary) }
            }
            if !open.isEmpty {
                Section("Waiting on you") { ForEach(open, id: \.at) { item in ProposalRow(item: item, model: model) } }
            }
            if !advice.isEmpty {
                Section("She would have…") { ForEach(advice, id: \.at) { item in AdviceRow(item: item) } }
            }
            if !record.isEmpty {
                Section("The record") { ForEach(record, id: \.at) { item in ProposalRow(item: item, model: model) } }
            }
        }
        .scrollContentBackground(.hidden)
        .background(SC.bg)
        .navigationTitle("Messages")
    }
}

struct AdviceRow: View {
    let item: MessageItem
    var body: some View {
        if case .advice(let would, let why) = item.kind {
            VStack(alignment: .leading, spacing: 4) {
                HStack { Chip(text: item.surfaceKey, tint: SC.ice); Spacer(); Text("t\(item.tick)").font(.caption2.monospacedDigit()).foregroundStyle(SC.dim) }
                Text(would).font(.body).foregroundStyle(SC.ink)
                Text(why).font(.footnote).foregroundStyle(.secondary)
            }
            .padding(.vertical, 4)
            .listRowBackground(SC.panel)
        }
    }
}

struct ProposalRow: View {
    let item: MessageItem
    @Bindable var model: BridgeModel
    @State private var busy = false

    var body: some View {
        if case .proposal(let id, let would, let why, let expires, let state) = item.kind {
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Chip(text: item.surfaceKey, tint: SC.ice)
                    Spacer()
                    stateChip(state, expires: expires)
                }
                Text(would).font(.body.weight(.medium)).foregroundStyle(SC.ink)
                Text(why).font(.footnote).foregroundStyle(.secondary)
                if state == .open {
                    HStack(spacing: 10) {
                        Button { act(id, true) } label: { Label("Approve", systemImage: "checkmark").frame(maxWidth: .infinity, minHeight: 44) }
                            .buttonStyle(.borderedProminent).tint(SC.green)
                        Button { act(id, false) } label: { Label("Deny", systemImage: "xmark").frame(maxWidth: .infinity, minHeight: 44) }
                            .buttonStyle(.bordered).tint(SC.red)
                    }
                    .disabled(busy)
                    Text("Lapses after t\(expires) if you say nothing.").font(.caption2).foregroundStyle(SC.dim)
                }
                Text("t\(item.tick) · \(id)").font(.caption2.monospacedDigit()).foregroundStyle(SC.dim)
            }
            .padding(.vertical, 4)
            .listRowBackground(SC.panel)
        }
    }

    func act(_ id: String, _ approved: Bool) {
        busy = true
        Task { await model.approve(id: id, approved: approved); busy = false }
    }

    @ViewBuilder
    func stateChip(_ s: MessageItem.ProposalState, expires: Int64) -> some View {
        switch s {
        case .open: Chip(text: "open until t\(expires)", tint: SC.amber)
        case .approved: Chip(text: "approved", tint: SC.green)
        case .denied: Chip(text: "denied", tint: SC.red)
        case .lapsed: Chip(text: "lapsed", tint: SC.dim)
        }
    }
}
