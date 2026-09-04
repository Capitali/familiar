import Foundation
import FamiliarSC

// The host's briefs, rendered into the plain lines the computer reads and the grounding
// check counts (wildhorse e6b1f0a: GET /ships/{world}/fuel, GET /ships/{world}/brief).
// Pure functions over the JSON, so a test pins what "how do I refuel?" is answered from.

public enum Briefs {
    /// The frame: what the captain is looking at, in one line.
    public static func frame(fromBrief b: JSONValue, worldInstance: String?) -> String? {
        guard let c = b["context"] else { return nil }
        var parts: [String] = []
        if let k = c["kind"]?.string { parts.append(k) }
        if let h = c["hull"]?.string { parts.append("hull \(h)" + (worldInstance.map { " (\($0))" } ?? "")) }
        if let cap = c["captain"]?.string { parts.append("captain \(cap)") }
        if let comp = c["computer"]?.string { parts.append("computer \(comp)") }
        return parts.isEmpty ? nil : parts.joined(separator: ", ")
    }

    /// The fuel picture as she can say it.
    public static func fuel(_ f: JSONValue) -> String {
        var out: [String] = []
        let fuel = f["fuel"]?.int ?? 0, cap = f["capacity"]?.int ?? 0
        out.append("Fuel aboard: \(fuel) of \(cap)." + (f["docked"]?.string.map { " Berthed at \($0)." } ?? " Under way."))
        if let c = f["credits"]?.int { out.append("Credits in hand: ℳ\(c).") }
        if let p = f["fill_price_here"]?.int { out.append("A full fill at this berth costs ℳ\(p).") }
        if f["stranded"]?.bool == true { out.append("She is STRANDED: no pump is reachable on the fuel aboard.") }
        let pumps = f["pumps"]?.array ?? []
        for p in pumps {
            let st = p["station"]?.string ?? "?"
            let ticks = p["ticks"]?.int ?? 0, cost = p["fuel_cost"]?.int ?? 0
            var line = "Pump \(st): \(ticks) ticks away, burns \(cost) fuel to reach" + (p["burn"]?.string.map { $0 == "standard" ? "" : " at \($0) burn" } ?? "")
            if p["here"]?.bool == true { line = "Pump \(st): here" }
            if p["reachable"]?.bool == true { line += ", reachable" } else if let s = p["short_by"]?.int { line += ", NOT reachable — short by \(s) fuel" }
            if let fp = p["fill_price"]?.int { line += "; a fill there costs ℳ\(fp)" + (p["affordable"]?.bool == true ? " (affordable)" : " (not affordable)") }
            out.append(line + ".")
        }
        if let can = f["can_reach"]?.array, !can.isEmpty { out.append("Reachable pumps: " + can.compactMap(\.string).joined(separator: ", ") + ".") }
        for s in f["saleable_here"]?.array ?? [] {
            let good = s["good"]?.string ?? "?", units = s["units"]?.int ?? 0, take = s["will_take"]?.int ?? 0, worth = s["worth"]?.int ?? 0, bid = s["bid"]?.int ?? 0
            out.append(take > 0 ? "This berth would buy \(take) of the \(units) \(good) aboard at bid \(bid), worth ℳ\(worth)." : "This berth will not take the \(units) \(good) aboard (bid \(bid), takes 0).")
        }
        if let t = f["tanker"] {
            let avail = t["available"]?.bool == true ? "available" : "not available"
            let will = t["pilot_will_call"]?.bool == true ? "the pilot will call it" : "the pilot will NOT call it on her own"
            out.append("Tanker: \(avail); \(will)." + (t["why"]?.string.map { " Why: " + squash($0) } ?? ""))
        }
        if let s = f["if_stranded"]?.string { out.append("Ways out when stranded: " + squash(s) + ".") }
        return out.joined(separator: "\n")
    }

