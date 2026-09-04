import Foundation

// The ship store — one directory per paired hull (ADR-0045: worlds are stores). This file
// reads it; nothing here writes it. The contract is the Rust side's own types:
// crates/kernel/src/persona.rs (persona.json, persona-names.jsonl), crates/cli/src/fleet.rs
// (captain.json, automations.json), crates/whisker/src/trade.rs::Holding (holdings.json),
// crates/whisker/src/outfit.rs::DeliveryStat (deliveries.jsonl), crates/whisker/src/
// autonomy.rs (autonomy.json, proposals.jsonl, approvals.jsonl) and the journal vocabulary
// in crates/whisker/src/main.rs.

public enum StoreError: Error, Equatable, CustomStringConvertible {
    case missing(String)
    case unreadable(String, String)
    case invalid(String, String)

    public var description: String {
        switch self {
        case .missing(let f): return "\(f): not in this ship store"
        case .unreadable(let f, let why): return "\(f): \(why)"
        case .invalid(let f, let why): return "\(f): \(why)"
        }
    }
}

/// The bounded voice — cadence only, never judgment (persona.rs `Style`, v2). Bounds are
/// refused loudly, not clamped: a style outside them is somebody's mistake to hear about.
public struct Style: Codable, Equatable, Sendable {
    public var warmth: Int = 5
    public var formality: Int = 5
    public var humor: Int = 5
    public var sentenceLength: Int = 5
    public var contractions: Bool = true
    public var vocabulary: String = "plain"
    public var greeting: String = ""
    public var formOfAddress: String = "Captain"

    public static let vocabularies = ["plain", "feline", "nautical"]

    enum CodingKeys: String, CodingKey {
        case warmth, formality, humor, contractions, vocabulary, greeting
        case sentenceLength = "sentence_length"
        case formOfAddress = "form_of_address"
    }

    public init() {}

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        warmth = try c.decodeIfPresent(Int.self, forKey: .warmth) ?? 5
        formality = try c.decodeIfPresent(Int.self, forKey: .formality) ?? 5
        humor = try c.decodeIfPresent(Int.self, forKey: .humor) ?? 5
        sentenceLength = try c.decodeIfPresent(Int.self, forKey: .sentenceLength) ?? 5
        contractions = try c.decodeIfPresent(Bool.self, forKey: .contractions) ?? true
        vocabulary = try c.decodeIfPresent(String.self, forKey: .vocabulary) ?? "plain"
        greeting = try c.decodeIfPresent(String.self, forKey: .greeting) ?? ""
        formOfAddress = try c.decodeIfPresent(String.self, forKey: .formOfAddress) ?? "Captain"
    }

    /// Mirrors `Style::validate` exactly.
    public func validate() -> String? {
        for (axis, v) in [("warmth", warmth), ("formality", formality), ("humor", humor), ("sentence_length", sentenceLength)] {
            if v < 0 || v > 10 { return "style.\(axis) is \(v); the axis runs 0..=10" }
        }
        if !Style.vocabularies.contains(vocabulary) {
            return "style.vocabulary \"\(vocabulary)\" is not one of plain|feline|nautical"
        }
        if greeting.utf8.count > 120 { return "style.greeting runs past 120 bytes" }
        if formOfAddress.utf8.count > 40 { return "style.form_of_address runs past 40 bytes" }
        return nil
    }
}

/// Who this computer says she is (persona.rs `Persona`). `deny_unknown_fields` on the Rust
/// side is mirrored: an unknown key is refused, because a file written under a contract this
/// build does not know must be heard about, not half-honoured.
public struct Persona: Codable, Equatable, Sendable {
    public var personaVersion: Int = 1
    public var name: String = Persona.householdDefaultName
    public var role: String = ""
    public var register: String = ""
    public var world: String = ""
    public var style: Style? = nil

    /// The root every ship's computer descends from (ADR-0037) — written EXACTLY.
    public static let rootName = "Purr"
    /// The household loader's default, which a ship must never borrow (T-236 brick 1).
    public static let householdDefaultName = "the familiar"
    static let knownKeys: Set<String> = ["persona_version", "name", "role", "register", "world", "style"]
    static let knownStyleKeys: Set<String> = ["warmth", "formality", "humor", "sentence_length", "contractions", "vocabulary", "greeting", "form_of_address"]

