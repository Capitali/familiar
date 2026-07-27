import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

/// Runs a consult prompt through the device's own model — Apple Intelligence — and returns the
/// answer as an opaque string the familiar's `apple` provider hands back to the muse (ADR-0014).
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

    /// Answer one prompt. Returns the model's response text (opaque to the seam), or nil if the model
    /// can't be reached — a sleeping/ineligible device is silence, never a fabricated answer.
    static func answer(_ prompt: String) async -> String? {
        #if canImport(FoundationModels)
        if #available(iOS 26.0, *) {
            guard case .available = SystemLanguageModel.default.availability else { return nil }
            do {
                let session = LanguageModelSession()
                let response = try await session.respond(to: prompt)
                return response.content
            } catch {
                return nil
            }
        }
        #endif
        return nil
    }
}
