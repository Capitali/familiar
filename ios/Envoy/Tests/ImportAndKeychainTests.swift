import Foundation
import Testing
@testable import Envoy

/// The credential seam is machinery, not commentary (reciprocal review, blocker 1):
/// the v1 bundle parses and validates; the bearer lives in this app's own Keychain item
/// and round-trips; the wire refuses an answer whose id is not the question's.

private func bundleJSON(
    version: Int = 1, origin: String = "https://134.209.168.50:47100/mcp",
    token: String = "tok"
) -> Data {
    Data(
        """
        {"version": \(version), "registration_id": "registration-abc",
         "alias": "Envoy (on-device)", "mcp_origin": "\(origin)", "bearer_token": "\(token)"}
        """.utf8)
}

@Suite struct ImportBundleValidation {
    @Test func a_v1_bundle_parses_with_the_script_field_names() throws {
        let bundle = try ImportBundle.parse(bundleJSON())
        #expect(bundle.registrationId == "registration-abc")
        #expect(bundle.alias == "Envoy (on-device)")
        #expect(bundle.bearerToken == "tok")
    }

    @Test func an_unknown_version_is_refused() {
        #expect(throws: ImportBundle.BundleError.self) {
            _ = try ImportBundle.parse(bundleJSON(version: 2))
        }
    }

    @Test func a_plain_http_origin_off_loopback_is_refused() {
        #expect(throws: ImportBundle.BundleError.self) {
            _ = try ImportBundle.parse(bundleJSON(origin: "http://134.209.168.50:47100/mcp"))
        }
    }

    @Test func a_non_mcp_path_is_refused_and_loopback_http_is_fixture_legal() throws {
        #expect(throws: ImportBundle.BundleError.self) {
            _ = try ImportBundle.parse(bundleJSON(origin: "https://134.209.168.50:47100/"))
        }
        _ = try ImportBundle.parse(bundleJSON(origin: "http://127.0.0.1:9/mcp"))
    }

    @Test func an_empty_bearer_is_refused() {
        #expect(throws: ImportBundle.BundleError.self) {
            _ = try ImportBundle.parse(bundleJSON(token: ""))
        }
    }
}

@Suite(.serialized) struct KeychainSeam {
    static let testService = "io.river.envoy.test-credential"

    @Test func the_bearer_round_trips_through_this_apps_keychain_item() {
        defer { EnvoyKeychain.deleteBearer(service: Self.testService) }
        #expect(EnvoyKeychain.storeBearer("secret-token", service: Self.testService))
        #expect(EnvoyKeychain.loadBearer(service: Self.testService) == "secret-token")
        #expect(EnvoyKeychain.deleteBearer(service: Self.testService))
        #expect(EnvoyKeychain.loadBearer(service: Self.testService) == nil)
    }

    @Test func storing_again_replaces_rather_than_duplicates() {
        defer { EnvoyKeychain.deleteBearer(service: Self.testService) }
        #expect(EnvoyKeychain.storeBearer("first", service: Self.testService))
        #expect(EnvoyKeychain.storeBearer("second", service: Self.testService))
        #expect(EnvoyKeychain.loadBearer(service: Self.testService) == "second")
    }
}

/// A door that answers with a mismatched JSON-RPC id — the client must refuse it.
final class WrongIdDoorProtocol: URLProtocol {
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let reply: [String: Any] = [
            "jsonrpc": "2.0", "id": "someone-elses-question",
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

@Suite struct IdCorrelation {
    @Test func an_answer_to_a_different_question_is_malformed() async throws {
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [WrongIdDoorProtocol.self]
        let door = try DoorClient(
            origin: URL(string: "http://127.0.0.1:9/mcp")!,
            session: URLSession(configuration: config))
        await #expect(throws: DoorClient.DoorError.self) {
            _ = try await door.call(tool: "familiar.hello", arguments: [:])
        }
    }
}
