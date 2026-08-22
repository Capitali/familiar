import SwiftUI
import FoundationModels

/// The Envoy's one screen: a small conversation with the on-device model, whose only
/// reach is the familiar's public door — or an honest statement of why there is no model.
struct ContentView: View {
    @State private var transcript: [Line] = []
    @State private var input = ""
    @State private var busy = false
    @State private var readiness: EnvoySession.Readiness?

    /// The door this Envoy speaks to. Configurable later (brick 2 pairs it with the
    /// provisioned credential); the default is the household's public door.
    @AppStorage("door_origin") private var doorOrigin = "https://134.209.168.50:47100/mcp"
    @AppStorage("partner_label") private var partnerLabel = "envoy-on-device"
    /// The door's TLS key pin (SPKI SHA-256 hex) — the mesh cert is self-signed, so a
    /// pin is how the Envoy knows it reached the right familiar. Empty = system trust.
    @AppStorage("door_spki_pin") private var doorPin = ""
    /// The bearer the door demands even from unregistered callers (the door token the
    /// human issued). Brick 2's ceremony replaces this with the Envoy's own credential.
    @AppStorage("door_token") private var doorToken = ""

    struct Line: Identifiable {
        let id = UUID()
        let role: String
        let text: String
    }

    var body: some View {
        VStack(spacing: 0) {
            switch readiness {
            case .none:
                ProgressView().task { readiness = makeSession() }
                Spacer()
            case .modelUnavailable(let why):
                ContentUnavailableView(
                    "The Envoy has no model", systemImage: "brain",
                    description: Text(why))
            case .ready(let session):
                conversation
                composer(session: session)
            }
        }
        .navigationTitle("Envoy")
        .frame(minWidth: 420, minHeight: 480)
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

    private func makeSession() -> EnvoySession.Readiness {
        guard let origin = URL(string: doorOrigin) else {
            return .modelUnavailable("The configured door origin is not a URL: \(doorOrigin)")
        }
        return EnvoySession.make(
            origin: origin, credential: doorToken.isEmpty ? nil : doorToken,
            spkiPin: doorPin.isEmpty ? nil : doorPin, partnerLabel: partnerLabel)
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
