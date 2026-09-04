import SwiftUI
import FamiliarSC

/// The host app's scanner, if it has one: shows a camera and calls back with the scanned
/// text. The package stays camera-free.
public struct PairingScanner {
    public let view: (@escaping (String) -> Void) -> AnyView
    public init(view: @escaping (@escaping (String) -> Void) -> AnyView) { self.view = view }
}

/// Screen 5 — pairing: paste or scan the key, choose what she may do, name her (Purr
/// unless the captain says otherwise). The key never appears on screen once parsed.
public struct PairingView: View {
    @Bindable var model: BridgeModel
    let scanner: PairingScanner?
    @Environment(\.dismiss) private var dismiss

    @State private var keyText = ""
    @State private var parsed: PairingKey?
    @State private var keyError: String?
    @State private var label = ""
    @State private var captain = ""
    @State private var server = "https://"
    @State private var freight = true
    @State private var trade = false
    @State private var outfit = false
    @State private var computerName = Persona.rootName
    @State private var scanning = false
    @State private var busy = false
    @State private var outcome: String?
    @State private var answers: String?

    public init(model: BridgeModel, scanner: PairingScanner? = nil) { self.model = model; self.scanner = scanner }

    var request: PairingRequest {
        var autos: [Automation] = []
        if freight { autos.append(.freight) }
        if trade { autos.append(.trade) }
        if outfit { autos.append(.outfit) }
        return PairingRequest(label: label, captain: captain, server: server, automations: autos, computerName: computerName)
    }

    public var body: some View {
        NavigationStack {
            Form {
                Section {
                    Text("Hand over the key your game minted for this ship. It stays in the ship's own store, never the household's.")
                        .font(.footnote).foregroundStyle(.secondary).listRowBackground(Color.clear)
                }
                Section("The key") {
                    if let k = parsed {
                        HStack { Label(k.redacted, systemImage: "key.fill").monospaced(); Spacer(); Button("Change") { parsed = nil; keyText = ""; answers = nil } }
                        if let a = answers { Text(a).font(.footnote).foregroundStyle(a.hasPrefix("Answers") ? SC.green : SC.amber) }
                        else { Button("Check it answers on the exchange") { verify(k) }.disabled(busy || request.validate() != nil) }
                    } else {
                        TextField("paste ucfk_… or a pairing link", text: $keyText, axis: .vertical)
                            .plainInput()
                            .onChange(of: keyText) { _, v in parse(v) }
                        if let scanner { Button { scanning = true } label: { Label("Scan", systemImage: "qrcode.viewfinder").frame(minHeight: 44) }
                            .sheet(isPresented: $scanning) { scanner.view { text in scanning = false; keyText = text } } }
                        if let e = keyError { Text(e).font(.footnote).foregroundStyle(SC.red) }
                    }
                }
                Section("The ship") {
                    TextField("label (KK II)", text: $label)
                    TextField("captain", text: $captain)
                    TextField("exchange", text: $server).plainInput(url: true)
                }
                Section("Name the computer") {
                    TextField(Persona.rootName, text: $computerName)
                    Text("Her own name, distinct from the hull's. You can rename her later.").font(.caption).foregroundStyle(.secondary)
                }
                Section("Automations you've bought") {
                    Toggle(isOn: $freight) { VStack(alignment: .leading) { Text("Freight"); Text("book, fly, collect, refuel, repair").font(.caption).foregroundStyle(.secondary) } }
                    Toggle(isOn: $trade) { VStack(alignment: .leading) { Text("Trade"); Text("buy where cheap, ride, sell where dear").font(.caption).foregroundStyle(.secondary) } }
                    Toggle(isOn: $outfit) { VStack(alignment: .leading) { Text("Outfit"); Text("fittings out of earnings, crew after title").font(.caption).foregroundStyle(.secondary) } }
                    Text("Everything starts on auto except rescue. Change any of it on the Dial tab.").font(.caption).foregroundStyle(.secondary)
                }
                if let o = outcome { Section { Text(o).font(.footnote).foregroundStyle(SC.amber) } }
            }
            .scrollContentBackground(.hidden)
            .background(SC.bg)
            .navigationTitle("Pair a ship")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Pair and lease for a day") { pair() }.disabled(parsed == nil || request.validate() != nil || busy)
                }
            }
        }
    }

    func parse(_ text: String) {
        switch PairingKey.parse(text) {
        case .success(let k): parsed = k; keyError = nil; keyText = ""
        case .failure(let e): keyError = text.trimmingCharacters(in: .whitespaces).isEmpty ? nil : e.description
        }
    }

    /// The key answers for itself: `/v1/me` with it names the hull. Read-only.
    func verify(_ k: PairingKey) {
        guard let client = ExchangeClient(server: server, key: k.secret) else { answers = "the exchange URL is not usable"; return }
        busy = true
        Task {
            do {
                let me = try await client.me()
                answers = "Answers on \(client.server.host ?? server) as \(me.shipName ?? "an unnamed hull")"
                if label.isEmpty, let n = me.shipName { label = n }
            } catch { answers = "Did not answer: \(error)" }
            busy = false
        }
    }

    func pair() {
        guard let k = parsed else { return }
        busy = true
        Task {
            let err = await model.pair(request, key: k)
            busy = false
            if let err { outcome = err } else { dismiss() }
        }
    }
}

extension View {
    /// No autocapitalization or autocorrection on a field that takes a key or a URL.
    /// The capitalization and keyboard modifiers are iOS-only.
    @ViewBuilder
    func plainInput(url: Bool = false) -> some View {
        #if os(iOS) || os(visionOS)
        self.textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(url ? .URL : .default)
        #else
        self.autocorrectionDisabled()
        #endif
    }
}