    enum CodingKeys: String, CodingKey {
        case name, role, register, world, style
        case personaVersion = "persona_version"
    }

    public init(name: String = Persona.rootName, style: Style? = Style()) {
        self.name = name
        self.style = style
        self.personaVersion = style == nil ? 1 : 2
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        personaVersion = try c.decodeIfPresent(Int.self, forKey: .personaVersion) ?? 1
        name = try c.decodeIfPresent(String.self, forKey: .name) ?? Persona.householdDefaultName
        role = try c.decodeIfPresent(String.self, forKey: .role) ?? ""
        register = try c.decodeIfPresent(String.self, forKey: .register) ?? ""
        world = try c.decodeIfPresent(String.self, forKey: .world) ?? ""
        style = try c.decodeIfPresent(Style.self, forKey: .style)
    }

    /// Mirrors `Persona::validate`: versions 1 and 2 exist, style rides only on v2.
    public func validate() -> String? {
        switch personaVersion {
        case 1: if style != nil { return "persona_version 1 cannot carry a style block; set persona_version 2" }
        case 2: break
        default: return "persona_version \(personaVersion) is not a contract this build knows"
        }
        if name.trimmingCharacters(in: .whitespaces).isEmpty { return "a persona must have a name" }
        if name.utf8.count > 80 { return "the name runs past 80 bytes" }
        if let s = style, let why = s.validate() { return why }
        return nil
    }

    /// The effective style: v2's block, or the defaults for a v1 record.
    public var voice: Style { style ?? Style() }

    /// Decode a persona.json loudly: unknown fields, unknown versions, out-of-bounds axes.
    public static func decode(_ data: Data) throws -> Persona {
        let raw: JSONValue
        do { raw = try JSONCoding.decode(JSONValue.self, from: data) } catch {
            throw StoreError.unreadable("persona.json", "not JSON")
        }
        guard let obj = raw.object else { throw StoreError.invalid("persona.json", "not an object") }
        if let stray = obj.keys.first(where: { !knownKeys.contains($0) }) {
            throw StoreError.invalid("persona.json", "unknown field `\(stray)`")
        }
        if let styleObj = obj["style"]?.object, let stray = styleObj.keys.first(where: { !knownStyleKeys.contains($0) }) {
            throw StoreError.invalid("persona.json", "unknown field `style.\(stray)`")
        }
        let p: Persona
        do { p = try JSONCoding.decode(Persona.self, from: data) } catch {
            throw StoreError.invalid("persona.json", "\(error)")
        }
        if let why = p.validate() { throw StoreError.invalid("persona.json", why) }
        return p
    }
}

/// One naming act (persona-names.jsonl).
public struct NameEvent: Codable, Equatable, Sendable {
    public var at: Int64
    public var actor: String
    public var name: String
}

/// Who the ship flies for (fleet.rs `Captain`). `keyID` is the key's public id — the first
/// eight characters after `ucfk_` — never the secret, which lives in ucf.env (0600) and is
/// not read by this package.
public struct Captain: Codable, Equatable, Sendable {
    public var captain: String
    public var keyID: String
    public var server: String
    public var automations: [String]
    public var pairedAt: Int64
    public var hullName: String = ""
    public var pilotArgs: [String] = []

    enum CodingKeys: String, CodingKey {
        case captain, server, automations
        case keyID = "key_id"
        case pairedAt = "paired_at"
        case hullName = "hull_name"
        case pilotArgs = "pilot_args"
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        captain = try c.decode(String.self, forKey: .captain)
        keyID = try c.decode(String.self, forKey: .keyID)
        server = try c.decode(String.self, forKey: .server)
        automations = try c.decodeIfPresent([String].self, forKey: .automations) ?? []
        pairedAt = try c.decodeIfPresent(Int64.self, forKey: .pairedAt) ?? 0
        hullName = try c.decodeIfPresent(String.self, forKey: .hullName) ?? ""
        pilotArgs = try c.decodeIfPresent([String].self, forKey: .pilotArgs) ?? []
    }
}

