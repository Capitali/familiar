import Foundation
import CryptoKit

/// A node's public identity on the mesh — exactly the Rust `NodeIdentity` shape. `node_id` is the
/// first 8 bytes of `SHA256(pubkey)` in hex, so any peer can recompute it from `pubkey`.
public struct NodeIdentity: Codable, Equatable {
    public let node_id: String
    public let pubkey: String
    public let label: String
}

/// This device's ed25519 keypair (via CryptoKit). The 32-byte seed is the secret — persist it in
/// the Keychain (the app does that); everything else derives from it deterministically.
public struct NodeKey {
    public let signing: Curve25519.Signing.PrivateKey
    public let label: String

    /// Restore from a stored 32-byte seed.
    public init(seed: Data, label: String) throws {
        self.signing = try Curve25519.Signing.PrivateKey(rawRepresentation: seed)
        self.label = label
    }

    /// Mint a fresh key (first enrollment).
    public init(label: String) {
        self.signing = Curve25519.Signing.PrivateKey()
        self.label = label
    }

    public var seed: Data { signing.rawRepresentation }
    public var pubkeyData: Data { signing.publicKey.rawRepresentation }
    public var pubkeyHex: String { Hex.encode(pubkeyData) }
    public var nodeId: String { Fingerprint.of(pubkeyData) }
    public var identity: NodeIdentity {
        NodeIdentity(node_id: nodeId, pubkey: pubkeyHex, label: label)
    }

    /// ed25519 signature over `msg`, hex-encoded (64 bytes) — the value the familiar verifies.
    public func sign(_ msg: Data) throws -> String {
        Hex.encode(try signing.signature(for: msg))
    }
}

/// The short pubkey fingerprint used as a node/group id: first 8 bytes of SHA-256, hex.
public enum Fingerprint {
    public static func of(_ pubkey: Data) -> String {
        let digest = SHA256.hash(data: pubkey)
        return Hex.encode(digest.prefix(8))
    }
}
