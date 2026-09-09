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
/// INTERIM, not the design (Ian's ruling 2026-09-07, filed as united-cat-foods-metal#86): the
/// ship's computer's name is the captain's, and it belongs on the captain's record in the
/// WORLD — a child namespace that follows the captain to every hull, station and raceway.
/// Until Jeff serves that field, direct mode keeps a per-key name the captain types here;
/// when the wire carries it, this store becomes at most a cache of the world's fact.
/// Do not build more on it.
public struct DevicePersonaStore: @unchecked Sendable {  // UserDefaults is thread-safe; the compiler does not know it
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
    public var client: ExchangeClient
    public let keyID: String
    public var personas = DevicePersonaStore()
    /// The pilot's MIND, when the app links FamiliarCore (T-237 B4, "one doctrine, two runtimes"):
    /// `whiskerAdvise(inputJson:)` — the same Rust doctrine the host runner flies, answering from
    /// the JSON this feed fetched. Nil in a shell without the core (tests, the package alone):
    /// then there is no pilot document and the computer says so.
    public var adviser: (@Sendable (String) -> String)?
    /// The pack's fuel price, until the exchange publishes `fuelPricePerUnit` (ucf-exchange#22).
    public var fuelPricePerUnit: Int64 = 2

    /// The seam this shell was built for. `whisker_advise` stamps its answer with
    /// `seam_version`; a mismatch means the core linked into this build and the shell's
    /// reading of it are not the same generation, and the verdict is refused rather than
    /// read past — the skew guard codex asked for in place of a manual promise (T-237 B4
    /// re-verification, finding 1). Bump together with `whisker::wire::SEAM_VERSION`.
    public static let seamVersion: Int64 = 2

    /// One gather of the pilot's mind: the reading, the raw verdict, the act it maps to when
    /// it maps to one, and the exact input the doctrine was handed (tests pin its shape).
    public struct Advice: Sendable {
        public var text: String
        public var verdict: JSONValue
        public var proposal: PilotProposal?
        public var input: JSONValue
        public var tick: Int64?
        /// Legs the exchange would not price / hull rungs it would not answer, for the reading.
        public var unpriced: Int
        public var unquotedRungs: Int
    }

    /// The last gather, kept a moment so the context read and the proposal read of one
    /// screen-open do not price every leg twice, and so both see ONE actionId. Keyed on the
    /// world's tick; a confirm always asks fresh.
    final class AdviceMemo: @unchecked Sendable {
        private let lock = NSLock()
        private var held: (tick: Int64?, at: Date, advice: Advice)?
        func take(tick: Int64?) -> Advice? {
            lock.lock(); defer { lock.unlock() }
            guard let h = held, h.tick == tick, Date().timeIntervalSince(h.at) < 30 else { return nil }
            return h.advice
        }
        func keep(_ a: Advice) { lock.lock(); held = (a.tick, Date(), a); lock.unlock() }
        func drop() { lock.lock(); held = nil; lock.unlock() }
    }
    let memo = AdviceMemo()

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

    /// The pilot document for what the gather returned: the doctrine's reading, or the
    /// plain fact that there is no mind in this shell, or the reason it could not be asked.
    /// Never absent — a missing document would read the same as "nothing to advise".
    static func pilotDocument(_ asked: Result<String?, Error>) -> ContextDocument {
        let title = "the pilot's mind — what the pilot would do right now, the dial surface it spends and the automation it needs; a reading, not an act"
        switch asked {
        case .success(let text?): return ContextDocument(name: "pilot", title: title, text: text)
        case .success(nil): return ContextDocument(name: "pilot", title: title, text: "This shell carries no pilot's mind (FamiliarCore is not linked), so the doctrine cannot be asked here.")
        case .failure(let error): return ContextDocument(name: "pilot", title: title, text: "The pilot's mind could not be asked: \(DirectFeed.describe(error)).")
        }
    }

    /// Error text a captain can read.
    static func describe(_ error: Error) -> String {
        if let e = error as? ExchangeError { return "\(e)" }
        if let f = error as? FeedError { return f.description }
        return (error as NSError).localizedDescription
    }

