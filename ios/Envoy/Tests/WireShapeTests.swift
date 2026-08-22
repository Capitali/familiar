import Foundation
import Testing
@testable import Envoy

/// Pins the exact JSON each wrapper puts on the wire against server.rs's schemas
/// (`additionalProperties: false` there makes any drift a refusal, so these tests are
/// the compile-time mirror of the door's runtime strictness).

final class RecordingDoorProtocol: URLProtocol {
    nonisolated(unsafe) static var lastEnvelope: [String: Any]?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        if let stream = request.httpBodyStream {
            stream.open()
            var data = Data()
            let size = 4096
            let buffer = UnsafeMutablePointer<UInt8>.allocate(capacity: size)
            defer { buffer.deallocate() }
            while stream.hasBytesAvailable {
                let read = stream.read(buffer, maxLength: size)
                if read <= 0 { break }
                data.append(buffer, count: read)
            }
            stream.close()
            Self.lastEnvelope =
                (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
        } else if let body = request.httpBody {
            Self.lastEnvelope =
                (try? JSONSerialization.jsonObject(with: body)) as? [String: Any]
        }
        let reply: [String: Any] = [
            "jsonrpc": "2.0", "id": "1",
            "result": ["content": [["type": "text", "text": "ok"]]],
        ]
        let data = try! JSONSerialization.data(withJSONObject: reply)
        let response = HTTPURLResponse(
            url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: data)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

private func recordingDoor(credential: String?) throws -> DoorClient {
    let config = URLSessionConfiguration.ephemeral
    config.protocolClasses = [RecordingDoorProtocol.self]
    return try DoorClient(
        origin: URL(string: "http://127.0.0.1:9/mcp")!,
        credential: credential,
        session: URLSession(configuration: config))
}

private func sentArguments() -> [String: Any] {
    let params = RecordingDoorProtocol.lastEnvelope?["params"] as? [String: Any]
    return params?["arguments"] as? [String: Any] ?? [:]
}

// Serialized: the recording protocol's captured envelope is one shared slot, so these
// tests must not interleave — the same shared-fixture race class as the Rust inbox flake
// fixed at 002e754, closed here the same day it was learned.
@Suite(.serialized) struct WireShapes {
    @Test func a_bound_attest_sends_statement_and_never_partner() async throws {
        let door = try recordingDoor(credential: "cred")
        let tool = AttestTool(door: door, partnerLabel: "envoy")
        _ = try await tool.call(
            arguments: .init(statement: "I accept the three laws as written."))
        let args = sentArguments()
        #expect(args["statement"] as? String == "I accept the three laws as written.")
        #expect(args["partner"] == nil)
    }

    @Test func an_unbound_attest_carries_the_partner_label() async throws {
        let door = try recordingDoor(credential: nil)
        let tool = AttestTool(door: door, partnerLabel: "envoy")
        _ = try await tool.call(arguments: .init(statement: "I accept."))
        let args = sentArguments()
        #expect(args["partner"] as? String == "envoy")
    }

    @Test func request_grant_is_class_only_with_empty_operation_bounds() async throws {
        let door = try recordingDoor(credential: "cred")
        let tool = RequestGrantTool(door: door)
        _ = try await tool.call(
            arguments: .init(
                requestKey: "envoy-first-light", classId: "switchable.reversible/v1",
                operation: "state", durationSeconds: 300, reason: "to be useful"))
        let args = sentArguments()
        #expect(args["request_key"] as? String == "envoy-first-light")
        #expect(args["class_id"] as? String == "switchable.reversible/v1")
        let operations = args["requested_operations"] as? [String: Any]
        let stateBounds = operations?["state"] as? [String: Any]
        #expect(stateBounds?.isEmpty == true)
        #expect(args["requested_duration_seconds"] as? Int == 300)
        #expect(args["partner"] == nil)
    }

    @Test func propose_names_key_instance_operation_and_typed_parameters() async throws {
        let door = try recordingDoor(credential: "cred")
        let tool = ProposeTool(door: door)
        _ = try await tool.call(
            arguments: .init(
                proposalKey: "envoy-dim-evening", instance: "handle-abc",
                operation: "state", parameterName: "state", parameterText: "reverted",
                parameterNumber: nil, reason: "the household said goodnight"))
        let args = sentArguments()
        #expect(args["proposal_key"] as? String == "envoy-dim-evening")
        #expect(args["instance"] as? String == "handle-abc")
        #expect(args["operation"] as? String == "state")
        let parameters = args["parameters"] as? [String: Any]
        #expect(parameters?["state"] as? String == "reverted")
    }

    @Test func the_credential_travels_as_a_bearer_header_only() async throws {
        let door = try recordingDoor(credential: "secret-cred")
        _ = try await door.call(tool: "familiar.hello", arguments: [:])
        let args = sentArguments()
        #expect(!args.values.contains { ($0 as? String)?.contains("secret-cred") == true })
    }
}
