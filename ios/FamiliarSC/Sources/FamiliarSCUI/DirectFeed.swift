import Foundation
import FamiliarSC

// Direct mode: the app talks to an exchange itself with the captain's own key — Jeff's PROD,
// or a dev world such as MacOnStick's LOCAL. No familiar host means no pilot: no proposals,
// no dial, no pairing; Felix observes, briefs and advises from the wire alone, and the fuel
// picture is computed here from stations, routes and quotes. Acts that need a host say so.
// When the host side moves to a server farm (Ian, 2026-09-04: "a virtual server farm in the
// cloud … a massively multiplayer universe"), a captain's Felix runs there and this mode is
// what the app does before, or without, one.

/// Where a Felix lives for one captain: a familiar host's fleet feed, or the exchange direct.
public enum Connection: Codable, Equatable, Sendable, Identifiable {
    case host(name: String, feedURL: String)
    case direct(name: String, exchangeURL: String, keyID: String)

    public var id: String {
        switch self {
        case .host(_, let u): return "host|" + u
        case .direct(_, let u, let k): return "direct|" + u + "|" + k
        }
    }
    public var name: String {
        switch self { case .host(let n, _): return n; case .direct(let n, _, _): return n }
    }
    public var isDirect: Bool { if case .direct = self { return true }; return false }
}

/// The exchanges a captain reaches for by name.
public enum KnownExchange {
    public static let prod = "https://srv1328560.hstgr.cloud"
    public static let local = "http://127.0.0.1:7877"
    public static func name(for url: String) -> String {
        if url.contains("127.0.0.1") || url.contains("localhost") { return "LOCAL" }
        if url.contains("srv1328560") { return "PROD" }
        return URL(string: url)?.host ?? url
    }
}

/// The captain's own persona in direct mode lives on the device (no host store): one name and
/// style per exchange key, defaulting to Purr until the captain names her.
public struct DevicePersonaStore {
    public let defaults: UserDefaults
    public init(defaults: UserDefaults = .standard) { self.defaults = defaults }
    func key(_ keyID: String) -> String { "sc.direct.persona." + keyID }
    public func load(keyID: String) -> Persona? {
        guard let d = defaults.data(forKey: key(keyID)) else { return nil }
        return try? Persona.decode(d)
    }
    public func save(_ p: Persona, keyID: String) {
        let enc = JSONEncoder(); enc.outputFormatting = [.sortedKeys]
        if let d = try? enc.encode(p) { defaults.set(d, forKey: key(keyID)) }
    }
}

public struct DirectFeed: ShipsFeed, CaptainActs {
    public let client: ExchangeClient
    public let keyID: String
    public var personas = DevicePersonaStore()
    /// The pack's fuel price, until the exchange publishes `fuelPricePerUnit` (ucf-exchange#22).
    public var fuelPricePerUnit: Int64 = 2

    public init?(exchange: String, key: String) {
        guard let c = ExchangeClient(server: exchange, key: key) else { return nil }
        client = c
        keyID = String(key.dropFirst(PairingKey.prefix.count).prefix(8))
    }

    var worldID: String { "direct-" + keyID }

    // MARK: reads

    public func ships() async throws -> [ShipSummary] {
        async let me = client.me()
        async let profile = client.profile()
        async let status = client.status()
        let (m, p, s) = try await (me, profile, status)
        let persona = personas.load(keyID: keyID)
        let entries = DirectFeed.journal(from: m, receipts: [])
        let report = TemplatedVoice(persona: persona ?? Persona(name: Persona.rootName, style: nil)).report(entries: entries.suffix(40).map { $0 }, hull: HullGlance(me: m))
        var out = ShipSummary(
            world: worldID, label: p.traderName ?? "the captain's hull", computer: persona?.name ?? Persona.rootName, named: persona != nil,
            hull: m.shipName ?? "", captain: p.traderName ?? "", server: client.server.absoluteString, automations: [],
            credits: m.credits, debt: m.debt, fuel: m.fuel, fuelCapacity: m.fuelCapacity, wearBps: m.wearBps,
            docked: m.docked, enRouteTo: m.enRouteTo, pilotAlive: false, leaseHoursLeft: nil, reachable: true,
            lastEvent: m.freight?.last?.event, lastAt: nil, mood: report.mood, openProposals: 0, sentence: report.headline,
            leasePrincipal: m.leasePrincipal, leaseServicePaid: m.leaseServicePaid
        )
        out.worldName = s.worldName
        return [out]
    }

    public func persona(world: String) async throws -> Persona? { personas.load(keyID: keyID) }

