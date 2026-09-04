import SwiftUI
import FamiliarSC

/// The captain's bridge as the canvas lays it out: four tabs — Ships, Bridge, Messages,
/// Dial — the last three on the selected ship (the first paired ship until the captain
/// picks another). Host it in a sheet, a window, or as an app's root.
public struct SCRootView: View {
    @Bindable var model: BridgeModel
    let scanner: PairingScanner?
    @State private var tab: Tab = .ships

    public enum Tab: Hashable { case ships, bridge, messages, dial }

    public init(model: BridgeModel, scanner: PairingScanner? = nil) { self.model = model; self.scanner = scanner }

    public var body: some View {
        TabView(selection: $tab) {
            NavigationStack { ShipsView(model: model, scanner: scanner) }
                .tabItem { Label("Ships", systemImage: "point.3.connected.trianglepath.dotted") }.tag(Tab.ships)
            NavigationStack { selected { ShipBridgeView(model: model, world: $0) } }
                .tabItem { Label("Bridge", systemImage: "gauge.with.dots.needle.33percent") }.tag(Tab.bridge)
            NavigationStack { selected { _ in MessageWindowView(model: model) } }
                .tabItem { Label("Messages", systemImage: "text.bubble") }.tag(Tab.messages)
                .badge(model.openProposals)
            NavigationStack { selected { _ in AutonomyDialView(model: model) } }
                .tabItem { Label("Dial", systemImage: "dial.medium") }.tag(Tab.dial)
        }
        .tint(SC.ice)
        .preferredColorScheme(.dark)
        .task {
            if model.ships.isEmpty { await model.refreshShips() }
            if model.world == nil, let first = model.ships.first { await model.open(world: first.world) }
        }
    }

    @ViewBuilder
    func selected<Content: View>(@ViewBuilder _ content: (String) -> Content) -> some View {
        if let w = model.world {
            content(w)
        } else {
            VStack(spacing: 10) {
                Text("No ship yet.").font(.headline)
                Text("Pair one on the Ships tab.").font(.footnote).foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(SC.bg)
        }
    }
}
