import Foundation

/// A glance at the live mesh game — just enough for the shell to chime when the ember
/// arrives and to know whose turn it is. The web layer renders the full view from raw JSON.
public struct GameGlance: Codable, Equatable {
    public var kind: String
    public var status: String
    public var holder: String
    public var holder_since: Int64
    public var turn_secs: Int64
}

/// `POST /mesh/game/act` — one signed move in the mesh game (begin / guess / line / pass /
/// close). The door runs the rules and answers in the judge's own words; the reply is shown
/// to the player verbatim.
public struct GameClient {
    public enum Outcome: Equatable {
        /// The judge's words — "✓ solved!", "the line is on the fire", "passed".
        case said(String)
        /// The door refused — "not your turn", "members only". Shown verbatim.
        case refused(String)
        case error(String)
    }

    public var node: NodeKey
    public var urlSession: URLSession

    public init(node: NodeKey, urlSession: URLSession = MeshTLS.session) {
        self.node = node
        self.urlSession = urlSession
    }

    public func act(
        _ act: String, kind: String? = nil, text: String = "", to: String = "",
        host: String, port: Int,
        now: Int64 = Int64(Date().timeIntervalSince1970),
        nonce: String = ObservationClient.freshNonce()
    ) async throws -> Outcome {
        var obj: [String: Any] = [
            "node": ["node_id": node.nodeId, "pubkey": node.pubkeyHex, "label": node.label],
            "act": act,
            "text": text,
            "to": to,
            "ts": now,
            "nonce": nonce,
        ]
        if let k = kind { obj["kind"] = k }
        guard JSONSerialization.isValidJSONObject(obj),
              let body = try? JSONSerialization.data(withJSONObject: obj) else {
            return .error("could not encode the move")
        }
        let sig = try node.sign(body)
        guard let url = URL(string: "https://\(host):\(port)/mesh/game/act") else {
            return .error("bad host")
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
        case 200: return .said(said)
        case 403: return .refused(said)
        default: return .error(said.isEmpty ? "host said \(code)" : said)
        }
    }
}
