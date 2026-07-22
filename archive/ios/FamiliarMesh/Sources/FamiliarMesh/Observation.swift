import Foundation

/// One derived observation — the semantic triple the familiar stores. Matches the Rust `ObsRecord`.
/// Derived only: e.g. `phone:ian reports location:home`, never raw samples.
public struct ObsRecord: Codable, Equatable {
    public var actor: String
    public var action: String
    public var object: String
    public var context: String
    public var confidence: Double

    public init(actor: String, action: String, object: String, context: String = "", confidence: Double = 0.9) {
        self.actor = actor
        self.action = action
        self.object = object
        self.context = context
        self.confidence = confidence
    }
}

/// The signed envelope POSTed to `/mesh/observe`. Matches the Rust `ObserveEnvelope`. The device
/// signs the *raw serialized bytes* of this value and sends the signature in `X-Familiar-Sig`, so
/// there's no canonicalization to match on the payload — only the embedded membership cert.
public struct ObserveEnvelope: Codable {
    public var node: NodeIdentity
    public var membership: Membership
    public var ts: Int64
    public var nonce: String
    public var observations: [ObsRecord]

    public init(node: NodeIdentity, membership: Membership, ts: Int64, nonce: String, observations: [ObsRecord]) {
        self.node = node
        self.membership = membership
        self.ts = ts
        self.nonce = nonce
        self.observations = observations
    }
}
