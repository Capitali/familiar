import Foundation
import FamiliarSC

// The captain's bridge reads one feed and performs a few acts. The feed is a protocol so the
// same screens run over ship stores on the host (the Mac, a fixture in previews) and over
// `familiar fleet serve` on the phone (the wire, wildhorse's half of B3). Every act here is
// the CAPTAIN's — approve/deny a proposal, set the dial, pair or unpair — done on a tap;
// nothing in these screens lets a model perform one.

/// One paired ship as the Ships screen shows it — the `fleet status` row, plus what the
/// journal's tail says about her computer's mood.
public struct ShipSummary: Identifiable, Equatable, Sendable {
    public var world: String
    public var label: String
    public var computer: String
    public var named: Bool
    public var hull: String
    public var captain: String
    public var server: String
    public var automations: [String]
    public var credits: Int64?
    public var debt: Int64?
    public var fuel: Int64?
    public var fuelCapacity: Int64?
    public var wearBps: Int64?
    public var docked: String?
    public var enRouteTo: String?
    public var pilotAlive: Bool
    public var leaseHoursLeft: Int64?
    public var reachable: Bool
    public var lastEvent: String?
    public var lastAt: Int64?
    public var mood: BridgeReport.Mood
    public var openProposals: Int
    /// One sentence in her voice — the latest fold's headline.
    public var sentence: String = ""
    public var leasePrincipal: Int64?
    public var leaseServicePaid: Int64?
    /// The merchant's book as `fleet status` computes it from receipts ∪ journal (wire only).
    public var trades: TradeBook?

    public var id: String { world }

    public init(world: String, label: String, computer: String, named: Bool, hull: String, captain: String, server: String, automations: [String], credits: Int64? = nil, debt: Int64? = nil, fuel: Int64? = nil, fuelCapacity: Int64? = nil, wearBps: Int64? = nil, docked: String? = nil, enRouteTo: String? = nil, pilotAlive: Bool, leaseHoursLeft: Int64? = nil, reachable: Bool, lastEvent: String? = nil, lastAt: Int64? = nil, mood: BridgeReport.Mood, openProposals: Int, sentence: String = "", leasePrincipal: Int64? = nil, leaseServicePaid: Int64? = nil, trades: TradeBook? = nil) {
        self.world = world; self.label = label; self.computer = computer; self.named = named; self.hull = hull
        self.captain = captain; self.server = server; self.automations = automations; self.credits = credits
        self.debt = debt; self.fuel = fuel; self.fuelCapacity = fuelCapacity; self.wearBps = wearBps
        self.docked = docked; self.enRouteTo = enRouteTo; self.pilotAlive = pilotAlive
        self.leaseHoursLeft = leaseHoursLeft; self.reachable = reachable; self.lastEvent = lastEvent
        self.lastAt = lastAt; self.mood = mood; self.openProposals = openProposals
        self.sentence = sentence; self.leasePrincipal = leasePrincipal; self.leaseServicePaid = leaseServicePaid; self.trades = trades
    }

    /// The canvas's one-word mood tag.
    public var moodWord: String {
        if openProposals > 0 { return "asking" }
        switch mood {
        case .steady: return "content"
        case .pleased: return "pleased"
        case .watchful: return "watchful"
        case .concerned: return "worried"
        }
    }

    public var hullGlance: HullGlance {
        HullGlance(shipName: hull.isEmpty ? nil : hull, docked: docked, enRouteTo: enRouteTo, credits: credits ?? 0,
                   debt: debt, fuel: fuel, fuelCapacity: fuelCapacity, wearBps: wearBps, leased: false)
    }
}

/// The merchant's book (`fleet status --json` → `trades`, wildhorse 1d4d098): realized P&L by
/// FIFO cost, with its two honesty marks — units sold with no lot behind them are SET ASIDE,
/// never counted in `realized`; lots whose basis is the pilot's own quoted ask (not a fill
/// receipt) make the profit they imply a CEILING.
public struct TradeBook: Equatable, Sendable {
    public var filled: Int64 = 0
    public var rejected: Int64 = 0
    public var realized: Int64 = 0
    public var costOfSold: Int64 = 0
    public var marginPct: Int64 = 0
    public var inventoryCost: Int64 = 0
    public var unmatchedUnits: Int64 = 0
    public var unmatchedProceeds: Int64 = 0
    public var quotedBasisLots: Int64 = 0

    public init() {}

    public init(row: JSONValue) {
        filled = row["filled"]?.int ?? 0; rejected = row["rejected"]?.int ?? 0
        realized = row["realized"]?.int ?? 0; costOfSold = row["cost_of_sold"]?.int ?? 0
        marginPct = row["margin_pct"]?.int ?? 0; inventoryCost = row["inventory_cost"]?.int ?? 0
        unmatchedUnits = row["unmatched_units"]?.int ?? 0; unmatchedProceeds = row["unmatched_proceeds"]?.int ?? 0
        quotedBasisLots = row["quoted_basis_lots"]?.int ?? 0
    }

