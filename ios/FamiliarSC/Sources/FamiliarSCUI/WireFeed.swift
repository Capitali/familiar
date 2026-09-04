import Foundation
import FamiliarSC

/// The phone's feed: `familiar fleet serve` on the household door (wildhorse's half of
/// B3), reached over Tailscale/mesh with the household door bearer. Paths and shapes as
/// agreed 2026-09-04:
///   GET  ships                          → {tick, tick_seconds, ships: [fleet status rows]}
///   GET  ships/{world}/journal?since=N  → {tick, tick_seconds, lines: [journal lines], next: N'}
///   GET  ships/{world}/proposals        → {tick, tick_seconds, proposals: [Proposal + state, answered_at?]}
///   GET  ships/{world}/dial             → {tick, tick_seconds, dial: {…}, bought: […]}
///   GET  ships/{world}/book             → {tick, tick_seconds, holdings: […], deliveries: […]}
///   (each ships row also carries `persona`: the store's persona.json verbatim, or null)
///   POST ships/{world}/approve {id, approved} → the Approval line
///   PUT  ships/{world}/dial {…}
///   POST pair {label, captain, server, key, automations, computer_name?} / POST unpair {world}
/// Proposal lapse is settled client-side exactly as whisker does: lapsed when
/// tick > expires_tick and no approval.
public struct WireFeed: ShipsFeed, CaptainActs {
    public let base: URL
    public let bearer: String
    public var session: URLSession = .shared
    public var prefix = "/"

    public init(base: URL, bearer: String) { self.base = base; self.bearer = bearer }

    func request(_ path: String, method: String = "GET", body: Data? = nil) -> URLRequest {
        // String-built, not appendingPathComponent: a query (`?since=`) must stay a query,
        // not be percent-encoded into the path.
        let root = base.absoluteString.hasSuffix("/") ? String(base.absoluteString.dropLast()) : base.absoluteString
        var r = URLRequest(url: URL(string: root + prefix + path) ?? base)
        r.httpMethod = method
        r.setValue("Bearer \(bearer)", forHTTPHeaderField: "Authorization")
        r.setValue("familiar-sc", forHTTPHeaderField: "X-Familiar-App")
        if let body { r.httpBody = body; r.setValue("application/json", forHTTPHeaderField: "Content-Type") }
        r.timeoutInterval = 15
        return r
    }

    func call(_ path: String, method: String = "GET", body: Data? = nil) async throws -> Data {
        let (data, resp) = try await session.data(for: request(path, method: method, body: body))
        let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
        guard (200..<300).contains(code) else { throw FeedError.refused("HTTP \(code) on \(path)") }
        return data
    }

    struct Envelope: Decodable {
        var tick: Int64?
        var tick_seconds: Int64?
        var ships: [JSONValue]?
        var lines: [JSONValue]?
        var next: Int64?
        var proposals: [JSONValue]?
        var dial: [String: String]?
        var bought: [String]?
        var holdings: [Holding]?
        var deliveries: [DeliveryStat]?
    }

    func envelope(_ path: String) async throws -> Envelope {
        try JSONDecoder().decode(Envelope.self, from: try await call(path))
    }

    static func summary(from row: JSONValue, tick: Int64?) -> ShipSummary? {
        guard let world = row["world"]?.string else { return nil }
        let computer = row["computer"]?.string ?? "(unnamed — `fleet rename` her)"
        return ShipSummary(
            world: world, label: row["label"]?.string ?? world, computer: computer, named: !computer.hasPrefix("("),
            hull: row["hull"]?.string ?? "", captain: row["captain"]?.string ?? "", server: row["server"]?.string ?? "",
            automations: row["automations"]?.array?.compactMap(\.string) ?? [],
            credits: row["credits"]?.int, debt: row["debt"]?.int, fuel: row["fuel"]?.int, fuelCapacity: row["fuelCapacity"]?.int,
            wearBps: row["wearBps"]?.int, docked: row["docked"]?.string, enRouteTo: row["enRouteTo"]?.string,
            pilotAlive: row["pilot_pid"]?.int != nil, leaseHoursLeft: row["lease_expires_in_h"]?.int,
            reachable: row["reachable"]?.bool ?? false, lastEvent: row["last_event"]?.string, lastAt: row["last_at"]?.int,
            mood: BridgeReport.Mood(rawValue: row["mood"]?.string ?? "") ?? .steady,
            openProposals: Int(row["open_proposals"]?.int ?? 0),
            leasePrincipal: row["leasePrincipal"]?.int, leaseServicePaid: row["leaseServicePaid"]?.int,
            trades: row["trades"].map { TradeBook(row: $0) }
        )
    }

