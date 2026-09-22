import Foundation

/// The device side of the device-oracle seam (ADR-0014). A member device pulls the prompts the
/// familiar has queued for it, and pushes back the answers its on-device model produced. Same signed,
/// membership-bearing envelope as the status/observe seams — the signature covers the exact bytes sent.
public struct ConsultClient {
    public struct Prompt: Codable {
        public var id: String
        public var prompt: String
        public var ts: Int64
        /// Which on-device generation strategy to use (ADR-0014): "script"/"theory" select guided
        /// generation; absent/"" / "free" is free-form. Optional so older servers that omit it decode.
        public var kind: String?
        /// May this consult leave covenant hardware (ADR-0038)? The hub boundary's decision;
        /// the device stacks its own consent on top before choosing Private Cloud Compute.
        /// Optional so older servers that omit it decode — absent means local.
        public var cloud_ok: Bool?
    }
    struct Answer: Codable {
        var id: String
        var json: String
        var node_id: String
        var ts: Int64
    }
    struct Pull: Codable {
        var membership: Membership
        var group_pubkey: String
        var nonce: String
        var ts: Int64
    }
    struct AnswerReport: Codable {
        var membership: Membership
        var group_pubkey: String
        var answer: Answer
        var nonce: String
        var ts: Int64
    }

    public var node: NodeKey
    public var membership: Membership
    public var groupPubkey: String
    public var host: String
    public var port: Int
    public var urlSession: URLSession

    public init(node: NodeKey, membership: Membership, groupPubkey: String, host: String, port: Int,
                urlSession: URLSession = MeshTLS.session) {
        self.node = node
        self.membership = membership
        self.groupPubkey = groupPubkey
        self.host = host
        self.port = port
        self.urlSession = urlSession
    }

    /// Pull the prompts waiting for this device. Returns [] on any failure so the caller idles quietly.
    public func pull(now: Int64 = Int64(Date().timeIntervalSince1970),
                     nonce: String = ObservationClient.freshNonce()) async -> [Prompt] {
        let req = Pull(membership: membership, group_pubkey: groupPubkey, nonce: nonce, ts: now)
        guard let body = try? JSONEncoder().encode(req), let sig = try? node.sign(body),
              let url = URL(string: "https://\(host):\(port)/mesh/consult") else { return [] }
        var r = URLRequest(url: url); r.httpMethod = "POST"; r.timeoutInterval = 10
        r.setValue(sig, forHTTPHeaderField: "X-Familiar-Sig")
        r.setValue("application/json", forHTTPHeaderField: "Content-Type")
        r.httpBody = body
        guard let (data, resp) = try? await urlSession.data(for: r),
              (resp as? HTTPURLResponse)?.statusCode == 200 else { return [] }
        return (try? JSONDecoder().decode([Prompt].self, from: data)) ?? []
    }

    /// Push an answer for a prompt. Returns whether the familiar accepted it (200).
    @discardableResult
    public func push(id: String, json: String,
                     now: Int64 = Int64(Date().timeIntervalSince1970),
                     nonce: String = ObservationClient.freshNonce()) async -> Bool {
        let answer = Answer(id: id, json: json, node_id: node.nodeId, ts: now)
        let report = AnswerReport(membership: membership, group_pubkey: groupPubkey, answer: answer, nonce: nonce, ts: now)
        guard let body = try? JSONEncoder().encode(report), let sig = try? node.sign(body),
              let url = URL(string: "https://\(host):\(port)/mesh/consult-answer") else { return false }
        var r = URLRequest(url: url); r.httpMethod = "POST"; r.timeoutInterval = 10
        r.setValue(sig, forHTTPHeaderField: "X-Familiar-Sig")
        r.setValue("application/json", forHTTPHeaderField: "Content-Type")
        r.httpBody = body
        guard let (_, resp) = try? await urlSession.data(for: r) else { return false }
        return ((resp as? HTTPURLResponse)?.statusCode ?? 0) == 200
    }
}
