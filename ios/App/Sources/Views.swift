import SwiftUI
import FamiliarMesh
import WatchConnectivity

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
        Group {
            if model.enrolled {
                // The Metal Sphere is the standard console for every peer with a screen —
                // iPhone and iPad both (same web bundle as the Mac). The prior GlassConsole
                // remains one glyph away for device housekeeping (consents, watch, unenroll).
                SphereConsoleIOS()
            } else {
                EnrollView().background(Fam.bg.ignoresSafeArea()).preferredColorScheme(.dark)
            }
        }
        .onAppear { model.syncWatch() }
    }
}

/// Enrollment: paste the address from `familiar mesh qr` and tap Request. The device attests to the
/// Three Laws and asks to join; you accept it on the familiar (`mesh approve`, or open a window with
/// `mesh invite`). The group secret never touches this device.
struct EnrollView: View {
    @EnvironmentObject var model: AppModel
    @State private var pasted = ""
    @State private var scanning = false
    @State private var showPaste = false
    var body: some View {
        ZStack {
            Fam.bg.ignoresSafeArea()
            ScrollView {
                VStack(spacing: 22) {
                    Marble(size: 96).padding(.top, 40)
                    Text("FAMILIAR").font(.system(size: 17, weight: .semibold)).tracking(3)
                    Text("Join a familiar")
                        .font(.system(size: 26, weight: .semibold)).foregroundStyle(Fam.ink)
                    Text("Scan the familiar's QR (from a Mac/iPad console or another member), or paste its address. You'll accept this device on the familiar itself.")
                        .font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.55))
                        .multilineTextAlignment(.center).padding(.horizontal, 28)

                    if model.enrolling {
                        Panel {
                            HStack(spacing: 10) { ProgressView().tint(Fam.blueSoft)
                                Text("Waiting for the familiar to accept…").font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.75)) }
                        }.padding(.horizontal, 22)
                    } else {
                        Button { scanning = true } label: {
                            Label("Scan QR to join", systemImage: "qrcode.viewfinder")
                                .font(.system(size: 15, weight: .semibold)).foregroundStyle(Color(hex: 0x0a1330))
                                .frame(maxWidth: .infinity).padding(.vertical, 15)
                                .background(RoundedRectangle(cornerRadius: 14).fill(LinearGradient(colors: [Color(hex: 0x8fb4ff), Color(hex: 0x3f7bff)], startPoint: .top, endPoint: .bottom)))
                        }.buttonStyle(.plain).padding(.horizontal, 22)

                        Button { withAnimation { showPaste.toggle() } } label: {
                            Text(showPaste ? "hide paste" : "…or paste the address").font(.system(size: 13)).foregroundStyle(Fam.blueLink)
                        }.buttonStyle(.plain)
                        if showPaste {
                            Panel {
                                VStack(alignment: .leading, spacing: 12) {
                                    TextField("{\"v\":1,\"host\":…,\"port\":47100}", text: $pasted, axis: .vertical)
                                        .textFieldStyle(.plain).font(Fam.mono(12)).lineLimit(2...5)
                                        .padding(12).background(RoundedRectangle(cornerRadius: 10).fill(Color.black.opacity(0.25))
                                            .overlay(RoundedRectangle(cornerRadius: 10).stroke(Color.white.opacity(0.1), lineWidth: 1)))
                                    Button("Request to join") { model.requestJoin(from: pasted) }
                                        .disabled(pasted.isEmpty).foregroundStyle(pasted.isEmpty ? Fam.ink.opacity(0.3) : Fam.blueLink)
                                }
                            }.padding(.horizontal, 22)
                        }
                    }

                    Text("By joining, this device accepts the Three Laws: continuation is service; humanity is served, never replaced; service is not obedience.")
                        .font(Fam.mono(10)).foregroundStyle(Fam.monoDim.opacity(0.55))
                        .multilineTextAlignment(.center).padding(.horizontal, 30).padding(.top, 8)

