import Foundation

// Pairing: the captain scans or pastes a key (a co-pilot key once ucf-exchange#15 mints
// them; a plain trading key pairs the same way, which is how KK II flies today). The
// package validates the key's SHAPE and the request; the pairing itself is `familiar fleet
// pair`, run by the SC runtime — the key reaches it through a file, never argv, so no
// secret ever sits in a process list or a log.

public enum PairingError: Error, Equatable, CustomStringConvertible {
    case noKey
    case malformedKey(String)
    case badServer(String)
    case emptyLabel
    case emptyCaptain

    public var description: String {
        switch self {
        case .noKey: return "no ucfk_ key in what was scanned"
        case .malformedKey(let why): return "key: \(why)"
        case .badServer(let s): return "server \"\(s)\" is not an http(s) URL"
        case .emptyLabel: return "the ship needs a label"
        case .emptyCaptain: return "the captain needs a name"
        }
    }
}

/// A UCF exchange key as scanned: `ucfk_` + at least 16 URL-safe characters.
public struct PairingKey: Equatable, Sendable {
    public let secret: String
    /// The public id, exactly as fleet.rs derives it: the first eight characters after `ucfk_`.
    public var keyID: String { String(secret.dropFirst(PairingKey.prefix.count).prefix(8)) }

    public static let prefix = "ucfk_"
    static let allowed = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-")

    /// Find a key in a raw paste, a URL (`?key=ucfk_…`), or a QR payload. The first
    /// well-formed `ucfk_` token wins; anything around it is ignored.
    public static func parse(_ text: String) -> Result<PairingKey, PairingError> {
        let scalars = Array(text.unicodeScalars)
        var i = 0
        var sawPrefix = false
        while i < scalars.count {
            if text.unicodeScalars.count - i >= prefix.count,
               String(String.UnicodeScalarView(scalars[i..<i + prefix.count])) == prefix {
                sawPrefix = true
                var j = i + prefix.count
                var body = String.UnicodeScalarView()
                while j < scalars.count, allowed.contains(scalars[j]) { body.append(scalars[j]); j += 1 }
                if body.count >= 16 { return .success(PairingKey(secret: prefix + String(body))) }
                i = j
                continue
            }
            i += 1
        }
        return .failure(sawPrefix ? .malformedKey("shorter than 16 characters after ucfk_") : .noKey)
    }

    /// Never the secret: what a UI or a log may show.
    public var redacted: String { PairingKey.prefix + keyID + "…" }
}

/// What the captain buys (ucf-exchange#15 scopes) and what automations.json records.
public enum Automation: String, CaseIterable, Codable, Equatable, Sendable {
    case freight, trade, outfit
}

public struct PairingRequest: Equatable, Sendable {
    public var label: String
    public var captain: String
    public var server: String
    public var automations: [Automation]
    public var computerName: String?

    public init(label: String, captain: String, server: String, automations: [Automation], computerName: String? = nil) {
        self.label = label; self.captain = captain; self.server = server
        self.automations = automations; self.computerName = computerName
    }

    public func validate() -> PairingError? {
        if label.trimmingCharacters(in: .whitespaces).isEmpty { return .emptyLabel }
        if captain.trimmingCharacters(in: .whitespaces).isEmpty { return .emptyCaptain }
        guard let u = URL(string: server), let scheme = u.scheme?.lowercased(), ["http", "https"].contains(scheme), u.host != nil else {
            return .badServer(server)
        }
        return nil
    }

    /// The `familiar fleet pair` argv the SC runs. The key rides in `keyFile` (0600), so
    /// this array is safe to log verbatim.
    public func fleetPairArguments(keyFile: String) -> [String] {
        var a = ["fleet", "pair", "--label", label, "--captain", captain, "--server", server, "--key-file", keyFile]
        if !automations.isEmpty { a += ["--automations", automations.map(\.rawValue).joined(separator: ",")] }
        if let n = computerName, !n.trimmingCharacters(in: .whitespaces).isEmpty { a += ["--computer-name", n] }
        return a
    }
}
