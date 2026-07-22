import XCTest
import CryptoKit
@testable import FamiliarMesh

/// Golden vector emitted by the Rust `cargo run -p familiar-mesh --example cert_vector`. ed25519 is
/// deterministic (RFC 8032), so from these fixed secrets the Rust and Swift implementations must
/// agree — this pins the ONE place Swift has to byte-match Rust: the membership CertBody + its
/// signature. If this passes, a Swift-minted cert verifies under `group::verify_membership`.
private enum Golden {
    static let groupSecret = "1111111111111111111111111111111111111111111111111111111111111111"
    static let groupPubkey = "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737"
    static let groupId = "10ba682c8ad13513"
    static let nodeSecret = "2222222222222222222222222222222222222222222222222222222222222222"
    static let nodePubkey = "a09aa5f47a6759802ff955f8dc2d2a14a5c99d23be97f864127ff9383455a4f0"
    static let nodeId = "1325b850c2871916"
    static let issued: Int64 = 1_700_000_000
    static let expiry: Int64 = 1_707_776_000
    static let cert = "5b795052ae26db9da67e17833085bd764df3421181a09beb182c74f8be092d889630c8b1ee32c59b456fc457986070bdfb615cf4c1a794b05f7c01483f3d600c"
    static let certBody = "{\"node_id\":\"1325b850c2871916\",\"node_pubkey\":\"a09aa5f47a6759802ff955f8dc2d2a14a5c99d23be97f864127ff9383455a4f0\",\"issued\":1700000000,\"expiry\":1707776000,\"group_id\":\"10ba682c8ad13513\"}"
}

final class CertConformanceTests: XCTestCase {

    func testKeyDerivationsMatchRust() throws {
        let node = try NodeKey(seed: Hex.decode(Golden.nodeSecret)!, label: "vector")
        XCTAssertEqual(node.pubkeyHex, Golden.nodePubkey, "ed25519 pubkey from seed")
        XCTAssertEqual(node.nodeId, Golden.nodeId, "node_id = SHA256(pubkey)[..8]")

        let groupId = try Cert.groupId(fromSecret: Hex.decode(Golden.groupSecret)!)
        XCTAssertEqual(groupId, Golden.groupId, "group_id derivation")
    }

    func testCanonicalCertBodyMatchesRustByteForByte() {
        let body = Cert.canonicalBody(
            nodeId: Golden.nodeId, nodePubkey: Golden.nodePubkey,
            issued: Golden.issued, expiry: Golden.expiry, groupId: Golden.groupId
        )
        // THE canonicalization proof — serde_json compact, struct field order, unquoted integers.
        XCTAssertEqual(body, Golden.certBody)
    }

    func testMintedCertVerifiesAndInteropsWithRustBothWays() throws {
        let gpk = try Curve25519.Signing.PublicKey(rawRepresentation: Hex.decode(Golden.groupPubkey)!)

        // Swift → Rust: a Swift-minted cert must verify under the group public key over the exact
        // canonical body. Because that body is byte-identical to Rust's (proven above) and ed25519
        // verification is standard, the familiar's `group::verify_membership` accepts this cert.
        let node = NodeIdentity(node_id: Golden.nodeId, pubkey: Golden.nodePubkey, label: "vector")
        let mine = try Cert.mint(
            groupSecret: Hex.decode(Golden.groupSecret)!, node: node,
            issued: Golden.issued, ttlSecs: Golden.expiry - Golden.issued,
            expectedGroupId: Golden.groupId
        )
        XCTAssertEqual(mine.group_id, Golden.groupId)
        XCTAssertEqual(mine.expiry, Golden.expiry)
        XCTAssertTrue(
            gpk.isValidSignature(Hex.decode(mine.cert)!, for: Data(Golden.certBody.utf8)),
            "Swift-minted cert must verify under the group key (⇒ Rust accepts it)"
        )

        // Rust → Swift: CryptoKit verifies the *Rust-produced* golden cert over the same body —
        // so the phone can also trust certs the familiar (or another Rust node) minted.
        XCTAssertTrue(
            gpk.isValidSignature(Hex.decode(Golden.cert)!, for: Data(Golden.certBody.utf8)),
            "Swift must verify a Rust-minted cert"
        )

        // Note: CryptoKit's ed25519 is randomized (hedged), so `mine.cert` != `Golden.cert` — that
        // is fine and expected. Interop rests on identical canonical bytes + standard verification,
        // not on signature byte-equality.
        XCTAssertNotEqual(mine.cert, Golden.cert, "CryptoKit ed25519 is randomized, not deterministic")
    }

    func testTamperedBodyFailsGroupVerification() throws {
        let gpk = try Curve25519.Signing.PublicKey(rawRepresentation: Hex.decode(Golden.groupPubkey)!)
        let tampered = Golden.certBody.replacingOccurrences(of: "1700000000", with: "1700000001")
        let ok = gpk.isValidSignature(Hex.decode(Golden.cert)!, for: Data(tampered.utf8))
        XCTAssertFalse(ok, "a changed issued must not verify against the original cert")
    }
}