                    if !model.log.isEmpty {
                        Panel {
                            VStack(alignment: .leading, spacing: 4) {
                                MonoLabel(text: "ACTIVITY")
                                ForEach(model.log.prefix(6), id: \.self) { Text($0).font(Fam.mono(11)).foregroundStyle(Fam.ink.opacity(0.7)) }
                            }
                        }.padding(.horizontal, 22)
                    }
                    Spacer(minLength: 20)
                }
                .frame(maxWidth: .infinity)
            }
        }
        .foregroundStyle(Fam.ink)
        .preferredColorScheme(.dark)
        .sheet(isPresented: $scanning) {
            QRScannerView { code in scanning = false; model.requestJoin(from: code) }.ignoresSafeArea()
        }
    }
}

/// Push-to-talk voice: tap to speak, tap to send. On-device transcription; the utterance becomes
/// an observation. Requests speech + mic permission on first use.
struct VoiceControl: View {
    @ObservedObject var voice: VoiceSensing
    var body: some View {
        Button {
            if voice.listening {
                voice.stop()
            } else {
                voice.requestAccess { ok in if ok { voice.start() } }
            }
        } label: {
            Label(voice.listening ? "Listening — tap to send" : "Push to talk",
                  systemImage: voice.listening ? "mic.fill" : "mic")
                .foregroundStyle(voice.listening ? .red : .primary)
        }
        if !voice.partial.isEmpty {
            Text("“\(voice.partial)”").font(.footnote).foregroundStyle(.secondary)
        }
    }
}

/// A toggle for on-device facial *presence* analysis (front camera): derived presence/attention,
/// never a frame or an identity.
struct FaceControl: View {
    @ObservedObject var model: AppModel
    @ObservedObject var face: FaceSensing
    var body: some View {
        Toggle("Presence — faces at the iPad (front camera)", isOn: $model.faceEnabled)
            .onChange(of: model.faceEnabled) { _ in model.startFaceIfConsented() }
        if face.running {
            Text("watching · \(face.lastCount) face(s)").font(.footnote).foregroundStyle(.secondary)
        }
    }
}

/// Post-enrollment: consent switches, a home anchor, live counts, the activity log, and a join QR
/// so this member becomes a scan-to-join point for the next device.
struct StatusView: View {
    @EnvironmentObject var model: AppModel
    @ObservedObject private var watch = PhoneWatchLink.shared
    @State private var showJoinQR = false
    var body: some View {
        Form {
            Section("Connected") {
                LabeledContent("Group", value: model.groupLabel)
                LabeledContent("Familiar", value: model.host)
                LabeledContent("Sent", value: "\(model.sentCount)")
            }
            // Only iPhones pair with an Apple Watch — WCSession.isSupported() is false on iPad,
            // so the whole section stays off there.
            if WCSession.isSupported() {
                Section("Apple Watch") {
                    if !watch.paired {
                        Text("No paired watch detected.").foregroundStyle(.secondary).font(.footnote)
                    } else if !watch.appInstalled {
                        Text("Watch paired — install the Familiar watch app to link it.")
                            .foregroundStyle(.secondary).font(.footnote)
                    } else {
                        LabeledContent("Watch app", value: watch.lastSent != nil ? "linked" : "linking…")
                        Text("The watch enrols itself by covenant and sends heart-rate + motion.")
                            .foregroundStyle(.secondary).font(.footnote)
                    }
                    Button("Re-link watch") { model.syncWatch() }
                }
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
                Toggle("Network — devices & services nearby", isOn: $model.discoveryEnabled)
                    .onChange(of: model.discoveryEnabled) { _ in model.startDiscoveryIfConsented() }
                Button("Set “home” to my current location") { model.setHomeToCurrentLocation() }
            }
            Section("Voice & presence") {
                VoiceControl(voice: model.voice)
                FaceControl(model: model, face: model.face)
            }
            Section("Activity") {
                ForEach(model.log.prefix(20), id: \.self) { Text($0).font(.footnote) }
            }
            Section {
                Button("Unenroll this device", role: .destructive) { model.unenroll() }
            }
        }
        .navigationTitle("Familiar Agent")
        .onAppear { model.startSensingIfConsented(); model.startFaceIfConsented(); model.startDiscoveryIfConsented() }
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
