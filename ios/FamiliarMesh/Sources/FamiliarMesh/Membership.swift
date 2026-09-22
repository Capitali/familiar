import Foundation
import CryptoKit

/// A membership certificate — the group key's signature binding this node to the group. Exactly
/// the Rust `Membership` shape. Holding the group secret (the join key) is what lets a device mint
/// its own cert; the familiar trusts it via `group::verify_membership`.
public struct Membership: Codable, Equatable {
    public let node_id: String
    public let node_pubkey: String
    public let issued: Int64
    public let expiry: Int64
    public let group_id: String
    public let cert: String
}

public enum MembershipError: Error {
    case badGroupSecret
    case groupMismatch(expected: String, gotSecretFor: String)
}

public enum Cert {
    /// The **exact** canonical bytes the group key signs — must match Rust's `serde_json::to_vec`
    /// of `CertBody` byte-for-byte: compact (no spaces), struct field order
    /// `node_id, node_pubkey, issued, expiry, group_id`, integers unquoted. Built by hand (not
    /// `JSONEncoder`, whose key order isn't guaranteed) so the signature verifies on the Rust side.
    /// All interpolated values are hex ids/keys or integers, so no JSON escaping is needed.
    public static func canonicalBody(
        nodeId: String, nodePubkey: String, issued: Int64, expiry: Int64, groupId: String
    ) -> String {
        "{\"node_id\":\"\(nodeId)\",\"node_pubkey\":\"\(nodePubkey)\",\"issued\":\(issued),\"expiry\":\(expiry),\"group_id\":\"\(groupId)\"}"
    }

    /// Derive the group id (fingerprint of the group public key) from the 32-byte group secret.
    public static func groupId(fromSecret secret: Data) throws -> String {
        guard let key = try? Curve25519.Signing.PrivateKey(rawRepresentation: secret) else {
            throw MembershipError.badGroupSecret
        }
        return Fingerprint.of(key.publicKey.rawRepresentation)
    }

    /// Mint this node's membership cert from the group secret. `expectedGroupId`, when provided
    /// (e.g. from the scanned enrollment payload), is checked against the secret so a mistyped
    /// secret fails loudly rather than producing a cert no one will accept.
    public static func mint(
        groupSecret: Data,
        node: NodeIdentity,
        issued: Int64,
        ttlSecs: Int64,
        expectedGroupId: String? = nil
    ) throws -> Membership {
        guard let groupKey = try? Curve25519.Signing.PrivateKey(rawRepresentation: groupSecret) else {
            throw MembershipError.badGroupSecret
        }
        let groupId = Fingerprint.of(groupKey.publicKey.rawRepresentation)
        if let want = expectedGroupId, want != groupId {
            throw MembershipError.groupMismatch(expected: want, gotSecretFor: groupId)
        }
        let expiry = issued + ttlSecs
        let body = canonicalBody(
            nodeId: node.node_id, nodePubkey: node.pubkey,
            issued: issued, expiry: expiry, groupId: groupId
        )
        let sig = try groupKey.signature(for: Data(body.utf8))
        return Membership(
            node_id: node.node_id, node_pubkey: node.pubkey,
            issued: issued, expiry: expiry, group_id: groupId, cert: Hex.encode(sig)
        )
    }
}

/// The default cert lifetime — mirrors the Rust `DEFAULT_CERT_TTL_SECS` (90 days).
public let defaultCertTTLSecs: Int64 = 90 * 24 * 60 * 60
