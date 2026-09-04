import Foundation

// The autonomy dial, read exactly as crates/whisker/src/autonomy.rs writes and reads it —
// Ian's ruling (2026-09-03): advise / confirm / auto, per control-surface category and
// family. The app SHOWS the dial and lets the captain change it; the change is the captain's
// act on their tap (the app writes autonomy.json itself), never a model's. This file keeps
// the precedence and the vocabulary in lockstep with the Rust side — the tests pin both.

public enum AutonomyLevel: String, Codable, CaseIterable, Equatable, Sendable {
    case advise, confirm, auto

    /// Mirrors `Level::parse` — the same aliases a captain may type.
    public static func parse(_ s: String) -> AutonomyLevel? {
        switch s.trimmingCharacters(in: .whitespaces).lowercased() {
        case "advise", "advice", "advisory": return .advise
        case "confirm", "ask": return .confirm
        case "auto", "automatic", "autonomous": return .auto
        default: return nil
        }
    }
}

/// A control surface, `family.category` — the pilot's doors today, plus racing's.
public enum ControlSurface: String, CaseIterable, Equatable, Sendable {
    case navigationCourse = "navigation.course"
    case navigationFuel = "navigation.fuel"
    case navigationRescue = "navigation.rescue"
    case freightBook = "freight.book"
    case freightCollect = "freight.collect"
    case freightCancel = "freight.cancel"
    case marketBuy = "market.buy"
    case marketSell = "market.sell"
    case marketCarry = "market.carry"
    case shipRepair = "ship.repair"
    case shipRefit = "ship.refit"
    case shipCrew = "ship.crew"
    case shipFrame = "ship.frame"
    case shipLease = "ship.lease"
    case racingPlot = "racing.plot"
    case racingLine = "racing.line"
    case racingRefusal = "racing.refusal"

    public static let families = ["navigation", "freight", "market", "ship", "racing"]

    public var key: String { rawValue }
    public var family: String { String(rawValue.split(separator: ".")[0]) }
    public var category: String { String(rawValue.split(separator: ".")[1]) }

    public static func parse(_ s: String) -> ControlSurface? {
        ControlSurface(rawValue: s.trimmingCharacters(in: .whitespaces))
    }

    /// Which automation grant a surface belongs to — a surface whose automation the captain
    /// has not bought is not on the dial at all (the grant model beneath it).
    public var automation: String? {
        switch family {
        case "navigation", "freight": return "freight"
        case "market": return "trade"
        case "ship": return "outfit"
        default: return nil
        }
    }
}

/// The dial as the store holds it: `{"navigation.course": "auto", "market": "confirm", "*": "auto"}`.
public struct AutonomyDial: Equatable, Sendable {
    public var settings: [String: AutonomyLevel]

    public init(settings: [String: AutonomyLevel] = [:]) { self.settings = settings }

    /// Most specific wins: category, then family, then `*`, then the default — auto for
    /// everything bought, except the tanker: `navigation.rescue` is ADVISE on its own,
    /// because a PAWS call is a multi-day strand that pins the hull.
    public func level(for s: ControlSurface) -> AutonomyLevel {
        if let l = settings[s.key] { return l }
        if let l = settings[s.family] { return l }
        if let l = settings["*"] { return l }
        return s == .navigationRescue ? .advise : .auto
    }

    /// Mirrors `Dial::set`: `*`, a family, or a surface; anything else is refused.
    public mutating func set(_ key: String, _ level: AutonomyLevel) -> String? {
        let k = key.trimmingCharacters(in: .whitespaces)
        let known = k == "*" || ControlSurface.families.contains(k) || ControlSurface.parse(k) != nil
        guard known else { return "unknown control surface `\(k)`" }
        settings[k] = level
        return nil
    }

    /// Pretty JSON in the file's own shape (a flat object, keys sorted).
    public func encoded() -> Data {
        let obj = Dictionary(uniqueKeysWithValues: settings.map { ($0.key, $0.value.rawValue) })
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        return (try? enc.encode(obj)) ?? Data("{}".utf8)
    }

    public static func decode(_ data: Data) throws -> AutonomyDial {
        let raw = try JSONCoding.decode([String: String].self, from: data)
        var dial = AutonomyDial()
        for (k, v) in raw {
            guard let l = AutonomyLevel(rawValue: v) else {
                throw StoreError.invalid("autonomy.json", "`\(k)`: \"\(v)\" is not advise|confirm|auto")
            }
            if let why = dial.set(k, l) { throw StoreError.invalid("autonomy.json", why) }
        }
        return dial
    }

    /// What whisker's own loader does with the file — and what the captain must be told.
    public enum Loaded: Equatable, Sendable {
        /// No file: every bought surface is auto (KK II today).
        case absent
        case dial(AutonomyDial)
        /// The file is there but whisker cannot parse it — and whisker's loader then
        /// treats it as ABSENT, i.e. auto everywhere. The app must say so, loudly.
        case malformed(String)

        public var dial: AutonomyDial {
            if case .dial(let d) = self { return d }
            return AutonomyDial()
        }
    }

    public static func load(from url: URL) -> Loaded {
        guard FileManager.default.fileExists(atPath: url.path) else { return .absent }
        guard let data = try? Data(contentsOf: url) else { return .malformed("unreadable") }
        do { return .dial(try decode(data)) } catch let e as StoreError { return .malformed(e.description) } catch {
            return .malformed("not JSON")
        }
    }
}
