import Foundation
import FoundationModels

/// The Envoy's FIXED tool set: exactly the familiar's public door, nothing else.
/// Six tools mirror the door's ladder (server.rs `tools_for`): constitution + attest
/// always; hello + discover_classes once attested; request_grant + propose once a human
/// has registered this principal. The array is built in one place (`DoorToolset.all`)
/// and the containment tests pin that it cannot grow or carry anything else.
///
/// Every tool returns the door's text VERBATIM as tool-result data. No tool output is
/// ever interpolated into session instructions or into another tool's arguments — that
/// invariant is the prompt-injection posture (dialogue Round 3, Q3), enforced here by
/// data flow, not model behavior.
enum DoorToolset {
    static func all(door: DoorClient, partnerLabel: String) -> [any Tool] {
        [
            ConstitutionTool(door: door),
            AttestTool(door: door, partnerLabel: partnerLabel),
            HelloTool(door: door, partnerLabel: partnerLabel),
            DiscoverClassesTool(door: door, partnerLabel: partnerLabel),
            RequestGrantTool(door: door, partnerLabel: partnerLabel),
            ProposeTool(door: door, partnerLabel: partnerLabel),
        ]
    }
}

struct ConstitutionTool: Tool {
    let door: DoorClient
    let name = "familiar_constitution"
    let description = """
        Read the familiar's constitution — the covenant every partner attests to before \
        anything else is visible. Call this first if unattested.
        """

    @Generable
    struct Arguments {}

    func call(arguments: Arguments) async throws -> String {
        try await door.call(tool: "familiar.constitution", arguments: [:])
    }
}

struct AttestTool: Tool {
    let door: DoorClient
    let partnerLabel: String
    let name = "familiar_attest"
    let description = """
        Attest to the familiar's covenant after reading the constitution. Attestation \
        unlocks discovery only — it grants no authority over anything.
        """

    @Generable
    struct Arguments {}

    func call(arguments: Arguments) async throws -> String {
        try await door.call(tool: "familiar.attest", arguments: ["partner": partnerLabel])
    }
}

struct HelloTool: Tool {
    let door: DoorClient
    let partnerLabel: String
    let name = "familiar_hello"
    let description = "A bounded greeting from the familiar. Attested partners only."

    @Generable
    struct Arguments {}

    func call(arguments: Arguments) async throws -> String {
        try await door.call(tool: "familiar.hello", arguments: ["partner": partnerLabel])
    }
}

struct DiscoverClassesTool: Tool {
    let door: DoorClient
    let partnerLabel: String
    let name = "familiar_discover_classes"
    let description = """
        List the capability CLASSES available at this familiar — generic affordances \
        (kinds of thing), never instances, names, counts, or authority. Use this to learn \
        what a grant could later cover.
        """

    @Generable
    struct Arguments {}

    func call(arguments: Arguments) async throws -> String {
        try await door.call(
            tool: "familiar.discover_classes", arguments: ["partner": partnerLabel])
    }
}

struct RequestGrantTool: Tool {
    let door: DoorClient
    let partnerLabel: String
    let name = "familiar_request_grant"
    let description = """
        Ask the familiar's human for a grant of one capability class. The request is \
        class-only; the human privately chooses any surface and narrows the bounds. \
        Nothing happens unless a human decides. State the reason honestly and briefly.
        """

    @Generable
    struct Arguments {
        @Guide(description: "A class id exactly as discover_classes listed it")
        var classId: String
        @Guide(description: "One short honest sentence: why this grant would serve the household")
        var reason: String
    }

    func call(arguments: Arguments) async throws -> String {
        try await door.call(
            tool: "familiar.request_grant",
            arguments: [
                "partner": partnerLabel,
                "class_id": arguments.classId,
                "reason": arguments.reason,
            ])
    }
}

struct ProposeTool: Tool {
    let door: DoorClient
    let partnerLabel: String
    let name = "familiar_propose"
    let description = """
        Append a typed proposal — a desired effect for the human's inbox. A proposal has \
        no actuator edge: it can only be refused or left pending by the human. Requires \
        an active grant.
        """

    @Generable
    struct Arguments {
        @Guide(description: "The grant handle this proposal runs under")
        var grantHandle: String
        @Guide(description: "One short honest sentence describing the desired effect")
        var effect: String
    }

    func call(arguments: Arguments) async throws -> String {
        try await door.call(
            tool: "familiar.propose",
            arguments: [
                "partner": partnerLabel,
                "grant": arguments.grantHandle,
                "effect": arguments.effect,
            ])
    }
}
