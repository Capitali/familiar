import Foundation

// The bridge report and its deterministic floor. `BridgeReport` is the only shape the UI
// renders — headline, facts, next act, mood — whether the templated renderer or Apple
// Intelligence produced it. The renderer is the T-236 brick-2 discipline made Swift: one
// fixture plus one persona renders byte-identically; style changes phrasing, never event
// order, amounts, ids, ticks, refusal reasons or severity; unknown events render neutrally;
// humor reads as zero around danger, loss, refusal or uncertainty.

public struct BridgeReport: Codable, Equatable, Sendable {
    public enum Mood: String, Codable, CaseIterable, Equatable, Sendable {
        case steady, pleased, watchful, concerned
    }
    public var headline: String
    public var facts: [String]
    public var nextAct: String
    public var mood: Mood

    public init(headline: String, facts: [String], nextAct: String, mood: Mood) {
        self.headline = headline; self.facts = facts; self.nextAct = nextAct; self.mood = mood
    }
}

/// What the wire adds to the journal when it is reachable — optional, so the floor still
/// speaks from the store alone.
public struct HullGlance: Equatable, Sendable {
    public var shipName: String?
    public var docked: String?
    public var enRouteTo: String?
    public var credits: Int64
    public var debt: Int64?
    public var fuel: Int64?
    public var fuelCapacity: Int64?
    public var wearBps: Int64?
    public var leased: Bool

    public init(shipName: String? = nil, docked: String? = nil, enRouteTo: String? = nil, credits: Int64, debt: Int64? = nil, fuel: Int64? = nil, fuelCapacity: Int64? = nil, wearBps: Int64? = nil, leased: Bool = false) {
        self.shipName = shipName; self.docked = docked; self.enRouteTo = enRouteTo; self.credits = credits
        self.debt = debt; self.fuel = fuel; self.fuelCapacity = fuelCapacity; self.wearBps = wearBps; self.leased = leased
    }

    public init(me: Me) {
        self.init(shipName: me.shipName, docked: me.docked, enRouteTo: me.enRouteTo, credits: me.credits, debt: me.debt,
                  fuel: me.fuel, fuelCapacity: me.fuelCapacity, wearBps: me.wearBps, leased: me.leased)
    }
}

public struct TemplatedVoice {
    public var persona: Persona
    /// How many facts a report carries at most (the rest are folded into a count line).
    public var maxFacts: Int = 12

    public init(persona: Persona) { self.persona = persona }

    var style: Style { persona.voice }

    // MARK: severity — what a report must never drop

    /// Higher tells first when the window is full. Chatter is 0 and is summarised, never listed.
    static func severity(_ e: JournalEntry) -> Int {
        switch e.event {
        case "distress-hold": return 9
        case "refused-at-the-door", "exchange-unreachable": return 8
        case "proposed", "proposal-lapsed": return 7
        case "traded", "position-opened", "load-closed", "outfitted", "trade-outcome", "fill": return 6
        case "advice", "carry-blocked", "carry-refused", "engage-refused", "refit-refused", "book-corrected", "retargeted": return 5
        case "acted", "engaged-drive", "carry-to-market", "unwedged-course", "adopted-held-contract", "freight": return 4
        case "held-at-the-gate", "watch-begins": return 3
        case "holding", "merchant-idle", "outfit-idle", "awaiting-pending-actions", "awaiting-our-own-fold": return 0
        default: return 2   // unknown: shown, neutrally
        }
    }

    static func isDanger(_ e: JournalEntry) -> Bool {
        ["distress-hold", "refused-at-the-door", "exchange-unreachable", "carry-blocked", "carry-refused",
         "engage-refused", "refit-refused", "held-at-the-gate"].contains(e.event)
            || (e.event == "trade-outcome" && (e.string("outcome") ?? "").hasPrefix("rejected"))
    }

    // MARK: the report

