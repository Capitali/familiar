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
        if model.needsConsentPrompt {
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

/// First-pair consent — shown once, right after enrollment, before any sensing starts. Off
/// by default; the human must explicitly opt each one in (or leave both off and continue).
struct WatchConsentView: View {
    @ObservedObject var model: WatchModel
    @State private var motion = false
    @State private var heart = false

    var body: some View {
        VStack(spacing: 6) {
            Text("Share from this watch?").font(.headline).multilineTextAlignment(.center)
            Toggle("Motion", isOn: $motion).font(.caption)
            Toggle("Heart rate", isOn: $heart).font(.caption)
            Button("Continue") { model.resolveConsent(motion: motion, heart: heart) }
                .font(.caption2)
        }
        .padding(4)
    }
}
