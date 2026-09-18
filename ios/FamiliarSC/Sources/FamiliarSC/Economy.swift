import Foundation

// T-241 — economic history and profit, by captain (Ian, 2026-09-09: "Familiar UCF views
// should include economic history/trend lines and analysis of profit in summary form for
// overview by captain"). The HOST computes it (`crates/cli/src/economy.rs`: every `credits`
// reading in a hull's journal is a point, every delta is exact money booked to the last act
// that could have moved it — or, where the exchange's own cash ledger answers, the fold's
// word on each credit — pooled per captain across hulls, with a least-squares trend and the
// host's own sentences). This file is the typed READ of that: `GET /captains/{id}/economy`
// and the `economy` summary the captain brief carries. Nothing here computes money; a
// number on the screen is the host's number, and the trend is a straight line through what
// happened — the screen says so.

/// One reading of the purse.
public struct EconomyPoint: Equatable, Sendable {
    public var at: Int64
    public var tick: Int64
    public var credits: Int64
    public var date: Date { Date(timeIntervalSince1970: TimeInterval(at)) }
    public init(at: Int64, tick: Int64, credits: Int64) { self.at = at; self.tick = tick; self.credits = credits }
    init?(json v: JSONValue) {
        guard let at = v["at"]?.int, let tick = v["tick"]?.int, let credits = v["credits"]?.int else { return nil }
        self.init(at: at, tick: tick, credits: credits)
    }
}

/// Where the money went over the window. Signed as the host signs it: in is positive.
public struct EconomyFlows: Equatable, Sendable {
    public var tradeBought: Int64 = 0
    public var tradeSold: Int64 = 0
    public var freight: Int64 = 0
    public var fuel: Int64 = 0
    public var repair: Int64 = 0
    public var outfit: Int64 = 0
    public var debtPaid: Int64 = 0
    public var dock: Int64 = 0
    public var other: Int64 = 0
    public var fills: Int64 = 0
    public var settles: Int64 = 0
    public init() {}
    init(json v: JSONValue) {
        tradeBought = v["trade_bought"]?.int ?? 0; tradeSold = v["trade_sold"]?.int ?? 0
        freight = v["freight"]?.int ?? 0; fuel = v["fuel"]?.int ?? 0; repair = v["repair"]?.int ?? 0
        outfit = v["outfit"]?.int ?? 0; debtPaid = v["debt_paid"]?.int ?? 0; dock = v["dock"]?.int ?? 0
        other = v["other"]?.int ?? 0; fills = v["fills"]?.int ?? 0; settles = v["settles"]?.int ?? 0
    }
    public var tradeNet: Int64 { tradeSold + tradeBought }

    /// One bar per cause that moved money, signed, in the host's order of telling: what
    /// came in first, then what went out. Zero buckets are not bars.
    public struct Bar: Equatable, Sendable, Identifiable {
        public var id: String { cause }
        public var cause: String
        public var amount: Int64
    }
    public var bars: [Bar] {
        let all: [(String, Int64)] = [
            ("freight", freight), ("trade sold", tradeSold), ("trade bought", tradeBought),
            ("fuel", fuel), ("repair", repair), ("outfit", outfit), ("debt paid", debtPaid),
            ("dock", dock), ("other", other),
        ]
        let nonzero = all.filter { $0.1 != 0 }.map { Bar(cause: $0.0, amount: $0.1) }
        return nonzero.filter { $0.amount > 0 } + nonzero.filter { $0.amount < 0 }
    }
    public var earned: Int64 { bars.filter { $0.amount > 0 }.map(\.amount).reduce(0, +) }
    public var spent: Int64 { bars.filter { $0.amount < 0 }.map(\.amount).reduce(0, +) }
}

