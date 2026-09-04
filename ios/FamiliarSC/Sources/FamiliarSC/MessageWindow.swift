import Foundation

// The message window (dialogue §3.5): the journal's advice and proposal lines, joined with
// proposals.jsonl ∪ approvals.jsonl so each proposal shows its state — open, approved,
// denied, lapsed. The persona voices these; the app's approve/deny buttons write the
// approval line (the captain's act). This is the feed; it never decides anything.

public struct MessageItem: Equatable, Sendable {
    public enum Kind: Equatable, Sendable {
        /// `advice`: what the computer would do and why; it did nothing.
        case advice(would: String, why: String)
        /// `proposed` + its state.
        case proposal(id: String, would: String, why: String, expiresTick: Int64, state: ProposalState)
    }
    public enum ProposalState: Equatable, Sendable {
        case open
        case approved(at: Int64)
        case denied(at: Int64)
        case lapsed
    }

    public var at: Int64
    public var tick: Int64
    public var surface: ControlSurface?
    public var surfaceKey: String
    public var kind: Kind
    /// How many journal lines this item stands for (the pilot re-says standing advice every
    /// twenty ticks; the captain wants it once, with how long it has stood).
    public var repeats: Int = 1
    /// The tick the pilot first said it.
    public var sinceTick: Int64 = 0

    public var needsTheCaptain: Bool {
        if case .proposal(_, _, _, _, .open) = kind { return true }
        return false
    }
}

public enum MessageWindow {
    /// Build the feed. `nowTick` decides whether an unanswered proposal is still open; pass
    /// the wire's tick when reachable, else the journal's last tick.
    public static func build(journal: [JournalEntry], proposals: [Proposal], approvals: [Approval], nowTick: Int64?) -> [MessageItem] {
        // The last word per proposal id wins.
        var verdict: [String: Approval] = [:]
        for a in approvals { verdict[a.id] = a }
        let lapsedIDs = Set(journal.filter { $0.event == "proposal-lapsed" }.compactMap { $0.string("id") })
        let filed: [String: Proposal] = Dictionary(proposals.map { ($0.id, $0) }, uniquingKeysWith: { _, b in b })

        var items: [MessageItem] = []
        for e in journal {
            switch e.event {
            case "advice":
                let key = e.string("surface") ?? ""
                items.append(MessageItem(
                    at: e.at, tick: e.tick ?? 0, surface: ControlSurface.parse(key), surfaceKey: key,
                    kind: .advice(would: e.string("would") ?? "", why: e.string("why") ?? "")
                ))
            case "proposed":
                guard let id = e.string("id") else { continue }
                let key = e.string("surface") ?? filed[id]?.surface ?? ""
                let expires = e.int("expires") ?? filed[id]?.expiresTick ?? (e.tick ?? 0)
                let state: MessageItem.ProposalState
                if let v = verdict[id] {
                    state = v.approved ? .approved(at: v.at) : .denied(at: v.at)
                } else if lapsedIDs.contains(id) {
                    state = .lapsed
                } else if let now = nowTick, now > expires {
                    state = .lapsed
                } else {
                    state = .open
                }
                items.append(MessageItem(
                    at: e.at, tick: e.tick ?? 0, surface: ControlSurface.parse(key), surfaceKey: key,
                    kind: .proposal(id: id, would: e.string("would") ?? filed[id]?.describe ?? "",
                                    why: e.string("why") ?? filed[id]?.why ?? "", expiresTick: expires, state: state)
                ))
            default:
                continue
            }
        }
        return items
    }

    /// The feed as a captain reads it: identical advice (same surface, same act) folded to one
    /// item carrying the count and the tick it was first said; proposals untouched (each is
    /// its own act). Order is by latest occurrence, newest last, like the journal.
    public static func collapsed(_ items: [MessageItem]) -> [MessageItem] {
        var out: [MessageItem] = []
        var index: [String: Int] = [:]
        for item in items {
            guard case .advice(let would, _) = item.kind else { out.append(item); continue }
            let key = item.surfaceKey + "|" + would
            if let i = index[key] {
                var merged = item
                merged.repeats = out[i].repeats + 1
                merged.sinceTick = out[i].sinceTick
                out.remove(at: i)
                for (k, v) in index where v > i { index[k] = v - 1 }
                index[key] = out.count
                out.append(merged)
            } else {
                var first = item
                first.sinceTick = item.tick
                index[key] = out.count
                out.append(first)
            }
        }
        return out
    }

    /// The approval line the app appends to approvals.jsonl on the captain's tap — the
    /// exact shape autonomy.rs reads. Kept here so the writer and the reader share one truth.
    public static func approvalLine(id: String, approved: Bool, at: Int64) -> String {
        "{\"id\":\(JSONValue.string(id).description),\"approved\":\(approved ? "true" : "false"),\"at\":\(at)}"
    }
}