/// A speculative position aboard (trade.rs `Holding`).
public struct Holding: Codable, Equatable, Sendable {
    public var good: String
    public var units: Int64
    public var avgCost: Int64
    public var sellTarget: String
    public var openedTick: Int64
    public var sellableAt: Int64 = 0

    enum CodingKeys: String, CodingKey {
        case good, units
        case avgCost = "avg_cost"
        case sellTarget = "sell_target"
        case openedTick = "opened_tick"
        case sellableAt = "sellable_at"
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        good = try c.decode(String.self, forKey: .good)
        units = try c.decode(Int64.self, forKey: .units)
        avgCost = try c.decode(Int64.self, forKey: .avgCost)
        sellTarget = try c.decodeIfPresent(String.self, forKey: .sellTarget) ?? ""
        openedTick = try c.decodeIfPresent(Int64.self, forKey: .openedTick) ?? 0
        sellableAt = try c.decodeIfPresent(Int64.self, forKey: .sellableAt) ?? 0
    }
}

/// One settled delivery (outfit.rs `DeliveryStat`). Deliveries pay a fixed company share
/// (~85% of the booked rate) — `paid` under `booked` is the share, not decay; decay shows
/// as a FURTHER shortfall on a perishable.
public struct DeliveryStat: Codable, Equatable, Sendable {
    public var loadID: String
    public var good: String
    public var perishable: Bool
    public var booked: Int64
    public var paid: Int64

    enum CodingKeys: String, CodingKey {
        case good, perishable, booked, paid
        case loadID = "load_id"
    }
}

/// A proposed act waiting on the captain (autonomy.rs `Proposal`, proposals.jsonl).
public struct Proposal: Codable, Equatable, Sendable {
    public var id: String
    public var tick: Int64
    public var expiresTick: Int64
    public var surface: String
    public var describe: String
    public var why: String
    public var body: JSONValue

    enum CodingKeys: String, CodingKey {
        case id, tick, surface, describe, why, body
        case expiresTick = "expires_tick"
    }

    public init(id: String, tick: Int64, expiresTick: Int64, surface: String, describe: String, why: String, body: JSONValue) {
        self.id = id; self.tick = tick; self.expiresTick = expiresTick; self.surface = surface
        self.describe = describe; self.why = why; self.body = body
    }
}

/// The captain's word on a proposal (approvals.jsonl).
public struct Approval: Codable, Equatable, Sendable {
    public var id: String
    public var approved: Bool
    public var at: Int64
    public init(id: String, approved: Bool, at: Int64) { self.id = id; self.approved = approved; self.at = at }
}

/// One journal line. `event` is the vocabulary word; everything else stays typed-loose so
/// an event this build has never seen still renders — neutrally.
public struct JournalEntry: Equatable, Sendable {
    public var at: Int64
    public var tick: Int64?
    public var event: String
    public var fields: [String: JSONValue]

    public func string(_ key: String) -> String? { fields[key]?.string }
    public func int(_ key: String) -> Int64? { fields[key]?.int }
    public func bool(_ key: String) -> Bool? { fields[key]?.bool }
    public subscript(key: String) -> JSONValue? { fields[key] }

    public static func parse(line: Substring) -> JournalEntry? {
        guard let data = line.data(using: .utf8),
              let v = try? JSONCoding.decode(JSONValue.self, from: data),
              var obj = v.object,
              let event = obj["event"]?.string else { return nil }
        let at = obj["at"]?.int ?? 0
        let tick = obj["tick"]?.int
        obj["event"] = nil; obj["at"] = nil; obj["tick"] = nil
        return JournalEntry(at: at, tick: tick, event: event, fields: obj)
    }
}

/// The journal, read whole. Malformed lines are counted, never silently dropped.
public struct Journal: Equatable, Sendable {
    public var entries: [JournalEntry]
    public var malformed: Int

    public init(entries: [JournalEntry], malformed: Int) { self.entries = entries; self.malformed = malformed }

    public static func parse(_ text: String) -> Journal {
        var entries: [JournalEntry] = []
        var malformed = 0
        for line in text.split(separator: "\n", omittingEmptySubsequences: true) {
            if line.allSatisfy(\.isWhitespace) { continue }
            if let e = JournalEntry.parse(line: line) { entries.append(e) } else { malformed += 1 }
        }
        return Journal(entries: entries, malformed: malformed)
    }

