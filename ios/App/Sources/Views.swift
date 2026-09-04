import SwiftUI
import FamiliarMesh
import WatchConnectivity

@main
struct FamiliarAgentApp: App {
    @StateObject private var model = AppModel()
    @Environment(\.scenePhase) private var scenePhase
    // APNs: the token callback lands on the app delegate (PushRegistration.swift).
    @UIApplicationDelegateAdaptor(PushDelegate.self) private var pushDelegate

    init() {
        // Must register before launch finishes (SPEC.md R12) — can't wait for a view's
        // onAppear, and can't depend on `model` existing yet (a background-only launch may
        // never create the SwiftUI scene at all). See BackgroundSync.swift.
        BackgroundSync.register()
    }

    var body: some Scene {
        WindowGroup {
            RootView(pushDelegate: pushDelegate).environmentObject(model)
        }
        .onChange(of: scenePhase) { phase in
            if phase == .background { BackgroundSync.scheduleNext() }
        }
    }
}

struct RootView: View {
    @EnvironmentObject var model: AppModel
    let pushDelegate: PushDelegate
    var body: some View {
        Group {
            if model.enrolled {
                // The Metal Sphere is the standard console for every peer with a screen —
                // iPhone and iPad both (same web bundle as the Mac). Device housekeeping
                // (consents, unenroll) lives on its Device screen, through the bridge.
                SphereConsoleIOS()
            } else {
                EnrollView().background(Fam.bg.ignoresSafeArea()).preferredColorScheme(.dark)
            }
        }
        // T-237 B3 preview: the ship's computer's bridge, behind its Settings flag — on both
        // sides of enrolment, because a captain's ship is not the household's business.
        .overlay(alignment: .bottomLeading) { ShipsComputerDoor() }
        .onAppear {
            model.syncWatch()
            if model.enrolled { PushRegistration.request(model, delegate: pushDelegate) }
        }
        .onChange(of: model.enrolled) { enrolled in
            // A device that just became a member registers for the ember's push right away.
            if enrolled { PushRegistration.request(model, delegate: pushDelegate) }
        }
    }
}

