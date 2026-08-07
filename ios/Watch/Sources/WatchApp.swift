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

    var mainBody: some View {
        VStack(spacing: 4) {
            Text("Familiar").font(.headline)
            if model.enrolled {
                Text("in \(model.groupLabel)").font(.caption2).foregroundStyle(.secondary)
                HStack(spacing: 10) {
                    if let hr = model.lastHeartRate { Label("\(hr)", systemImage: "heart.fill").font(.caption) }
                    Text("↑\(model.sentCount)").font(.caption2)
                }
            } else if model.enrolling {
                ProgressView()
                Text("joining…").font(.caption2)
            } else {
                Text("Open the iPhone app to link this watch.").font(.caption2)
                    .multilineTextAlignment(.center)
            }
            ForEach(model.log.prefix(3), id: \.self) { Text($0).font(.system(size: 10)).foregroundStyle(.secondary) }
        }
        .padding(4)
        .onAppear { model.start() }
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
                Text(kind == "campfire" ? "The ember is yours" : "Your turn")
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
