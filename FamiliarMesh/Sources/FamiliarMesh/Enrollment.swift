import Foundation

/// Where to reach a familiar, for the covenant handshake — produced by `familiar mesh qr` (or, later,
/// Bonjour discovery). It carries only the **address** and a cosmetic label; the group secret never
/// leaves the familiar. (`secret`/`group` are accepted but ignored — kept optional so an older
/// secret-bearing payload still parses, and so the field can be dropped entirely.)
public struct EnrollmentPayload: Codable, Equatable {
    public var v: Int
    public var label: String
    public var host: String
    public var port: Int
    public var group: String?
    public var secret: String?

    public init(v: Int = 1, label: String, host: String, port: Int, group: String? = nil, secret: String? = nil) {
        self.v = v
        self.label = label
        self.host = host
        self.port = port
        self.group = group
        self.secret = secret
    }

    /// Parse the JSON string carried by the QR/paste. Requires only a reachable `host`/`port`.
    public static func parse(_ json: String) -> EnrollmentPayload? {
        guard let data = json.data(using: .utf8),
              let p = try? JSONDecoder().decode(EnrollmentPayload.self, from: data),
              !p.host.isEmpty, p.port > 0
        else { return nil }
        return p
    }
}
