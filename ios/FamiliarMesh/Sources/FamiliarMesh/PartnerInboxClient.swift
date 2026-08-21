import Foundation

public enum PartnerParameterBound: Codable, Equatable {
    case enumeration([String])
    case number(min: Double, max: Double)

    private enum CodingKeys: String, CodingKey { case kind, values, min, max }
    private enum Kind: String, Codable { case enumeration = "enum", number }

    public init(from decoder: Decoder) throws {
        let box = try decoder.container(keyedBy: CodingKeys.self)
        switch try box.decode(Kind.self, forKey: .kind) {
        case .enumeration:
            self = .enumeration(try box.decode([String].self, forKey: .values))
        case .number:
            self = .number(
                min: try box.decode(Double.self, forKey: .min),
                max: try box.decode(Double.self, forKey: .max)
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var box = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .enumeration(let values):
            try box.encode(Kind.enumeration, forKey: .kind)
            try box.encode(values, forKey: .values)
        case .number(let min, let max):
            try box.encode(Kind.number, forKey: .kind)
            try box.encode(min, forKey: .min)
            try box.encode(max, forKey: .max)
        }
    }
}

public typealias PartnerOperationBounds = [String: [String: PartnerParameterBound]]

public enum PartnerJSONValue: Codable, Equatable {
    case string(String)
    case number(Double)
    case bool(Bool)
    case object([String: PartnerJSONValue])
    case array([PartnerJSONValue])
    case null

    public init(from decoder: Decoder) throws {
        let box = try decoder.singleValueContainer()
        if box.decodeNil() { self = .null }
        else if let value = try? box.decode(Bool.self) { self = .bool(value) }
        else if let value = try? box.decode(Double.self) { self = .number(value) }
        else if let value = try? box.decode(String.self) { self = .string(value) }
        else if let value = try? box.decode([String: PartnerJSONValue].self) { self = .object(value) }
        else { self = .array(try box.decode([PartnerJSONValue].self)) }
    }

    public func encode(to encoder: Encoder) throws {
        var box = encoder.singleValueContainer()
        switch self {
        case .string(let value): try box.encode(value)
        case .number(let value): try box.encode(value)
        case .bool(let value): try box.encode(value)
        case .object(let value): try box.encode(value)
        case .array(let value): try box.encode(value)
        case .null: try box.encodeNil()
        }
    }
}

public struct PartnerInbox: Codable, Equatable {
    public var pending_requests: [PartnerGrantRequest]
    public var active_grants: [PartnerGrant]
    public var pending_proposals: [PartnerProposal]
    public var warnings: [String]
}

public struct PartnerSurfaceChoice: Codable, Equatable {
    public var surface: String
    public var description: String
}

public struct PartnerGrantRequest: Codable, Equatable {
    public var request_id: String
    public var partner_alias: String
    public var credential_fingerprint: String
    public var class_id: String
    public var requested_operations: PartnerOperationBounds
    public var requested_duration_seconds: Int64?
    public var reason_quote: String?
    public var eligible_surfaces: [PartnerSurfaceChoice]
}

public struct PartnerGrant: Codable, Equatable {
    public var grant_id: String
    public var partner_alias: String
    public var credential_fingerprint: String
    public var surface: String
    public var allowed_operations: PartnerOperationBounds
    public var expires_at: Int64
}

public struct PartnerProposal: Codable, Equatable {
    public var proposal_id: String
    public var partner_alias: String
    public var credential_fingerprint: String
    public var surface: String
    public var class_id: String
    public var operation: String
    public var parameters: [String: PartnerJSONValue]
    public var reason_quote: String?
}

private struct PartnerInboxEnvelope: Codable {
    var node: NodeIdentity
    var membership: Membership
    var ts: Int64
    var nonce: String
}

/// Fresh signed reads of the human-addressed partner projection. Nothing is cached to disk.
public struct PartnerInboxClient {
    public enum ReadError: Error, Equatable {
        case encoding
        case decoding
        case http(status: Int, body: String)
        case transport(String)
    }

    public var session: ObservationClient.Session
    public var urlSession: URLSession

    public init(session: ObservationClient.Session, urlSession: URLSession = MeshTLS.session) {
        self.session = session
        self.urlSession = urlSession
    }

    public func fetchWithRaw(
        now: Int64 = Int64(Date().timeIntervalSince1970),
        nonce: String = ObservationClient.freshNonce()
    ) async throws -> (PartnerInbox, Data) {
        let request = try makeRequest(now: now, nonce: nonce)

        let data: Data
        let response: URLResponse
        do { (data, response) = try await urlSession.data(for: request) }
        catch { throw ReadError.transport(error.localizedDescription) }
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        guard status == 200 else {
            throw ReadError.http(status: status, body: String(data: data, encoding: .utf8) ?? "")
        }
        guard let view = try? JSONDecoder().decode(PartnerInbox.self, from: data) else {
            throw ReadError.decoding
        }
        return (view, data)
    }

    /// Internal for byte/signature conformance tests before URLSession turns the body into a
    /// stream. Production reads use the exact same request.
    func makeRequest(now: Int64, nonce: String) throws -> URLRequest {
        let envelope = PartnerInboxEnvelope(
            node: session.node.identity,
            membership: session.membership,
            ts: now,
            nonce: nonce
        )
        guard let body = try? JSONEncoder().encode(envelope) else { throw ReadError.encoding }
        let signature: String
        do { signature = try session.node.sign(body) }
        catch { throw ReadError.encoding }
        var request = URLRequest(url: session.url)
        request.httpMethod = "POST"
        request.timeoutInterval = 10
        request.setValue(signature, forHTTPHeaderField: "X-Familiar-Sig")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = body
        return request
    }

    public static func inboxURL(host: String, port: Int) -> URL? {
        URL(string: "https://\(host):\(port)/mesh/partner-inbox")
    }
}
