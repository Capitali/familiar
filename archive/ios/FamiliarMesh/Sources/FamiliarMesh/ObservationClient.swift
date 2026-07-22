import Foundation

/// Posts signed observation batches to a familiar's `/mesh/observe`. The device is a pure client
/// (it never serves gossip). The signature covers the exact bytes we transmit, carried in the
/// `X-Familiar-Sig` header — so the server verifies `node.verify(rawBody, header)` with no
/// canonicalization to reconcile.
public struct ObservationClient {
    public enum PostError: Error, Equatable {
        case notEnrolled
        case encoding
        case http(status: Int, body: String)
        case transport(String)
    }

    /// What a device carries after enrollment: its keypair, its minted cert, and where to send.
    public struct Session {
        public var node: NodeKey
        public var membership: Membership
        public var url: URL
        public init(node: NodeKey, membership: Membership, url: URL) {
            self.node = node
            self.membership = membership
            self.url = url
        }
    }

    public var session: Session
    public var urlSession: URLSession

    public init(session: Session, urlSession: URLSession = .shared) {
        self.session = session
        self.urlSession = urlSession
    }

    /// Build, sign, and POST a batch. Returns the count the familiar reported recorded. `now` and
    /// `nonce` are injectable for testing; in the app they default to the clock and a random token.
    @discardableResult
    public func send(
        _ observations: [ObsRecord],
        now: Int64 = Int64(Date().timeIntervalSince1970),
        nonce: String = ObservationClient.freshNonce()
    ) async throws -> Int {
        guard !observations.isEmpty else { return 0 }
        let envelope = ObserveEnvelope(
            node: session.node.identity,
            membership: session.membership,
            ts: now,
            nonce: nonce,
            observations: observations
        )
        guard let body = try? JSONEncoder().encode(envelope) else { throw PostError.encoding }
        let sig = try session.node.sign(body)

        var req = URLRequest(url: session.url)
        req.httpMethod = "POST"
        req.setValue(sig, forHTTPHeaderField: "X-Familiar-Sig")
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = body

        let (data, response): (Data, URLResponse)
        do {
            (data, response) = try await urlSession.data(for: req)
        } catch {
            throw PostError.transport(error.localizedDescription)
        }
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        let text = String(data: data, encoding: .utf8) ?? ""
        guard status == 200 else { throw PostError.http(status: status, body: text) }
        // Body is "recorded N".
        return Int(text.split(separator: " ").last.map(String.init) ?? "") ?? observations.count
    }

    /// A short random nonce (hex of 8 bytes) — a repeat within the server's window is a replay.
    public static func freshNonce() -> String {
        var bytes = [UInt8](repeating: 0, count: 8)
        for i in bytes.indices { bytes[i] = UInt8.random(in: 0...255) }
        return Hex.encode(bytes)
    }
}
