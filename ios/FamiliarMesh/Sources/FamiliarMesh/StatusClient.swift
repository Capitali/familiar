import Foundation

/// A member's status heartbeat (ADR-0017): POSTs this device's own status to the lighthouse so the
/// mesh has a complete, always-fresh presence picture — who's online, which human is present, and
/// how each member is reaching the mesh — whatever path each member is on. Status flows through the
/// always-on lighthouse; data takes the best confirmed path. Mirrors `ObservationClient`'s signing
/// envelope: the signature covers the exact bytes sent (`X-Familiar-Sig`), verified server-side over
/// the raw body with no canonicalization to reconcile.
public struct StatusClient {
    /// The status a member reports about itself. Field names match the Rust `status::MemberStatus`;
    /// `group_ref` and `updated_at` are stamped by the receiving node, so we send them empty/zero.
    public struct Member: Codable {
        public var node_id: String
        public var group_ref: String
        public var actor: String
        public var label: String
        public var present_human: String
        public var present_via: String
        public var present_since: Int64
        public var connectivity: String
        public var tailnet_addr: String
        public var tailnet_up: Bool
        public var updated_at: Int64

        /// How the present human was established — "binding" | "face" | "asked" | "inherited"
        /// (ADR-0019). Empty when nobody is identified.
        public var present_confidence: Double

        public init(
            node_id: String, actor: String, label: String, present_human: String,
            connectivity: String, tailnet_addr: String = "", tailnet_up: Bool = false,
            present_via: String = "", present_since: Int64 = 0, present_confidence: Double = 0
        ) {
            self.node_id = node_id
            self.group_ref = ""
            self.actor = actor
            self.label = label
            self.present_human = present_human
            // A presence claim without its provenance and confidence is a guess wearing a fact's
            // clothes (ADR-0019) — these travel with it or it should not be believed.
            self.present_via = present_via
            self.present_since = present_since
            self.present_confidence = present_confidence
            self.connectivity = connectivity
            self.tailnet_addr = tailnet_addr
            self.tailnet_up = tailnet_up
            self.updated_at = 0
        }
    }

    struct Report: Codable {
        var membership: Membership
        var group_pubkey: String
        var status: Member
        var nonce: String
        var ts: Int64
    }

    public var node: NodeKey
    public var membership: Membership
    public var groupPubkey: String
    public var urlSession: URLSession

    public init(node: NodeKey, membership: Membership, groupPubkey: String, urlSession: URLSession = MeshTLS.session) {
        self.node = node
        self.membership = membership
        self.groupPubkey = groupPubkey
        self.urlSession = urlSession
    }

    /// POST the heartbeat to `https://host:port/mesh/status`. Best-effort; returns whether the node
    /// accepted it (200). Throws only on a transport failure so the caller can try another host.
    @discardableResult
    public func send(
        _ status: Member,
        host: String,
        port: Int,
        now: Int64 = Int64(Date().timeIntervalSince1970),
        nonce: String = ObservationClient.freshNonce()
    ) async throws -> Bool {
        let report = Report(membership: membership, group_pubkey: groupPubkey, status: status, nonce: nonce, ts: now)
        guard let body = try? JSONEncoder().encode(report) else { return false }
        let sig = try node.sign(body)
        guard let url = URL(string: "https://\(host):\(port)/mesh/status") else { return false }
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.timeoutInterval = 8
        req.setValue(sig, forHTTPHeaderField: "X-Familiar-Sig")
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = body
        let (_, resp) = try await urlSession.data(for: req)
        return ((resp as? HTTPURLResponse)?.statusCode ?? 0) == 200
    }
}
