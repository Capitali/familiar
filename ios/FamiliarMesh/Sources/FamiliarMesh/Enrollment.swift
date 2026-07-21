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
    /// Every address the familiar can be reached at, most-universal first (tailnet, then LAN).
    /// Optional on the wire — older payloads carry only `host`.
    public var hosts: [String]?
    public var group: String?
    public var secret: String?

    public init(v: Int = 1, label: String, host: String, port: Int, hosts: [String]? = nil,
                group: String? = nil, secret: String? = nil) {
        self.v = v
        self.label = label
        self.host = host
        self.port = port
        self.hosts = hosts
        self.group = group
        self.secret = secret
    }

    /// The addresses to try, in order — `hosts` when present, else just `host`. The device should
    /// walk this list on every failure: whichever interface it is on (wifi, cellular, VPN), some
    /// candidate may be reachable when the others are not.
    public var candidateHosts: [String] {
        let list = (hosts ?? []).filter { !$0.isEmpty }
        return list.isEmpty ? [host] : list
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
