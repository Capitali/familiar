import SwiftUI
import FamiliarMesh

@main
struct FamiliarAgentApp: App {
    @StateObject private var model = AppModel()
    var body: some Scene {
        WindowGroup { RootView().environmentObject(model) }
    }
}

struct RootView: View {
    @EnvironmentObject var model: AppModel
    var body: some View {
        NavigationStack {
            if model.enrolled { StatusView() } else { EnrollView() }
        }
    }
}

/// Enrollment: paste the payload from `familiar mesh qr` (QR camera-scan is a follow-up). The
/// group secret it carries is what lets this device mint its membership cert.
struct EnrollView: View {
    @EnvironmentObject var model: AppModel
    @State private var pasted = ""
    var body: some View {
        Form {
            Section("Enroll this device") {
                Text("On the familiar, run `familiar mesh qr` and paste the payload here (QR scan coming next).")
                    .font(.footnote).foregroundStyle(.secondary)
                TextField("{\"v\":1,\"secret\":…}", text: $pasted, axis: .vertical)
                    .font(.system(.footnote, design: .monospaced))
                    .lineLimit(3...6)
                Button("Enroll") { model.enroll(from: pasted) }
                    .disabled(pasted.isEmpty)
            }
            if !model.log.isEmpty {
                Section("Activity") { ForEach(model.log.prefix(6), id: \.self) { Text($0).font(.footnote) } }
            }
        }
        .navigationTitle("Familiar Agent")
    }
}

/// Post-enrollment: consent switches, a home anchor, live counts, and the activity log.
struct StatusView: View {
    @EnvironmentObject var model: AppModel
    var body: some View {
        Form {
            Section("Connected") {
                LabeledContent("Group", value: model.groupLabel)
                LabeledContent("Familiar", value: model.host)
                LabeledContent("Sent", value: "\(model.sentCount)")
            }
            Section("What this device shares (derived only)") {
                Toggle("Location — home / away", isOn: $model.locationEnabled)
                    .onChange(of: model.locationEnabled) { _ in model.startSensingIfConsented() }
                Toggle("Motion — walking / driving / still", isOn: $model.motionEnabled)
                    .onChange(of: model.motionEnabled) { _ in model.startSensingIfConsented() }
                Button("Set “home” to my current location") { model.setHomeToCurrentLocation() }
            }
            Section("Activity") {
                ForEach(model.log.prefix(20), id: \.self) { Text($0).font(.footnote) }
            }
            Section {
                Button("Unenroll this device", role: .destructive) { model.unenroll() }
            }
        }
        .navigationTitle("Familiar Agent")
        .onAppear { model.startSensingIfConsented() }
    }
}
