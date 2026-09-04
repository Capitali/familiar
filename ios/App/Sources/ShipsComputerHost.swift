import SwiftUI
import FamiliarSC
import FamiliarSCUI

/// The captain's bridge inside the Familiar app (T-237 B3 preview host), behind the
/// `flag.shipsComputer` switch in Settings. With a fleet feed address set (`sc.feedURL`,
/// wildhorse's `familiar fleet serve` on the household door) the bridge reads the real
/// fleet over the door's bearer; without one it shows the fixture fleet so the screens
/// can be walked on a device from TestFlight before the host half lands.
enum ShipsComputerFlag {
    static var enabled: Bool { UserDefaults.standard.bool(forKey: "flag.shipsComputer") }
    /// What the phone presents to `fleet serve` — until wildhorse settles whether the
    /// fleet feed takes a bearer or a node-signed request, it is a Settings field.
    static var feedBearer: String? {
        let s = UserDefaults.standard.string(forKey: "sc.feedBearer")?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return s.isEmpty ? nil : s
    }
    static var feedURL: URL? {
        let s = UserDefaults.standard.string(forKey: "sc.feedURL")?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return s.isEmpty ? nil : URL(string: s)
    }
}

/// A floating door onto the bridge — shown over the sphere when the flag is on.
struct ShipsComputerDoor: View {
    @EnvironmentObject var model: AppModel
    @State private var open = false
    @AppStorage("flag.shipsComputer") private var flag = false

    var body: some View {
        if flag {
            Button { open = true } label: {
                Label("Ship's computer", systemImage: "cat")
                    .font(.footnote.weight(.semibold))
                    .padding(.horizontal, 12).padding(.vertical, 8)
                    .background(.ultraThinMaterial, in: Capsule())
            }
            .padding()
            .fullScreenCover(isPresented: $open) { ShipsComputerHost(bearer: ShipsComputerFlag.feedBearer) }
        }
    }
}

struct ShipsComputerHost: View {
    let bearer: String?
    @Environment(\.dismiss) private var dismiss
    @AppStorage("consent.pcc") private var consentPCC = false
    @State private var bridge: BridgeModel

    init(bearer: String?) {
        self.bearer = bearer
        let consent = VoiceConsent(privateCloudCompute: UserDefaults.standard.bool(forKey: "consent.pcc"))
        if let url = ShipsComputerFlag.feedURL, let bearer, !bearer.isEmpty {
            let wire = WireFeed(base: url, bearer: bearer)
            _bridge = State(initialValue: BridgeModel(feed: wire, acts: wire, voiceConsent: consent))
        } else {
            let fixture = FixtureFeed()
            _bridge = State(initialValue: BridgeModel(feed: fixture, acts: fixture, voiceConsent: consent))
        }
    }

    var body: some View {
        SCRootView(
            model: bridge,
            scanner: PairingScanner { done in AnyView(QRScanSheet(onScan: done)) },
            onClose: { dismiss() },
            fixtureNote: ShipsComputerFlag.feedURL == nil ? "Fixture fleet — set a fleet feed in Settings → Familiar to see your ships." : nil
        )
        .onChange(of: consentPCC) { _, v in bridge.voiceConsent = VoiceConsent(privateCloudCompute: v) }
        .task {
            // Notices for every ship, each once, through the app's existing notification grant.
            let notifier = CaptainNotifier()
            for ship in bridge.ships {
                if let entries = try? await bridge.feed.journal(world: ship.world, sinceTick: nil) {
                    await notifier.deliver(NoticePolicy.notices(for: entries.suffix(200).map { $0 }), world: ship.world, computer: ship.computer)
                }
            }
        }
    }
}

/// The app's QR scanner, adapted to the package's callback shape.
struct QRScanSheet: View {
    let onScan: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    var body: some View {
        NavigationStack {
            QRScannerView { code in onScan(code); dismiss() }
                .ignoresSafeArea()
                .navigationTitle("Scan the key")
                .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        }
    }
}
