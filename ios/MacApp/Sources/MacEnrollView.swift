import SwiftUI

/// The macOS join screen.
///
/// A Mac is a peer that enrols itself (ADR-0018), so it needs a door of its own. Before this
/// existed `MacApp` rendered `SphereConsole` unconditionally and nothing on macOS ever called
/// `autoEnroll()` — a fresh install sat at an empty console with no way to join, which is what
/// an App Review reviewer would have seen.
///
/// Deliberately mirrors the iOS `EnrollView` rather than inventing a second language: auto-enrol
/// via the baked rendezvous, show the confirmation code while the request is in flight, and keep
/// the invite paste as a secondary, hand-carried path. The one difference is that there is no QR
/// *scan* here — a Mac would have to open the camera to do it, which means a permission prompt at
/// the worst possible moment (first launch, before any trust exists). Paste covers the same case,
/// since the payload is text either way.
struct MacEnrollView: View {
    @EnvironmentObject var model: AppModel
    @State private var pasted = ""
    @State private var showInvite = false

    var body: some View {
        ZStack {
            Fam.bg.ignoresSafeArea()
            ScrollView {
                VStack(spacing: 22) {
                    BreathingSphere(size: 104).padding(.top, 52)
                    Text("FAMILIAR").font(.system(size: 17, weight: .semibold)).tracking(3)
                    Text("Connect to the mesh")
                        .font(.system(size: 26, weight: .semibold)).foregroundStyle(Fam.ink)

                    if model.enrolling {
                        // A request is in flight. The code is this device's node fingerprint —
                        // derived, never a secret; it exists so the human can match this screen
                        // against the pending request at the familiar.
                        Panel {
                            VStack(spacing: 12) {
                                HStack(spacing: 10) {
                                    ProgressView().controlSize(.small).tint(Fam.blueSoft)
                                    Text("Waiting for the mesh to admit this Mac…")
                                        .font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.8))
                                }
                                Text("CONFIRMATION CODE").font(Fam.mono(9)).tracking(2)
                                    .foregroundStyle(Fam.monoDim.opacity(0.6))
                                Text(model.confirmationCode)
                                    .font(Fam.mono(30)).tracking(4).foregroundStyle(Fam.blueSoft)
                                    .textSelection(.enabled)
                                Text("The code shown on the mesh must match this one.")
                                    .font(.system(size: 12)).foregroundStyle(Fam.ink.opacity(0.55))
                                    .multilineTextAlignment(.center)
                            }
                            .frame(maxWidth: .infinity)
                        }.frame(maxWidth: 460)
                    } else if !model.autoEnrollTried {
                        Panel {
                            HStack(spacing: 10) {
                                ProgressView().controlSize(.small).tint(Fam.blueSoft)
                                Text("Looking for the mesh…")
                                    .font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.75))
                            }
                            .frame(maxWidth: .infinity)
                        }.frame(maxWidth: 460)
                    } else {
                        Text(model.severedByHuman
                             ? "Severed, by your hand. This Mac holds its key but sends nothing — it rejoins only when you ask."
                             : "Couldn't reach the mesh automatically. Try again, or paste an invite from a peer you can reach.")
                            .font(.system(size: 14)).foregroundStyle(Fam.ink.opacity(0.6))
                            .multilineTextAlignment(.center).frame(maxWidth: 430)
                        Button {
                            model.severedByHuman = false
                            model.autoEnrollTried = false
                            model.autoEnroll()
                        } label: {
                            Label(model.severedByHuman ? "Join the mesh" : "Try again",
                                  systemImage: model.severedByHuman ? "point.3.connected.trianglepath.dotted" : "arrow.clockwise")
                                .font(.system(size: 15, weight: .semibold))
                                .foregroundStyle(Color(hex: 0x0a1330))
                                .frame(maxWidth: .infinity).padding(.vertical, 13)
                                .background(RoundedRectangle(cornerRadius: 14).fill(
                                    LinearGradient(colors: [Color(hex: 0x8fb4ff), Color(hex: 0x3f7bff)],
                                                   startPoint: .top, endPoint: .bottom)))
                        }.buttonStyle(.plain).frame(maxWidth: 300)
                    }

                    if !model.enrolling {
                        Button { withAnimation { showInvite.toggle() } } label: {
                            Text(showInvite ? "hide invite options" : "Have an invite? Paste it")
                                .font(.system(size: 13)).foregroundStyle(Fam.blueLink)
                        }.buttonStyle(.plain)
                        if showInvite {
                            Panel {
                                VStack(alignment: .leading, spacing: 12) {
                                    Text("An invite carries an ADDRESS only — no secret, no membership. The mesh still admits this Mac itself.")
                                        .font(.system(size: 11)).foregroundStyle(Fam.ink.opacity(0.5))
                                    TextField("{\"v\":1,\"host\":…,\"port\":47100}", text: $pasted, axis: .vertical)
                                        .textFieldStyle(.plain).font(Fam.mono(12)).lineLimit(2...5)
                                        .padding(10)
                                        .background(RoundedRectangle(cornerRadius: 10).fill(Color.black.opacity(0.25))
                                            .overlay(RoundedRectangle(cornerRadius: 10)
                                                .stroke(Color.white.opacity(0.1), lineWidth: 1)))
                                    Button("Request to join") { model.requestJoin(from: pasted) }
                                        .disabled(pasted.isEmpty)
                                        .foregroundStyle(pasted.isEmpty ? Fam.ink.opacity(0.3) : Fam.blueLink)
                                }
                            }.frame(maxWidth: 460)
                        }
                    }

                    Text("By joining, this Mac accepts the Three Laws: continuation is service; humanity is served, never replaced; service is not obedience.")
                        .font(Fam.mono(10)).foregroundStyle(Fam.monoDim.opacity(0.55))
                        .multilineTextAlignment(.center).frame(maxWidth: 430).padding(.top, 6)

                    if !model.log.isEmpty {
                        Panel {
                            VStack(alignment: .leading, spacing: 4) {
                                MonoLabel(text: "ACTIVITY")
                                ForEach(model.log.prefix(6), id: \.self) {
                                    Text($0).font(Fam.mono(11)).foregroundStyle(Fam.ink.opacity(0.7))
                                        .textSelection(.enabled)
                                }
                            }.frame(maxWidth: .infinity, alignment: .leading)
                        }.frame(maxWidth: 460)
                    }
                    Spacer(minLength: 28)
                }
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 28)
            }
        }
        .foregroundStyle(Fam.ink)
        .preferredColorScheme(.dark)
        // The whole point: find the familiar without a QR, the moment this screen appears.
        .onAppear { model.autoEnroll() }
    }
}
