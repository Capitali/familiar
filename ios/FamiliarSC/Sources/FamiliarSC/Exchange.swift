import Foundation

// The UCF exchange's /v1 wire, typed and READ-ONLY. Every shape here is pinned by a fixture
// captured from PROD (Tests/Fixtures/wire/*.json). Fields the familiar does not use today
// are left to the decoder to ignore; fields that may be absent are optional — the exchange
// adds keys between content versions and a client that refused a new key would read the
// whole market as empty that day.
//
// ONE method POSTs an action, `file(_:actionId:)`, and only the captain's confirmed act
// reaches it (T-237 B4 finding 3, direct mode's confirm-to-act path): Apple Intelligence
// never places an action on the exchange, and no read path can. Through a host the pilot
// files acts and the captain approves them in the ship store. `ActionAck` is a CLOCK
// (resolvesAtTick), never a verdict — outcomes come from /v1/receipts (trades) and
// /v1/me.freight (freight).

public struct ExchangeStatus: Codable, Equatable {
    public var tick: Int64
    public var tickDurationSec: Int64?
    public var worldName: String?
    public var contentVersion: Int64?
    public var nextTickAt: String?
    public var stateHash: String?
}

public struct FreightEvent: Codable, Equatable {
    public var event: String
    public var outcome: String?
    public var tick: Int64
    public var loadId: String?
    public var freightPaid: Int64?
    public var goodsPaid: Int64?
    public var unitsDelivered: Int64?
}

/// The pending overlay: an action filed on this key whose fold has not landed. Absent
/// (`null`) when nothing is in flight.
public struct PendingAction: Codable, Equatable {
    public var verb: String
    public var loadId: String?
    public var resolvesAtTick: Int64?
}

public struct Contract: Codable, Equatable {
    public var loadId: String
    public var status: String?
    public var unitsInHold: Int64?
    public var escrow: Int64?
    public var pickupDeadlineTick: Int64?
    public var deliverDeadlineTick: Int64?
}

public struct CargoLot: Codable, Equatable {
    public var good: String
    public var units: Int64
}

/// `/v1/me` — the hull as the exchange sees it this tick.
public struct Me: Codable, Equatable {
    public var shipName: String?
    public var tick: Int64?
    public var docked: String?
    public var enRouteTo: String?
    public var arriveTick: Int64?
    public var departedTick: Int64?
    public var route: [String]?
    public var credits: Int64
    public var debt: Int64?
    public var fuel: Int64?
    public var fuelCapacity: Int64?
    public var holdUsed: Int64?
    public var holdCapacity: Int64?
    public var wearBps: Int64?
    public var effectiveAccelMilliG: Int64?
    public var titled: Bool?
    public var leasePrincipal: Int64?
    public var leaseServicePaid: Int64?
    public var fittings: [String]?
    public var frame: String?
    public var frameId: String?
    public var nextFrame: String?
    public var nextFrameCost: Int64?
    public var crewBerths: Int64?
    public var crewHireCost: Int64?
    public var standingBps: Int64?
    public var marginAvailable: Int64?
    public var marginDrawn: Int64?
    public var bookedLoad: String?
    public var contract: Contract?
    public var contracts: [Contract]?
    public var cargo: [CargoLot]?
    public var freight: [FreightEvent]?
    public var pendingActions: [PendingAction]?

    /// Under way = not berthed (whisker's own reading: PROD reports `route: []` mid-crossing).
    public var underWay: Bool { docked == nil }
    /// Leased iron: not titled and a lease principal outstanding.
    public var leased: Bool { !(titled ?? true) && (leasePrincipal ?? 0) > 0 }
}

/// One row of the load board (`/v1/loadboard`, open or mine).
public struct Load: Codable, Equatable {
    public var loadId: String
    public var good: String?
    public var serviceClass: String?
    public var origin: String
    public var dest: String
    public var units: Int64?
    public var estimatedNet: Int64?
    public var estimatedCost: Int64?
    public var freightRate: Int64?
    public var payableToMe: Int64?
    public var deadheadTicks: Int64?
    public var haulTicks: Int64?
    public var loadingTicks: Int64?
    public var unloadingTicks: Int64?
    public var expiresAtTick: Int64?
    public var postedAtTick: Int64?
    public var status: String?
    public var mine: Bool?
    public var heldForOther: Bool?
    public var assignable: Bool?

