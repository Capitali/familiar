import Foundation

/// A member's federation tap, travelling (ADR-0033): welcome a pending sibling MESH, or
/// sever a standing one. Same signed-member envelope as a standing vote — the console has
/// no data dir; the act goes to the door, and every node converges at the next exchange.
public struct FederateClient {
    public enum Outcome: Equatable {
        case done(String)
        case refused(String)
    }

    public var node: NodeKey
    public var membership: Membership
    public var groupPubkey: String
    public var urlSession: URLSession

    public init(node: NodeKey, membership: Membership, groupPubkey: String,
                urlSession: URLSession = MeshTLS.session) {
        self.node = node
        self.membership = membership
        self.groupPubkey = groupPubkey
        self.urlSession = urlSession
    }

    struct Act: Codable {
        var membership: Membership
        var group_pubkey: String
        var subject_group_id: String
        var act: String
        var reason: String
        var nonce: String
        var ts: Int64
    }

    /// `act` is "welcome" or "sever". `reason` rides with sever (standing withdrawal keeps
    /// its reason); empty for welcome.
    public func cast(
        subjectGroupId: String,
        act: String,
        reason: String = "",
        host: String,
        port: Int,
        now: Int64 = Int64(Date().timeIntervalSince1970),
        nonce: String = ObservationClient.freshNonce()
    ) async throws -> Outcome {
        let envelope = Act(membership: membership, group_pubkey: groupPubkey,
                           subject_group_id: subjectGroupId, act: act, reason: reason,
                           nonce: nonce, ts: now)
        guard let body = try? JSONEncoder().encode(envelope) else {
            return .refused("could not encode the act")
        }
        let sig = try node.sign(body)
        guard let url = URL(string: "https://\(host):\(port)/mesh/federate-act") else {
            return .refused("bad host")
        }
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.timeoutInterval = 8
        req.setValue(sig, forHTTPHeaderField: "X-Familiar-Sig")
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = body
        let (data, resp) = try await urlSession.data(for: req)
        let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
        let said = String(data: data, encoding: .utf8) ?? ""
        return code == 200 ? .done(said) : .refused(said.isEmpty ? "host said \(code)" : said)
    }
}
