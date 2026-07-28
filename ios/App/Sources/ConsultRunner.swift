import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

// Guided-generation schemas (ADR-0014). Each matches one shape of the muse's compact-JSON contract.
// `@Generable` constrains the on-device model to emit exactly these fields — which both guarantees
// valid JSON (no ```json fences, no prose) and tends to stop a small model hanging on an open-ended
// "write a script" the way free-form `respond(to:)` does.
#if canImport(FoundationModels)
@available(iOS 26.0, *)
@Generable
struct ScriptAnswer {
    @Guide(description: "A self-contained POSIX /bin/sh script that takes one safe step toward the goal, writing only under the current directory and reading/transmitting no personal data.")
    var script: String
}

@available(iOS 26.0, *)
@Generable
struct TheoryAnswer {
    @Guide(description: "One concrete question the observations raise.")
    var question: String
    @Guide(description: "A short, testable theory that would answer it.")
    var theory: String
    @Guide(description: "One next step that would test the theory.")
    var direction: String
}
#endif

/// Runs a consult prompt through the device's own model — Apple Intelligence — and returns the
/// answer as an opaque JSON string the familiar's `apple` provider hands back to the muse (ADR-0014).
/// On-device only; Private Cloud Compute is a separate consent, off here. Where FoundationModels or
/// Apple Intelligence isn't present it returns `nil` and the familiar's provider chain simply rolls on.
enum ConsultRunner {
    /// Whether this device can actually answer a consult right now.
    static var available: Bool {
        #if canImport(FoundationModels)
        if #available(iOS 26.0, *) {
            if case .available = SystemLanguageModel.default.availability { return true }
        }
        #endif
        return false
    }

    /// A short, human-readable reason for the console's Device screen.
    static var state: String {
        #if canImport(FoundationModels)
        if #available(iOS 26.0, *) {
            switch SystemLanguageModel.default.availability {
            case .available: return "available"
            case .unavailable(.deviceNotEligible): return "model-missing"
            case .unavailable(.appleIntelligenceNotEnabled): return "apple-intelligence-off"
            case .unavailable(.modelNotReady): return "model-loading"
            case .unavailable: return "unavailable"
            }
        }
        #endif
        return "unsupported"
    }

    /// Answer one prompt, using the generation strategy named by `kind` (ADR-0014):
    ///   "script" → guided generation into `ScriptAnswer`  → `{"script": …}`
    ///   "theory" → guided generation into `TheoryAnswer`  → `{"question":…,"theory":…,"direction":…}`
    ///   otherwise ("" / "free") → free-form `respond(to:)`, returned verbatim.
    /// Returns the model's response as a JSON string (opaque to the seam), or nil if the model can't
    /// be reached — a sleeping/ineligible device is silence, never a fabricated answer.
    ///
    /// Bounded by `timeout`: on-device generation can *hang* on some prompts (a guardrail stall, an
    /// open-ended ask the model chews on forever). Because the caller (`serviceConsults`) serializes on
    /// a `servicingConsults` flag reset only when this returns, an unbounded hang would pin that flag
    /// true and wedge every future consult until the app is killed — exactly the failure that stalled
    /// the A9 campaign every 1-2 cells. Racing generation against a sleep guarantees this returns, so a
    /// stuck generation degrades to "this device didn't answer that one" (the adapter retries) and the
    /// loop self-heals. A late-finishing generation is simply discarded.
    static func answer(_ prompt: String, kind: String? = nil, timeout: TimeInterval = 90) async -> String? {
        #if canImport(FoundationModels)
        if #available(iOS 26.0, *) {
            guard case .available = SystemLanguageModel.default.availability else { return nil }
            return await withTaskGroup(of: String?.self) { group in
                group.addTask { await generate(prompt, kind: kind) }
                group.addTask {
                    try? await Task.sleep(nanoseconds: UInt64(timeout * 1_000_000_000))
                    return nil   // timeout sentinel — indistinguishable from silence, which is correct
                }
                let first = await group.next() ?? nil
                group.cancelAll()
                return first
            }
        }
        #endif
        return nil
    }

    #if canImport(FoundationModels)
    /// The actual generation, dispatched by strategy. Returns a JSON string or nil on any failure.
    @available(iOS 26.0, *)
    private static func generate(_ prompt: String, kind: String?) async -> String? {
        do {
            let session = LanguageModelSession()
            switch (kind ?? "").lowercased() {
            case "script":
                let a = try await session.respond(to: prompt, generating: ScriptAnswer.self).content
                return jsonString(["script": a.script])
            case "theory":
                let a = try await session.respond(to: prompt, generating: TheoryAnswer.self).content
                return jsonString(["question": a.question, "theory": a.theory, "direction": a.direction])
            default:
                return try await session.respond(to: prompt).content
            }
        } catch {
            return nil
        }
    }

    /// Encode a string→string map to a compact JSON string with correct escaping.
    private static func jsonString(_ dict: [String: String]) -> String? {
        guard let data = try? JSONSerialization.data(withJSONObject: dict, options: []) else { return nil }
        return String(data: data, encoding: .utf8)
    }
    #endif
}
