import SwiftUI
import Observation
import Security
import FamiliarSC
import FamiliarSCUI

/// The connections a captain keeps: exchange keys and host bearers in the Keychain, the
/// rest in UserDefaults. Secrets never appear in a log or on screen once saved.
@Observable
final class ConnectionStore {
    var connections: [Connection] = [] { didSet { persist() } }
    var activeID: String? { didSet { UserDefaults.standard.set(activeID, forKey: "ucf.activeConnection") } }

    init() {
        if let d = UserDefaults.standard.data(forKey: "ucf.connections"), let c = try? JSONDecoder().decode([Connection].self, from: d) { connections = c }
        activeID = UserDefaults.standard.string(forKey: "ucf.activeConnection") ?? connections.first?.id
    }

    var active: Connection? { connections.first { $0.id == activeID } ?? connections.first }

    func persist() {
        if let d = try? JSONEncoder().encode(connections) { UserDefaults.standard.set(d, forKey: "ucf.connections") }
        if activeID == nil || !connections.contains(where: { $0.id == activeID }) { activeID = connections.first?.id }
    }

    func add(_ c: Connection, secret: String) {
        Keychain.set(secret, account: c.id)
        connections.removeAll { $0.id == c.id }
        connections.append(c)
        activeID = c.id
    }

    func remove(_ c: Connection) {
        Keychain.delete(account: c.id)
        connections.removeAll { $0.id == c.id }
    }

    func secret(for c: Connection) -> String? { Keychain.get(account: c.id) }
}

enum Keychain {
    static let service = "io.river.familiar.ucf"
    static func set(_ value: String, account: String) {
        delete(account: account)
        let q: [String: Any] = [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: account,
                                kSecValueData as String: Data(value.utf8), kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly]
        SecItemAdd(q as CFDictionary, nil)
    }
    static func get(account: String) -> String? {
        let q: [String: Any] = [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: account,
                                kSecReturnData as String: true, kSecMatchLimit as String: kSecMatchLimitOne]
        var out: AnyObject?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let d = out as? Data else { return nil }
        return String(data: d, encoding: .utf8)
    }
    static func delete(account: String) {
        let q: [String: Any] = [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: account]
        SecItemDelete(q as CFDictionary)
    }
}

/// Add, pick and remove connections; her voice and Private Cloud Compute consent live here too.
struct ConnectionsView: View {
    @Bindable var connections: ConnectionStore
    @Environment(\.dismiss) private var dismiss
    @AppStorage("consent.pcc") private var consentPCC = false

    @State private var mode = 0                      // 0 direct, 1 host
    @State private var exchangeChoice = 0            // 0 PROD, 1 LOCAL, 2 custom
    @State private var customExchange = ""
    @State private var keyText = ""
    @State private var parsed: PairingKey?
    @State private var keyError: String?
    @State private var scanning = false
    @State private var enrolName = ""
    @State private var hostName = ""
    @State private var feedURL = ""
    @State private var bearer = ""
    @State private var busy = false
    @State private var outcome: String?
    private let speaker = Speaker()

    var exchangeURL: String {
        switch exchangeChoice { case 0: return KnownExchange.prod; case 1: return KnownExchange.local; default: return customExchange }
    }