    public func report(entries: [JournalEntry], hull: HullGlance? = nil, openProposals: Int = 0) -> BridgeReport {
        let chatter = entries.filter { TemplatedVoice.severity($0) == 0 }.count
        let told = TemplatedVoice.collapse(entries.filter { TemplatedVoice.severity($0) > 0 })
        // Keep the most consequential, then tell them in journal order.
        let kept: [JournalEntry]
        if told.count > maxFacts {
            let ranked = told.enumerated().sorted { a, b in
                let sa = TemplatedVoice.severity(a.element), sb = TemplatedVoice.severity(b.element)
                return sa != sb ? sa > sb : a.offset > b.offset
            }.prefix(maxFacts).map(\.offset).sorted()
            kept = ranked.map { told[$0] }
        } else {
            kept = told
        }

        var facts = kept.map(fact(for:))
        if told.count > kept.count { facts.append("…and \(told.count - kept.count) more lines in the journal") }
        if chatter > 0 { facts.append(chatterLine(entries: entries, count: chatter)) }
        if let h = hull { facts.insert(hullLine(h), at: 0) }

        let danger = entries.contains(where: TemplatedVoice.isDanger)
        let money = entries.contains { ["traded", "load-closed", "position-opened", "outfitted"].contains($0.event) }
        let mood: BridgeReport.Mood = danger ? .concerned : openProposals > 0 ? .watchful : money ? .pleased : .steady
        return BridgeReport(
            headline: headline(mood: mood, count: told.count, openProposals: openProposals),
            facts: facts,
            nextAct: nextAct(entries: entries, hull: hull, openProposals: openProposals, mood: mood),
            mood: mood
        )
    }

    /// A run of identical trouble lines (the exchange gone for an hour) is one fact with a
    /// count, not sixty facts — the count is kept, so nothing is lost.
    static func collapse(_ entries: [JournalEntry]) -> [JournalEntry] {
        var out: [JournalEntry] = []
        for e in entries {
            if e.event == "exchange-unreachable", var last = out.last, last.event == e.event, last.string("why") == e.string("why") {
                let n = (last.int("repeats") ?? 1) + 1
                last.fields["repeats"] = .number(Double(n))
                last.at = e.at
                out[out.count - 1] = last
            } else {
                out.append(e)
            }
        }
        return out
    }

    // MARK: cadence (style bends these; nothing below carries a fact)

    func address() -> String { style.formOfAddress.isEmpty ? "Captain" : style.formOfAddress }

    func flavour(_ mood: BridgeReport.Mood) -> String {
        // Vocabulary colours the headline only, and only while nothing is wrong.
        guard mood == .steady || mood == .pleased else { return "" }
        switch style.vocabulary {
        case "feline": return style.humor >= 7 ? " Whiskers steady." : " Purring along."
        case "nautical": return style.humor >= 7 ? " Fair winds." : " All hands steady."
        default: return ""
        }
    }

    func have() -> String { style.contractions ? "I've" : "I have" }
    func sheIs() -> String { style.contractions ? "she's" : "she is" }

    func headline(mood: BridgeReport.Mood, count: Int, openProposals: Int) -> String {
        let greet = style.greeting.isEmpty ? "" : style.greeting + " "
        let who = address()
        let warm = style.warmth >= 7 ? "\(who), " : style.warmth <= 2 ? "" : "\(who). "
        let lines: String
        switch mood {
        case .concerned: lines = "Something needs you. \(count) lines that matter below."
        case .watchful: lines = "\(openProposals) proposal\(openProposals == 1 ? "" : "s") waiting on your word."
        case .pleased: lines = "\(have()) money to report. \(count) lines that matter."
        case .steady: lines = count == 0 ? "Nothing to report but the hum." : "Quiet watch. \(count) lines that matter."
        }
        let formal = style.formality >= 8 ? "Report follows. " : ""
        return (greet + warm + formal + lines + flavour(mood)).trimmingCharacters(in: .whitespaces)
    }

