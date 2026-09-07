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
    /// The request was STARTED but a faster rival won and this lap was cancelled
    /// before it could answer: reachability unknown, but the attempt is real. It
    /// must age the door toward expiry (a poisoned door cancelled every round
    /// would otherwise be knocked on forever) without being booked as a miss (a
    /// merely slower live door must not be demoted by its rival's speed).
    case attempted
}

/// Per-door memory the race plans against. Codable so the shell can persist it
/// beside the enrollment; absent history reads as healthy (a new door deserves
/// its doctrine rank).
public struct DoorHealth: Codable, Equatable, Sendable {
    /// Consecutive failures since the last success.
    public var consecutiveFails: Int
    /// Seconds-since-epoch of the last successful read, 0 = never seen answer.
    public var lastSuccess: TimeInterval
    /// Seconds-since-epoch of the last attempt of any outcome. Diagnostic only —
    /// every miss refreshes it, so it must never drive expiry (codex, T-231
    /// re-verification: "recently touched" is not "recently alive").
    public var lastAttempt: TimeInterval
    /// Seconds-since-epoch when the current run of silence began: the last
    /// answer if the door ever answered, else the FIRST attempt. Refreshed only
    /// by a success, never by a miss — this is the age expiry is measured by.
    /// 0 = never attempted.
    public var silentSince: TimeInterval

    public init(
        consecutiveFails: Int = 0,
        lastSuccess: TimeInterval = 0,
        lastAttempt: TimeInterval = 0,
        silentSince: TimeInterval = 0
    ) {
        self.consecutiveFails = consecutiveFails
        self.lastSuccess = lastSuccess
        self.lastAttempt = lastAttempt
        self.silentSince = silentSince
    }

    private enum CodingKeys: String, CodingKey {
        case consecutiveFails, lastSuccess, lastAttempt, silentSince
    }

    /// Rows persisted before `silentSince` existed decode with the best age the
    /// old fields can give: the last answer, else the last attempt (which for a
    /// never-answered door under-reports its age — the door then gets at most
    /// one more week of chances, never an eternity).
    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        consecutiveFails = try c.decodeIfPresent(Int.self, forKey: .consecutiveFails) ?? 0
        lastSuccess = try c.decodeIfPresent(TimeInterval.self, forKey: .lastSuccess) ?? 0
        lastAttempt = try c.decodeIfPresent(TimeInterval.self, forKey: .lastAttempt) ?? 0
        silentSince = try c.decodeIfPresent(TimeInterval.self, forKey: .silentSince)
            ?? (lastSuccess > 0 ? lastSuccess : lastAttempt)
    }

    /// When the current silence began, for rows built by the memberwise init
    /// without `silentSince` (tests, and the legacy shape above).
    var silenceStart: TimeInterval {
        if silentSince > 0 { return silentSince }
        return lastSuccess > 0 ? lastSuccess : lastAttempt
    }

    /// The newest evidence of any kind — what the remembered-door bound ranks by.
    var freshest: TimeInterval { max(lastSuccess, lastAttempt, silentSince) }
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
    /// The most non-lighthouse, non-current doors a shell remembers. Roaming
    /// learns a new LAN address per lease; without a bound both the candidate
    /// list and the health map would grow for the life of the enrollment.
    public static let maxRememberedDoors = 16
    /// A continuously healthy door's timestamps are checkpointed to the store
    /// no more often than this; transitions (a new door, a streak change up to
    /// demotion, a silence beginning or ending, a door forgotten) persist at once.
    public static let checkpointSeconds: TimeInterval = 3600

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
            !isExpired(host, health: health, lighthouse: lighthouse, now: now)
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
        // A silence begins at the first attempt and is ended only by an answer.
        if h.silentSince == 0 { h.silentSince = h.lastSuccess > 0 ? h.lastSuccess : now }
        switch outcome {
        case .success:
            h.consecutiveFails = 0
            h.lastSuccess = now
            h.silentSince = now
        case .failure:
            h.consecutiveFails += 1
        case .attempted:
            break
        }
        next[host] = h
        return next
    }

    /// Has this door been silent for the whole window? The lighthouse never
    /// expires; a door with no history, or one never attempted, is not expired.
    static func isExpired(
        _ host: String,
        health: [String: DoorHealth],
        lighthouse: String,
        now: TimeInterval
    ) -> Bool {
        if host == lighthouse { return false }
        guard let h = health[host] else { return false }
        let since = h.silenceStart
        guard since > 0 else { return false }
        return now - since >= expireAfterSeconds
    }

    /// Expiry as ONE transition over both stores (codex, T-231 re-verification):
    /// a door silent for the window leaves the remembered candidates AND the
    /// health map, so it is forgotten rather than filtered forever, and a later
    /// re-learning starts it clean. `keep` (the current enrollment door) and the
    /// lighthouse are never forgotten. Then the remembered set is bounded: past
    /// `maxRememberedDoors` the doors with the oldest evidence go first (a door
    /// with no row yet counts as the freshest — it has not had its chances).
    /// Health rows for doors no longer remembered are dropped. Pure.
    public static func forget(
        hosts: [String],
        health: [String: DoorHealth],
        lighthouse: String,
        keep: Set<String>,
        now: TimeInterval
    ) -> (hosts: [String], health: [String: DoorHealth], forgotten: [String]) {
        let pinned = { (host: String) in host == lighthouse || keep.contains(host) }
        var forgotten: [String] = []
        var kept = hosts.filter { host in
            if pinned(host) { return true }
            if isExpired(host, health: health, lighthouse: lighthouse, now: now) {
                forgotten.append(host)
                return false
            }
            return true
        }
        let bounded = kept.filter { !pinned($0) }
        if bounded.count > maxRememberedDoors {
            let byAge = bounded.sorted { a, b in
                (health[a]?.freshest ?? now) > (health[b]?.freshest ?? now)
            }
            let drop = Set(byAge.dropFirst(maxRememberedDoors))
            kept.removeAll { drop.contains($0) }
            forgotten.append(contentsOf: bounded.filter { drop.contains($0) })
        }
        let remembered = Set(kept)
        let prunedHealth = health.filter { remembered.contains($0.key) || $0.key == lighthouse }
        return (kept, prunedHealth, forgotten)
    }

    /// Whether the live map differs from what is on disk in a way worth a write.
    /// Transitions persist at once: a door appearing or vanishing, its streak
    /// changing anywhere up to the demotion threshold (past it the count is not
    /// policy), a silence beginning or ending. A healthy door's moving
    /// timestamps are only checkpointed once `checkpointSeconds` have passed
    /// since the persisted ones — so a 5 s poll loop against a live lighthouse
    /// writes the store about once an hour, not once per poll.
    public static func needsCheckpoint(
        persisted: [String: DoorHealth],
        live: [String: DoorHealth],
        checkpointSeconds: TimeInterval = checkpointSeconds
    ) -> Bool {
        if Set(persisted.keys) != Set(live.keys) { return true }
        for (host, l) in live {
            guard let p = persisted[host] else { return true }
            if min(p.consecutiveFails, demoteAfterFails) != min(l.consecutiveFails, demoteAfterFails) {
                return true
            }
            // A silence beginning is a new row (above); a silence ending after
            // misses is a streak change (above). Otherwise silentSince moves
            // with every answer and is a timestamp like the other two.
            if abs(l.silentSince - p.silentSince) >= checkpointSeconds { return true }
            if abs(l.lastAttempt - p.lastAttempt) >= checkpointSeconds { return true }
            if abs(l.lastSuccess - p.lastSuccess) >= checkpointSeconds { return true }
        }
        return false
    }
}