    /// The ship's brief: what is aboard, the dial in a sentence, open proposals, advice folded.
    public static func brief(_ b: JSONValue) -> String {
        var out: [String] = []
        if let a = b["aboard"], let units = a["units"]?.object, !units.isEmpty {
            let list = units.keys.sorted().map { "\(units[$0]?.int ?? 0) \($0)" }.joined(separator: ", ")
            out.append("Aboard: \(list)" + (a["cost"]?.int.map { " (cost ℳ\($0))" } ?? "") + ".")
        }
        if let d = b["dial"]?.object {
            let levels = Set(d.values.compactMap(\.string))
            if levels.count == 1, let l = levels.first { out.append("Autonomy dial: everything on \(l).") }
            else {
                let s = d.keys.sorted().map { "\($0)=\(d[$0]?.string ?? "?")" }.joined(separator: ", ")
                out.append("Autonomy dial: \(s).")
            }
        }
        let open = b["open_proposals"]?.array ?? []
        out.append(open.isEmpty ? "No proposal waiting on the captain." : "Proposals waiting on the captain: " + open.compactMap { $0["would"]?.string ?? $0["describe"]?.string }.joined(separator: "; ") + ".")
        for a in (b["standing_advice"] ?? b["advice"])?.array ?? [] {
            // Live shape: {event, what, surface?, since_tick, times}; older: {would, why}.
            let what = a["what"]?.string ?? a["would"]?.string ?? ""
            let why = a["why"]?.string ?? ""
            let ev = a["event"]?.string.map { $0.replacingOccurrences(of: "-", with: " ") } ?? "advice"
            let since = a["since_tick"]?.int, times = a["times"]?.int ?? 1
            out.append("Standing (\(ev)): \(what)" + (why.isEmpty ? "" : " — \(why)") + (since.map { " (since t\($0), said \(times) times)" } ?? "") + ".")
        }
        for r in (b["recent"]?.array ?? []).prefix(8) {
            guard let ev = r["event"]?.string else { continue }
            let t = r["tick"]?.int.map { "t\($0)" } ?? "·"
            let why = r["why"]?.string ?? r["decision"]?.string ?? ""
            out.append("Recent \(t): \(ev)" + (why.isEmpty ? "" : " — \(why)") + ".")
        }
        return out.joined(separator: "\n")
    }

    /// The captain's slug as the host keys it: lowercased, spaces to hyphens, parentheses dropped.
    public static func captainSlug(_ name: String) -> String {
        let lowered = name.lowercased()
        var out = ""
        for ch in lowered {
            if ch.isLetter || ch.isNumber { out.append(ch) }
            else if ch == " " || ch == "-" || ch == "_" { if !out.hasSuffix("-") { out.append("-") } }
        }
        while out.hasSuffix("-") { out.removeLast() }
        return out
    }

    /// The captain's brief: his computer, his hulls, the pooled book, what waits on him.
    public static func captain(_ b: JSONValue) -> String {
        var out: [String] = []
        let name = b["captain"]?.string ?? b["context"]?["name"]?.string ?? "the captain"
        let computer = b["computer"]?.string ?? b["context"]?["computer"]?.string ?? "her"
        out.append("Captain \(name); his computer across the fleet is \(computer).")
        let ships = b["ships"]?.array ?? []
        for s in ships {
            let hull = s["ship"]?.string ?? s["hull"]?.string ?? s["label"]?.string ?? "?"
            let world = s["world_name"]?.string ?? ""
            var line = "Hull \(hull)" + (world.isEmpty ? "" : " (\(world))")
            if let d = s["docked"]?.string { line += ": berthed at \(d)" } else if let to = s["enRouteTo"]?.string { line += ": under way for \(to)" } else { line += ": under way" }
            if let c = s["credits"]?.int { line += ", ℳ\(c)" }
            if let f = s["fuel"]?.int { line += ", fuel \(f)" + (s["fuelCapacity"]?.int.map { "/\($0)" } ?? "") }
            if let e = s["last_event"]?.string { line += ", last: \(e.replacingOccurrences(of: "-", with: " "))" }
            out.append(line + ".")
        }
        if let k = b["book"] {
            var parts: [String] = []
            if let c = k["pooled_credits"]?.int { parts.append("ℳ\(c) pooled") }
            if let d = k["debt"]?.int { parts.append("ℳ\(d) debt") }
            if let r = k["trades_realized"]?.int { parts.append("ℳ\(r) realized on trades") }
            if let a = k["aboard_at_cost"]?.int { parts.append("ℳ\(a) aboard at cost") }
            if !parts.isEmpty { out.append("The fleet's book: " + parts.joined(separator: ", ") + ".") }
        }
        // Live: a COUNT; older shape: the proposals themselves.
        if let n = b["open_proposals"]?.int {
            out.append(n == 0 ? "No proposal waits on the captain anywhere in the fleet." : "\(n) proposal\(n == 1 ? "" : "s") wait on the captain across the fleet.")
        } else {
            let open = b["open_proposals"]?.array ?? []
            out.append(open.isEmpty ? "No proposal waits on the captain anywhere in the fleet." : "Waiting on the captain: " + open.compactMap { $0["would"]?.string ?? $0["describe"]?.string }.joined(separator: "; ") + ".")
        }
        return out.joined(separator: "\n")
    }

    static func squash(_ s: String) -> String {
        s.split(whereSeparator: \.isWhitespace).joined(separator: " ")
    }
}
