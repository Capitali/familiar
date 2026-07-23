import SwiftUI

/// The consent-gate surface the Mac console has never had (SPEC.md R9) — a real, human-facing
/// preferences window (⌘,) that reads/writes the same `boundary.json` the Rust core enforces,
/// mirroring the read-only `GateStates` every peer already sees on the worldview, but writable
/// here because this IS the human's own interface — the sanctioned "human edits the file" path.
struct MacConsentSettings: View {
    @EnvironmentObject var bridge: SphereBridge
    @State private var gates = MacBoundary.load()

    var body: some View {
        Form {
            Section("What this familiar may sense") {
                Toggle("Location", isOn: binding(\.allow_location))
                Toggle("Microphone (push-to-talk)", isOn: binding(\.allow_microphone))
                Toggle("Network discovery", isOn: binding(\.allow_network_discovery))
                Toggle("Camera", isOn: binding(\.allow_camera))
                Toggle("Face recognition", isOn: binding(\.allow_face_recognition))
                    .disabled(!gates.allow_camera)
                    .help(gates.allow_camera ? "" : "Requires camera to be enabled first")
            }
            if bridge.face.needsIdentification || bridge.face.proposedHandle != nil {
                Section("Who's this?") {
                    MacFaceIdentifyPrompt(face: bridge.face)
                }
            }
            Text("Off by default. Each toggle opens the same gate the mesh already shows read-only to peers — this window is the one place it's actually writable, because it's you.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(20)
        .frame(width: 420)
        .onAppear { gates = MacBoundary.load() }
    }

    private func binding(_ key: WritableKeyPath<MacBoundary.Gates, Bool>) -> Binding<Bool> {
        Binding(
            get: { gates[keyPath: key] },
            set: { newValue in
                gates[keyPath: key] = newValue
                MacBoundary.set { $0[keyPath: key] = newValue }
            }
        )
    }
}

/// The interactive-identification fallback, macOS side (SPEC.md R10 hard requirement) — mirrors
/// iOS's FaceIdentifyPrompt. Shown inline in the settings window since the Mac console's web
/// layer isn't being touched for this (see MacFaceSensing.swift's doc comment).
struct MacFaceIdentifyPrompt: View {
    @ObservedObject var face: MacFaceSensing
    @State private var typed = ""

    var body: some View {
        if let proposed = face.proposedHandle {
            Text("Is this \(proposed)?").font(.caption)
            HStack {
                Button("Yes") { face.confirmIdentity(handle: proposed) }
                Button("No — someone else") { face.proposedHandle = nil; face.needsIdentification = true }
            }
        } else {
            Text("I don't recognize this person yet — who is it?").font(.caption)
            HStack {
                TextField("Name", text: $typed).textFieldStyle(.roundedBorder)
                Button("Confirm") {
                    let name = typed.trimmingCharacters(in: .whitespaces)
                    guard !name.isEmpty else { return }
                    face.confirmIdentity(handle: name)
                    typed = ""
                }
            }
        }
    }
}