    /// The doctrine's service class as a drive multiplier (bps) — doctrine.rs `LoadRow`.
    public var classBps: Int64 {
        switch serviceClass ?? "standard" {
        case "economy": return 5_000
        case "express": return 20_000
        case "priority": return 30_000
        default: return 10_000
        }
    }
    /// Total ticks of the PILOT's time: reposition + haul + handling at both ends.
    public var pilotTicks: Int64 {
        (deadheadTicks ?? 0) + (haulTicks ?? 0) + 2 * max(loadingTicks ?? 8, 8)
    }
}

/// One good on a berth's board (`/v1/stations/{id}/quotes`).
public struct Quote: Codable, Equatable {
    public var good: String
    public var displayName: String?
    public var ask: Int64
    public var bid: Int64
    public var mid: Int64?
    public var stock: Int64?
    public var capacity: Int64?
    public var equilibrium: Int64?
    public var maxBuyUnits: Int64?
    public var maxSellUnits: Int64?
}

public struct StationQuotes: Codable, Equatable {
    public var station: String
    public var tick: Int64?
    public var goods: [Quote]
}

/// One good's mid and shelf at one station (`/v1/galaxy/prices`).
public struct GalaxyPrice: Codable, Equatable {
    public var good: String
    public var station: String
    public var mid: Int64
    /// The buyer's SHELF, not its headroom: a full shelf pays but takes nothing.
    public var stock: Int64?
}

/// The map's mids — TWO shapes by the exchange's design: a bare array while the survey dial
/// is zero, `{rows, unsurveyed}` once an operator files it. Both decode.
public struct GalaxyPrices: Equatable {
    public var rows: [GalaxyPrice]
    public var unsurveyed: [String]

    public static func decode(_ data: Data) throws -> GalaxyPrices {
        if let rows = try? JSONCoding.decode([GalaxyPrice].self, from: data) {
            return GalaxyPrices(rows: rows, unsurveyed: [])
        }
        struct Wrapped: Decodable { var rows: [GalaxyPrice]; var unsurveyed: [String]? }
        let w = try JSONCoding.decode(Wrapped.self, from: data)
        return GalaxyPrices(rows: w.rows, unsurveyed: w.unsurveyed ?? [])
    }
}

/// One fill or rejection (`/v1/receipts`) — the merchant's book is built from these.
public struct Receipt: Codable, Equatable {
    public var good: String
    public var side: String
    public var units: Int64
    public var tick: Int64
    public var outcome: String?
    public var station: String?
    public var subtotal: Int64?
    public var tax: Int64?
    public var total: Int64?
}

public struct Station: Codable, Equatable {
    public var id: String
    public var displayName: String?
    public var body: String?
    public var role: String?
    public var stationClass: String?
    public var sellsFuel: Bool?
    public var tradesGoods: Bool?
}

public struct RouteLeg: Codable, Equatable {
    public var from: String
    public var to: String
    public var fromBody: String?
    public var toBody: String?
    public var distanceKm: Double?
    public var ticks: Int64?
    public var hours: Double?
    public var fuel: Int64?
    public var deltaVKmS: Double?
    public var peakSpeedKmS: Double?
    public var assistBody: String?
    public var note: String?
}

/// The exchange's price for THIS hull at one service class — `/v1/route?…&hull=me&serviceClass=`
/// (ucf-exchange `RouteHandler.HullQuote`, 5f28c45e). The world's own figure for a rung,
/// wear and fittings applied, which the doctrine treats as authoritative over its model
/// (T-237 B4 re-verification, finding 1). Absent from every plan that did not ask.
public struct HullQuote: Codable, Equatable {
    public var actor: String?
    public var accelMilliG: Int64?
    public var serviceClass: String?
    public var serviceAccelMilliG: Int64?
    public var totalTicks: Int64
    public var totalFuel: Int64
}

/// `/v1/route?from=&to=` — tonight's geometry, priced.
public struct Route: Codable, Equatable {
    public var from: String
    public var to: String
    public var summary: String?
    public var driveAccelG: Double?
    public var legs: [RouteLeg]
    public var forHull: HullQuote?

    public var fuel: Int64 { legs.reduce(0) { $0 + ($1.fuel ?? 0) } }
    public var ticks: Int64 { legs.reduce(0) { $0 + ($1.ticks ?? 0) } }
}

public struct ProfileStats: Codable, Equatable {
    public var deliveries: Int64?
    public var freightEarned: Int64?
    public var tradesFilled: Int64?
    public var tradesRejected: Int64?
    public var goodsBought: Int64?
    public var goodsSold: Int64?
    public var unitsHauled: Int64?
    public var taxPaid: Int64?
    public var contractsAbandoned: Int64?
}

