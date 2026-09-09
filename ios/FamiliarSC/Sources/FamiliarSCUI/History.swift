import Foundation
import FamiliarSC

// T-239 — the lived-in ship. A hull's EARNED history as a record the familiar keeps: routes
// flown, deliveries completed, repairs and refits, distress survived, escort work when the
// journal carries it. Every mark cites the journal ticks it came from, and NOTHING in it is
// purchasable or editable from the app — the ethics rail of the customization dialogue
// (docs/reviews/2026-08-31-ship-customization-dialogue.md, Round 3): earned history is the one
// currency that cannot be bought, and paid items never change it. This file is pure: journal +
// book in, marks out; no store write, no wire POST, no engine change. Fixtures pin a
// synthesized store, never a real hull's journal.

/// One line of name lineage — the host's fleet-wide `captains/names.jsonl` (Ian, 2026-09-08,
/// verbatim: "Names are unique. We remember names. Names are important to the familiar. Lineage
/// is important. We do not forget names."), or a ship store's own `persona-names.jsonl`.
public struct NameLine: Equatable, Sendable {
    /// `captain` | `hull` | `computer`
    public var kind: String
    public var name: String
    public var holder: String
    /// `paired` | `named` | `renamed` | `reassigned` | `unpaired`
    public var act: String
    public var from: String
    public var by: String
    /// Wall-clock seconds — the ledger keeps time, not ticks.
    public var at: Int64
    public init(kind: String, name: String, holder: String = "", act: String, from: String = "", by: String = "", at: Int64) {
        self.kind = kind; self.name = name; self.holder = holder; self.act = act; self.from = from; self.by = by; self.at = at
    }
    /// A store's own naming trail, as lineage: the computer's names on this hull.
    public init(naming e: NameEvent) {
        self.init(kind: "computer", name: e.name, act: e.actor == "pairing" ? "paired" : "named", by: e.actor, at: e.at)
    }
    /// One ledger row off the wire; nil for a row without a kind or a name.
    public init?(ledger v: JSONValue) {
        guard let kind = v["kind"]?.string, let name = v["name"]?.string else { return nil }
        self.init(kind: kind, name: name, holder: v["holder"]?.string ?? "", act: v["act"]?.string ?? "named",
                  from: v["from"]?.string ?? "", by: v["by"]?.string ?? "", at: v["at"]?.int ?? 0)
    }
}

public struct ShipHistory: Equatable, Sendable {
    public struct Mark: Equatable, Sendable, Identifiable {
        public enum Kind: String, Equatable, Sendable, CaseIterable {
            case name, route, delivery, repair, refit, distress, rescue, escort
        }
        public var id: String { kind.rawValue + ":" + key }
        public var kind: Kind
        /// What makes this mark one mark (a station pair, a fitting, a load…).
        public var key: String
        public var text: String
        public var count: Int
        public var firstTick: Int64
        public var lastTick: Int64
        /// The journal ticks this mark is made of — the citation, in journal order.
        public var ticks: [Int64]
    }

    public var marks: [Mark]
    public var firstTick: Int64?
    public var lastTick: Int64?

    public func marks(of kind: Mark.Kind) -> [Mark] { marks.filter { $0.kind == kind } }

    /// The record, from a hull's journal and book alone — and the names she and her people
    /// have worn, when the store or the host remembers them.
    public static func from(journal: [JournalEntry], book: ShipBook, names: [NameLine] = []) -> ShipHistory {
        var builder = Builder()
        for e in journal.sorted(by: { ($0.tick ?? 0, $0.at) < ($1.tick ?? 0, $1.at) }) { builder.take(e) }
        builder.take(book: book)
        var h = builder.finish()
        let lineage = names.sorted { $0.at < $1.at }.map { n -> Mark in
            let who = n.kind == "computer" ? "the computer" : n.kind == "hull" ? "the hull" : "the captain"
            var text: String
            switch n.act {
            case "paired": text = "\(who) was \(n.name) from the pairing"
            case "renamed": text = "\(who) became \(n.name)" + (n.from.isEmpty ? "" : ", was \(n.from)")
            case "reassigned": text = "\(who) \(n.name) passed to another captain" + (n.from.isEmpty ? "" : " from \(n.from)")
            case "unpaired": text = "\(who) \(n.name) was unpaired"
            default: text = "\(who) was named \(n.name)" + (n.from.isEmpty ? "" : ", was \(n.from)")
            }
            if !n.by.isEmpty && n.by != "pairing" { text += " (by \(n.by))" }
            text += " on " + ShipHistory.day(n.at)
            return Mark(kind: .name, key: "\(n.kind):\(n.name):\(n.at)", text: text, count: 1, firstTick: 0, lastTick: 0, ticks: [])
        }
        h.marks = lineage + h.marks
        return h
    }

