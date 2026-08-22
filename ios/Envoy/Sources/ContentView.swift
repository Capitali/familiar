import SwiftUI
import FoundationModels
import UniformTypeIdentifiers

/// The Envoy's one screen: a small conversation with the on-device model, whose only
/// reach is the familiar's public door — or an honest statement of why there is no model.
/// The bearer lives ONLY in this app's Keychain item; the import sheet is the single way
/// it gets there (from Brick 2's provisioning bundle).
struct ContentView: View {
    @State private var transcript: [Line] = []
    @State private var input = ""
    @State private var busy = false
    @State private var readiness: EnvoySession.Readiness?
    @State private var doorStatus: EnvoySession.DoorStatus?
    @State private var showImport = false
    @State private var importError: String?
    @State private var hasCredential = EnvoyKeychain.loadBearer() != nil

    // Non-secret configuration. The bearer is deliberately NOT here.
    @AppStorage("door_origin") private var doorOrigin = "https://134.209.168.50:47100/mcp"
    @AppStorage("door_spki_pin") private var doorPin = ""
    @AppStorage("partner_label") private var partnerLabel = "envoy-on-device"
    @AppStorage("registration_id") private var registrationId = ""

    struct Line: Identifiable {
        let id = UUID()
        let role: String
        let text: String
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            switch readiness {
            case .none:
                ProgressView().task { await refresh() }
                Spacer()
            case .modelUnavailable(let why):
                ContentUnavailableView(
                    "The Envoy has no model", systemImage: "brain",
                    description: Text(why))
            case .ready(let session, _):
                conversation
                composer(session: session)
            }
        }
        .navigationTitle("Envoy")
        .frame(minWidth: 440, minHeight: 500)
        .sheet(isPresented: $showImport) { importSheet }
    }

    private var header: some View {
        HStack(spacing: 8) {
            Circle()
                .fill(doorStatus?.bound == true ? .green : (doorStatus?.reachable == true ? .yellow : .red))
                .frame(width: 8, height: 8)
            Text(doorStatus?.note ?? "checking the door…")
                .font(.caption).foregroundStyle(.secondary)
                .lineLimit(1)
            Spacer()
            Button(hasCredential ? "Re-import credential" : "Import credential…") {
                importError = nil
                showImport = true
            }
            .font(.caption)
        }
        .padding(.horizontal)
        .padding(.vertical, 6)
    }

    private var conversation: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 10) {
                ForEach(transcript) { line in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(line.role).font(.caption2).foregroundStyle(.secondary)
                        Text(line.text).textSelection(.enabled)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .padding()
        }
    }

    private func composer(session: LanguageModelSession) -> some View {
        HStack {
            TextField("Ask the Envoy…", text: $input)
                .textFieldStyle(.roundedBorder)
                .onSubmit { send(session: session) }
            Button("Send") { send(session: session) }
                .disabled(busy || input.trimmingCharacters(in: .whitespaces).isEmpty)
        }
        .padding()
    }

    // MARK: Import

    @State private var deleteAfterImport = true
    @State private var pinField = ""

    private var importSheet: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Import the Envoy's credential").font(.headline)
            Text(
                "Choose the provisioning bundle (envoy-import.json). Its bearer goes into "
                    + "this app's own Keychain and nowhere else. The door's TLS pin is not "
                    + "secret — paste it so the Envoy knows it reached the right familiar."
            )
            .font(.caption).foregroundStyle(.secondary)
            TextField("Door SPKI pin (SHA-256 hex)", text: $pinField)
                .textFieldStyle(.roundedBorder).font(.system(.caption, design: .monospaced))
            Toggle("Delete the bundle file after import", isOn: $deleteAfterImport)
            if let importError {
                Text(importError).font(.caption).foregroundStyle(.red)
            }
            HStack {
                Spacer()
                Button("Cancel") { showImport = false }
                Button("Choose bundle…") { importBundleFile() }
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding()
        .frame(width: 440)
        .onAppear { pinField = doorPin }
    }

    private func importBundleFile() {
        #if os(macOS)
            let panel = NSOpenPanel()
            panel.allowedContentTypes = [.json]
            panel.allowsMultipleSelection = false
            panel.canChooseDirectories = false
            guard panel.runModal() == .OK, let url = panel.url else { return }
            do {
                let bundle = try ImportBundle.parse(try Data(contentsOf: url))
                guard EnvoyKeychain.storeBearer(bundle.bearerToken) else {
                    importError = "the Keychain refused the bearer"
                    return
                }
                doorOrigin = bundle.mcpOrigin
                registrationId = bundle.registrationId
                doorPin = pinField.trimmingCharacters(in: .whitespaces).lowercased()
                if deleteAfterImport { try? FileManager.default.removeItem(at: url) }
                hasCredential = true
                showImport = false
                readiness = nil  // re-probe with the new credential
            } catch {
                importError = String(describing: error)
            }
        #else
            importError = "import on iOS arrives with the iOS build"
        #endif
    }

    // MARK: Session

    private func refresh() async {
        guard let origin = URL(string: doorOrigin) else {
            readiness = .modelUnavailable("The configured door origin is not a URL: \(doorOrigin)")
            return
        }
        let result = await EnvoySession.make(
            origin: origin, credential: EnvoyKeychain.loadBearer(),
            spkiPin: doorPin.isEmpty ? nil : doorPin, partnerLabel: partnerLabel)
        if case .ready(_, let status) = result { doorStatus = status }
        readiness = result
    }

    private func send(session: LanguageModelSession) {
        let prompt = input.trimmingCharacters(in: .whitespaces)
        guard !prompt.isEmpty, !busy else { return }
        input = ""
        transcript.append(Line(role: "you", text: prompt))
        busy = true
        Task {
            defer { busy = false }
            do {
                let response = try await session.respond(to: prompt)
                transcript.append(Line(role: "envoy", text: response.content))
            } catch {
                transcript.append(Line(role: "envoy · error", text: String(describing: error)))
            }
        }
    }
}