/// The biggest single move in the window and what the host booked it to.
public struct EconomyMove: Equatable, Sendable {
    public var at: Int64
    public var tick: Int64
    public var delta: Int64
    public var cause: String
    init?(json v: JSONValue?) {
        guard let v, let at = v["at"]?.int, let tick = v["tick"]?.int, let delta = v["delta"]?.int else { return nil }
        self.at = at; self.tick = tick; self.delta = delta; self.cause = v["cause"]?.string ?? ""
    }
}

public struct EconomySummary: Equatable, Sendable {
    public var windowDays: Double = 0
    public var readings: Int = 0
    public var creditsStart: Int64 = 0
    public var creditsNow: Int64 = 0
    public var delta: Int64 = 0
    /// ℳ per day: the slope of a straight line through every reading in the window.
    public var trendPerDay: Double = 0
    public var best: EconomyMove?
    public var worst: EconomyMove?
    public init() {}
    init(json v: JSONValue) {
        windowDays = v["window_days"]?.double ?? 0; readings = Int(v["readings"]?.int ?? 0)
        creditsStart = v["credits_start"]?.int ?? 0; creditsNow = v["credits_now"]?.int ?? 0
        delta = v["delta"]?.int ?? 0; trendPerDay = v["trend_per_day"]?.double ?? 0
        best = EconomyMove(json: v["best"]); worst = EconomyMove(json: v["worst"])
    }
}

/// One history — a hull's, or the captain's pooled one. `analysis` is the host's sentences,
/// verbatim; `source` is where the flows came from (`exchange` = the fold's own ledger,
/// `journal` = attributed from the ship's journal, `mixed` = a pool of both).
public struct EconomyHistory: Equatable, Sendable, Identifiable {
    public var id: String { world ?? "pooled" }
    public var world: String?
    public var label: String?
    public var points: [EconomyPoint]
    public var flows: EconomyFlows
    public var summary: EconomySummary
    public var analysis: [String]
    public var source: String
    public init(world: String? = nil, label: String? = nil, points: [EconomyPoint] = [], flows: EconomyFlows = EconomyFlows(), summary: EconomySummary = EconomySummary(), analysis: [String] = [], source: String = "") {
        self.world = world; self.label = label; self.points = points; self.flows = flows; self.summary = summary; self.analysis = analysis; self.source = source
    }
    /// The host's shape (`economy::to_json`). Nil when it is not one — a `summary` is the
    /// least a history has; points are optional (the brief's summary carries none).
    public init?(json v: JSONValue) {
        guard let s = v["summary"] else { return nil }
        self.init(
            world: v["world"]?.string, label: v["label"]?.string,
            points: (v["points"]?.array ?? []).compactMap(EconomyPoint.init(json:)),
            flows: EconomyFlows(json: v["flows"] ?? .object([:])),
            summary: EconomySummary(json: s),
            analysis: (v["analysis"]?.array ?? []).compactMap(\.string),
            source: v["flows_source"]?.string ?? ""
        )
    }
    /// What the flows are the word of, for the screen.
    public var sourceWords: String {
        switch source {
        case "exchange": return "flows from the exchange's own ledger"
        case "journal": return "flows attributed from the ship's journal"
        case "mixed": return "flows from the exchange's ledger where it answered, the journal elsewhere"
        default: return ""
        }
    }
}

/// `GET /captains/{id}/economy?window=…` — every hull the captain flies, and the pool.
public struct CaptainEconomy: Equatable, Sendable {
    public static let windows = ["24h", "7d", "30d"]
    public var captain: String
    public var captainID: String
    public var since: Int64
    public var now: Int64
    public var hulls: [EconomyHistory]
    public var pooled: EconomyHistory
    public init?(json v: JSONValue) {
        guard let pooled = v["pooled"].flatMap(EconomyHistory.init(json:)) else { return nil }
        captain = v["captain"]?.string ?? ""; captainID = v["captain_id"]?.string ?? ""
        since = v["since"]?.int ?? 0; now = v["now"]?.int ?? 0
        hulls = (v["hulls"]?.array ?? []).compactMap(EconomyHistory.init(json:))
        self.pooled = pooled
    }
}