    func nextAct(entries: [JournalEntry], hull: HullGlance?, openProposals: Int, mood: BridgeReport.Mood) -> String {
        if openProposals > 0 { return "Say yes or no to the open proposal\(openProposals == 1 ? "" : "s") — each lapses after four folds." }
        if let last = entries.last(where: { $0.event == "distress-hold" }) {
            return "Distress hold: \(last.string("why") ?? "the pilot is holding") — your call, \(address())."
        }
        if let trouble = entries.lastIndex(where: TemplatedVoice.isDanger) {
            let e = entries[trouble]
            let since = entries[(trouble + 1)...].last(where: { $0.tick != nil })?.tick
            var what = fact(for: e)
            if what.hasPrefix("·: ") { what.removeFirst(3) }
            if let t = since { return "Last trouble — \(what) — and the pilot has flown on since (t\(t))." }
            return "Last trouble — \(what) — nothing has moved since; your call, \(address())."
        }
        if let h = hull, let f = h.fuel, let cap = h.fuelCapacity, cap > 0, f * 5 < cap {
            return "Fuel \(f)/\(cap): the pilot will divert to a pump before the next leg."
        }
        if let h = hull, let to = h.enRouteTo { return "Under way for \(to); nothing to do until she berths." }
        if let h = hull, let d = h.docked { return "Berthed at \(d); the pilot reads the board each fold." }
        return "Nothing needs you. The pilot keeps watch."
    }

    func hullLine(_ h: HullGlance) -> String {
        var parts: [String] = []
        if let n = h.shipName { parts.append(n) }
        if let d = h.docked { parts.append("berthed at \(d)") } else if let to = h.enRouteTo { parts.append("under way for \(to)") } else { parts.append("under way") }
        parts.append("ℳ\(h.credits)")
        if let d = h.debt, d > 0 { parts.append("debt ℳ\(d)") }
        if let f = h.fuel { parts.append("fuel \(f)" + (h.fuelCapacity.map { "/\($0)" } ?? "")) }
        if let w = h.wearBps { parts.append("wear \(w)bps") }
        if h.leased { parts.append("leased hull") }
        return parts.joined(separator: " — ")
    }

    /// The quiet folds, told by REASON, once each: the captain wants "she is saving for the
    /// hold extension" once, not four hundred times (wildhorse, 2026-09-03). The fold count
    /// rides along per reason so nothing is lost; the same reason at a different amount is
    /// one reason.
    func chatterLine(entries: [JournalEntry], count: Int) -> String {
        var order: [String] = []
        var counts: [String: Int] = [:]
        var firstWording: [String: String] = [:]
        for e in entries where TemplatedVoice.severity(e) == 0 {
            let w = e.string("why") ?? e.event
            let shape = String(w.unicodeScalars.filter { !CharacterSet.decimalDigits.contains($0) })
            if counts[shape] == nil { order.append(shape); firstWording[shape] = w }
            counts[shape, default: 0] += 1
        }
        let reasons = order.prefix(4).map { "\(firstWording[$0]!) (\(counts[$0]!) fold\(counts[$0]! == 1 ? "" : "s"))" }
        return "Quiet between the lines: " + reasons.joined(separator: "; ")
    }

    // MARK: one fact per event — ids, amounts, ticks verbatim

    func t(_ e: JournalEntry) -> String { e.tick.map { "t\($0)" } ?? "·" }

