import SwiftUI

@main
struct FamiliarWatchApp: App {
    @StateObject private var model = WatchModel()
    var body: some Scene {
        WindowGroup { WatchRootView().environmentObject(model) }
    }
}

struct WatchRootView: View {
    @EnvironmentObject var model: WatchModel
    var body: some View {
        if let kind = model.emberKind {
            EmberView(kind: kind) { model.emberKind = nil }
        } else if model.needsConsentPrompt {
            WatchConsentView(model: model)
        } else {
            mainBody
        }
    }

    // The wrist's resting face (B18): the blue globe orb, orbiting slowly. No health data — the
    // watch is a presence and a signal, not a dashboard. It shows the ember when a notification
    // arrives (handled above), and voice comes later. Enrolled → just the orb; not yet → a quiet
    // line to link it.
    var mainBody: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            if model.enrolled {
                if model.humanName.isEmpty {
                    // Linked, but the mesh still doesn't know whose wrist this is — a watch is
                    // established through its phone (ADR-0028), so the honest instruction is to go
                    // there, not to offer a dead-end path here (the watch has no good text entry).
                    VStack(spacing: 8) {
                        WatchOrbView().frame(width: 84, height: 84).opacity(0.6)
                        Text("Say who you are in Familiar on your iPhone — this watch will follow.")
                            .font(.caption2).foregroundStyle(.secondary).multilineTextAlignment(.center)
                    }
                } else {
                    WatchOrbView()
                }
            } else if model.enrolling {
                VStack(spacing: 6) { WatchOrbView().frame(width: 90, height: 90); Text("joining…").font(.caption2).foregroundStyle(.secondary) }
            } else {
                VStack(spacing: 8) {
                    WatchOrbView().frame(width: 84, height: 84).opacity(0.5)
                    Text("Open the iPhone app to link this watch.")
                        .font(.caption2).foregroundStyle(.secondary).multilineTextAlignment(.center)
                }
            }
        }
        .onAppear { model.start() }
    }
}

/// The blue globe orb — a slowly rotating sphere with meridians and one orbiting dot, the
/// familiar's resting sign on the wrist (B18). Matches the console's orbit-glyph language, in
/// the mesh's blue rather than the exit control's cyan.
struct WatchOrbView: View {
    var body: some View {
        TimelineView(.animation) { tl in
            let t = tl.date.timeIntervalSinceReferenceDate
            Canvas { ctx, size in
                let c = CGPoint(x: size.width / 2, y: size.height / 2)
                let r = min(size.width, size.height) * 0.34
                let ink = Color(red: 0.52, green: 0.72, blue: 1.0)
                let glow = Color(red: 0.30, green: 0.55, blue: 1.0)
                var halo = Path(); halo.addEllipse(in: CGRect(x: c.x - r * 1.35, y: c.y - r * 1.35, width: 2.7 * r, height: 2.7 * r))
                ctx.fill(halo, with: .radialGradient(.init(colors: [glow.opacity(0.28), .clear]), center: c, startRadius: r * 0.6, endRadius: r * 1.5))
                var sphere = Path(); sphere.addEllipse(in: CGRect(x: c.x - r, y: c.y - r, width: 2 * r, height: 2 * r))
                ctx.stroke(sphere, with: .color(ink.opacity(0.85)), lineWidth: 1.6)
                var equator = Path(); equator.addEllipse(in: CGRect(x: c.x - r, y: c.y - r * 0.34, width: 2 * r, height: 0.68 * r))
                ctx.stroke(equator, with: .color(ink.opacity(0.5)), lineWidth: 1.1)
                let squash = abs(sin(t * 0.5)) * 0.9 + 0.1   // the meridian breathing = the slow spin
                var meridian = Path(); meridian.addEllipse(in: CGRect(x: c.x - r * squash, y: c.y - r, width: 2 * r * squash, height: 2 * r))
                ctx.stroke(meridian, with: .color(ink.opacity(0.5)), lineWidth: 1.1)
                let ang = t * (2 * .pi / 9)                  // one orbit every 9s — slow
                let dot = CGPoint(x: c.x + cos(ang) * r * 1.3, y: c.y + sin(ang) * r * 1.3)
                var dp = Path(); dp.addEllipse(in: CGRect(x: dot.x - 3, y: dot.y - 3, width: 6, height: 6))
                ctx.fill(dp, with: .color(ink))
            }
        }
    }
}

/// The ember has reached this wrist (the law of the fire: every device of the holder shows
/// it). A big living flame, a glow that breathes, and one line of what to do. Tap to dismiss —
/// the answer itself happens on whichever device has a keyboard.
struct EmberView: View {
    let kind: String
    let dismiss: () -> Void
    @State private var flare = false

    var body: some View {
        ZStack {
            RadialGradient(colors: [.orange.opacity(flare ? 0.45 : 0.2), .black],
                           center: .center, startRadius: 6, endRadius: flare ? 130 : 90)
                .ignoresSafeArea()
            VStack(spacing: 6) {
                Image(systemName: "flame.fill")
                    .font(.system(size: 64))
                    .foregroundStyle(
                        LinearGradient(colors: [.yellow, .orange, .red],
                                       startPoint: .top, endPoint: .bottom))
                    .shadow(color: .orange.opacity(flare ? 0.9 : 0.4), radius: flare ? 22 : 10)
                    .scaleEffect(flare ? 1.12 : 0.92)
                    .animation(.easeInOut(duration: 0.9).repeatForever(autoreverses: true), value: flare)
                Text(kind == "campfire" ? "The ember is yours"
                     : kind == "changeling" ? "Find the human"
                     : kind == "pact" ? "Allow, consent, or refuse?" : "Your turn")
                    .font(.headline)
                Text("answer from any of your devices")
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
            }
        }
        .onAppear { flare = true }
        .onTapGesture { dismiss() }
    }
}

/// First-pair consent — shown once, right after enrollment, before any sensing starts. Off
/// by default; the human must explicitly opt each one in (or leave both off and continue).
struct WatchConsentView: View {
    @ObservedObject var model: WatchModel
    @State private var motion = false
    @State private var heart = false
    @State private var location = false

    var body: some View {
        ScrollView {
            VStack(spacing: 6) {
                Text("Share from this watch?").font(.headline).multilineTextAlignment(.center)
                Toggle("Motion", isOn: $motion).font(.caption)
                Toggle("Heart rate", isOn: $heart).font(.caption)
                Toggle("Location", isOn: $location).font(.caption)
                Button("Continue") {
                    model.resolveConsent(motion: motion, heart: heart, location: location)
                }
                .font(.caption2)
            }
            .padding(4)
        }
    }
}