    public func ships() async throws -> [ShipSummary] {
        let e = try await envelope("ships")
        return (e.ships ?? []).compactMap { WireFeed.summary(from: $0, tick: e.tick) }
    }

    /// The ships row's `persona` — the store's persona.json verbatim (decoded with the same
    /// loud loader as the store), or nil when the computer has not been named.
    public func persona(world: String) async throws -> Persona? {
        let e = try await envelope("ships")
        guard let row = (e.ships ?? []).first(where: { $0["world"]?.string == world }), let p = row["persona"], p != .null else { return nil }
        return try Persona.decode(Data(p.description.utf8))
    }

    public func journal(world: String, sinceTick: Int64?) async throws -> [JournalEntry] {
        var since: Int64 = 0
        var out: [JournalEntry] = []
        for _ in 0..<64 {   // bounded: a runaway cursor is the server's bug, not a phone hang
            let e = try await envelope("ships/\(world)/journal?since=\(since)")
            let lines = e.lines ?? []
            out += lines.compactMap { JournalEntry.parse(line: Substring($0.description)) }
            guard let next = e.next, next > since, !lines.isEmpty else { break }
            since = next
        }
        return sinceTick.map { t in out.filter { ($0.tick ?? -1) >= t } } ?? out
    }

    public func window(world: String) async throws -> [MessageItem] {
        let j = try await journal(world: world, sinceTick: nil)
        let e = try await envelope("ships/\(world)/proposals")
        var proposals: [Proposal] = []
        var approvals: [Approval] = []
        for p in e.proposals ?? [] {
            guard let data = p.description.data(using: .utf8), let prop = try? JSONDecoder().decode(Proposal.self, from: data) else { continue }
            proposals.append(prop)
            if let state = p["state"]?.string, state == "approved" || state == "denied" {
                approvals.append(Approval(id: prop.id, approved: state == "approved", at: p["answered_at"]?.int ?? 0))
            }
        }
        return MessageWindow.build(journal: j, proposals: proposals, approvals: approvals, nowTick: e.tick)
    }

    public func dial(world: String) async throws -> DialSheet {
        let e = try await envelope("ships/\(world)/dial")
        let data = try JSONEncoder().encode(e.dial ?? [:])
        let loaded: AutonomyDial.Loaded
        do { loaded = e.dial == nil ? .absent : .dial(try AutonomyDial.decode(data)) } catch let err as StoreError { loaded = .malformed(err.description) }
        return DialSheet(loaded: loaded, bought: e.bought ?? [])
    }

    public func book(world: String) async throws -> ShipBook {
        let e = try await envelope("ships/\(world)/book")
        return ShipBook(holdings: e.holdings ?? [], deliveries: e.deliveries ?? [])
    }

    public func approve(world: String, proposalID: String, approved: Bool) async throws {
        let body = try JSONEncoder().encode(["id": JSONValue.string(proposalID), "approved": JSONValue.bool(approved)])
        _ = try await call("ships/\(world)/approve", method: "POST", body: body)
    }
    public func setDial(world: String, dial: AutonomyDial) async throws {
        _ = try await call("ships/\(world)/dial", method: "PUT", body: dial.encoded())
    }
    public func pair(_ request: PairingRequest, key: PairingKey) async throws {
        var obj: [String: JSONValue] = ["label": .string(request.label), "captain": .string(request.captain), "server": .string(request.server),
                                        "key": .string(key.secret), "automations": .array(request.automations.map { .string($0.rawValue) })]
        if let n = request.computerName { obj["computer_name"] = .string(n) }
        _ = try await call("pair", method: "POST", body: try JSONEncoder().encode(obj))
    }
    public func unpair(world: String) async throws {
        _ = try await call("unpair", method: "POST", body: try JSONEncoder().encode(["world": world]))
    }
}