    static func day(_ at: Int64) -> String {
        let f = ISO8601DateFormatter(); f.formatOptions = [.withFullDate]
        return f.string(from: Date(timeIntervalSince1970: TimeInterval(at)))
    }

    /// The story as the bridge tells it and the voice is grounded on — plain sentences,
    /// every number and tick from the marks.
    public var story: String {
        if marks.isEmpty { return "No history yet: nothing flown, nothing delivered, nothing survived. The record begins with her first leg." }
        var lines: [String] = []
        func section(_ kind: Mark.Kind, _ head: String) {
            let ms = marks(of: kind)
            guard !ms.isEmpty else { return }
            lines.append(head + ": " + ms.map { $0.text + ($0.ticks.isEmpty ? "" : " [" + $0.ticks.map { "t\($0)" }.joined(separator: ", ") + "]") }.joined(separator: "; ") + ".")
        }
        section(.name, "Names")
        section(.route, "Routes flown")
        section(.delivery, "Deliveries")
        section(.repair, "Repairs")
        section(.refit, "Refits")
        section(.rescue, "Rescues")
        section(.distress, "Distress survived")
        section(.escort, "Escort work")
        if let f = firstTick, let l = lastTick { lines.append("The record runs t\(f)–t\(l). Nothing here can be bought or edited; it is what she did, and the names are not forgotten.") }
        return lines.joined(separator: "\n")
    }

    // MARK: - the builder

    struct Builder {
        var routes: [String: (from: String, to: String, ticks: [Int64])] = [:]
        var routeOrder: [String] = []
        var repairs: [Int64] = []
        var refits: [String: (station: String, price: Int64, ticks: [Int64])] = [:]
        var refitOrder: [String] = []
        var rescues: [Int64] = []
        var escorts: [String: [Int64]] = [:]
        var escortOrder: [String] = []
        /// An open distress: the tick it began, closed by the next act that moves her.
        var distressOpen: Int64?
        var distressSurvived: [(began: Int64, ended: Int64, why: String)] = []
        var distressWhy: String = ""
        var closedLoads: [String: Int64] = [:]
        var berth: String?
        var first: Int64?
        var last: Int64?

        mutating func take(_ e: JournalEntry) {
            guard let tick = e.tick else { return }
            first = first.map { min($0, tick) } ?? tick
            last = last.map { max($0, tick) } ?? tick
            if let d = e.string("docked"), !d.isEmpty { berth = d }
            switch e.event {
            case "engaged-drive", "unwedged-course", "carry-to-market":
                guard let to = e.string("to"), !to.isEmpty else { return }
                let from = berth ?? "?"
                let key = from + "→" + to
                if routes[key] == nil { routes[key] = (from, to, []); routeOrder.append(key) }
                routes[key]!.ticks.append(tick)
                berth = to   // she arrives where she engaged for, unless the journal says otherwise later
                closeDistress(at: tick)
            case "acted":
                let d = e.string("decision") ?? ""
                if d.hasPrefix("Repair") { repairs.append(tick); closeDistress(at: tick) }
                if d.hasPrefix("CallPaws") { rescues.append(tick); closeDistress(at: tick) }
                if d.hasPrefix("Travel") || d.hasPrefix("DivertToPump") { closeDistress(at: tick) }
            case "outfitted":
                guard let f = e.string("fitting") else { return }
                if refits[f] == nil { refits[f] = (e.string("at_station") ?? "?", e.int("price") ?? 0, []); refitOrder.append(f) }
                refits[f]!.ticks.append(tick)
            case "distress-hold":
                if distressOpen == nil { distressOpen = tick; distressWhy = e.string("why") ?? "" }
            case "load-closed":
                if let l = e.string("load"), (e.string("why") ?? "").hasPrefix("settled") { closedLoads[l] = tick }
            case "escort-booked", "convoy-completed", "escorted":
                let post = e.string("post") ?? e.string("to") ?? "escort"
                if escorts[post] == nil { escorts[post] = []; escortOrder.append(post) }
                escorts[post]!.append(tick)
            default:
                break
            }
        }

