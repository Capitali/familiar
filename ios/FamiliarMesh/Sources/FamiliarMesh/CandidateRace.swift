// CandidateRace — the launch read stops paying a timeout per dead door (T-231).
//
// The complaint, live on Ian's iPad (2026-08-31): the hub's LAN lease moved on a
// router reboot, the remembered door went stale, and every cold launch bled a full
// connect timeout against it before the lighthouse rescued the read. Roaming will
// recreate that situation forever, so the walk has to be cheap in the face of it.
//
// The shape: the doctrine's preference order (nearest peer, lighthouse, tailnet
// last — ADR-0012/ADR-0017) becomes a HEAD START, not a serial wall. Every
// candidate gets a start delay proportional to its rank; the first success wins
// and the losers are cancelled. A healthy preferred door still wins every race —
// its head start beats any rival's round trip — while a dead one costs the next
// runner only the stagger, milliseconds instead of a timeout.
//
// Around the race sits per-door HEALTH, remembered across launches: a door that
// keeps failing is demoted to the back of its own tier (preference is latency,
// never authority — losing streaks are latency evidence), and a door that has
// answered nothing for days is dropped from the walk entirely. The lighthouse is
// exempt from expiry: it is the one address the doctrine says must always be
// worth a knock.

import Foundation

/// What one launch-read attempt learned about one door.
public enum DoorOutcome: Sendable {
    case success
    case failure
}

/// Per-door memory the race plans against. Codable so the shell can persist it
/// beside the enrollment; absent history reads as healthy (a new door deserves
/// its doctrine rank).
public struct DoorHealth: Codable, Equatable, Sendable {
    /// Consecutive failures since the last success.
    public var consecutiveFails: Int
    /// Seconds-since-epoch of the last successful read, 0 = never seen answer.
    public var lastSuccess: TimeInterval
    /// Seconds-since-epoch of the last attempt of any outcome.
    public var lastAttempt: TimeInterval

    public init(consecutiveFails: Int = 0, lastSuccess: TimeInterval = 0, lastAttempt: TimeInterval = 0) {
        self.consecutiveFails = consecutiveFails
        self.lastSuccess = lastSuccess
        self.lastAttempt = lastAttempt
    }
}

/// One starter in the race: the door and how long to hold it at the line.
public struct RaceStarter: Equatable, Sendable {
    public let host: String
    /// Milliseconds to wait before this candidate starts its read.
    public let delayMs: Int

    public init(host: String, delayMs: Int) {
        self.host = host
        self.delayMs = delayMs
    }
}

public enum CandidateRace {
    /// A door loses its within-tier place after this many straight misses.
    public static let demoteAfterFails = 3
    /// A door that has not answered for this long leaves the walk (except the
    /// lighthouse, and except a door that has NEVER answered but is younger
    /// than this — a fresh enrollment's doors deserve their first chances).
    public static let expireAfterSeconds: TimeInterval = 7 * 24 * 3600
    /// The head start between successive starters. Long enough that a live
    /// earlier door wins its race on any sane network, short enough that three
    /// dead doors cost under a second, not a minute.
    public static let staggerMs = 350

    /// Plan the race. `ordered` is the doctrine's preference order (the caller
    /// keeps using its existing `readOrderedCandidates`); this function only
    /// demotes the limping within it, expires the long-dead, and assigns the
    /// stagger. `lighthouse` never expires. Deduplication and validity are the
    /// caller's (existing) responsibility.
    public static func plan(
        ordered: [String],
        health: [String: DoorHealth],
        lighthouse: String,
        now: TimeInterval
    ) -> [RaceStarter] {
        let alive = ordered.filter { host in
            if host == lighthouse { return true }
            guard let h = health[host] else { return true }
            if h.lastSuccess > 0 {
                return now - h.lastSuccess < expireAfterSeconds
            }
            // Never answered: expire only once we have been trying that long.
            if h.lastAttempt > 0 {
                return now - h.lastAttempt < expireAfterSeconds
            }
            return true
        }
        // Stable partition: the healthy keep the doctrine's order, the limping
        // follow in theirs — demotion, not banishment. A single sort would do,
        // but the two-pass keeps "stable within each group" impossible to break.
        let healthy = alive.filter { (health[$0]?.consecutiveFails ?? 0) < demoteAfterFails }
        let limping = alive.filter { (health[$0]?.consecutiveFails ?? 0) >= demoteAfterFails }
        return (healthy + limping).enumerated().map { i, host in
            RaceStarter(host: host, delayMs: i * staggerMs)
        }
    }

    /// Record one attempt's outcome. A success clears the streak and revives a
    /// demoted door on the spot; a failure lengthens the streak. Pure — the
    /// caller owns persistence.
    public static func settle(
        _ health: [String: DoorHealth],
        host: String,
        outcome: DoorOutcome,
        now: TimeInterval
    ) -> [String: DoorHealth] {
        var next = health
        var h = next[host] ?? DoorHealth()
        h.lastAttempt = now
        switch outcome {
        case .success:
            h.consecutiveFails = 0
            h.lastSuccess = now
        case .failure:
            h.consecutiveFails += 1
        }
        next[host] = h
        return next
    }
}