    /// The hull's freight ledger and receipts as journal lines the voice can tell: the exchange
    /// keeps them as `{event, outcome, tick, loadId, freightPaid…}`; each becomes an event
    /// `freight` (or `receipt`) with the text as `why`, so the floor renders it verbatim.
    public static func journal(from me: Me, receipts: [Receipt]) -> [JournalEntry] {
        var out: [JournalEntry] = []
        for f in me.freight ?? [] {
            var fields: [String: JSONValue] = ["why": .string(f.event)]
            if let o = f.outcome { fields["outcome"] = .string(o) }
            if let l = f.loadId { fields["load"] = .string(l) }
            if let p = f.freightPaid, p != 0 { fields["credits_paid"] = .number(Double(p)) }
            if let u = f.unitsDelivered, u != 0 { fields["units"] = .number(Double(u)) }
            out.append(JournalEntry(at: 0, tick: f.tick, event: f.event.hasPrefix("rejected") ? "refused-at-the-door" : "freight", fields: fields))
        }
        for r in receipts {
            out.append(JournalEntry(at: 0, tick: r.tick, event: "trade-outcome", fields: [
                "side": .string(r.side), "units": .number(Double(r.units)), "good": .string(r.good),
                "outcome": .string(r.outcome ?? "filled"), "total": .number(Double(r.total ?? 0)),
            ]))
        }
        return out.sorted { ($0.tick ?? 0) < ($1.tick ?? 0) }
    }

    public func journal(world: String, sinceTick: Int64?) async throws -> [JournalEntry] {
        async let me = client.me()
        async let receipts = client.receipts()
        let all = DirectFeed.journal(from: try await me, receipts: (try? await receipts) ?? [])
        return sinceTick.map { t in all.filter { ($0.tick ?? -1) >= t } } ?? all
    }

    public func window(world: String) async throws -> [MessageItem] { [] }
    public func dial(world: String) async throws -> DialSheet { DialSheet(loaded: .absent, bought: []) }

    public func book(world: String) async throws -> ShipBook {
        let m: Me = try await client.me()
        let events: [FreightEvent] = m.freight ?? []
        let deliveries = events.compactMap { f -> DeliveryStat? in
            guard let l = f.loadId, let p = f.freightPaid, p > 0, f.event.hasPrefix("delivered") || f.outcome == "paid" else { return nil }
            return DeliveryStat(loadID: l, good: "", perishable: false, booked: p, paid: p)
        }
        return ShipBook(holdings: [], deliveries: deliveries)
    }

    public func context(world: String, worldInstance: String?) async throws -> (frame: String?, documents: [ContextDocument]) {
        let m = try await client.me()
        let name = personas.load(keyID: keyID)?.name ?? Persona.rootName
        let frame = "ship, hull \(m.shipName ?? "?") (\(worldInstance ?? KnownExchange.name(for: client.server.absoluteString))), captain \((try? await client.profile())?.traderName ?? "?"), computer \(name) — direct to the exchange, no pilot aboard"
        var docs: [ContextDocument] = []
        if let fuel = try? await fuelPicture(me: m) { docs.append(ContextDocument(name: "fuel", title: "fuel picture — fuel aboard, every pump with distance, cost and reachability, what this berth would buy, the ways out when stranded (computed by the app from the wire)", text: fuel)) }
        return (frame, docs)
    }

