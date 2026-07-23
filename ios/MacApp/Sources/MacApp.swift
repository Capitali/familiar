import SwiftUI

// The macOS console — the Metal Sphere (Claude Design import, ADR-0008): satellite globe +
// hologram in a web layer, street surface on REAL Apple Maps, all daemon I/O native over the
// loopback seam. See SphereWebView.swift. (The former SwiftUI console — Fam palette, rail,
// Metal orb — was legacy and is gone; git history has it if archaeology is ever needed.)

@main
struct FamiliarMacApp: App {
    @StateObject private var bridge = SphereBridge()

    var body: some Scene {
        WindowGroup {
            SphereConsole()
                .environmentObject(bridge)
                .frame(minWidth: 1040, minHeight: 720)
        }
        .windowStyle(.hiddenTitleBar)
        .commands {
            CommandMenu("Familiar") {
                Button(bridge.mic.isListening ? "Stop Talking" : "Push to Talk") {
                    bridge.micTapped()
                }
                .keyboardShortcut("t", modifiers: [.command, .shift])
            }
        }

        Settings {
            MacConsentSettings()
        }
    }
}
