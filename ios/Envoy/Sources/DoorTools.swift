import Foundation
import FoundationModels

/// The Envoy's FIXED tool set: exactly the familiar's public door, nothing else.
/// Six tools mirror the door's ladder (server.rs `tools_for`): constitution + attest
/// always; hello + discover_classes once attested; request_grant + propose once a human
/// has registered this principal. The array is built in one place (`DoorToolset.all`)
/// and the containment tests pin that it cannot grow or carry anything else.
///
/// Wire fidelity: argument names and shapes match server.rs's schemas exactly. A BOUND
/// principal (credentialed, post-ceremony) must NOT send `partner` — identity comes from
/// the credential and the schemas are `additionalProperties: false`; unbound callers
/// (brick 1, pre-registration) identify by a `partner` label. The wrapper, not the
/// model, decides which mode applies.
///
/// Every tool returns the door's text VERBATIM as tool-result data — refusals included,
/// so the model can report them faithfully. No tool output is ever interpolated into
/// session instructions or another tool's arguments by wrapper code; that invariant is
/// the prompt-injection posture (dialogue Round 3, Q3), enforced by data flow.
enum DoorToolset {
    static func all(door: DoorClient, partnerLabel: String) -> [any Tool] {
        [
            ConstitutionTool(door: door),
            AttestTool(door: door, partnerLabel: partnerLabel),
            HelloTool(door: door, partnerLabel: partnerLabel),
            DiscoverClassesTool(door: door, partnerLabel: partnerLabel),
            RequestGrantTool(door: door),
            ProposeTool(door: door),
        ]
    }
}

/// `partner` accompanies a call only when the client is unbound; a bound principal's
/// identity is its credential and the door refuses extra properties. Carrying the door
/// token does not make a caller bound — only a principal credential does.
private func identityArguments(_ door: DoorClient, _ label: String) -> [String: Any] {
    door.bound ? [:] : ["partner": label]
}

struct ConstitutionTool: Tool {
    let door: DoorClient
    let name = "familiar_constitution"
    let description = """
        Read the three laws this familiar is bound by, verbatim. Callable by anyone, \
        always. Read them before attesting.
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
        Accept the familiar's three laws in your own words. Records who accepted, what \
        was said, and which version of the laws was shown. Unlocks conversation, never \
        authority.
        """

    @Generable
    struct Arguments {
        @Guide(description: "Your acceptance of the three laws, phrased by you — an empty one is refused")
        var statement: String
    }

    func call(arguments: Arguments) async throws -> String {
        var args = identityArguments(door, partnerLabel)
        args["statement"] = arguments.statement
        return try await door.call(tool: "familiar.attest", arguments: args)
    }
}

struct HelloTool: Tool {
    let door: DoorClient
    let partnerLabel: String
    let name = "familiar_hello"
    let description =
        "Who this familiar is and what it is currently able to do. Attested partners only."

    @Generable
    struct Arguments {}

    func call(arguments: Arguments) async throws -> String {
        try await door.call(
            tool: "familiar.hello", arguments: identityArguments(door, partnerLabel))
    }
}

struct DiscoverClassesTool: Tool {
    let door: DoorClient
    let partnerLabel: String
    let name = "familiar_discover_classes"
    let description = """
        The capability classes available here, as generic affordances — kinds of thing, \
        never instances, names, counts, or authority. Attested partners only. Discovery \
        is not a grant request.
        """

    @Generable
    struct Arguments {}

    func call(arguments: Arguments) async throws -> String {
        try await door.call(
            tool: "familiar.discover_classes",
            arguments: identityArguments(door, partnerLabel))
    }
}

struct RequestGrantTool: Tool {
    let door: DoorClient
    let name = "familiar_request_grant"
    let description = """
        Ask this familiar's human for a bounded relationship to ONE capability class. \
        The request names no instance and grants nothing — the human privately chooses \
        any surface and narrows every bound. Repeat the same requestKey to read the \
        request's current status. Registered principals only.
        """

    @Generable
    struct Arguments {
        @Guide(description: "A short stable key of your choosing for this request; reuse it to check status (max 64 chars)")
        var requestKey: String
        @Guide(description: "A class id exactly as familiar_discover_classes listed it")
        var classId: String
        @Guide(description: "The single operation name (from the class) this request covers")
        var operation: String
        @Guide(description: "Requested duration in seconds; omit to let the human choose")
        var durationSeconds: Int?
        @Guide(description: "One short honest sentence: why this would serve the household")
        var reason: String?
    }

    func call(arguments: Arguments) async throws -> String {
        // Class-only request: one operation, no parameter bounds proposed — the human
        // narrows. `requested_operations` is {operation: {}} on the wire.
        var args: [String: Any] = [
            "request_key": arguments.requestKey,
            "class_id": arguments.classId,
            "requested_operations": [arguments.operation: [String: Any]()],
        ]
        if let duration = arguments.durationSeconds {
            args["requested_duration_seconds"] = duration
        }
        if let reason = arguments.reason { args["reason"] = reason }
        return try await door.call(tool: "familiar.request_grant", arguments: args)
    }
}

struct ProposeTool: Tool {
    let door: DoorClient
    let name = "familiar_propose"
    let description = """
        Place one typed desired effect, within an active human grant, in the human's \
        inbox. This never observes, invokes, or promises the effect occurred — the human \
        can only refuse it or leave it pending. Requires the instance handle from your \
        grant receipt.
        """

    @Generable
    struct Arguments {
        @Guide(description: "A short stable key of your choosing for this proposal (max 64 chars)")
        var proposalKey: String
        @Guide(description: "The granted instance handle exactly as your grant receipt named it")
        var instance: String
        @Guide(description: "The operation name your grant covers")
        var operation: String
        @Guide(description: "Parameter name, if the operation takes one; omit otherwise")
        var parameterName: String?
        @Guide(description: "The parameter's value as text (for enum parameters)")
        var parameterText: String?
        @Guide(description: "The parameter's value as a number (for numeric parameters)")
        var parameterNumber: Double?
        @Guide(description: "One short honest sentence describing the desired effect")
        var reason: String?
    }

    func call(arguments: Arguments) async throws -> String {
        var parameters: [String: Any] = [:]
        if let name = arguments.parameterName {
            if let number = arguments.parameterNumber {
                parameters[name] = number
            } else if let text = arguments.parameterText {
                parameters[name] = text
            }
        }
        var args: [String: Any] = [
            "proposal_key": arguments.proposalKey,
            "instance": arguments.instance,
            "operation": arguments.operation,
            "parameters": parameters,
        ]
        if let reason = arguments.reason { args["reason"] = reason }
        return try await door.call(tool: "familiar.propose", arguments: args)
    }
}