/// `/v1/profile` — the key's standing.
public struct Profile: Codable, Equatable {
    public var traderName: String?
    public var app: String?
    public var standing: String?
    public var scopes: [String]?
    public var activeContract: String?
    public var memberSince: String?
    public var netWorth: Int64?
    public var stats: ProfileStats?
}

/// The sky, as `/v1/reference.bodies` serves it — the integer sky the ΔV bridge (B4) draws.
public struct Body: Codable, Equatable {
    public var id: String
    public var name: String?
    public var kind: String?
    public var orbitRadiusKm: Double?
    public var periodDays: Double?
    public var radiusKm: Double?
    public var angleDegrees: Double?
    public var hasStation: Bool?
    public var hasRings: Bool?
}

public struct Good: Codable, Equatable {
    public var id: String
    public var displayName: String?
    public var category: String?
    public var basePrice: Int64?
    public var decayBps: Int64?
    public var consumedAt: [String]?
    public var madeFrom: [String]?
}

public struct Recipe: Codable, Equatable {
    public var id: String
    public var station: String
    public var displayName: String?
    public var inputs: [String: Int64]
    public var outputs: [String: Int64]
    public var ticksPerCycle: Int64
}

public struct ReferenceStation: Codable, Equatable {
    public var id: String
    public var displayName: String?
    public var body: String?
    public var bodyID: String?
    public var dockFee: Int64?
    public var consumes: [String]?
    public var produces: [String]?
}

/// `/v1/reference` — the world's constants. `params` stays loose: it is the pack's own
/// tuning and grows without notice.
public struct Reference: Codable, Equatable {
    public var contentVersion: Int64?
    public var tickSeconds: Int64?
    public var ticksPerDay: Int64?
    public var driveAccelG: Double?
    public var params: [String: JSONValue]?
    public var bodies: [Body]?
    public var goods: [Good]?
    public var stations: [ReferenceStation]?
    public var recipes: [Recipe]?
}

/// An action acknowledgement: a clock, never a verdict.
public struct ActionAck: Codable, Equatable {
    public var actionId: String
    public var receivedSeq: Int64?
    public var resolvesAtTick: Int64?
    /// The credential that filed it — how a captain's client tells its own filings from a
    /// co-pilot's (additive on the wire, 2026-09-07).
    public var filedBy: String?
}

/// Pure decoders — what the tests pin, what the client calls.
public enum ExchangeWire {
    public static func status(_ d: Data) throws -> ExchangeStatus { try JSONCoding.decode(ExchangeStatus.self, from: d) }
    public static func me(_ d: Data) throws -> Me { try JSONCoding.decode(Me.self, from: d) }
    public static func loads(_ d: Data) throws -> [Load] { try JSONCoding.decode([Load].self, from: d) }
    public static func quotes(_ d: Data) throws -> StationQuotes { try JSONCoding.decode(StationQuotes.self, from: d) }
    public static func galaxy(_ d: Data) throws -> GalaxyPrices { try GalaxyPrices.decode(d) }
    public static func receipts(_ d: Data) throws -> [Receipt] { try JSONCoding.decode([Receipt].self, from: d) }
    public static func stations(_ d: Data) throws -> [Station] { try JSONCoding.decode([Station].self, from: d) }
    public static func route(_ d: Data) throws -> Route { try JSONCoding.decode(Route.self, from: d) }
    public static func profile(_ d: Data) throws -> Profile { try JSONCoding.decode(Profile.self, from: d) }
    public static func reference(_ d: Data) throws -> Reference { try JSONCoding.decode(Reference.self, from: d) }
    public static func ack(_ d: Data) throws -> ActionAck { try JSONCoding.decode(ActionAck.self, from: d) }
}

public enum ExchangeError: Error, Equatable, CustomStringConvertible {
    case badURL(String)
    case http(Int, String)
    case transport(String)
    case decode(String, String)
    /// The exchange refused an act at the door (a 4xx on `/v1/actions`), with its own words.
    case refused(Int, String)

    public var description: String {
        switch self {
        case .badURL(let u): return "bad exchange URL \(u)"
        case .http(let code, let path): return "HTTP \(code) on \(path)"
        case .transport(let why): return why
        case .decode(let path, let why): return "\(path): \(why)"
        case .refused(let code, let why): return "the exchange refused it (\(code)): \(why)"
        }
    }
}

/// A key's wire to one exchange: every read, and the ONE write a captain's confirmed act
/// makes. Bearer auth and the app header exactly as the Rust side sends them (fleet.rs
/// `wire_get`), app name `familiar-sc`.
public struct ExchangeClient: Sendable {
    public let server: URL
    let key: String
    public var app: String = "familiar-sc"
    public var session: URLSession = .shared

