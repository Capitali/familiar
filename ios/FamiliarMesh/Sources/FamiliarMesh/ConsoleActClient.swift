import Foundation

/// The two deliberately narrow writes a full-standing console may ask its current door to make.
/// The wire shape mirrors Rust's internally tagged `console_act::ConsoleAct`.
public enum ConsoleAct: Equatable {
    case disableRule(String)
    case nameDevice(String)
}

struct ConsoleActBody: Codable {
    var kind: String
    var rule_id: String?
    var name: String?

    init(_ act: ConsoleAct) {
        switch act {
        case .disableRule(let id):
            kind = "disable_rule"
            rule_id = id
            name = nil
        case .nameDevice(let value):
            kind = "name_device"
            rule_id = nil
            name = value
        }
    }
}

private struct ConsoleActEnvelope: Codable {
    var node: NodeIdentity
    var membership: Membership
    var ts: Int64
    var nonce: String
    var act: ConsoleActBody
}

/// Sends a signed member write to `/mesh/console-act`. The signature covers the literal request
/// bytes, exactly like worldview reads and observation batches; no JSON canonicalization is shared
/// across languages.
public struct ConsoleActClient {
    public enum ActError: Error, Equatable {
        case encoding
        case http(status: Int, body: String)
        case transport(String)
    }

    public var session: ObservationClient.Session
    public var urlSession: URLSession

    public init(session: ObservationClient.Session, urlSession: URLSession = MeshTLS.session) {
        self.session = session
        self.urlSession = urlSession
    }

    @discardableResult
    public func send(
        _ act: ConsoleAct,
        now: Int64 = Int64(Date().timeIntervalSince1970),
        nonce: String = ObservationClient.freshNonce()
    ) async throws -> String {
        let request = try makeRequest(act, now: now, nonce: nonce)

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await urlSession.data(for: request)
        } catch {
            throw ActError.transport(error.localizedDescription)
        }
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        let message = String(data: data, encoding: .utf8) ?? ""
        guard status == 200 else { throw ActError.http(status: status, body: message) }
        return message
    }

    /// Kept internal so conformance tests can verify the exact bytes and signature before
    /// `URLSession` turns `httpBody` into an upload stream.
    func makeRequest(_ act: ConsoleAct, now: Int64, nonce: String) throws -> URLRequest {
        let envelope = ConsoleActEnvelope(
            node: session.node.identity,
            membership: session.membership,
            ts: now,
            nonce: nonce,
            act: ConsoleActBody(act)
        )
        guard let body = try? JSONEncoder().encode(envelope) else { throw ActError.encoding }
        let signature = try session.node.sign(body)

        var request = URLRequest(url: session.url)
        request.httpMethod = "POST"
        request.timeoutInterval = 10
        request.setValue(signature, forHTTPHeaderField: "X-Familiar-Sig")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = body
        return request
    }

    public static func consoleActURL(host: String, port: Int) -> URL? {
        URL(string: "https://\(host):\(port)/mesh/console-act")
    }
}