    /// The caveat the card shows when the number is not the whole truth; nil when it is.
    public var caveat: String? {
        var parts: [String] = []
        if unmatchedUnits > 0 { parts.append("ℳ\(unmatchedProceeds) from \(unmatchedUnits) unmatched unit\(unmatchedUnits == 1 ? "" : "s") set aside") }
        if quotedBasisLots > 0 { parts.append("\(quotedBasisLots) lot\(quotedBasisLots == 1 ? "" : "s") at a quoted basis, so the profit is a ceiling") }
        return parts.isEmpty ? nil : parts.joined(separator: "; ")
    }
}

/// The dial as a screen edits it: the file's settings plus which automations are bought.
public struct DialSheet: Equatable, Sendable {
    public var loaded: AutonomyDial.Loaded
    public var bought: [String]
    public init(loaded: AutonomyDial.Loaded, bought: [String]) { self.loaded = loaded; self.bought = bought }
}

/// The merchant's book and the delivery record, glanced.
public struct ShipBook: Equatable, Sendable {
    public var holdings: [Holding]
    public var deliveries: [DeliveryStat]
    public init(holdings: [Holding], deliveries: [DeliveryStat]) { self.holdings = holdings; self.deliveries = deliveries }

    public var hauls: Int { deliveries.count }
    public var freightPaid: Int64 { deliveries.reduce(0) { $0 + $1.paid } }
    public var freightBooked: Int64 { deliveries.reduce(0) { $0 + $1.booked } }
    public var inventoryAtCost: Int64 { holdings.reduce(0) { $0 + $1.units * $1.avgCost } }
}

public enum FeedError: Error, Equatable, CustomStringConvertible {
    case unavailable(String)
    case needsHost(String)
    case refused(String)
    public var description: String {
        switch self {
        case .unavailable(let s): return s
        case .needsHost(let s): return "needs the ship's host: \(s)"
        case .refused(let s): return s
        }
    }
}

public protocol ShipsFeed: Sendable {
    func ships() async throws -> [ShipSummary]
    func persona(world: String) async throws -> Persona?
    /// The journal from a tick on (nil = all of it), oldest first.
    func journal(world: String, sinceTick: Int64?) async throws -> [JournalEntry]
    func window(world: String) async throws -> [MessageItem]
    func dial(world: String) async throws -> DialSheet
    func book(world: String) async throws -> ShipBook
}

public protocol CaptainActs: Sendable {
    func approve(world: String, proposalID: String, approved: Bool) async throws
    func setDial(world: String, dial: AutonomyDial) async throws
    func pair(_ request: PairingRequest, key: PairingKey) async throws
    func unpair(world: String) async throws
}

// MARK: - The store feed: ship stores on this machine (the Mac host, or a copied fixture)

/// Reads `<worlds>/*/` — every directory with a captain.json is a paired ship. The hull
/// glance comes from the journal's last `holding`/`acted` line (credits, fuel, berth), since
/// the store holds no wire; the Mac host's console can pass a wire glance in later.
public struct StoreFeed: ShipsFeed {
    public let worlds: URL
    /// Ticks of journal the mood is judged over (one PROD day = 288).
    public var moodWindowTicks: Int64 = 288

    public init(worlds: URL) { self.worlds = worlds }

    func stores() -> [ShipStore] {
        let dirs = (try? FileManager.default.contentsOfDirectory(at: worlds, includingPropertiesForKeys: nil)) ?? []
        return dirs.filter { FileManager.default.fileExists(atPath: $0.appendingPathComponent("captain.json").path) }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
            .map { ShipStore(directory: $0) }
    }

    func store(_ world: String) throws -> ShipStore {
        guard let s = stores().first(where: { $0.worldID == world }) else { throw FeedError.unavailable("no paired ship \(world)") }
        return s
    }

