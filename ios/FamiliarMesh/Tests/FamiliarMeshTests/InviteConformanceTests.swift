import XCTest
@testable import FamiliarMesh

/// The invite token's canonical body is the ONE new place Swift must byte-match Rust
/// (ADR-0026, evidence class E3): the door reconstructs `record::InviteBody` with serde_json
/// and verifies the signature over those exact bytes. The Rust side pins the same golden values
/// in `record::tests::the_invite_body_wire_format_is_pinned_for_the_swift_twin` — change the
/// two together or every Swift-minted invite dies at the door.
private enum Golden {
    static let nodeSecret = Data(repeating: 0x22, count: 32)
    static let tokenId = "00112233445566778899aabbccddeeff"
    static let groupId = "10ba682c8ad13513"
    static let mintedByNode = "1325b850c2871916"
    static let handle = "betty"
    static let issued: Int64 = 1_700_000_000
    static let expires: Int64 = 1_700_000_600
    static let body = "{\"token_id\":\"00112233445566778899aabbccddeeff\",\"group_id\":\"10ba682c8ad13513\",\"minted_by_node\":\"1325b850c2871916\",\"expected_handle\":\"betty\",\"issued\":1700000000,\"expires\":1700000600}"
    static let sig = "d6e8500da43c344b017d182c6689f3bad3e2c9273b3ad3038427a8956ae5b6ea999d63fe2c62f950a294b9fa9d9f9e5f00f640edcc8dc0e75818c454d467af0c"
}

final class InviteConformanceTests: XCTestCase {

    func testCanonicalBodyMatchesRustByteForByte() {
        XCTAssertEqual(
            InviteToken.canonicalBody(tokenId: Golden.tokenId, groupId: Golden.groupId,
                                      mintedByNode: Golden.mintedByNode,
                                      expectedHandle: Golden.handle,
                                      issued: Golden.issued, expires: Golden.expires),
            Golden.body
        )
    }

    func testSignatureOverCanonicalBodyMatchesRust() throws {
        // CryptoKit randomizes ed25519 signatures (unlike dalek's deterministic RFC 8032), so
        // Swift can't reproduce the golden bytes. Verifying instead proves the same two facts:
        // the Rust-minted golden signature validating under the Swift-derived public key over the
        // Swift-built body means seed→key derivation and the canonical bytes both match Rust.
        let node = try NodeKey(seed: Golden.nodeSecret, label: "vector")
        let rustSig = try XCTUnwrap(Hex.decode(Golden.sig))
        XCTAssertTrue(
            node.signing.publicKey.isValidSignature(rustSig, for: Data(Golden.body.utf8)),
            "Rust golden signature must verify under the Swift-derived key over the Swift body"
        )
        // And the reverse direction: a Swift signature over those bytes verifies too, so a
        // Swift-minted token passes the Rust door (dalek accepts any valid RFC 8032 signature).
        let swiftSig = try XCTUnwrap(Hex.decode(node.sign(Data(Golden.body.utf8))))
        XCTAssertTrue(node.signing.publicKey.isValidSignature(swiftSig, for: Data(Golden.body.utf8)))
    }

    func testJsonStringEscapesLikeSerde() {
        XCTAssertEqual(InviteToken.jsonString("plain"), "\"plain\"")
        XCTAssertEqual(InviteToken.jsonString("a\"b\\c"), "\"a\\\"b\\\\c\"")
        XCTAssertEqual(InviteToken.jsonString("a\nb\tc"), "\"a\\nb\\tc\"")
        XCTAssertEqual(InviteToken.jsonString("bell\u{07}"), "\"bell\\u0007\"")
    }

    func testMintedTokenIsWellFormedAndTenMinutes() throws {
        let node = try NodeKey(seed: Golden.nodeSecret, label: "vector")
        let m = Membership(node_id: node.nodeId, node_pubkey: node.pubkeyHex,
                           issued: Golden.issued, expiry: Golden.issued + 86_400,
                           group_id: Golden.groupId, cert: "")
        let t = try InviteToken.mint(node: node, membership: m, expectedHandle: "",
                                     now: Golden.issued)
        XCTAssertEqual(t.token_id.count, 32, "16 random bytes, hex")
        XCTAssertTrue(t.token_id.allSatisfy { $0.isHexDigit })
        XCTAssertEqual(t.expires - t.issued, InviteToken.ttlSecs)
        XCTAssertEqual(t.group_id, Golden.groupId)
        XCTAssertEqual(t.sig.count, 128)
    }
}