    public init?(server: String, key: String) {
        let trimmed = server.hasSuffix("/") ? String(server.dropLast()) : server
        guard let u = URL(string: trimmed) else { return nil }
        self.server = u
        self.key = key
    }

    public func request(_ path: String) -> URLRequest {
        // A path is one of this file's own constants plus a station slug; a malformed one
        // falls back to the server root rather than trapping, and the decoder then refuses.
        var r = URLRequest(url: URL(string: server.absoluteString + path) ?? server)
        r.httpMethod = "GET"
        r.setValue("Bearer \(key)", forHTTPHeaderField: "Authorization")
        r.setValue(app, forHTTPHeaderField: "X-UCF-App")
        r.timeoutInterval = 15
        return r
    }

    public func get(_ path: String) async throws -> Data {
        let data: Data
        let resp: URLResponse
        do { (data, resp) = try await session.data(for: request(path)) } catch {
            throw ExchangeError.transport("\(error.localizedDescription)")
        }
        let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
        guard (200..<300).contains(code) else { throw ExchangeError.http(code, path) }
        return data
    }

    func fetch<T>(_ path: String, _ decode: (Data) throws -> T) async throws -> T {
        let d = try await get(path)
        do { return try decode(d) } catch { throw ExchangeError.decode(path, "\(error)") }
    }

    public func status() async throws -> ExchangeStatus { try await fetch("/v1/status", ExchangeWire.status) }
    public func me() async throws -> Me { try await fetch("/v1/me", ExchangeWire.me) }
    public func profile() async throws -> Profile { try await fetch("/v1/profile", ExchangeWire.profile) }
    public func loadboard(mine: Bool = false) async throws -> [Load] {
        try await fetch(mine ? "/v1/loadboard?mine=true" : "/v1/loadboard", ExchangeWire.loads)
    }
    public func quotes(station: String) async throws -> StationQuotes {
        try await fetch("/v1/stations/\(station)/quotes", ExchangeWire.quotes)
    }
    public func galaxyPrices() async throws -> GalaxyPrices { try await fetch("/v1/galaxy/prices", ExchangeWire.galaxy) }
    public func receipts() async throws -> [Receipt] { try await fetch("/v1/receipts", ExchangeWire.receipts) }
    public func stations() async throws -> [Station] { try await fetch("/v1/stations", ExchangeWire.stations) }
    public func route(from: String, to: String) async throws -> Route {
        try await fetch("/v1/route?from=\(from)&to=\(to)", ExchangeWire.route)
    }
    /// The same geometry priced for the hull this key flies at one service class — the
    /// `forHull` block the doctrine's `quote_at_burn` reads. `me` is the only hull a key may
    /// ask about (the exchange refuses any other).
    public func route(from: String, to: String, forHullAt serviceClass: String) async throws -> Route {
        try await fetch("/v1/route?from=\(from)&to=\(to)&hull=me&serviceClass=\(serviceClass)", ExchangeWire.route)
    }
    public func reference() async throws -> Reference { try await fetch("/v1/reference", ExchangeWire.reference) }

    /// `POST /v1/actions` — the captain's confirmed act, and nothing else, comes through here.
    /// `actionId` is the idempotency handle and the contract is RETRY THE ID, NEVER THE
    /// INTENT (the owner's words, ucf-exchange#14): a re-sent act carries the SAME id, or a
    /// transient failure after the exchange accepted it becomes a double filing. The caller
    /// minted the id when it showed the act, and keeps it until the act is confirmed or
    /// dropped. The exchange answers 202 with a clock; the outcome comes back on the ledger.
    public func file(_ body: [String: JSONValue], actionId: String) async throws -> ActionAck {
        var full = body
        full["actionId"] = .string(actionId)
        var r = request("/v1/actions")
        r.httpMethod = "POST"
        r.setValue("application/json", forHTTPHeaderField: "Content-Type")
        r.httpBody = Data(JSONValue.object(full).description.utf8)
        let data: Data
        let resp: URLResponse
        do { (data, resp) = try await session.data(for: r) } catch {
            throw ExchangeError.transport("\(error.localizedDescription)")
        }
        let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
        guard (200..<300).contains(code) else {
            let why = (try? JSONDecoder().decode(JSONValue.self, from: data))?["error"]?.string
            throw ExchangeError.refused(code, why ?? "HTTP \(code) on /v1/actions")
        }
        do { return try ExchangeWire.ack(data) } catch { throw ExchangeError.decode("/v1/actions", "\(error)") }
    }
}