    public static func summary(of s: ShipStore, moodWindowTicks: Int64 = 288) -> ShipSummary {
        let captain: Captain? = try? s.captain()
        let journal: Journal = (try? s.journal()) ?? Journal(entries: [], malformed: 0)
        let entries = journal.entries
        let last: JournalEntry? = entries.last
        // Credits and fuel ride on many lines (acted, holding, traded, outfitted…): the
        // last line that carries them is the freshest truth the store has.
        let lastMoney: JournalEntry? = entries.last(where: { $0.int("credits") != nil })
        let lastFuel: JournalEntry? = entries.last(where: { $0.int("fuel") != nil })
        let lastHull: JournalEntry? = entries.last(where: { $0.event == "holding" || $0.event == "acted" })
        let nowTick: Int64 = journal.lastTick ?? 0
        let fromTick: Int64 = nowTick > moodWindowTicks ? nowTick - moodWindowTicks : 0
        let window: [JournalEntry] = journal.since(tick: fromTick)
        let items: [MessageItem] = MessageWindow.build(journal: entries, proposals: s.proposals(), approvals: s.approvals(), nowTick: nowTick)
        let open: Int = items.filter { $0.needsTheCaptain }.count
        let persona: Persona? = (try? s.persona()) ?? nil
        let voice = TemplatedVoice(persona: persona ?? Persona(name: "?", style: nil))
        let report: BridgeReport = voice.report(entries: window, openProposals: open)
        let underWay: Bool = lastHull?.string("why") == "under way"
        let lastLeg: JournalEntry? = entries.last(where: { $0.event == "engaged-drive" || $0.event == "unwedged-course" })
        var out = ShipSummary(
            world: s.worldID, label: s.worldID, computer: s.computerName(), named: persona != nil,
            hull: captain?.hullName ?? "", captain: captain?.captain ?? "", server: captain?.server ?? "",
            automations: (try? s.automations()) ?? captain?.automations ?? [],
            pilotAlive: s.pilotPID() != nil, reachable: false,
            mood: report.mood, openProposals: open
        )
        out.credits = lastMoney?.int("credits")
        out.fuel = lastFuel?.int("fuel")
        out.docked = lastHull?.string("docked")
        out.enRouteTo = underWay ? lastLeg?.string("to") : nil
        out.lastEvent = last?.event
        out.lastAt = last?.at
        out.sentence = report.headline
        return out
    }

    public func ships() async throws -> [ShipSummary] { stores().map { StoreFeed.summary(of: $0, moodWindowTicks: moodWindowTicks) } }
    public func persona(world: String) async throws -> Persona? { try store(world).persona() }
    public func journal(world: String, sinceTick: Int64?) async throws -> [JournalEntry] {
        let j = try store(world).journal()
        return sinceTick.map { j.since(tick: $0) } ?? j.entries
    }
    public func window(world: String) async throws -> [MessageItem] {
        let s = try store(world)
        let j = try s.journal()
        return MessageWindow.build(journal: j.entries, proposals: s.proposals(), approvals: s.approvals(), nowTick: j.lastTick)
    }
    public func dial(world: String) async throws -> DialSheet {
        let s = try store(world)
        return DialSheet(loaded: s.dial(), bought: (try? s.automations()) ?? [])
    }
    public func book(world: String) async throws -> ShipBook {
        let s = try store(world)
        return ShipBook(holdings: s.holdings(), deliveries: s.deliveries())
    }
}

/// The captain's acts on a store this machine holds. Pairing needs the `familiar` binary
/// (the key answers for itself on the exchange, the world is commissioned and leased), so
/// on a bare store it is refused with the exact argv the host should run.
public struct StoreCaptainActs: CaptainActs {
    public let worlds: URL
    public init(worlds: URL) { self.worlds = worlds }

    func dir(_ world: String) throws -> URL {
        let d = worlds.appendingPathComponent(world)
        guard FileManager.default.fileExists(atPath: d.appendingPathComponent("captain.json").path) else {
            throw FeedError.unavailable("no paired ship \(world)")
        }
        return d
    }

    public func approve(world: String, proposalID: String, approved: Bool) async throws {
        let d = try dir(world)
        let line = MessageWindow.approvalLine(id: proposalID, approved: approved, at: Int64(Date().timeIntervalSince1970)) + "\n"
        let url = d.appendingPathComponent("approvals.jsonl")
        if let h = try? FileHandle(forWritingTo: url) {
            defer { try? h.close() }
            try h.seekToEnd()
            try h.write(contentsOf: Data(line.utf8))
        } else {
            try Data(line.utf8).write(to: url)
        }
    }

    public func setDial(world: String, dial: AutonomyDial) async throws {
        let d = try dir(world)
        // Atomic: tmp + rename, the same discipline as the persona writer.
        let tmp = d.appendingPathComponent(".autonomy.json.tmp")
        try dial.encoded().write(to: tmp)
        _ = try FileManager.default.replaceItemAt(d.appendingPathComponent("autonomy.json"), withItemAt: tmp)
    }

    public func pair(_ request: PairingRequest, key: PairingKey) async throws {
        throw FeedError.needsHost("run `familiar " + request.fleetPairArguments(keyFile: "<key-file>").joined(separator: " ") + "`")
    }

    public func unpair(world: String) async throws {
        throw FeedError.needsHost("run `familiar fleet unpair \(world)`")
    }
}
