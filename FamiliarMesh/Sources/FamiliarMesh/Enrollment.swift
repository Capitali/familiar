import Foundation

/// The enrollment payload a device scans (QR) or pastes — produced by `familiar mesh qr`. Carries
/// the group secret (which *is* membership — trusted-screen only), the group id/label for a sanity
/// check, and where to reach this familiar.
public struct EnrollmentPayload: Codable, Equatable {
    public var v: Int
    public var secret: String  // group secret, hex — the join key
    public var group: String   // group_id, for cross-check
    public var label: String
    public var host: String
    public var port: Int

    public init(v: Int = 1, secret: String, group: String, label: String, host: String, port: Int) {
        self.v = v
        self.secret = secret
        self.group = group
        self.label = label
        self.host = host
        self.port = port
    }

    /// Parse the JSON string carried by the QR/paste. Returns nil if malformed or the secret is
    /// not a 32-byte hex value.
    public static func parse(_ json: String) -> EnrollmentPayload? {
        guard let data = json.data(using: .utf8),
              let p = try? JSONDecoder().decode(EnrollmentPayload.self, from: data),
              let secret = Hex.decode(p.secret), secret.count == 32
        else { return nil }
        return p
    }

    public var secretData: Data? { Hex.decode(secret) }

    /// The base URL for the observation endpoint.
    public var observeURL: URL? { URL(string: "http://\(host):\(port)/mesh/observe") }
}