        mutating func closeDistress(at tick: Int64) {
            guard let began = distressOpen, tick > began else { return }
            distressSurvived.append((began, tick, distressWhy)); distressOpen = nil; distressWhy = ""
        }

        var deliveries: [(DeliveryStat, Int64?)] = []
        mutating func take(book: ShipBook) {
            deliveries = book.deliveries.map { ($0, closedLoads[$0.loadID]) }
        }

        func finish() -> ShipHistory {
            var marks: [ShipHistory.Mark] = []
            for k in routeOrder {
                let r = routes[k]!
                marks.append(.init(kind: .route, key: k, text: "\(r.from) → \(r.to), flown \(r.ticks.count) time\(r.ticks.count == 1 ? "" : "s")", count: r.ticks.count, firstTick: r.ticks.first!, lastTick: r.ticks.last!, ticks: r.ticks))
            }
            if !deliveries.isEmpty {
                let paid = deliveries.reduce(0) { $0 + $1.0.paid }
                let ticks = deliveries.compactMap(\.1).sorted()
                let goods = Array(Set(deliveries.map(\.0.good).filter { !$0.isEmpty })).sorted().joined(separator: ", ")
                marks.append(.init(kind: .delivery, key: "all", text: "\(deliveries.count) load\(deliveries.count == 1 ? "" : "s") delivered, ℳ\(paid) freight paid" + (goods.isEmpty ? "" : " (\(goods))"), count: deliveries.count, firstTick: ticks.first ?? 0, lastTick: ticks.last ?? 0, ticks: ticks))
            }
            if !repairs.isEmpty { marks.append(.init(kind: .repair, key: "yard", text: "repaired at the yard \(repairs.count) time\(repairs.count == 1 ? "" : "s")", count: repairs.count, firstTick: repairs.first!, lastTick: repairs.last!, ticks: repairs)) }
            for f in refitOrder {
                let r = refits[f]!
                marks.append(.init(kind: .refit, key: f, text: "fitted \(f) at \(r.station) for ℳ\(r.price)", count: r.ticks.count, firstTick: r.ticks.first!, lastTick: r.ticks.last!, ticks: r.ticks))
            }
            if !rescues.isEmpty { marks.append(.init(kind: .rescue, key: "paws", text: "called the PAWS tanker \(rescues.count) time\(rescues.count == 1 ? "" : "s")", count: rescues.count, firstTick: rescues.first!, lastTick: rescues.last!, ticks: rescues)) }
            for (i, d) in distressSurvived.enumerated() {
                marks.append(.init(kind: .distress, key: "\(d.began)", text: "held in distress from t\(d.began) to t\(d.ended)" + (d.why.isEmpty ? "" : " (\(d.why))") + ", and flew again", count: 1, firstTick: d.began, lastTick: d.ended, ticks: [d.began, d.ended]))
                _ = i
            }
            if let open = distressOpen {
                marks.append(.init(kind: .distress, key: "\(open)-open", text: "in distress since t\(open)" + (distressWhy.isEmpty ? "" : " (\(distressWhy))") + " — not yet survived", count: 1, firstTick: open, lastTick: open, ticks: [open]))
            }
            for p in escortOrder {
                let t = escorts[p]!
                marks.append(.init(kind: .escort, key: p, text: "escort work on \(p), \(t.count) time\(t.count == 1 ? "" : "s")", count: t.count, firstTick: t.first!, lastTick: t.last!, ticks: t))
            }
            return ShipHistory(marks: marks, firstTick: first, lastTick: last)
        }
    }
}
