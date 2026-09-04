import SwiftUI

// The macOS console — the Metal Sphere (Claude Design import, ADR-0008): satellite globe +
// hologram in a web layer, street surface on REAL Apple Maps, all daemon I/O native over the
// loopback seam. See SphereWebView.swift. (The former SwiftUI console — Fam palette, rail,
// Metal orb — was legacy and is gone; git history has it if archaeology is ever needed.)

@main
struct FamiliarMacApp: App {
    // The shared client core (Shared/Sources/AppModel.swift) — the same one the iOS shells run.
    // It enrols this Mac as a peer, reads the worldview over the mesh, heartbeats status and
    // services consults; the bridge below only renders it.
    @StateObject private var model: AppModel
    @StateObject private var bridge: SphereBridge

    init() {
        let model = AppModel()
        _model = StateObject(wrappedValue: model)
        _bridge = StateObject(wrappedValue: SphereBridge(model: model))
    }

    var body: some Scene {
        WindowGroup {
            // Same gate as the iOS RootView: the console is for members. An unenrolled Mac gets
            // the join screen — before this it got the console regardless, with nothing in it and
            // no door (ADR-0018 made the Mac a peer; it needs a peer's way in).
            Group {
                if model.enrolled {
                    SphereConsole()
                } else {
                    MacEnrollView()
                }
            }
            .environmentObject(bridge)
            .environmentObject(model)
            .frame(minWidth: 1040, minHeight: 720)
        }
        .windowStyle(.hiddenTitleBar)
        .commands {
            CommandMenu("Familiar") {
                Button(bridge.mic.isListening ? "Stop Talking" : "Push to Talk") {
                    bridge.micTapped()
                }
                .keyboardShortcut("t", modifiers: [.command, .shift])
                ShipsComputerMenu()
            }
        }

        // T-237 B3: the captain's bridge, its own window, behind the Settings switch.
        Window("Ship's computer", id: "ships-computer") {
            ShipsComputerMacHost()
        }
        .defaultSize(width: 860, height: 760)

        Settings {
            MacConsentSettings()
                .environmentObject(bridge)
                .environmentObject(model)
        }
    }
}


/// The menu item that opens the bridge — present only while the switch is on.
struct ShipsComputerMenu: View {
    @AppStorage("flag.shipsComputer") private var flag = false
    @Environment(\.openWindow) private var openWindow
    var body: some View {
        if flag {
            Divider()
            Button("Ship's computer…") { openWindow(id: "ships-computer") }
                .keyboardShortcut("s", modifiers: [.command, .shift])
        }
    }
}
