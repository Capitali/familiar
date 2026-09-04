import SwiftUI
import FamiliarSC

/// Screen 4 — the autonomy dial: one segmented advise / confirm / auto per category,
/// grouped by family, with a family-level override; `navigation.rescue` defaults to advise.
/// Only bought automations appear (the grant model beneath the dial). Saving is the
/// captain's act: the whole object is written, then re-read.
public struct AutonomyDialView: View {
    @Bindable var model: BridgeModel
    @State private var draft = AutonomyDial()
    @State private var dirty = false

    public init(model: BridgeModel) { self.model = model }

    /// The families whose automation the captain has bought — racing has no grant yet.
    var families: [String] {
        let bought = Set(model.dial?.bought ?? [])
        return ControlSurface.families.filter { fam in
            ControlSurface.allCases.contains { $0.family == fam && ($0.automation.map(bought.contains) ?? false) }
        }
    }

    public var body: some View {
        Form {
            Section {
                Text("What \(model.computerName) may do on her own. Advise says it; confirm asks you; auto does it and tells you.")
                    .font(.footnote).foregroundStyle(.secondary).listRowBackground(Color.clear)
            }
            if case .malformed(let why)? = model.dial?.loaded {
                Section {
                    Label("autonomy.json is malformed (\(why)). The pilot reads it as ABSENT — auto everywhere. Saving here rewrites it.", systemImage: "exclamationmark.triangle")
                        .font(.footnote).foregroundStyle(SC.red)
                }
            }
            if case .absent? = model.dial?.loaded, !dirty {
                Section { Text("No dial yet: everything bought runs on auto; the tanker advises.").font(.footnote).foregroundStyle(.secondary) }
            }
            Section {
                row(key: "*", label: "Everything", subtitle: "the default under every family")
            }
            ForEach(families, id: \.self) { fam in
                Section(fam.capitalized) {
                    row(key: fam, label: "All \(fam)", subtitle: "family override")
                    ForEach(ControlSurface.allCases.filter { $0.family == fam }, id: \.rawValue) { s in
                        row(key: s.key, label: s.category, subtitle: subtitle(for: s))
                    }
                }
            }
            if families.isEmpty {
                Section { Text("Nothing bought yet — the dial has no surfaces to set.").font(.footnote).foregroundStyle(.secondary) }
            }
        }
        .scrollContentBackground(.hidden)
        .background(SC.bg)
        .navigationTitle("Autonomy")
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Save") { Task { await model.save(dial: draft); dirty = false } }.disabled(!dirty)
            }
        }
        .onAppear { draft = model.dial?.loaded.dial ?? AutonomyDial(); dirty = false }
        .onChange(of: model.dial) { _, new in if !dirty { draft = new?.loaded.dial ?? AutonomyDial() } }
    }

    func subtitle(for s: ControlSurface) -> String {
        switch s {
        case .navigationRescue: return "the tanker — a PAWS call is a multi-day strand; advise unless you say otherwise"
        case .navigationCourse: return "travel, engage, carry legs"
        case .navigationFuel: return "refuel, divert to a pump"
        case .freightBook: return "book a load"
        case .freightCollect: return "collect at the origin"
        case .freightCancel: return "cancel a booking"
        case .marketBuy: return "open a position"
        case .marketSell: return "sell a position"
        case .marketCarry: return "carry a position to market"
        case .shipRepair: return "yard repair"
        case .shipRefit: return "buy a fitting"
        case .shipCrew: return "hire after title"
        case .shipFrame: return "a bigger frame"
        case .shipLease: return "the buyout"
        case .racingPlot: return "lay a course"
        case .racingLine: return "risk past the safe line"
        case .racingRefusal: return "never auto"
        }
    }

    /// One row: the setting for `key` (or "inherit"), with the effective level shown for a
    /// surface when it inherits.
    func row(key: String, label: String, subtitle: String) -> some View {
        let binding = Binding<String>(
            get: { draft.settings[key]?.rawValue ?? "inherit" },
            set: { v in
                if v == "inherit" { draft.settings.removeValue(forKey: key) } else if let l = AutonomyLevel(rawValue: v) { _ = draft.set(key, l) }
                dirty = true
            }
        )
        return VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(label).font(.body.weight(.medium)).foregroundStyle(SC.ink)
                Spacer()
                if let s = ControlSurface.parse(key), draft.settings[key] == nil {
                    Chip(text: "→ \(draft.level(for: s).rawValue)", tint: SC.dim)
                }
            }
            Text(subtitle).font(.caption).foregroundStyle(.secondary)
            Picker(label, selection: binding) {
                Text("inherit").tag("inherit")
                Text("advise").tag("advise")
                Text("confirm").tag("confirm")
                Text("auto").tag("auto")
            }
            .pickerStyle(.segmented)
            .labelsHidden()
        }
        .padding(.vertical, 4)
        .listRowBackground(SC.panel)
    }
}