    /// The fuel picture, computed here: pumps are the stations that sell fuel; each is priced by
    /// `/v1/route` from where she stands; the berth's quotes say what her hold would fetch.
    public func fuelPicture(me m: Me) async throws -> String {
        let stations = try await client.stations()
        let here = m.docked ?? m.enRouteTo ?? ""
        let fuel = m.fuel ?? 0, cap = m.fuelCapacity ?? 0
        var pumps: [JSONValue] = []
        for st in stations where st.sellsFuel == true {
            if st.id == here {
                pumps.append(.object(["station": .string(st.id), "here": .bool(true), "ticks": .number(0), "fuel_cost": .number(0), "reachable": .bool(true),
                                      "fill_price": .number(Double((cap - fuel) * fuelPricePerUnit)), "affordable": .bool((cap - fuel) * fuelPricePerUnit <= m.credits)]))
                continue
            }
            guard !here.isEmpty, let r = try? await client.route(from: here, to: st.id) else { continue }
            let cost = r.fuel, reachable = cost <= fuel
            let fill = (cap - max(0, fuel - cost)) * fuelPricePerUnit
            var o: [String: JSONValue] = ["station": .string(st.id), "here": .bool(false), "ticks": .number(Double(r.ticks)), "fuel_cost": .number(Double(cost)),
                                          "reachable": .bool(reachable), "fill_price": .number(Double(fill)), "affordable": .bool(fill <= m.credits)]
            if !reachable { o["short_by"] = .number(Double(cost - fuel)) }
            pumps.append(.object(o))
        }
        var saleable: [JSONValue] = []
        if let d = m.docked, let q = try? await client.quotes(station: d) {
            for lot in m.cargo ?? [] where lot.units > 0 {
                if let quote = q.goods.first(where: { $0.good == lot.good }) {
                    let take = min(lot.units, quote.maxSellUnits ?? 0)
                    saleable.append(.object(["good": .string(lot.good), "units": .number(Double(lot.units)), "bid": .number(Double(quote.bid)), "will_take": .number(Double(take)), "worth": .number(Double(take * quote.bid))]))
                }
            }
        }
        let reachable = pumps.filter { $0["reachable"]?.bool == true }.compactMap { $0["station"]?.string }
        let picture: JSONValue = .object([
            "fuel": .number(Double(fuel)), "capacity": .number(Double(cap)), "docked": m.docked.map { .string($0) } ?? .null,
            "credits": .number(Double(m.credits)), "fill_price_here": .number(Double((cap - fuel) * fuelPricePerUnit)),
            "stranded": .bool(reachable.isEmpty && m.docked != nil), "can_reach": .array(reachable.map { .string($0) }),
            "pumps": .array(pumps), "saleable_here": .array(saleable),
            "tanker": .object(["available": .bool(true), "pilot_will_call": .bool(false), "why": .string("a PAWS call-out is days of transit and pins the hull where it stands (metal#59); no pilot is aboard in direct mode, so calling it is the captain's own act in the game")]),
            "if_stranded": .string("sell what this berth will take for credits, wait for a load whose origin is reachable, ask another captain (metal#75 proposes fuel between hulls), or call the tanker knowingly"),
        ])
        return Briefs.fuel(picture) + "\n(Fuel priced at the pack's \(fuelPricePerUnit) ℳ per unit until the exchange publishes its own.)"
    }

    // MARK: acts — no host, so only what the device itself holds

    public func approve(world: String, proposalID: String, approved: Bool) async throws { throw FeedError.needsHost("proposals come from a pilot, and there is no pilot in direct mode") }
    public func setDial(world: String, dial: AutonomyDial) async throws { throw FeedError.needsHost("the dial governs a pilot, and there is no pilot in direct mode") }
    public func pair(_ request: PairingRequest, key: PairingKey) async throws { throw FeedError.needsHost("pairing runs a pilot on a familiar host; add a host connection to pair") }
    public func unpair(world: String) async throws { throw FeedError.needsHost("nothing is paired in direct mode; remove the connection instead") }
    public func rename(world: String, computer: String) async throws -> String? {
        var p = personas.load(keyID: keyID) ?? Persona(name: computer, style: Style())
        p.name = computer; p.personaVersion = 2
        if p.style == nil { p.style = Style() }
        personas.save(p, keyID: keyID)
        return "named on this device; a familiar host would carry it across the fleet"
    }
    public func setAutomations(world: String, automations: [Automation]) async throws -> String? { throw FeedError.needsHost("automations are a pilot's grants; there is no pilot in direct mode") }
    public func setCaptain(world: String, captain: String) async throws -> String? { throw FeedError.needsHost("captains are a host's records; the exchange already knows this key's trader") }

    // MARK: enrolment on a dev world

    /// `POST /v1/enrol` on an exchange that allows it (a dev world): a new pilot and key.
    public static func enrol(exchange: String, traderName: String?, deviceID: String) async throws -> (key: String, traderName: String, welcome: String) {
        guard let url = URL(string: (exchange.hasSuffix("/") ? String(exchange.dropLast()) : exchange) + "/v1/enrol") else { throw FeedError.refused("bad exchange URL") }
        var r = URLRequest(url: url); r.httpMethod = "POST"
        r.setValue("application/json", forHTTPHeaderField: "Content-Type"); r.setValue("UCF Familiar", forHTTPHeaderField: "X-UCF-App")
        var body: [String: JSONValue] = ["app": .string("UCF Familiar"), "deviceId": .string(deviceID)]
        if let t = traderName, !t.isEmpty { body["traderName"] = .string(t) }
        r.httpBody = try JSONEncoder().encode(body)
        let (data, resp) = try await URLSession.shared.data(for: r)
        let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
        guard (200..<300).contains(code) else {
            let why = (try? JSONDecoder().decode(JSONValue.self, from: data))?["error"]?.string
            throw FeedError.refused(why ?? "HTTP \(code) on /v1/enrol")
        }
        let v = try JSONDecoder().decode(JSONValue.self, from: data)
        guard let key = v["key"]?.string else { throw FeedError.refused("the exchange answered without a key") }
        return (key, v["traderName"]?.string ?? "", v["welcome"]?.string ?? "")
    }
}
