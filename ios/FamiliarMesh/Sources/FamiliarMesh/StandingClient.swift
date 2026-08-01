import Foundation

/// Casting a standing decision over the mesh (ADR-0020).
///
/// A phone has no data dir to write and no daemon beside it, so recognising a guest cannot be a
/// local file edit the way it is on the Mac. The decision travels: it is signed with this device's
/// membership and posted to the node it reads from — in practice the minting door, the one
/// permanent fixture (ADR-0018) — and every other node converges on that roll at the next exchange.
///
/// **First decision wins.** The host answers 409 if the node has already been recognised or denied,
/// which is reported rather than retried: two people tapping different buttons must not produce a
/// roll that flips with packet order.
public struct StandingClient {
    public enum Outcome: Equatable {
        case decided(String)
        /// Someone already decided this one. Carries the host's word for what happened.
        case alreadyDecided(String)
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

    struct Vote: Codable {
        var membership: Membership
        var group_pubkey: String
        var subject: String
        var act: String
        var nonce: String
        var ts: Int64
    }

    /// `act` is "grant" (recognise) or "deny" (not now).
    public func cast(
        subject: String,
        act: String,
        host: String,
        port: Int,
        now: Int64 = Int64(Date().timeIntervalSince1970),
        nonce: String = ObservationClient.freshNonce()
    ) async throws -> Outcome {
        let vote = Vote(membership: membership, group_pubkey: groupPubkey,
                        subject: subject, act: act, nonce: nonce, ts: now)
        guard let body = try? JSONEncoder().encode(vote) else {
            return .refused("could not encode the vote")
        }
        let sig = try node.sign(body)
        guard let url = URL(string: "https://\(host):\(port)/mesh/standing") else {
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
        switch code {
        case 200: return .decided(said)
        case 409: return .alreadyDecided(said)
        default: return .refused(said.isEmpty ? "host said \(code)" : said)
        }
    }
}
