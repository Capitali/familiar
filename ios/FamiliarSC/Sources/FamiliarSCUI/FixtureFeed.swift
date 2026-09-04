import Foundation
import FamiliarSC

/// An in-memory fleet for previews and for the phone until `fleet serve` is live: a
/// synthesized journal in the pilot's vocabulary (no captain's real record), one ship
/// named Purr with a proposal waiting, one unnamed ship paired before T-236.
public struct FixtureFeed: ShipsFeed, CaptainActs {
    public final class Box: @unchecked Sendable {
        var approvals: [String: [Approval]] = [:]
        var dials: [String: AutonomyDial] = ["world-fixture-purr": {
            var d = AutonomyDial(); _ = d.set("market.buy", .confirm); _ = d.set("navigation", .advise); return d
        }()]
        let lock = NSLock()
        func sync<T>(_ f: () -> T) -> T { lock.lock(); defer { lock.unlock() }; return f() }
    }
    let box = Box()

    public init() {}

    public static let journalText = """
    {"at":1700000000,"event":"watch-begins","instance":"world-fixture-purr","exchange":"http://exchange.example","automations":["Freight","Trade","Outfit"]}
    {"at":1700000100,"tick":100,"event":"adopted-held-contract","load":"L1","status":"inTransit"}
    {"at":1700000200,"tick":101,"event":"holding","credits":5000,"docked":"whisker-hollow","fuel":500,"why":"waiting on the crane"}
    {"at":1700000400,"tick":103,"event":"acted","credits":5000,"decision":"Collect { load_id: \\"L1\\" }","fuel":500,"resolves":104}
    {"at":1700000600,"tick":105,"event":"engaged-drive","resolves":120,"to":"foxys-diner"}
    {"at":1700000700,"tick":110,"event":"holding","credits":5000,"docked":null,"fuel":470,"why":"under way"}
    {"at":1700000800,"tick":120,"event":"load-closed","credits":5600,"load":"L1","why":"settled: payment taken"}
    {"at":1700001000,"tick":122,"event":"traded","credits":5000,"good":"ore","resolves":123,"side":"buy","units":40,"why":""}
    {"at":1700001200,"tick":123,"event":"position-opened","ask":15,"est_margin":120,"good":"ore","sell_target":"io-slagworks","sellable_at":411,"units":40}
    {"at":1700001900,"tick":170,"event":"outfit-idle","credits":5000,"reserve":4560,"why":"saving for drive-tune: ℳ9000 + reserve ℳ4560 > ℳ5000 in hand"}
    {"at":1700002500,"tick":200,"event":"advice","surface":"navigation.course","would":"fly to foxys-diner now, 98 fuel, refuel on credit","why":"a tanker call is a multi-day strand","body":{"type":"travel","station":"foxys-diner"}}
    {"at":1700002800,"tick":210,"event":"proposed","id":"p-fedcba9876543210","surface":"market.buy","would":"buy 30 gravy-base at 20","why":"margin 25% at velvet-array","expires":214}
    {"at":1700002900,"tick":211,"event":"holding","credits":5000,"docked":"foxys-diner","fuel":98,"why":"proposal waiting on the captain"}
    """

    static let unnamedJournalText = """
    {"at":1700000000,"event":"watch-begins","instance":"world-fixture-old","exchange":"http://exchange.example","automations":["Freight"]}
    {"at":1700000200,"tick":7530,"event":"holding","credits":7132,"docked":null,"fuel":166,"why":"under way"}
    {"at":1700000300,"tick":7531,"event":"engaged-drive","resolves":7543,"to":"foxys-diner"}
    """

    static let purr = Persona(name: "Purr", style: {
        var s = Style(); s.warmth = 7; s.humor = 6; s.vocabulary = "feline"; s.greeting = "Mrrp."; return s
    }())

    func journalFor(_ world: String) -> Journal {
        Journal.parse(world == "world-fixture-purr" ? FixtureFeed.journalText : FixtureFeed.unnamedJournalText)
    }

