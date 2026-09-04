import SwiftUI
import FamiliarSC
import FamiliarSCUI

/// The captain's bridge on the Mac console (T-237 B3): its own window, "Familiar ▸ Ship's
/// computer…" (⌘⇧S), behind the same `flag.shipsComputer` switch as iOS — in Settings (⌘,)
/// here, with the fleet feed URL and its bearer. Native, so it runs on a Mac whose boot
/// policy refuses iPad apps (MacOnStick boots an external volume under Permissive Security,
/// 2026-09-04). Same package, same feed, same Felix; the mic and the voice are the Mac's.
enum ShipsComputerFlag {
    static var enabled: Bool { UserDefaults.standard.bool(forKey: "flag.shipsComputer") }
    static var feedURL: URL? {
        let s = UserDefaults.standard.string(forKey: "sc.feedURL")?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return s.isEmpty ? nil : URL(string: s)
    }
    static var feedBearer: String? {
        let s = UserDefaults.standard.string(forKey: "sc.feedBearer")?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return s.isEmpty ? nil : s
    }
}

struct ShipsComputerMacHost: View {
    @AppStorage("consent.pcc") private var consentPCC = false
    @AppStorage("sc.feedURL") private var feedURL = ""
    @AppStorage("sc.feedBearer") private var feedBearer = ""
    @State private var bridge: BridgeModel?
    @State private var key = ""

    var body: some View {
        Group {
            if let bridge {
                SCRootView(model: bridge, fixtureNote: ShipsComputerFlag.feedURL == nil ? "Fixture fleet — set the fleet feed in Settings (⌘,) to see your ships." : nil)
            } else {
                ProgressView()
            }
        }
        .frame(minWidth: 720, minHeight: 640)
        .task(id: feedURL + "|" + feedBearer) { rebuild() }
        .onChange(of: consentPCC) { _, v in bridge?.voiceConsent = VoiceConsent(privateCloudCompute: v) }
    }

    /// A new model whenever the feed or the bearer changes, so a pasted bearer takes effect
    /// without relaunching.
    func rebuild() {
        let consent = VoiceConsent(privateCloudCompute: consentPCC)
        if let url = ShipsComputerFlag.feedURL, let bearer = ShipsComputerFlag.feedBearer {
            let wire = WireFeed(base: url, bearer: bearer)
            bridge = BridgeModel(feed: wire, acts: wire, voiceConsent: consent)
        } else {
            let fixture = FixtureFeed()
            bridge = BridgeModel(feed: fixture, acts: fixture, voiceConsent: consent)
        }
    }
}

/// The Settings section: the switch, the feed, the bearer, the PCC consent for her voice.
struct ShipsComputerMacSettings: View {
    @AppStorage("flag.shipsComputer") private var flag = false
    @AppStorage("sc.feedURL") private var feedURL = ""
    @AppStorage("sc.feedBearer") private var feedBearer = ""
    @AppStorage("consent.pcc") private var consentPCC = false

    var body: some View {
        Section("Ship's computer (T-237, preview)") {
            Toggle("Show the bridge (Familiar ▸ Ship's computer…)", isOn: $flag)
            TextField("Fleet feed", text: $feedURL, prompt: Text("http://100.78.40.47:7899"))
                .textFieldStyle(.roundedBorder).disableAutocorrection(true)
            SecureField("Feed bearer", text: $feedBearer)
                .textFieldStyle(.roundedBorder)
            Toggle("Private Cloud Compute for her voice", isOn: $consentPCC)
            Text("The bearer is the fleet feed's own token (fleet-serve.token on the host), not an exchange key. With no feed the bridge shows a fixture fleet.")
                .font(.caption).foregroundStyle(.secondary)
        }
    }
}
