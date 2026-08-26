// The familiar's App Intents — the first T-227 sweep brick (read-only, decided in the
// adoption dialogue rounds 2–3).
//
// Three fences, all structural:
// 1. EXTERNAL-INDEXED PROJECTION: everything an intent says comes from IntentProjection
//    (FamiliarMesh) — counts, canonical kinds, the oracle line, the FACT of an open
//    question. No names, no observation text, no entity identifiers an index could grow
//    a household graph from. Widening this means widening the projection type, which is
//    a reviewed act.
// 2. SIDE-EFFECT FREEDOM: perform() reads a cached projection and returns words. It
//    marks nothing seen, answers nothing, mints no observation, stages nothing, and
//    donates no entities. "Read-only" is the whole transaction.
// 3. NO REASONING: these intents consult no model, so no `allow_llm` question arises.
//    An intent that reasons rides the full ADR-0038 stack the day one is proposed.
//
// authenticationPolicy is stricter than the projection strictly needs (the content is
// kind-only by construction): nothing is served on a locked device at all. Loosening to
// `.alwaysAllowed` would be defensible under the projection fence; it is deliberately
// not done without a dialogue round saying so.

import AppIntents
import FamiliarMesh
import Foundation

/// "What has the familiar noticed?" — the kind-only glance.
struct FamiliarNoticedIntent: AppIntent {
    static var title: LocalizedStringResource = "Check the Familiar"
    static var description = IntentDescription(
        "A kind-only glance at what the familiar holds: counts and service kinds, never names."
    )
    static var supportedModes: IntentModes = .background
    static var authenticationPolicy: IntentAuthenticationPolicy = .requiresAuthentication

    func perform() async throws -> some IntentResult & ProvidesDialog {
        // stored() is the freshness fence: absent, expired, or severed all read as nil, and
        // the honest answer is the same — this device has no CURRENT reading to speak.
        guard let p = IntentProjection.stored() else {
            return .result(
                dialog: "No fresh reading from the familiar — open the app to refresh."
            )
        }
        var line =
            "The familiar holds \(p.observationCount) observations across \(p.peerCount) peer\(p.peerCount == 1 ? "" : "s")."
        if !p.serviceKinds.isEmpty {
            line += " Services around: \(p.serviceKinds.joined(separator: ", "))."
        }
        if p.openQuestion {
            line += " It is holding an open question."
        }
        return .result(dialog: IntentDialog(stringLiteral: line))
    }
}

/// "How is the familiar's oracle?" — the on-device model's availability line, verbatim.
struct FamiliarOracleIntent: AppIntent {
    static var title: LocalizedStringResource = "Familiar Oracle State"
    static var description = IntentDescription(
        "The on-device model's availability — a statement about the model, not the household."
    )
    static var supportedModes: IntentModes = .background
    static var authenticationPolicy: IntentAuthenticationPolicy = .requiresAuthentication

    func perform() async throws -> some IntentResult & ProvidesDialog {
        let line = IntentProjection.stored()?.oracleLine
        return .result(
            dialog: IntentDialog(
                stringLiteral: line ?? "No fresh oracle reading — open the app to refresh."
            )
        )
    }
}

/// Instant Siri/Spotlight availability, no user setup (phrases must carry the app name).
struct FamiliarAppShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: FamiliarNoticedIntent(),
            phrases: [
                "What has \(.applicationName) noticed",
                "Check \(.applicationName)",
            ],
            shortTitle: "Check the Familiar",
            systemImageName: "circle.hexagongrid"
        )
        AppShortcut(
            intent: FamiliarOracleIntent(),
            phrases: [
                "How is \(.applicationName)'s oracle",
                "Is \(.applicationName) ready",
            ],
            shortTitle: "Oracle State",
            systemImageName: "brain"
        )
    }
}