    func proposalsFor(_ world: String) -> [Proposal] {
        guard world == "world-fixture-purr" else { return [] }
        return [Proposal(id: "p-fedcba9876543210", tick: 210, expiresTick: 214, surface: "market.buy", describe: "buy 30 gravy-base at 20", why: "margin 25% at velvet-array", body: .object(["type": .string("buy")]))]
    }

    func approvalsFor(_ world: String) -> [Approval] { box.sync { box.approvals[world] ?? [] } }

    public func ships() async throws -> [ShipSummary] {
        let w1 = try await window(world: "world-fixture-purr")
        return [
            ShipSummary(world: "world-fixture-purr", label: "KK II (fixture)", computer: "Purr", named: true, hull: "🐈‍⬛ Kibble Klipper II", captain: "A. Captain", server: "http://exchange.example", automations: ["freight", "trade", "outfit"], credits: 5000, debt: 21400, fuel: 98, fuelCapacity: 600, wearBps: 1104, docked: "foxys-diner", pilotAlive: true, leaseHoursLeft: 20, reachable: true, lastEvent: "holding", lastAt: 1700002900, mood: w1.contains(where: \.needsTheCaptain) ? .watchful : .steady, openProposals: w1.filter(\.needsTheCaptain).count),
            ShipSummary(world: "world-fixture-old", label: "Old hull (fixture)", computer: "(unnamed — `fleet rename` her)", named: false, hull: "Sardine Sprint", captain: "A. Captain", server: "http://exchange.example", automations: ["freight"], credits: 7132, fuel: 166, fuelCapacity: 600, enRouteTo: "foxys-diner", pilotAlive: false, leaseHoursLeft: 1, reachable: false, lastEvent: "engaged-drive", lastAt: 1700000300, mood: .steady, openProposals: 0),
        ]
    }
    public func persona(world: String) async throws -> Persona? { world == "world-fixture-purr" ? FixtureFeed.purr : nil }
    public func journal(world: String, sinceTick: Int64?) async throws -> [JournalEntry] {
        let j = journalFor(world); return sinceTick.map { j.since(tick: $0) } ?? j.entries
    }
    public func window(world: String) async throws -> [MessageItem] {
        let j = journalFor(world)
        return MessageWindow.build(journal: j.entries, proposals: proposalsFor(world), approvals: approvalsFor(world), nowTick: j.lastTick)
    }
    public func dial(world: String) async throws -> DialSheet {
        let d = box.sync { box.dials[world] }
        return DialSheet(loaded: d.map { .dial($0) } ?? .absent, bought: world == "world-fixture-purr" ? ["freight", "trade", "outfit"] : ["freight"])
    }
    public func book(world: String) async throws -> ShipBook {
        guard world == "world-fixture-purr" else { return ShipBook(holdings: [], deliveries: []) }
        let h = try JSONDecoder().decode([Holding].self, from: Data(#"[{"good":"ore","units":40,"avg_cost":15,"sell_target":"io-slagworks","opened_tick":123,"sellable_at":411}]"#.utf8))
        let d = try JSONDecoder().decode([DeliveryStat].self, from: Data(#"[{"load_id":"L1","good":"bluefin-reserve","perishable":true,"booked":322,"paid":274}]"#.utf8))
        return ShipBook(holdings: h, deliveries: d)
    }

    public func approve(world: String, proposalID: String, approved: Bool) async throws {
        box.sync { box.approvals[world, default: []].append(Approval(id: proposalID, approved: approved, at: Int64(Date().timeIntervalSince1970))) }
    }
    public func setDial(world: String, dial: AutonomyDial) async throws { box.sync { box.dials[world] = dial } }
    public func pair(_ request: PairingRequest, key: PairingKey) async throws {
        throw FeedError.needsHost("the fixture fleet cannot pair; on the host: familiar " + request.fleetPairArguments(keyFile: "<key-file>").joined(separator: " "))
    }
    public func unpair(world: String) async throws { throw FeedError.needsHost("the fixture fleet cannot unpair") }
}