    public func context(world: String, worldInstance: String?) async throws -> (frame: String?, documents: [ContextDocument]) {
        let m = try await client.me()
        let name = personas.load(keyID: keyID)?.name ?? Persona.rootName
        // The frame says where the mind is: in direct mode the pilot PROCESS is on the host (or
        // nowhere), while the pilot's MIND — the same doctrine — answers from this device when
        // the shell links the core. "No pilot aboard" read as a fault on Ian's iPad (2026-09-07).
        let mind = adviser == nil ? "no pilot's mind in this shell" : "the pilot's mind answers from this device; no pilot process aboard"
        let frame = "ship, hull \(m.shipName ?? "?") (\(worldInstance ?? KnownExchange.name(for: client.server.absoluteString))), captain \((try? await client.profile())?.traderName ?? "?"), computer \(name) — direct to the exchange; \(mind)"
        var docs: [ContextDocument] = []
        // A gather that fails is SAID, never dropped: a legitimate "nothing to advise" and a
        // broken read must not look the same (the silent-departure lesson, metal#79).
        let asked: Result<String?, Error>
        do { asked = .success(try await pilotAdvice(me: m)?.text) } catch { asked = .failure(error) }
        docs.append(DirectFeed.pilotDocument(asked))
        let fuelTitle = "fuel picture — fuel aboard, every pump with distance, cost and reachability, what this berth would buy, the ways out when stranded (computed by the app from the wire)"
        do { docs.append(ContextDocument(name: "fuel", title: fuelTitle, text: try await fuelPicture(me: m))) }
        catch { docs.append(ContextDocument(name: "fuel", title: fuelTitle, text: "The fuel picture could not be read: \(DirectFeed.describe(error)).")) }
        return (frame, docs)
    }

    /// What the pilot would do now, from the doctrine itself — the reading and the verdict.
    public func pilotAdvice(me m: Me) async throws -> (text: String, verdict: JSONValue)? {
        try await advice(me: m).map { ($0.text, $0.verdict) }
    }

