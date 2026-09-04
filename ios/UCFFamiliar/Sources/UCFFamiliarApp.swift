import SwiftUI
import FamiliarSC
import FamiliarSCUI

// UCF Familiar — the ship's computer as its own app, and nothing else (Ian, 2026-09-04): a
// companion to United Cat Foods that runs against Jeff's PROD world or a dev instance, with
// Apple Intelligence on the device (and Private Cloud Compute) as its brains, so it can stay
// entirely on the iPad, iPhone or Mac. A familiar host is optional: add one to get a pilot
// that flies while the phone sleeps; without one, Felix observes, briefs, advises and talks
// straight from the exchange.

@main
struct UCFFamiliarApp: App {
    @State private var connections = ConnectionStore()

    var body: some Scene {
        WindowGroup {
            UCFFamiliarRoot(connections: connections)
                .preferredColorScheme(.dark)
        }
    }
}

/// The captain's fleets: one bridge per connection, a picker when there is more than one,
/// and the Connections screen when there is none.
struct UCFFamiliarRoot: View {
    @Bindable var connections: ConnectionStore
    @AppStorage("consent.pcc") private var consentPCC = false
    @State private var models: [String: BridgeModel] = [:]
    @State private var showConnections = false

    var body: some View {
        Group {
            if let active = connections.active, let model = model(for: active) {
                SCRootView(model: model, scanner: PairingScanner.camera, onClose: nil, fixtureNote: nil)
                    .overlay(alignment: .topTrailing) { header(active) }
            } else {
                ConnectionsView(connections: connections)
            }
        }
        .sheet(isPresented: $showConnections) { ConnectionsView(connections: connections) }
        .onChange(of: connections.connections) { _, _ in models = [:] }
        .onChange(of: consentPCC) { _, v in models.values.forEach { $0.voiceConsent = VoiceConsent(privateCloudCompute: v) } }
    }

    func header(_ active: Connection) -> some View {
        Menu {
            ForEach(connections.connections) { c in
                Button { connections.activeID = c.id } label: {
                    Label(c.name + (c.isDirect ? " · direct" : " · host"), systemImage: c.id == active.id ? "checkmark" : (c.isDirect ? "antenna.radiowaves.left.and.right" : "server.rack"))
                }
            }
            Divider()
            Button { showConnections = true } label: { Label("Connections…", systemImage: "gearshape") }
        } label: {
            Label(active.name, systemImage: active.isDirect ? "antenna.radiowaves.left.and.right" : "server.rack")
                .font(.footnote.weight(.semibold)).padding(.horizontal, 12).padding(.vertical, 7)
                .background(.ultraThinMaterial, in: Capsule())
        }
        .padding(.trailing, 14).padding(.top, 6)
    }

    func model(for c: Connection) -> BridgeModel? {
        if let m = models[c.id] { return m }
        let consent = VoiceConsent(privateCloudCompute: consentPCC)
        let m: BridgeModel
        switch c {
        case .host(_, let feedURL):
            guard let url = URL(string: feedURL), let bearer = connections.secret(for: c) else { return nil }
            let wire = WireFeed(base: url, bearer: bearer)
            m = BridgeModel(feed: wire, acts: wire, voiceConsent: consent)
        case .direct(_, let exchange, _):
            guard let key = connections.secret(for: c), let direct = DirectFeed(exchange: exchange, key: key) else { return nil }
            m = BridgeModel(feed: direct, acts: direct, voiceConsent: consent)
        }
        DispatchQueue.main.async { models[c.id] = m }
        return m
    }
}
