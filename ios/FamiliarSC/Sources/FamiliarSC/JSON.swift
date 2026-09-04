import Foundation

/// A JSON value as the wire and the journal carry it — kept so a record can hold fields
/// this build does not type (an unknown journal event renders neutrally, never guessed).
public enum JSONValue: Equatable, Codable, CustomStringConvertible, Sendable {
    case null
    case bool(Bool)
    case number(Double)
    case string(String)
    case array([JSONValue])
    case object([String: JSONValue])

    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if c.decodeNil() { self = .null; return }
        if let b = try? c.decode(Bool.self) { self = .bool(b); return }
        if let n = try? c.decode(Double.self) { self = .number(n); return }
        if let s = try? c.decode(String.self) { self = .string(s); return }
        if let a = try? c.decode([JSONValue].self) { self = .array(a); return }
        if let o = try? c.decode([String: JSONValue].self) { self = .object(o); return }
        throw DecodingError.dataCorruptedError(in: c, debugDescription: "not a JSON value")
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self {
        case .null: try c.encodeNil()
        case .bool(let b): try c.encode(b)
        case .number(let n):
            if n == n.rounded(), abs(n) < 9.0e15 { try c.encode(Int64(n)) } else { try c.encode(n) }
        case .string(let s): try c.encode(s)
        case .array(let a): try c.encode(a)
        case .object(let o): try c.encode(o)
        }
    }

    public var string: String? { if case .string(let s) = self { return s }; return nil }
    public var int: Int64? {
        if case .number(let n) = self, n == n.rounded(), abs(n) < 9.0e15 { return Int64(n) }
        return nil
    }
    public var double: Double? { if case .number(let n) = self { return n }; return nil }
    public var bool: Bool? { if case .bool(let b) = self { return b }; return nil }
    public var array: [JSONValue]? { if case .array(let a) = self { return a }; return nil }
    public var object: [String: JSONValue]? { if case .object(let o) = self { return o }; return nil }
    public subscript(key: String) -> JSONValue? { object?[key] }

    /// Compact, key-sorted text — stable across runs, so a rendered line that carries an
    /// unknown payload is byte-identical every time.
    public var description: String {
        switch self {
        case .null: return "null"
        case .bool(let b): return b ? "true" : "false"
        case .number(let n):
            if n == n.rounded(), abs(n) < 9.0e15 { return String(Int64(n)) }
            return String(n)
        case .string(let s): return "\"" + s.replacingOccurrences(of: "\"", with: "\\\"") + "\""
        case .array(let a): return "[" + a.map(\.description).joined(separator: ",") + "]"
        case .object(let o):
            return "{" + o.keys.sorted().map { "\"\($0)\":\(o[$0]!.description)" }.joined(separator: ",") + "}"
        }
    }
}

enum JSONCoding {
    static let decoder: JSONDecoder = { JSONDecoder() }()
    static func decode<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        try decoder.decode(type, from: data)
    }
}