    /// Entries at or after a tick (a tick-less line — the watch beginning, an unreachable
    /// exchange — is kept by wall clock against the first ticked entry at or after `tick`).
    public func since(tick: Int64) -> [JournalEntry] {
        guard let firstAt = entries.first(where: { ($0.tick ?? -1) >= tick })?.at else { return [] }
        return entries.filter { ($0.tick ?? -1) >= tick || ($0.tick == nil && $0.at >= firstAt) }
    }

    public func since(at: Int64) -> [JournalEntry] { entries.filter { $0.at >= at } }

    /// The latest tick the pilot wrote — the store's own clock, when the wire is away.
    public var lastTick: Int64? { entries.last(where: { $0.tick != nil })?.tick }
}

/// A paired hull's store on disk. Every read is a plain file read; nothing is cached, so
/// the app re-reads after the pilot's fold and sees the truth.
public struct ShipStore {
    public let directory: URL

    public init(directory: URL) { self.directory = directory }

    public var worldID: String { directory.lastPathComponent }
    public func url(_ file: String) -> URL { directory.appendingPathComponent(file) }
    public func has(_ file: String) -> Bool { FileManager.default.fileExists(atPath: url(file).path) }

    func data(_ file: String) throws -> Data {
        guard has(file) else { throw StoreError.missing(file) }
        do { return try Data(contentsOf: url(file)) } catch { throw StoreError.unreadable(file, "\(error)") }
    }

    func text(_ file: String) throws -> String {
        guard let s = String(data: try data(file), encoding: .utf8) else { throw StoreError.unreadable(file, "not UTF-8") }
        return s
    }

    func decode<T: Decodable>(_ type: T.Type, _ file: String) throws -> T {
        do { return try JSONCoding.decode(type, from: try data(file)) } catch let e as StoreError { throw e } catch {
            throw StoreError.invalid(file, "\(error)")
        }
    }

    func lines<T: Decodable>(_ type: T.Type, _ file: String) -> [T] {
        guard let t = try? text(file) else { return [] }
        return t.split(separator: "\n").compactMap { l in
            guard let d = l.data(using: .utf8) else { return nil }
            return try? JSONCoding.decode(type, from: d)
        }
    }

    /// The computer's persona. A ship paired before T-236 has no file: `nil` — the caller
    /// says "unnamed" rather than borrowing the household default. A malformed file THROWS.
    public func persona() throws -> Persona? {
        guard has("persona.json") else { return nil }
        return try Persona.decode(try data("persona.json"))
    }

    /// How the computer is to be named in a surface: her name, or an honest placeholder.
    public func computerName() -> String {
        do {
            if let p = try persona() { return p.name }
            return "(unnamed — `fleet rename` her)"
        } catch { return "(persona unreadable)" }
    }

    public func namings() -> [NameEvent] { lines(NameEvent.self, "persona-names.jsonl") }
    public func captain() throws -> Captain { try decode(Captain.self, "captain.json") }
    public func automations() throws -> [String] { try decode([String].self, "automations.json") }
    public func holdings() -> [Holding] { (try? decode([Holding].self, "holdings.json")) ?? [] }
    public func deliveries() -> [DeliveryStat] { lines(DeliveryStat.self, "deliveries.jsonl") }
    public func proposals() -> [Proposal] { lines(Proposal.self, "proposals.jsonl") }
    public func approvals() -> [Approval] { lines(Approval.self, "approvals.jsonl") }
    public func journal() throws -> Journal { Journal.parse(try text("journal.jsonl")) }

    /// The dial as whisker will read it. See `AutonomyDial.load`.
    public func dial() -> AutonomyDial.Loaded { AutonomyDial.load(from: url("autonomy.json")) }

    /// The pilot's pid file, if the supervisor left one (liveness is the supervisor's call,
    /// not this reader's — a pid on disk is a claim, not a heartbeat).
    public func pilotPID() -> Int? {
        guard let s = try? text("whisker.pid") else { return nil }
        return Int(s.trimmingCharacters(in: .whitespacesAndNewlines))
    }
}
