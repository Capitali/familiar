import SwiftUI
import Charts
import FamiliarSC

/// Money over time — the captain's economic history and profit, pooled across his hulls
/// (T-241). Read-only: the host's points, the host's flows, the host's sentences. The trend
/// is a straight line through what happened, and the screen says so.
struct EconomyView: View {
    @Bindable var model: BridgeModel
    @State private var perHull = false

    var body: some View {
        List {
            Section {
                Picker("Window", selection: Binding(get: { model.economyWindow }, set: { w in Task { await model.loadEconomy(window: w) } })) {
                    ForEach(CaptainEconomy.windows, id: \.self) { Text($0).tag($0) }
                }
                .pickerStyle(.segmented)
                .listRowBackground(Color.clear)
            }
            if model.loadingEconomy, model.economy == nil {
                Section { Label("reading the captain's ledger…", systemImage: "hourglass").font(.footnote).foregroundStyle(SC.dim).listRowBackground(SC.panel) }
            }
            if let e = model.economyError {
                Section { Label(e, systemImage: "exclamationmark.triangle").font(.footnote).foregroundStyle(SC.amber).listRowBackground(SC.panel) }
            }
            if let e = model.economy {
                let h = e.pooled
                Section("\(e.captain.isEmpty ? "The captain" : e.captain) — ℳ\(h.summary.creditsStart) → ℳ\(h.summary.creditsNow)") {
                    VStack(alignment: .leading, spacing: 10) {
                        HStack(spacing: 0) {
                            stat("Change", signed(h.summary.delta))
                            stat("Trend", String(format: "%@ℳ%.0f/day", h.summary.trendPerDay >= 0 ? "+" : "−", abs(h.summary.trendPerDay)))
                            stat("Earned", signed(h.flows.earned)); stat("Spent", signed(h.flows.spent))
                        }
                        trend(e)
                        if e.hulls.count > 1 {
                            Toggle("Each hull on its own line", isOn: $perHull).font(.caption).foregroundStyle(SC.dim).tint(SC.blue)
                        }
                        Text("A straight line through \(h.summary.readings) reading\(h.summary.readings == 1 ? "" : "s") over \(String(format: "%.1f", h.summary.windowDays)) days — the trend is what happened, not a forecast.")
                            .font(.caption2).foregroundStyle(SC.dim)
                    }
                    .listRowBackground(SC.panel)
                }
                if !h.flows.bars.isEmpty {
                    Section("Where the money went") {
                        bars(h.flows)
                            .listRowBackground(SC.panel)
                        if !h.sourceWords.isEmpty { Text(h.sourceWords).font(.caption2).foregroundStyle(SC.dim).listRowBackground(SC.panel) }
                    }
                }
                if !h.analysis.isEmpty {
                    Section("In summary") {
                        ForEach(Array(h.analysis.enumerated()), id: \.offset) { _, line in
                            Text(line).font(.footnote).foregroundStyle(SC.ink)
                        }
                        if let b = h.summary.best { move("Best move", b) }
                        if let w = h.summary.worst, w != h.summary.best { move("Worst move", w) }
                    }
                    .listRowBackground(SC.panel)
                }
                if e.hulls.count > 1 {
                    Section("By hull") {
                        ForEach(e.hulls) { hull in
                            VStack(alignment: .leading, spacing: 2) {
                                HStack { Text(hull.label ?? hull.world ?? "?").font(.subheadline).foregroundStyle(SC.ice); Spacer(); Text(signed(hull.summary.delta)).font(.subheadline.monospacedDigit()).foregroundStyle(hull.summary.delta >= 0 ? SC.green : SC.red) }
                                if let first = hull.analysis.first { Text(first).font(.caption2).foregroundStyle(SC.dim) }
                            }
                        }
                    }
                    .listRowBackground(SC.panel)
                }
            }
        }
        .scrollContentBackground(.hidden)
        .background(SC.bg)
        .navigationTitle("Money over time")
        .task(id: model.world) { if model.economy == nil { await model.loadEconomy() } }
        .refreshable { await model.loadEconomy() }
    }

    // MARK: pieces

    @ViewBuilder
    func trend(_ e: CaptainEconomy) -> some View {
        let series: [EconomyHistory] = perHull && e.hulls.count > 1 ? e.hulls : [e.pooled]
        if series.allSatisfy({ $0.points.isEmpty }) {
            Text("no readings in this window").font(.caption).foregroundStyle(SC.dim)
        } else {
            Chart {
                ForEach(series) { h in
                    ForEach(h.points, id: \.at) { p in
                        LineMark(x: .value("when", p.date), y: .value("ℳ", p.credits))
                            .foregroundStyle(by: .value("hull", h.label ?? (h.world == nil ? "pooled" : h.world!)))
                            .interpolationMethod(.stepEnd)
                    }
                }
            }
            .chartLegend(series.count > 1 ? .visible : .hidden)
            .chartYAxisLabel("ℳ")
            .frame(height: 180)
        }
    }

    func bars(_ f: EconomyFlows) -> some View {
        Chart(f.bars) { b in
            BarMark(x: .value("ℳ", b.amount), y: .value("cause", b.cause))
                .foregroundStyle(b.amount >= 0 ? SC.green : SC.red)
                .annotation(position: b.amount >= 0 ? .trailing : .leading) {
                    Text(signed(b.amount)).font(.caption2.monospacedDigit()).foregroundStyle(SC.ice)
                }
        }
        .chartXAxis(.hidden)
        .frame(height: CGFloat(max(1, f.bars.count)) * 26 + 12)
    }

    func move(_ title: String, _ m: EconomyMove) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(title).font(.caption).foregroundStyle(SC.dim)
            Text("\(signed(m.delta)) — \(m.cause) (t\(m.tick))").font(.caption.monospacedDigit()).foregroundStyle(m.delta >= 0 ? SC.green : SC.red)
        }
    }

    func stat(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label).font(.caption2).foregroundStyle(SC.dim)
            Text(value).font(.subheadline.monospacedDigit()).foregroundStyle(SC.ink)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    func signed(_ n: Int64) -> String { n >= 0 ? "+ℳ\(n)" : "−ℳ\(-n)" }
}
