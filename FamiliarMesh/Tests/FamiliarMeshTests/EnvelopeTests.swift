import XCTest
@testable import FamiliarMesh

final class EnvelopeTests: XCTestCase {

    func testRoundTripSignVerifyEnvelope() throws {
        let node = try NodeKey(seed: Data(repeating: 0x33, count: 32), label: "iPhone")
        let m = try Cert.mint(
            groupSecret: Data(repeating: 0x11, count: 32),
            node: node.identity, issued: 1_700_000_000, ttlSecs: defaultCertTTLSecs
        )
        let env = ObserveEnvelope(
            node: node.identity, membership: m, ts: 1_700_000_100, nonce: "abcd",
            observations: [ObsRecord(actor: "phone:ian", action: "reports", object: "location:home")]
        )
        let body = try JSONEncoder().encode(env)
        let sig = try node.sign(body)
        // The familiar verifies node.verify(rawBody, sig): reproduce that check here.
        XCTAssertTrue(node.signing.publicKey.isValidSignature(Hex.decode(sig)!, for: body))
    }

    func testEnrollmentPayloadParsesAndRejectsJunk() {
        let good = """
        {"v":1,"secret":"1111111111111111111111111111111111111111111111111111111111111111",\
        "group":"10ba682c8ad13513","label":"river","host":"100.64.0.5","port":47100}
        """
        let p = EnrollmentPayload.parse(good)
        XCTAssertEqual(p?.port, 47100)
        XCTAssertEqual(p?.observeURL?.absoluteString, "http://100.64.0.5:47100/mesh/observe")
        XCTAssertEqual(p?.secretData?.count, 32)

        XCTAssertNil(EnrollmentPayload.parse("not json"))
        XCTAssertNil(EnrollmentPayload.parse(#"{"v":1,"secret":"beef","group":"x","label":"y","host":"h","port":1}"#),
                     "a short secret is rejected")
    }

    func testHexRoundTrips() {
        let bytes: [UInt8] = [0x00, 0x0f, 0xa0, 0xff, 0x10]
        let hex = Hex.encode(bytes)
        XCTAssertEqual(hex, "000fa0ff10")
        XCTAssertEqual([UInt8](Hex.decode(hex)!), bytes)
        XCTAssertNil(Hex.decode("xyz"))
        XCTAssertNil(Hex.decode("abc")) // odd length
    }
}