/// Enrollment (ADR-0012): the device finds the familiar **automatically** via the rendezvous
/// (the lighthouse) and asks to join, showing a confirmation code the human matches when they
/// approve it at the familiar. QR/paste stays as an option ("have an invite?") — an offline or
/// hand-carried path — but is no longer the front door. The group secret never touches this device.
struct EnrollView: View {
    @EnvironmentObject var model: AppModel
    @State private var pasted = ""
    @State private var scanning = false
    @State private var showInvite = false
    var body: some View {
        ZStack {
            Fam.bg.ignoresSafeArea()
            ScrollView {
                VStack(spacing: 22) {
                    BreathingSphere(size: 96).padding(.top, 40)
                    Text("FAMILIAR").font(.system(size: 17, weight: .semibold)).tracking(3)
                    Text("Connect to the mesh")
                        .font(.system(size: 26, weight: .semibold)).foregroundStyle(Fam.ink)

                    if model.enrolling {
                        // Auto-enroll (or a manual request) is in flight — say what is being
                        // tried (T-120: progress, not silence), and show the code to match.
                        Panel {
                            VStack(spacing: 12) {
                                HStack(spacing: 10) {
                                    ProgressView().tint(Fam.blueSoft)
                                    Text(model.joinProgress.detail.isEmpty
                                         ? "Waiting for the mesh to admit this device…"
                                         : model.joinProgress.detail)
                                        .font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.8))
                                }
                                if model.joinProgress.stage == .awaitingAdmission, model.joinProgress.tries > 0 {
                                    Text("still asking — \(model.joinProgress.tries) checks, ~\(max(1, model.joinProgress.tries * 2 / 60)) min")
                                        .font(Fam.mono(10)).foregroundStyle(Fam.monoDim.opacity(0.7))
                                }
                                Text("CONFIRMATION CODE").font(Fam.mono(9)).tracking(2).foregroundStyle(Fam.monoDim.opacity(0.6))
                                Text(model.confirmationCode)
                                    .font(Fam.mono(30)).tracking(4).foregroundStyle(Fam.blueSoft)
                                Text("The code shown on the mesh must match this one.")
                                    .font(.system(size: 12)).foregroundStyle(Fam.ink.opacity(0.55))
                                    .multilineTextAlignment(.center)
                            }
                        }.padding(.horizontal, 22)
                    } else if !model.autoEnrollTried || model.joinProgress.stage == .seekingDirectory {
                        // Directory lookup in flight — autoEnrollTried flips true before the
                        // fetch, so without the stage check this screen read "couldn't reach"
                        // DURING the first, possibly slow, lighthouse call (T-120).
                        Panel {
                            HStack(spacing: 10) { ProgressView().tint(Fam.blueSoft)
                                Text(model.joinProgress.stage == .seekingDirectory && !model.joinProgress.detail.isEmpty
                                     ? model.joinProgress.detail : "Looking for the mesh…")
                                    .font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.75)) }
                        }.padding(.horizontal, 22)
                    } else {
                        // Severed by the human: the device waits for an explicit ask — it must
                        // never quietly rejoin (there was no way to leave, or to test arriving).
                        Text(model.severedByHuman
                             ? "Severed, by your hand. This device holds its key but sends nothing — it rejoins only when you ask."
                             : (model.joinProgress.detail.isEmpty
                                ? "Couldn't reach the mesh automatically. Retry, or use an invite from a peer you can see."
                                : model.joinProgress.detail))
                            .font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.6))
                            .multilineTextAlignment(.center).padding(.horizontal, 28)
                        if !model.severedByHuman, !model.joinProgress.causes.isEmpty {
                            // Name WHAT failed, per address — a diagnosable refusal, not a shrug.
                            VStack(alignment: .leading, spacing: 3) {
                                ForEach(model.joinProgress.causes.prefix(4), id: \.self) {
                                    Text($0).font(Fam.mono(10)).foregroundStyle(Fam.monoDim.opacity(0.65))
                                }
                            }.padding(.horizontal, 28)
                        }
                        Button {
                            model.severedByHuman = false
                            model.autoEnrollTried = false
                            model.autoEnroll()
                        } label: {
                            Label(model.severedByHuman ? "Join the mesh" : "Try again",
                                  systemImage: model.severedByHuman ? "point.3.connected.trianglepath.dotted" : "arrow.clockwise")
                                .font(.system(size: 15, weight: .semibold)).foregroundStyle(Color(hex: 0x0a1330))
                                .frame(maxWidth: .infinity).padding(.vertical, 15)
                                .background(RoundedRectangle(cornerRadius: 14).fill(LinearGradient(colors: [Color(hex: 0x8fb4ff), Color(hex: 0x3f7bff)], startPoint: .top, endPoint: .bottom)))
                        }.buttonStyle(.plain).padding(.horizontal, 22)
                    }

                    // QR / paste — always available, but secondary.
                    if !model.enrolling {
                        Button { withAnimation { showInvite.toggle() } } label: {
                            Text(showInvite ? "hide invite options" : "Have an invite? Scan or paste")
                                .font(.system(size: 13)).foregroundStyle(Fam.blueLink)
                        }.buttonStyle(.plain)
                        if showInvite {
                            VStack(spacing: 12) {
                                Button { scanning = true } label: {
                                    Label("Scan QR", systemImage: "qrcode.viewfinder")
                                        .font(.system(size: 14, weight: .medium)).foregroundStyle(Fam.blueLink)
                                }.buttonStyle(.plain)
                                Panel {
                                    VStack(alignment: .leading, spacing: 12) {
                                        TextField("{\"v\":1,\"host\":…,\"port\":47100}", text: $pasted, axis: .vertical)
                                            .textFieldStyle(.plain).font(Fam.mono(12)).lineLimit(2...5)
                                            .padding(12).background(RoundedRectangle(cornerRadius: 10).fill(Color.black.opacity(0.25))
                                                .overlay(RoundedRectangle(cornerRadius: 10).stroke(Color.white.opacity(0.1), lineWidth: 1)))
                                        Button("Request to join") { model.requestJoin(from: pasted) }
                                            .disabled(pasted.isEmpty).foregroundStyle(pasted.isEmpty ? Fam.ink.opacity(0.3) : Fam.blueLink)
                                    }
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
        .onAppear { model.autoEnroll() }   // find the familiar without a QR the moment we land here
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
/// never a frame or an identity. Recognition (matching a face to a known person) is a separate,
/// sharper toggle — SPEC.md R10, "strongly sensitive" per docs/design-orientation-and-mesh.md.
struct FaceControl: View {
    @ObservedObject var model: AppModel
    @ObservedObject var face: FaceSensing
    var body: some View {
        Toggle("Presence — faces at the iPad (front camera)", isOn: $model.faceEnabled)
            .onChange(of: model.faceEnabled) { _ in model.startFaceIfConsented() }
        Toggle("Recognition — match faces to people I know", isOn: $model.faceRecognitionEnabled)
            .disabled(!model.faceEnabled)
            .onChange(of: model.faceRecognitionEnabled) { _ in model.startFaceIfConsented() }
        if face.running {
            Text("watching · \(face.lastCount) face(s)").font(.footnote).foregroundStyle(.secondary)
        }
        if face.needsIdentification || face.proposedHandle != nil {
            FaceIdentifyPrompt(face: face)
        }
    }
}

/// The interactive-identification fallback (SPEC.md R10, a hard requirement): a face is present
/// and engaged but not confidently matched, so the familiar asks rather than silently treating
/// the interaction as anyone's. When recognition *does* propose a guess, this becomes a
/// confirm-or-correct prompt instead — a wrong link must always be fixable, never sticky.
struct FaceIdentifyPrompt: View {
    @EnvironmentObject var model: AppModel
    @ObservedObject var face: FaceSensing
    @State private var typed = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let proposed = face.proposedHandle {
                Text("Is this \(proposed)?").font(.footnote)
                HStack {
                    Button("Yes") {
                        face.confirmIdentity(handle: proposed)
                        model.confirmPresentHuman(proposed)   // rung 3 → an `asked` claim
                    }
                    Button("No — someone else") { face.proposedHandle = nil; face.needsIdentification = true }
                        .foregroundStyle(.secondary)
                }
            } else {
                Text("I don't recognize this person yet — who is it?").font(.footnote)
                HStack {
                    TextField("Name", text: $typed).textFieldStyle(.roundedBorder)
                    Button("Confirm") {
                        let name = typed.trimmingCharacters(in: .whitespaces)
                        guard !name.isEmpty else { return }
                        face.confirmIdentity(handle: name)
                        model.confirmPresentHuman(name)       // rung 3 → an `asked` claim
                        typed = ""
                    }
                }
            }
        }
        .padding(.vertical, 4)
    }
}

/// Post-enrollment: consent switches, a home anchor, live counts, the activity log, and a join QR
/// so this member becomes a scan-to-join point for the next device.
struct StatusView: View {
    @EnvironmentObject var model: AppModel
    @ObservedObject private var watch = PhoneWatchLink.shared
    @State private var showJoinQR = false
    @State private var joinQRHandoff = false
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
                    .onChange(of: model.discoveryEnabled) { _ in model.startDiscoveryIfAuthorized() }
                // This toggle can be ON while nothing is surveyed — the household boundary is the
                // authority and this switch only narrows it (T-228 Q2). Without the state line the
                // switch would quietly claim an authority it does not have.
                Text(model.discoveryState)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
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
        .onAppear { model.startSensingIfConsented(); model.startFaceIfConsented(); model.startDiscoveryIfAuthorized() }
        .sheet(isPresented: $showJoinQR) {
            VStack(spacing: 16) {
                Text(joinQRHandoff ? "Hand off to my new device" : "Join \(model.groupLabel)").font(.headline)
                // Re-minted per render: an invite token is single-use and lives ten minutes, so
                // the QR on screen is always spendable (ADR-0026). Handoff names this device's
                // own human — the scanner becomes theirs, no third person involved.
                if let payload = model.invitePayload(handoff: joinQRHandoff), let img = QRKit.image(from: payload) {
                    Image(uiImage: img)
                        .interpolation(.none)
                        .resizable()
                        .scaledToFit()
                        .frame(maxWidth: 320, maxHeight: 320)
                    Text(joinQRHandoff
                         ? "Scan on the new device — it joins as \(model.attributedHuman)"
                         : "Scan with another device to join this familiar")
                        .font(.footnote).foregroundStyle(.secondary)
                    if case .member = model.membership, model.attributedHuman != "observer" {
                        Toggle("This is my own new device", isOn: $joinQRHandoff)
                            .frame(maxWidth: 320)
                            .font(.footnote)
                    }
                } else {
                    Text("No address yet.").foregroundStyle(.secondary)
                }
                Button("Done") { showJoinQR = false }
            }
            .padding()
        }
    }
}
