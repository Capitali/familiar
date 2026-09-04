import Foundation

// Captain notices: which journal lines become a notification on the captain's device. A
// policy over the vocabulary, pure — the app schedules the UNNotification, the policy only
// says what deserves one. Nothing in the pilot's chatter (holding, idle, awaiting) does.

public struct CaptainNotice: Equatable {
    public enum Kind: String, Equatable {
        case needsTheCaptain      // a proposal is open, or lapsed unanswered
        case advice               // the computer would have acted and held
        case money                // a fill, a paid delivery, a position opened, a refit bought
        case hull                 // a refit bought (a leg engaged is the pilot's routine, not news)
        case distress             // distress-hold, a refusal at the door, the exchange gone
        case lease                // the lease is near lapse
    }
    public var kind: Kind
    public var title: String
    public var body: String
    public var tick: Int64?
    public var at: Int64
}

public enum NoticePolicy {
    /// Notices for a run of journal entries, in journal order. `exchange-unreachable` is
    /// collapsed to the first of a run — 108 lines of "connection refused" is one fact.
    public static func notices(for entries: [JournalEntry]) -> [CaptainNotice] {
        var out: [CaptainNotice] = []
        var unreachableRun = false
        for e in entries {
            if e.event != "exchange-unreachable" { unreachableRun = false }
            func add(_ kind: CaptainNotice.Kind, _ title: String, _ body: String) {
                out.append(CaptainNotice(kind: kind, title: title, body: body, tick: e.tick, at: e.at))
            }
            let t = e.tick.map { "t\($0)" } ?? ""
            switch e.event {
            case "proposed":
                add(.needsTheCaptain, "Your word, please", "\(e.string("would") ?? "an act") — \(e.string("why") ?? "") (until t\(e.int("expires") ?? 0))")
            case "proposal-lapsed":
                add(.needsTheCaptain, "A proposal lapsed", "\(e.string("would") ?? "an act") waited for a yes that never came")
            case "advice":
                add(.advice, "She would have…", "\(e.string("would") ?? "") — \(e.string("why") ?? "")")
            case "traded":
                let side = e.string("side") == "sell" ? "Sold" : "Bought"
                add(.money, "\(side) \(e.int("units") ?? 0) \(e.string("good") ?? "")", "ℳ\(e.int("credits") ?? 0) in hand, \(t)")
            case "position-opened":
                add(.money, "Position opened", "\(e.int("units") ?? 0) \(e.string("good") ?? "") at ask \(e.int("ask") ?? 0), bound for \(e.string("sell_target") ?? "?"); sellable from t\(e.int("sellable_at") ?? 0)")
            case "load-closed":
                add(.money, "Load \(e.string("load") ?? "") closed", "\(e.string("why") ?? "") — ℳ\(e.int("credits") ?? 0)")
            case "outfitted":
                add(.hull, "Fitted \(e.string("fitting") ?? "")", "ℳ\(e.int("price") ?? 0) at \(e.string("at_station") ?? "the yard"), \(t)")
            case "distress-hold":
                add(.distress, "Distress hold", e.string("why") ?? "holding for the captain")
            case "refused-at-the-door":
                add(.distress, "Refused at the door", "\(e.string("decision") ?? "") — \(e.string("why") ?? "")")
            case "carry-blocked":
                add(.distress, "Carry blocked", e.string("why") ?? "")
            case "carry-refused":
                add(.distress, "Carry refused", "\(e.string("good") ?? "") → \(e.string("to") ?? "?") — \(e.string("why") ?? "")")
            case "exchange-unreachable":
                if !unreachableRun { add(.distress, "Exchange unreachable", e.string("why") ?? "") }
                unreachableRun = true
            default:
                continue
            }
        }
        return out
    }

    /// The lease notice, from `fleet status`' hours-left. Inside two hours the supervisor
    /// renews only when a human passed `--renew` — so the captain must hear early.
    public static func leaseNotice(hoursLeft: Int64, at: Int64) -> CaptainNotice? {
        if hoursLeft < 0 { return CaptainNotice(kind: .lease, title: "Lease expired", body: "the pilot cannot fly her until the lease is renewed", tick: nil, at: at) }
        if hoursLeft <= 4 { return CaptainNotice(kind: .lease, title: "Lease lapses in \(hoursLeft)h", body: "renew with `familiar fleet run --renew`", tick: nil, at: at) }
        return nil
    }
}
