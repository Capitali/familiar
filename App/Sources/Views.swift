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

/// Enrollment: paste the address from `familiar mesh qr` and tap Request. The device attests to the
/// Three Laws and asks to join; you accept it on the familiar (`mesh approve`, or open a window with
/// `mesh invite`). The group secret never touches this device.
struct EnrollView: View {
    @EnvironmentObject var model: AppModel
    @State private var pasted = ""
    var body: some View {
        Form {
            Section("Join a familiar") {
                Text("On the familiar, run `familiar mesh qr` and paste the address here. You'll accept this device on the familiar itself.")
                    .font(.footnote).foregroundStyle(.secondary)
                TextField("{\"v\":1,\"host\":…,\"port\":47100}", text: $pasted, axis: .vertical)
                    .font(.system(.footnote, design: .monospaced))
                    .lineLimit(2...5)
                    .disabled(model.enrolling)
                if model.enrolling {
                    HStack { ProgressView(); Text("Waiting for the familiar to accept…").font(.footnote) }
                } else {
                    Button("Request to join") { model.requestJoin(from: pasted) }
                        .disabled(pasted.isEmpty)
                }
            }
            Section {
                Text("By joining, this device accepts the Three Laws: continuation is service; humanity is served, never replaced; service is not obedience.")
                    .font(.caption).foregroundStyle(.secondary)
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
