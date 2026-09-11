import SwiftUI
import FamiliarSC

/// The captain's bridge, one path: the ships, then a ship — and that screen IS Felix.
/// No tabs, no doors, no overlays (Ian, 2026-09-04: "simple and elegant").
public struct SCRootView: View {
    @Bindable var model: BridgeModel
    let scanner: PairingScanner?
    let onClose: (() -> Void)?
    let fixtureNote: String?
    /// A host's one control on the fleet screen (UCF Familiar's connection picker). It rides
    /// in the navigation bar with the stack's own items, so it can never be drawn over them:
    /// TestFlight build 7 floated it as an overlay and it landed on top of "Pair a ship"
    /// (Capitali/familiar#6).
    let hostItem: AnyView?

    public init(model: BridgeModel, scanner: PairingScanner? = nil, onClose: (() -> Void)? = nil, fixtureNote: String? = nil) {
        self.model = model; self.scanner = scanner; self.onClose = onClose; self.fixtureNote = fixtureNote; self.hostItem = nil
    }

    public init<Item: View>(model: BridgeModel, scanner: PairingScanner? = nil, onClose: (() -> Void)? = nil, fixtureNote: String? = nil,
                            @ViewBuilder hostItem: () -> Item) {
        self.model = model; self.scanner = scanner; self.onClose = onClose; self.fixtureNote = fixtureNote; self.hostItem = AnyView(hostItem())
    }

    public var body: some View {
        NavigationStack {
            ShipsView(model: model, scanner: scanner, fixtureNote: fixtureNote)
                .toolbar {
                    if let onClose {
                        ToolbarItem(placement: .cancellationAction) { Button("Close") { onClose() } }
                    }
                    if let hostItem {
                        ToolbarItem(placement: .topBarLeading) { hostItem }
                    }
                }
        }
        .tint(SC.ice)
        .preferredColorScheme(.dark)
    }
}
