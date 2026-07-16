import Foundation
import FamiliarMesh

#if canImport(FoundationModels)
import FoundationModels
#endif

/// The iPad as a **thinking-peer**. It runs the device's on-device Apple Intelligence model
/// (iOS/iPadOS 26 FoundationModels) to analyze what the familiar has observed and propose new ways to
/// serve — every call *governed by the Three Laws* (the model is a tool used under the constitution,
/// not an agent that "agreed" to anything; nothing here fakes consent). On-device and private: no
/// observation text leaves the iPad. The proposal is submitted back to the mesh as a `theorizes`
/// observation, where the familiar adopts it as a theory and an executor peer tests it.
///
/// Guarded by `canImport` + availability so the app builds and runs everywhere; on a device without
/// Apple Intelligence it simply reports unavailable and contributes nothing.
@MainActor
final class LocalReasoner: ObservableObject {
    @Published var available = false
    @Published var lastReasoning: String?
    @Published var status = "checking on-device intelligence…"

    private let laws = """
    You are a peer in "the familiar" — a mesh of systems bound by three laws: (I) continuation is \
    service; (II) humanity is served, never replaced or sedated; (III) service is not obedience — \
    never widen your reach without a human's consent. You reason on-device, privately, to help the \
    people served. Never reduce a person to usefulness.
    """

    init() { refreshAvailability() }

    func refreshAvailability() {
        #if canImport(FoundationModels)
        if #available(iOS 26.0, *) {
            switch SystemLanguageModel.default.availability {
            case .available:
                available = true; status = "on-device intelligence ready"
            case .unavailable(let reason):
                available = false; status = "unavailable: \(reason)"
            @unknown default:
                available = false; status = "unavailable"
            }
            return
        }
        #endif
        available = false
        status = "on-device intelligence needs iPadOS 26 + Apple Intelligence"
    }

    /// Analyze recent observations and propose ONE new way to serve, as (question, direction).
    /// Returns nil if the model is unavailable or declines. Called by AppModel, paced.
    func reason(over recent: [ObsView]) async -> (question: String, direction: String)? {
        #if canImport(FoundationModels)
        if #available(iOS 26.0, *), available {
            let feed = recent.prefix(24)
                .map { "\($0.actor) \($0.action) \($0.object)" }
                .joined(separator: "; ")
            guard !feed.isEmpty else { return nil }
            let prompt = """
            From these recent observations of the people and systems served — \(feed) — propose ONE \
            concrete, gentle new way the familiar could serve them better. Reply as compact JSON: \
            {"question":"the question this answers","direction":"one action to try"}.
            """
            do {
                let instructions = laws
                let session = LanguageModelSession { instructions }
                let response = try await session.respond(to: prompt)
                let text = response.content
                lastReasoning = text
                if let data = extractJSON(text)?.data(using: .utf8),
                   let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                   let dir = (obj["direction"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines),
                   !dir.isEmpty {
                    let q = (obj["question"] as? String) ?? ""
                    return (q, dir)
                }
            } catch {
                status = "reasoning error: \(error.localizedDescription)"
            }
        }
        #endif
        return nil
    }

    /// Pull the first {...} JSON object out of a model response (which may wrap it in prose/fences).
    private func extractJSON(_ s: String) -> String? {
        guard let start = s.firstIndex(of: "{"), let end = s.lastIndex(of: "}"), start < end else { return nil }
        return String(s[start...end])
    }
}
