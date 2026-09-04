import SwiftUI
import FamiliarSC

/// The ship's paperwork, editable from the bridge (Ian, 2026-09-04: "Should have the ability
/// to edit/update these within the UI"): the computer's name (one per captain — renaming
/// here renames her for every ship), what the pilot may do, which captain the ship flies
/// for, and unpairing. Every save is the captain's act through `CaptainActs`.
public struct ShipSettingsView: View {
    @Bindable var model: BridgeModel
    @Environment(\.dismiss) private var dismiss

    @State private var computer = ""
    @State private var captain = ""
    @State private var freight = false
    @State private var trade = false
    @State private var outfit = false
    @State private var busy = false
    @State private var outcome: BridgeModel.ActOutcome?
    @State private var confirmUnpair = false

    public init(model: BridgeModel) { self.model = model }

    var summary: ShipSummary? { model.summary }
    var boughtNow: [Automation] {
        var a: [Automation] = []
        if freight { a.append(.freight) }
        if trade { a.append(.trade) }
        if outfit { a.append(.outfit) }
        return a
    }
    var automationsChanged: Bool {
        Set(boughtNow.map(\.rawValue)) != Set(summary?.automations ?? [])
    }

    public var body: some View {
        NavigationStack {
            Form {
                if let s = summary {
                    Section("The computer") {
                        TextField(s.named ? s.computer : Persona.rootName, text: $computer)
                        Text("One computer per captain: this renames \(s.named ? s.computer : "her") for every ship \(s.captain.isEmpty ? "you fly" : s.captain + " flies").").font(.caption).foregroundStyle(.secondary)
                        Button("Rename") { act { await model.rename(computer: computer.trimmingCharacters(in: .whitespaces)) } }
                            .disabled(busy || computer.trimmingCharacters(in: .whitespaces).isEmpty || computer == s.computer)
                    }
                    Section("Her voice") {
                        VoicePicker(speaker: model.speaker)
                    }
                    Section("What the pilot may do") {
                        Toggle(isOn: $freight) { VStack(alignment: .leading) { Text("Freight"); Text("book, fly, collect, refuel, repair").font(.caption).foregroundStyle(.secondary) } }
                        Toggle(isOn: $trade) { VStack(alignment: .leading) { Text("Trade"); Text("buy where cheap, ride, sell where dear").font(.caption).foregroundStyle(.secondary) } }
                        Toggle(isOn: $outfit) { VStack(alignment: .leading) { Text("Outfit"); Text("fittings out of earnings, crew after title").font(.caption).foregroundStyle(.secondary) } }
                        Text("A scope your key holds. What is on here appears on the Dial; a grant takes effect when the pilot next starts.").font(.caption).foregroundStyle(.secondary)
                        Button("Save automations") { act { await model.setAutomations(boughtNow) } }
                            .disabled(busy || !automationsChanged)
                    }
                    Section("The captain") {
                        TextField("captain", text: $captain)
                        Text("The record this ship flies under. Ships under one captain share one computer and one purse; a test rig keeps its own.").font(.caption).foregroundStyle(.secondary)
                        Button("Move ship to this captain") { act { await model.setCaptain(captain.trimmingCharacters(in: .whitespaces)) } }
                            .disabled(busy || captain.trimmingCharacters(in: .whitespaces).isEmpty || captain == s.captain)
                    }
                    Section {
                        LabeledContent("World", value: s.world).font(.caption.monospaced())
                        LabeledContent("Exchange", value: s.server).font(.caption)
                        Button("Unpair this ship", role: .destructive) { confirmUnpair = true }
                            .confirmationDialog("Unpair \(s.hull.isEmpty ? s.label : s.hull)?", isPresented: $confirmUnpair, titleVisibility: .visible) {
                                Button("Unpair — stop the pilot, destroy the key", role: .destructive) {
                                    act { let e = await model.unpair(world: s.world); if e == nil { model.world = nil; dismiss() }; return BridgeModel.ActOutcome(ok: e == nil, text: e ?? "Unpaired.") }
                                }
                            } message: { Text("The journal, the delivery record and the computer's persona stay for you. The key is destroyed.") }
                    }
                }
                if let o = outcome { Section { Label(o.text, systemImage: o.ok ? "checkmark.circle" : "exclamationmark.triangle").font(.footnote).foregroundStyle(o.ok ? SC.green : SC.amber) } }
            }
            .scrollContentBackground(.hidden)
            .background(SC.bg)
            .navigationTitle(summary.map { $0.hull.isEmpty ? $0.label : $0.hull } ?? "Ship")
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
            .onAppear(perform: seed)
            .onChange(of: model.ships) { _, _ in seed() }
        }
    }

    func seed() {
        guard let s = summary else { return }
        captain = s.captain
        let a = Set(s.automations)
        freight = a.contains("freight"); trade = a.contains("trade"); outfit = a.contains("outfit")
    }

    func act(_ f: @escaping () async -> BridgeModel.ActOutcome) {
        busy = true; outcome = nil
        Task { outcome = await f(); busy = false }
    }
}


import AVFoundation

/// Pick the voice she speaks with, from what the device has installed; try it aloud.
struct VoicePicker: View {
    let speaker: Speaker
    @AppStorage(Speaker.chosenVoiceKey) private var chosen = ""
    private let voices = Speaker.candidates()

    var body: some View {
        Picker("Voice", selection: $chosen) {
            Text("Best installed").tag("")
            ForEach(voices, id: \.identifier) { v in
                Text("\(v.name) · \(Speaker.qualityWord(v)) · \(v.language)").tag(v.identifier)
            }
        }
        Button("Try it") { speaker.speak("Captain, this is how I sound. Fuel one hundred thirty-five of six hundred, berthed at titania cold store.") }
        if !voices.contains(where: { $0.quality == .premium || $0.quality == .enhanced }) {
            Text("Only compact voices are installed. For a natural one, download a premium or enhanced English voice in Settings → Accessibility → Spoken Content → Voices, then pick it here.")
                .font(.caption).foregroundStyle(.secondary)
        }
    }
}
