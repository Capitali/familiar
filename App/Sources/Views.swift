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
    @State private var scanning = false
    var body: some View {
        Form {
            Section("Join a familiar") {
                Text("Scan the familiar's QR (from the Glass or another member's device), or paste its address. You'll accept this device on the familiar itself.")
                    .font(.footnote).foregroundStyle(.secondary)
                if model.enrolling {
                    HStack { ProgressView(); Text("Waiting for the familiar to accept…").font(.footnote) }
                } else {
                    Button {
                        scanning = true
                    } label: {
                        Label("Scan QR to join", systemImage: "qrcode.viewfinder")
                    }
                    DisclosureGroup("…or paste the address") {
                        TextField("{\"v\":1,\"host\":…,\"port\":47100}", text: $pasted, axis: .vertical)
                            .font(.system(.footnote, design: .monospaced))
                            .lineLimit(2...5)
                        Button("Request to join") { model.requestJoin(from: pasted) }
                            .disabled(pasted.isEmpty)
                    }
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
        .sheet(isPresented: $scanning) {
            QRScannerView { code in
                scanning = false
                model.requestJoin(from: code)
            }
            .ignoresSafeArea()
        }
    }
}

/// Post-enrollment: consent switches, a home anchor, live counts, the activity log, and a join QR
/// so this member becomes a scan-to-join point for the next device.
struct StatusView: View {
    @EnvironmentObject var model: AppModel
    @State private var showJoinQR = false
    var body: some View {
        Form {
            Section("Connected") {
                LabeledContent("Group", value: model.groupLabel)
                LabeledContent("Familiar", value: model.host)
                LabeledContent("Sent", value: "\(model.sentCount)")
            }
            Section("Invite another device") {
                Text("Show this QR for a new device to scan — it joins this familiar directly (you accept it on the familiar). It carries only the address, no secret.")
                    .font(.caption).foregroundStyle(.secondary)
                Button {
                    showJoinQR = true
                } label: {
                    Label("Show join QR", systemImage: "qrcode")
                }
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
        .sheet(isPresented: $showJoinQR) {
            VStack(spacing: 16) {
                Text("Join \(model.groupLabel)").font(.headline)
                if let payload = model.addressPayload, let img = QRKit.image(from: payload) {
                    Image(uiImage: img)
                        .interpolation(.none)
                        .resizable()
                        .scaledToFit()
                        .frame(maxWidth: 320, maxHeight: 320)
                    Text("Scan with another device to join this familiar").font(.footnote).foregroundStyle(.secondary)
                } else {
                    Text("No address yet.").foregroundStyle(.secondary)
                }
                Button("Done") { showJoinQR = false }
            }
            .padding()
        }
    }
}
