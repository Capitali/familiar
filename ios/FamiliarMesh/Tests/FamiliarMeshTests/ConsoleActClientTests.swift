import XCTest
@testable import FamiliarMesh

private final class ConsoleActURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        do {
            guard let handler = Self.handler else {
                throw URLError(.badServerResponse)
            }
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}

final class ConsoleActClientTests: XCTestCase {
    override func tearDown() {
        ConsoleActURLProtocol.handler = nil
        super.tearDown()
    }

    func testDisableRuleUsesSignedTaggedEnvelope() async throws {
        let node = try NodeKey(seed: Data(repeating: 0x44, count: 32), label: "iPad")
        let membership = try Cert.mint(
            groupSecret: Data(repeating: 0x22, count: 32),
            node: node.identity,
            issued: 1_780_000_000,
            ttlSecs: defaultCertTTLSecs
        )
        let url = try XCTUnwrap(ConsoleActClient.consoleActURL(host: "door.test", port: 47100))
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [ConsoleActURLProtocol.self]
        let urlSession = URLSession(configuration: config)
        var captured: URLRequest?
        ConsoleActURLProtocol.handler = { request in
            captured = request
            let response = HTTPURLResponse(
                url: try XCTUnwrap(request.url),
                statusCode: 200,
                httpVersion: nil,
                headerFields: nil
            )!
            return (response, Data("disabled rule abcd1234".utf8))
        }

        let session = ObservationClient.Session(node: node, membership: membership, url: url)
        let client = ConsoleActClient(session: session, urlSession: urlSession)
        let signedRequest = try client.makeRequest(
            .disableRule("abcd1234"),
            now: 1_780_000_100,
            nonce: "01020304"
        )
        let reply = try await client.send(
            .disableRule("abcd1234"),
            now: 1_780_000_100,
            nonce: "01020304"
        )

        XCTAssertEqual(reply, "disabled rule abcd1234")
        let request = try XCTUnwrap(captured)
        XCTAssertEqual(request.httpMethod, "POST")
        XCTAssertEqual(request.url?.path, "/mesh/console-act")
        let body = try XCTUnwrap(signedRequest.httpBody)
        let json = try XCTUnwrap(
            JSONSerialization.jsonObject(with: body) as? [String: Any]
        )
        let act = try XCTUnwrap(json["act"] as? [String: Any])
        XCTAssertEqual(act["kind"] as? String, "disable_rule")
        XCTAssertEqual(act["rule_id"] as? String, "abcd1234")
        XCTAssertNil(act["name"])
        XCTAssertEqual(json["nonce"] as? String, "01020304")
        let signature = try XCTUnwrap(
            signedRequest.value(forHTTPHeaderField: "X-Familiar-Sig")
        )
        XCTAssertTrue(
            node.signing.publicKey.isValidSignature(try XCTUnwrap(Hex.decode(signature)), for: body)
        )
    }

    func testNameDevicePayloadAndWorldviewRulesDecode() throws {
        let act = ConsoleActBody(.nameDevice("Aphelion"))
        let actJSON = try XCTUnwrap(
            JSONSerialization.jsonObject(with: JSONEncoder().encode(act)) as? [String: Any]
        )
        XCTAssertEqual(actJSON["kind"] as? String, "name_device")
        XCTAssertEqual(actJSON["name"] as? String, "Aphelion")
        XCTAssertNil(actJSON["rule_id"])

        let raw = Data(#"""
        {
          "group_label":"river","node_id":"door","presence":1,"withdrawn":false,
          "service":1,"capacity":1,"observation_count":0,"peers":[],"recent":[],
          "rules":[{"id":"abcd1234","sentence":"away → lights dim (for ian)",
                    "enabled":true,"disabled_reason":""}]
        }
        """#.utf8)
        let worldview = try JSONDecoder().decode(Worldview.self, from: raw)
        XCTAssertEqual(worldview.rules, [
            RuleView(
                id: "abcd1234",
                sentence: "away → lights dim (for ian)",
                enabled: true,
                disabled_reason: ""
            )
        ])
    }

    func testPartnerDecisionIsTypedAndCannotNameAHuman() throws {
        let bounds: PartnerOperationBounds = [
            "set_state": ["state": .enumeration(["on"])]
        ]
        let body = ConsoleActBody(.decideGrant(
            requestId: "request-1",
            surface: "reading-lamp",
            allowedOperations: bounds,
            expiresAt: 1_780_000_300
        ))
        let json = try XCTUnwrap(
            JSONSerialization.jsonObject(with: JSONEncoder().encode(body)) as? [String: Any]
        )
        XCTAssertEqual(json["kind"] as? String, "decide_grant")
        XCTAssertEqual(json["request_id"] as? String, "request-1")
        XCTAssertEqual(json["surface"] as? String, "reading-lamp")
        XCTAssertEqual(json["expires_at"] as? Int64, 1_780_000_300)
        XCTAssertNil(json["human"])
        XCTAssertNil(json["registered_by"])
    }

    func testPartnerInboxUsesItsOwnSignedRouteAndDecodesPrivateProjection() async throws {
        let node = try NodeKey(seed: Data(repeating: 0x55, count: 32), label: "iPad")
        let membership = try Cert.mint(
            groupSecret: Data(repeating: 0x22, count: 32),
            node: node.identity,
            issued: 1_780_000_000,
            ttlSecs: defaultCertTTLSecs
        )
        let url = try XCTUnwrap(PartnerInboxClient.inboxURL(host: "door.test", port: 47100))
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [ConsoleActURLProtocol.self]
        let urlSession = URLSession(configuration: config)
        var captured: URLRequest?
        let responseJSON = #"{"pending_requests":[],"active_grants":[],"pending_proposals":[],"warnings":[]}"#
        ConsoleActURLProtocol.handler = { request in
            captured = request
            let response = HTTPURLResponse(
                url: try XCTUnwrap(request.url),
                statusCode: 200,
                httpVersion: nil,
                headerFields: nil
            )!
            return (response, Data(responseJSON.utf8))
        }

        let session = ObservationClient.Session(node: node, membership: membership, url: url)
        let client = PartnerInboxClient(session: session, urlSession: urlSession)
        let signedRequest = try client.makeRequest(now: 1_780_000_100, nonce: "inbox-1")
        let (view, _) = try await client.fetchWithRaw(now: 1_780_000_100, nonce: "inbox-1")
        XCTAssertEqual(view.pending_requests, [])
        let request = try XCTUnwrap(captured)
        XCTAssertEqual(request.url?.path, "/mesh/partner-inbox")
        let body = try XCTUnwrap(signedRequest.httpBody)
        let json = try XCTUnwrap(
            JSONSerialization.jsonObject(with: body) as? [String: Any]
        )
        XCTAssertEqual(json["nonce"] as? String, "inbox-1")
        XCTAssertNil(json["human"])
        let signature = try XCTUnwrap(
            signedRequest.value(forHTTPHeaderField: "X-Familiar-Sig")
        )
        XCTAssertTrue(
            node.signing.publicKey.isValidSignature(
                try XCTUnwrap(Hex.decode(signature)), for: body
            )
        )
    }
}