    var body: some View {
        NavigationStack {
            Form {
                if !connections.connections.isEmpty {
                    Section("Your fleets") {
                        ForEach(connections.connections) { c in
                            HStack {
                                Image(systemName: c.isDirect ? "antenna.radiowaves.left.and.right" : "server.rack").foregroundStyle(SC.ice)
                                VStack(alignment: .leading) {
                                    Text(c.name).font(.body.weight(.medium))
                                    Text(c.isDirect ? "direct to the exchange — Felix observes and advises; no pilot" : "through a familiar host — a pilot flies, proposals, the dial").font(.caption).foregroundStyle(.secondary)
                                }
                                Spacer()
                                if c.id == connections.activeID { Image(systemName: "checkmark").foregroundStyle(SC.green) }
                            }
                            .contentShape(Rectangle())
                            .onTapGesture { connections.activeID = c.id; dismiss() }
                        }
                        .onDelete { idx in idx.map { connections.connections[$0] }.forEach(connections.remove) }
                    }
                }
                Section("Add a fleet") {
                    Picker("How", selection: $mode) { Text("Direct to an exchange").tag(0); Text("Through a familiar host").tag(1) }.pickerStyle(.segmented)
                    if mode == 0 {
                        Picker("Exchange", selection: $exchangeChoice) { Text("PROD").tag(0); Text("LOCAL").tag(1); Text("Custom").tag(2) }.pickerStyle(.segmented)
                        if exchangeChoice == 2 { TextField("https://…", text: $customExchange).textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL) }
                        if let k = parsed {
                            HStack { Label(k.redacted, systemImage: "key.fill").monospaced(); Spacer(); Button("Change") { parsed = nil; keyText = "" } }
                        } else {
                            TextField("paste your ucfk_ key", text: $keyText, axis: .vertical).textInputAutocapitalization(.never).autocorrectionDisabled()
                                .onChange(of: keyText) { _, v in
                                    switch PairingKey.parse(v) { case .success(let k): parsed = k; keyError = nil; keyText = ""; case .failure(let e): keyError = v.trimmingCharacters(in: .whitespaces).isEmpty ? nil : e.description }
                                }
                            Button { scanning = true } label: { Label("Scan", systemImage: "qrcode.viewfinder") }
                                .sheet(isPresented: $scanning) { QRScanSheet { text in scanning = false; keyText = text } }
                            if let e = keyError { Text(e).font(.footnote).foregroundStyle(SC.red) }
                        }
                        Button("Add \(KnownExchange.name(for: exchangeURL))") { addDirect() }.disabled(parsed == nil || busy || URL(string: exchangeURL) == nil)
                        if exchangeChoice != 0 {
                            TextField("trader name (optional)", text: $enrolName)
                            Button("Enrol a new pilot on this dev world") { enrol() }.disabled(busy || URL(string: exchangeURL) == nil)
                            Text("Dev worlds that allow anonymous enrolment mint a key here. PROD issues keys through UCF Haul.").font(.caption).foregroundStyle(.secondary)
                        }
                    } else {
                        TextField("name (Luke's fleet)", text: $hostName)
                        TextField("fleet feed URL", text: $feedURL).textInputAutocapitalization(.never).autocorrectionDisabled().keyboardType(.URL)
                        SecureField("feed bearer", text: $bearer)
                        Button("Add host") { addHost() }.disabled(busy || URL(string: feedURL) == nil || bearer.trimmingCharacters(in: .whitespaces).isEmpty)
                        Text("A familiar host runs the pilot (whisker) and serves the fleet feed. Today that is a Mac of yours; in production, a server farm.").font(.caption).foregroundStyle(.secondary)
                    }
                }
                Section("Her voice") { VoicePicker(speaker: speaker) }
                Section("Her brains") {
                    Toggle("Private Cloud Compute", isOn: $consentPCC)
                    Text("On the device by default: Apple Intelligence answers from the journal and the wire. Private Cloud Compute (OS 27) lets her reason over a whole day; nothing is stored off the device.").font(.caption).foregroundStyle(.secondary)
                }
                if let o = outcome { Section { Text(o).font(.footnote).foregroundStyle(SC.amber) } }
            }
            .scrollContentBackground(.hidden).background(SC.bg)
            .navigationTitle("UCF Familiar")
            .toolbar { if !connections.connections.isEmpty { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } } }
        }
        .preferredColorScheme(.dark)
    }

    func addDirect() {
        guard let k = parsed else { return }
        busy = true
        Task {
            // The key answers for itself before it is kept.
            if let c = ExchangeClient(server: exchangeURL, key: k.secret) {
                do {
                    let p = try await c.profile()
                    let name = (p.traderName ?? "captain") + " · " + KnownExchange.name(for: exchangeURL)
                    connections.add(.direct(name: name, exchangeURL: exchangeURL, keyID: k.keyID), secret: k.secret)
                    outcome = "Added. \(p.traderName ?? "The key") answers on \(KnownExchange.name(for: exchangeURL))."
                    parsed = nil
                    dismiss()
                } catch { outcome = "The key did not answer: \(error)" }
            }
            busy = false
        }
    }

    func enrol() {
        busy = true
        Task {
            do {
                let device = UserDefaults.standard.string(forKey: "ucf.deviceId") ?? { let d = UUID().uuidString; UserDefaults.standard.set(d, forKey: "ucf.deviceId"); return d }()
                let r = try await DirectFeed.enrol(exchange: exchangeURL, traderName: enrolName, deviceID: device)
                if case .success(let k) = PairingKey.parse(r.key) {
                    connections.add(.direct(name: r.traderName + " · " + KnownExchange.name(for: exchangeURL), exchangeURL: exchangeURL, keyID: k.keyID), secret: k.secret)
                    outcome = r.welcome.isEmpty ? "Enrolled as \(r.traderName)." : r.welcome
                    dismiss()
                } else { outcome = "The exchange answered with a key this app does not recognise." }
            } catch { outcome = "Enrolment refused: \(error)" }
            busy = false
        }
    }

    func addHost() {
        connections.add(.host(name: hostName.isEmpty ? "familiar host" : hostName, feedURL: feedURL), secret: bearer.trimmingCharacters(in: .whitespaces))
        bearer = ""; dismiss()
    }
}
