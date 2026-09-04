import SwiftUI
import FamiliarSC

/// The captain's bridge, one path: the ships, then a ship — and that screen IS Felix.
/// No tabs, no doors, no overlays (Ian, 2026-09-04: "simple and elegant").
public struct SCRootView: View {
    @Bindable var model: BridgeModel
    let scanner: PairingScanner?
    let onClose: (() -> Void)?
    let fixtureNote: String?

    public init(model: BridgeModel, scanner: PairingScanner? = nil, onClose: (() -> Void)? = nil, fixtureNote: String? = nil) {
        self.model = model; self.scanner = scanner; self.onClose = onClose; self.fixtureNote = fixtureNote
    }

    public var body: some View {
        NavigationStack {
            ShipsView(model: model, scanner: scanner, fixtureNote: fixtureNote)
                .toolbar {
                    if let onClose {
                        ToolbarItem(placement: .cancellationAction) { Button("Close") { onClose() } }
                    }
                }
        }
        .tint(SC.ice)
        .preferredColorScheme(.dark)
    }
}
