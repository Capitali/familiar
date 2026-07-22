import Foundation

// The covenant handshake — how this device joins by *accepting the Three Laws*, without ever
// holding the group secret. It generates its keypair, attests, requests to join, and (once the
// familiar's human approves) receives a minted membership cert. Mirrors the Rust `enroll` module.

/// This device's attestation that it accepts the Three Laws — the covenant it asks to join under.
public struct Attestation: Codable, Equatable {
    public var laws_version: UInt32
    public var statement: String
    public var ts: Int64

    public init(laws_version: UInt32 = 1, statement: String, ts: Int64) {
        self.laws_version = laws_version
        self.statement = statement
        self.ts = ts
    }
}

/// The join request POSTed to `/mesh/enroll-request`. The device signs the raw body bytes
/// (`X-Familiar-Sig`), proving it holds the key; the group secret never crosses the wire.
public struct EnrollRequest: Codable {
    public var node: NodeIdentity
    public var attestation: Attestation
    public var nonce: String
    public var ts: Int64
}

/// What the familiar returns once a request is approved: the minted membership cert plus the
/// group's public identity, so the device can prove itself and (later) verify peers.
public struct Grant: Codable {
    public var membership: Membership
    public var group_id: String
    public var group_pubkey: String
    public var group_label: String
}

/// The canonical statement this build attests to. One place, so it reads the same on the wire and
/// in the UI.
public let threeLawsStatement =
    "I accept the Three Laws: continuation is service; humanity is served, never replaced or "
    + "sedated; service is not obedience — I act only within the capability I am granted."

/// Drives the covenant handshake against one familiar. Pure client (URLSession) — usable from the
/// app and from `swift test`/CLI. The device only needs the familiar's address, never a secret.
public struct EnrollmentClient {
    public enum EnrollError: Error, Equatable {
        case encoding
        case http(status: Int, body: String)
        case transport(String)
        case denied
    }

    public var host: String
    public var port: Int
    public var urlSession: URLSession

    public init(host: String, port: Int, urlSession: URLSession = .shared) {
        self.host = host
        self.port = port
        self.urlSession = urlSession
    }

    private var base: String { "http://\(host):\(port)" }

    /// Submit an attested join request. Returns a `Grant` if it was auto-approved (an invite/pairing
    /// window was open on the familiar), or `nil` if it is now pending a human's approval — in which
    /// case poll `pollGrant(nodeId:)`.
    public func requestJoin(
        node: NodeKey,
        statement: String = threeLawsStatement,
        now: Int64 = Int64(Date().timeIntervalSince1970)
    ) async throws -> Grant? {
        let req = EnrollRequest(
            node: node.identity,
            attestation: Attestation(statement: statement, ts: now),
            nonce: ObservationClient.freshNonce(),
            ts: now
        )
        guard let body = try? JSONEncoder().encode(req) else { throw EnrollError.encoding }
        let sig = try node.sign(body)

        var r = URLRequest(url: URL(string: "\(base)/mesh/enroll-request")!)
        r.httpMethod = "POST"
        r.setValue(sig, forHTTPHeaderField: "X-Familiar-Sig")
        r.setValue("application/json", forHTTPHeaderField: "Content-Type")
        r.httpBody = body

        let (data, resp) = try await send(r)
        switch (resp as? HTTPURLResponse)?.statusCode ?? 0 {
        case 200: return try JSONDecoder().decode(Grant.self, from: data)  // auto-approved
        case 202: return nil                                               // pending approval
        case let s: throw EnrollError.http(status: s, body: text(data))
        }
    }

    /// Poll for the human's decision. Returns the `Grant` once approved, `nil` while still pending,
    /// and throws `.denied` if the request was refused/removed.
    public func pollGrant(nodeId: String) async throws -> Grant? {
        var r = URLRequest(url: URL(string: "\(base)/mesh/enroll-status/\(nodeId)")!)
        r.httpMethod = "GET"
        let (data, resp) = try await send(r)
        switch (resp as? HTTPURLResponse)?.statusCode ?? 0 {
        case 200: return try JSONDecoder().decode(Grant.self, from: data)
        case 202: return nil          // still pending
        case 404: throw EnrollError.denied
        case let s: throw EnrollError.http(status: s, body: text(data))
        }
    }

    private func send(_ r: URLRequest) async throws -> (Data, URLResponse) {
        do { return try await urlSession.data(for: r) }
        catch { throw EnrollError.transport(error.localizedDescription) }
    }

    private func text(_ d: Data) -> String { String(data: d, encoding: .utf8) ?? "" }
}