    func fact(for e: JournalEntry) -> String {
        let s = { (k: String) in e.string(k) ?? "" }
        let i = { (k: String) in e.int(k).map(String.init) ?? "?" }
        switch e.event {
        case "acted":
            return "\(t(e)): \(s("decision")) — ℳ\(i("credits")), fuel \(i("fuel")), resolves t\(i("resolves"))"
        case "load-closed":
            return "\(t(e)): load \(s("load")) closed — \(s("why")); ℳ\(i("credits"))"
        case "adopted-held-contract":
            return "\(t(e)): adopted held contract \(s("load")) (\(s("status")))"
        case "traded":
            let verb = s("side") == "sell" ? "sold" : "bought"
            let why = s("why").isEmpty ? "" : " — \(s("why"))"
            return "\(t(e)): \(verb) \(i("units")) \(s("good")) — ℳ\(i("credits"))\(why)"
        case "fill":
            return "\(t(e)): filled \(i("units")) \(s("good")) at basis \(i("basis")), spent ℳ\(i("spent")) — ℳ\(i("credits"))"
        case "trade-outcome":
            return "\(t(e)): \(s("side")) \(i("units")) \(s("good")): \(s("outcome"))"
        case "position-opened":
            let clock = (e.int("sellable_at") ?? 0) > 0 ? "sellable from t\(i("sellable_at"))" : "sellable when the exchange says"
            return "\(t(e)): opened \(i("units")) \(s("good")) at ask \(i("ask")), bound for \(s("sell_target")); \(clock) — the exchange's minimum hold, so \(sheIs()) riding under freight till then"
        case "carry-to-market":
            return "\(t(e)): carrying \(s("good")) to \(s("to")), arrives t\(i("resolves"))"
        case "carry-blocked":
            return "\(t(e)): carry blocked — \(s("why"))"
        case "carry-refused":
            return "\(t(e)): carry \(s("good")) → \(s("to")) refused — \(s("why"))"
        case "retargeted", "book-corrected":
            return "\(t(e)): \(e.event.replacingOccurrences(of: "-", with: " ")) — \(s("why"))"
        case "outfitted":
            return "\(t(e)): fitted \(s("fitting")) for ℳ\(i("price")) at \(s("at_station")) (reserve ℳ\(i("reserve"))) — ℳ\(i("credits"))"
        case "outfit-idle":
            return "\(t(e)): no refit — \(s("why"))"
        case "refit-refused":
            return "\(t(e)): refit \(s("fitting")) refused — \(s("why"))"
        case "engaged-drive":
            return "\(t(e)): drive engaged for \(s("to")), arrives t\(i("resolves"))"
        case "unwedged-course":
            return "\(t(e)): course to \(s("to")) unwedged, arrives t\(i("resolves"))"
        case "engage-refused":
            return "\(t(e)): engage refused — \(s("why"))"
        case "advice":
            return "\(t(e)): \(have()) advised [\(s("surface"))]: \(s("would")) — \(s("why")); I did nothing"
        case "proposed":
            return "\(t(e)): proposed [\(s("surface"))] \(s("would")) — \(s("why")); waiting on you until t\(i("expires")) (\(s("id")))"
        case "proposal-lapsed":
            return "\(t(e)): proposal lapsed [\(s("surface"))] \(s("would")) (\(s("id")))"
        case "distress-hold":
            return "\(t(e)): DISTRESS HOLD — \(s("why"))"
        case "refused-at-the-door":
            return "\(t(e)): \(s("decision")) refused at the door — \(s("why"))"
        case "held-at-the-gate":
            let d = s("decision")
            return "\(t(e)): held at the gate\(d.isEmpty ? "" : " (\(d))") — \(s("why"))"
        case "exchange-unreachable":
            let n = e.int("repeats") ?? 1
            return "\(t(e)): exchange unreachable — \(s("why"))" + (n > 1 ? " (×\(n))" : "")
        case "freight":
            // The exchange's own ledger line, verbatim, with what it paid when it paid.
            var line = "\(t(e)): \(s("why"))"
            if let p = e.int("credits_paid"), p != 0 { line += " — ℳ\(p) paid" }
            if !s("load").isEmpty && !s("why").contains(s("load")) { line += " [\(s("load"))]" }
            return line
        case "watch-begins":
            return "\(t(e)): watch began on \(s("exchange")) with \(e["automations"]?.description ?? "[]")"
        case "awaiting-pending-actions":
            return "\(t(e)): waiting out pending \(e["verbs"]?.description ?? "[]") until t\(i("resolves"))"
        case "awaiting-our-own-fold":
            return "\(t(e)): waiting out our own fold until t\(i("resolves"))"
        default:
            // Neutral: the word and its payload, key-sorted — never guessed at.
            let payload = JSONValue.object(e.fields).description
            return "\(t(e)): \(e.event) \(payload == "{}" ? "" : payload)".trimmingCharacters(in: .whitespaces)
        }
    }
}