    /// The pilot's mind asked over what the host runner reads before a fold — `/v1/me`, the
    /// open board, the stations, the yard's repair rate, the captain's own contract as ITS OWN
    /// object (the open board never carries it; finding 2) — with the legs priced as the
    /// runner prices them and, on every leg to a pump, the exchange's price for THIS hull at
    /// the standard and economy rungs (`rungs`), which the doctrine treats as authoritative
    /// over its model (finding 1). No dial is sent: none exists on this device (finding 5).
    /// The doctrine judges; nothing is acted. Nil without an adviser.
    public func advice(me m: Me, fresh: Bool = false) async throws -> Advice? {
        guard let adviser else { return nil }
        if !fresh, let held = memo.take(tick: m.tick) { return held }
        let me = try JSONDecoder().decode(JSONValue.self, from: try await client.get("/v1/me"))
        let board = try JSONDecoder().decode(JSONValue.self, from: try await client.get("/v1/loadboard"))
        let stations = try JSONDecoder().decode(JSONValue.self, from: try await client.get("/v1/stations"))
        // The captain's own board is a REQUIRED read: a 500, a timeout or an unreadable shape
        // here used to read as "no active contract", and a hull under contract could be
        // shown — and after the same failure on the fresh re-read, FILED — a freight-idle act
        // (codex T-237 B4 re-verification r2, finding 1). Now the gather fails, named.
        let mineData = try await client.get("/v1/loadboard?mine=true")
        let mine: JSONValue
        do { mine = try JSONDecoder().decode(JSONValue.self, from: mineData) }
        catch { throw ExchangeError.decode("/v1/loadboard?mine=true", "\(error)") }
        let repair = (try? await client.reference())?.params?["repairCostPerHundredBps"]?.double.map { Int64($0) } ?? 40
        let here = m.docked ?? m.enRouteTo ?? ""
        let pumps = Set((stations.array ?? []).filter { $0["sellsFuel"]?.bool == true }.compactMap { $0["id"]?.string })
        // Price what the runner prices: the five best rows by estimated net (here → origin, origin → dest)
        // and every pump from here — the doctrine's reachable_pump needs those to judge a divert.
        var pairs: [(String, String)] = []
        let rows = (board.array ?? []).filter { $0["heldForOther"]?.bool != true }
            .sorted { ($0["estimatedNet"]?.double ?? 0) > ($1["estimatedNet"]?.double ?? 0) }.prefix(5)
        for r in rows { if let o = r["origin"]?.string, let d = r["dest"]?.string { pairs.append((here, o)); pairs.append((o, d)) } }
        for p in pumps.sorted() { pairs.append((here, p)) }
        var seen = Set<String>()
        let legs = pairs.filter { (from, to) in !from.isEmpty && from != to && seen.insert("\(from)→\(to)").inserted }
        // Price the legs concurrently (a phone's network pays each round trip in full; the
        // serial walk took seconds and a cancelled view swallowed it whole). A leg the
        // exchange cannot price is left out and COUNTED, so "no pump in reach" is honest;
        // a hull rung it will not answer is counted too, and that pump is then modelled
        // from the reference quote exactly as the host does when the world will not say.
        let priced: [(row: JSONValue, unquoted: Int)] = await withTaskGroup(of: (JSONValue, Int)?.self) { group in
            for (from, to) in legs {
                group.addTask { [client] in
                    guard let r = try? await client.route(from: from, to: to) else { return nil }
                    var row: [String: JSONValue] = ["from": .string(from), "to": .string(to), "fuel": .number(Double(r.fuel)),
                                                    "legs_km": .array(r.legs.compactMap { $0.distanceKm.map { .number(Double($0)) } })]
                    var unquoted = 0
                    if pumps.contains(to) {
                        var rungs: [String: JSONValue] = [:]
                        for name in ["standard", "economy"] {
                            if let h = (try? await client.route(from: from, to: to, forHullAt: name))?.forHull {
                                rungs[name] = .object(["fuel": .number(Double(h.totalFuel)), "ticks": .number(Double(h.totalTicks))])
                            } else { unquoted += 1 }
                        }
                        if !rungs.isEmpty { row["rungs"] = .object(rungs) }
                    }
                    return (.object(row), unquoted)
                }
            }
            var out: [(JSONValue, Int)] = []
            for await r in group { if let r { out.append(r) } }
            return out
        }
        // Keys hoisted out of the closures: four optional chains with ?? and + inside one
        // sort closure is the type-checker's classic blow-up under -O (Xcode 26.5 Release
        // archive on wildhorse, 2026-09-08 — a debug build passes what Release rejects).
        func routeKey(_ r: JSONValue) -> String { (r["from"]?.string ?? "") + "→" + (r["to"]?.string ?? "") }
        let routes = priced.map(\.row).sorted { routeKey($0) < routeKey($1) }
        let unquotedRungs = priced.reduce(0) { $0 + $1.unquoted }
        // The captain's live contract: the row the ledger still holds open, ranked the way the
        // host tracks it (a hull in transit first, then one booked and waiting, then one
        // delivered whose money waits). The seam reads the ledger word from /v1/me.freight
        // itself, so a row the ledger has settled or lost is dropped there, as on the host.
        let live = (mine.array ?? []).filter { !["settled", "expired", "cancelled", "lost"].contains($0["status"]?.string ?? "") }
        func rank(_ r: JSONValue) -> Int {
            switch r["status"]?.string { case "inTransit", "pickedUp": return 0; case "booked", "assigned", "awaitingPickup": return 1; case "delivered": return 2; default: return 3 }
        }
        func activeKey(_ r: JSONValue) -> String { "\(rank(r)):" + (r["loadId"]?.string ?? "") }
        let active = live.min { activeKey($0) < activeKey($1) }
        var input: [String: JSONValue] = ["me": me, "board": board, "stations": stations, "routes": .array(routes),
                                          "repair_per_hundred_bps": .number(Double(repair))]
        if let active { input["active"] = .object(["row": active]) }
        let inputValue = JSONValue.object(input)
        // Fail CLOSED on an inconsistent record: the ledger (/v1/me.freight) says a contract is
        // open — the same reading the doctrine makes — but the mine board carries no row for
        // it. The doctrine would be told the hull is idle; it is not asked at all.
        let openOnLedger = DirectFeed.openLoads(me: me)
        let liveIDs = Set(live.compactMap { $0["loadId"]?.string })
        let missing = openOnLedger.keys.filter { !liveIDs.contains($0) }.sorted()
        if !missing.isEmpty {
            var advice = Advice(text: "", verdict: .null, proposal: nil, input: inputValue, tick: m.tick, unpriced: legs.count - routes.count, unquotedRungs: unquotedRungs)
            advice.text = "The pilot's mind was not asked: the ledger says " + missing.map { "\($0) is \(openOnLedger[$0] ?? "open")" }.joined(separator: ", ")
                + " but the captain's board carries no such row. A contract that is open on one read and absent on the other is an inconsistent record, and nothing is judged or filed on it. Pull to read again."
            memo.keep(advice); return advice
        }
        let out = adviser(inputValue.description)
        var advice = Advice(text: "", verdict: .null, proposal: nil, input: inputValue, tick: m.tick, unpriced: legs.count - routes.count, unquotedRungs: unquotedRungs)
        guard let data = out.data(using: .utf8), let verdict = try? JSONDecoder().decode(JSONValue.self, from: data) else {
            advice.text = "The pilot's mind answered in words the shell could not read: \(out.prefix(200))"
            memo.keep(advice); return advice
        }
        advice.verdict = verdict
        if let err = verdict["error"]?.string { advice.text = "The pilot's mind could not read the wire: \(err)"; memo.keep(advice); return advice }
        guard verdict["seam_version"]?.int == DirectFeed.seamVersion else {
            let got = verdict["seam_version"]?.int.map { "seam \($0)" } ?? "an unstamped seam"
            advice.text = "The pilot's mind speaks \(got) and this shell was built for seam \(DirectFeed.seamVersion): the app and the core inside it are not the same generation. Nothing is read from this verdict, and nothing can be filed from it; update the app."
            memo.keep(advice); return advice
        }
        var notes: [String] = []
        if advice.unpriced > 0 { notes.append("Priced \(routes.count) of \(legs.count) legs — the exchange would not price the rest, so a pump or a load it needed may read as out of reach.") }
        if unquotedRungs > 0 { notes.append("The exchange did not price this hull at \(unquotedRungs) pump rung\(unquotedRungs == 1 ? "" : "s"); those pumps are judged from the reference quote, as the host does when the world will not say.") }
        advice.text = ([Briefs.pilot(verdict, governed: false)] + notes).joined(separator: "\n")
        if let act = ExchangeAct.from(decision: verdict["decision"] ?? .null, docked: verdict["ship"]?["docked"]?.string) {
            advice.proposal = PilotProposal(actionId: "ucff-" + UUID().uuidString.lowercased(), act: act,
                                            reasons: Briefs.reasons(verdict["reasons"] ?? .null),
                                            surface: verdict["surface"]?.string, tick: m.tick)
        }
        memo.keep(advice)
        return advice
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
            // The pilot's rungs: standard first, economy only when standard cannot reach.
            let legs = r.legs.compactMap { $0.distanceKm.map { Int64($0) } }
            let plan = BurnRungs.plan(legsKm: legs, quotedAtReference: r.fuel, hullAccelMilliG: m.effectiveAccelMilliG ?? BurnRungs.referenceAccelMilliG, tank: fuel)
            let cost = plan.fuel, reachable = plan.reaches
            let fill = (cap - max(0, fuel - cost)) * fuelPricePerUnit
            var o: [String: JSONValue] = ["station": .string(st.id), "here": .bool(false), "ticks": .number(Double(r.ticks)), "fuel_cost": .number(Double(cost)),
                                          "reachable": .bool(reachable), "fill_price": .number(Double(fill)), "affordable": .bool(fill <= m.credits), "burn": .string(plan.burn)]
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
            "tanker": .object(["available": .bool(true), "pilot_will_call": .bool(false), "why": .string("the tanker's speed is a world dial the wire does not publish (metal#59 raised it on PROD at tick 7842; a rescue that took days now takes about 70 ticks there, but this app cannot read the dial, so it quotes no arrival); no pilot is aboard in direct mode, so calling it is the captain's own act in the game")]),
            "if_stranded": .string("sell what this berth will take for credits, wait for a load whose origin is reachable, ask another captain (metal#75 proposes fuel between hulls), or call the tanker knowingly"),
        ])
        return Briefs.fuel(picture) + "\n(Fuel priced at the pack's \(fuelPricePerUnit) ℳ per unit until the exchange publishes its own.)"
    }

    /// The loads the ledger still holds open, and the word it holds them at — `doctrine::ledger_word`
    /// applied per load over `/v1/me.freight`: "payment taken"/"collected" is settled, reverted /
    /// expired / lapsed / cancel is lost, a rejection with no prior word is lost, else delivered >
    /// picked up > booked (booked when the ledger only says departed/arrived).
    static func openLoads(me: JSONValue) -> [String: String] {
        var events: [String: [String]] = [:]
        var order: [String] = []
        for f in me["freight"]?.array ?? [] {
            guard let id = f["loadId"]?.string, let e = f["event"]?.string else { continue }
            if events[id] == nil { order.append(id) }
            events[id, default: []].append(e)
        }
        var out: [String: String] = [:]
        for id in order {
            var word: String?
            var closed = false
            for e in events[id] ?? [] {
                let l = e.lowercased()
                if l.contains("payment taken") || l.contains("collected") { closed = true; break }
                if l.contains("reverted") || l.contains("expired") || l.contains("lapsed") || l.contains("cancel") { closed = true; break }
                if l.contains("rejected") { if word == nil { closed = true; break }; continue }
                if l.contains("delivered") { word = "delivered" }
                else if l.contains("pickedup") || l.contains("picked up") { if word != "delivered" { word = "picked up" } }
                else if l.contains("booked"), word == nil { word = "booked" }
            }
            if !closed { out[id] = word ?? "booked" }
        }
        return out
    }

    // MARK: acts — no host, so only what the device itself holds, and the one act the captain confirms

    public func pilotProposal(world: String) async throws -> PilotProposal? {
        try await advice(me: try await client.me())?.proposal
    }

    /// The captain's tap. The world moved while the captain read, so the mind is asked again,
    /// FRESH, and the act is filed only if it would still make the same act — the same act,
    /// not the same words. The proposal's own actionId goes on the wire: a retry after a
    /// transport failure carries the same id, never a second intent. Nothing else in this
    /// feed can reach `file`.
    public func confirm(_ p: PilotProposal, world: String) async throws -> String {
        guard adviser != nil else { throw FeedError.unavailable("this shell carries no pilot's mind, so there is nothing to confirm") }
        let now = try await advice(me: try await client.me(), fresh: true)
        // A fresh read that could not ask the mind at all (an inconsistent record, a seam it
        // cannot read) refuses in its own words; nothing is filed on it.
        if let now, now.verdict == .null { memo.drop(); throw FeedError.refused(now.text) }
        guard let live = now?.proposal, live.act == p.act else {
            memo.drop()
            let would = now?.proposal?.act.sentence ?? ("hold" + (now?.verdict["decision"]?["why"]?.string.map { " — \($0)" } ?? ""))
            throw FeedError.refused("the pilot's mind has moved since you read it — it would now \(would). Read it again before confirming; nothing was filed.")
        }
        let ack = try await client.file(p.act.body, actionId: p.actionId)
        memo.drop()
        return "Filed: \(p.act.sentence) (\(ack.actionId))" + (ack.resolvesAtTick.map { " — the fold answers at t\($0)" } ?? "") + ". The outcome lands on the ledger, not here."
    }

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
