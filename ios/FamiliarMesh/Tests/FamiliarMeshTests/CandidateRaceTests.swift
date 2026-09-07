import XCTest
@testable import FamiliarMesh

/// T-231: the launch read races its doors — the doctrine's preference is a head
/// start, dead doors are demoted or expired, and the lighthouse is always worth a
/// knock. Every rule here exists because a stale remembered LAN door cost Ian's
/// iPad a full connect timeout per cold launch (2026-08-31, the .10→.130 lease).
final class CandidateRaceTests: XCTestCase {

    private let lighthouse = "lighthouse.example"
    private let now: TimeInterval = 1_788_300_000

    func testHealthyDoorsKeepTheDoctrinesOrderWithAStagger() {
        // No history: the plan IS the preference order, staggered — the head
        // start, not a serial wall.
        let plan = CandidateRace.plan(
            ordered: ["lan", lighthouse, "100.100.1.2"],
            health: [:],
            lighthouse: lighthouse,
            now: now
        )
        XCTAssertEqual(plan.map(\.host), ["lan", lighthouse, "100.100.1.2"])
        XCTAssertEqual(plan.map(\.delayMs), [0, 350, 700])
    }

    func testALimpingDoorIsDemotedBehindTheHealthyNotBanished() {
        // Three straight misses: the stale door starts LAST — it still runs
        // (roaming may have brought it back), it just never blocks a launch.
        let health = [
            "stale-lan": DoorHealth(consecutiveFails: 3, lastSuccess: now - 3600, lastAttempt: now - 10)
        ]
        let plan = CandidateRace.plan(
            ordered: ["stale-lan", lighthouse, "100.100.1.2"],
            health: health,
            lighthouse: lighthouse,
            now: now
        )
        XCTAssertEqual(plan.map(\.host), [lighthouse, "100.100.1.2", "stale-lan"])
        XCTAssertEqual(plan.first?.delayMs, 0, "the healthy front-runner starts immediately")
    }

    func testTwoMissesAreAHiccupNotADemotion() {
        let health = ["lan": DoorHealth(consecutiveFails: 2, lastSuccess: now - 60, lastAttempt: now - 5)]
        let plan = CandidateRace.plan(
            ordered: ["lan", lighthouse],
            health: health,
            lighthouse: lighthouse,
            now: now
        )
        XCTAssertEqual(plan.map(\.host), ["lan", lighthouse])
    }

    func testADoorDeadForAWeekLeavesTheWalk() {
        let health = [
            "old-door": DoorHealth(
                consecutiveFails: 40,
                lastSuccess: now - CandidateRace.expireAfterSeconds - 1,
                lastAttempt: now - 30
            )
        ]
        let plan = CandidateRace.plan(
            ordered: ["old-door", lighthouse],
            health: health,
            lighthouse: lighthouse,
            now: now
        )
        XCTAssertEqual(plan.map(\.host), [lighthouse])
    }

    func testTheLighthouseNeverExpires() {
        // Even a lighthouse that has failed for a month stays in the race —
        // it is the doctrine's always-worth-a-knock address.
        let health = [
            lighthouse: DoorHealth(
                consecutiveFails: 500,
                lastSuccess: now - 30 * 24 * 3600,
                lastAttempt: now - 30
            )
        ]
        let plan = CandidateRace.plan(
            ordered: [lighthouse],
            health: health,
            lighthouse: lighthouse,
            now: now
        )
        XCTAssertEqual(plan.map(\.host), [lighthouse])
    }

    func testANeverAnsweredDoorGetsItsChancesUntilTheClockRunsOut() {
        // Fresh enrollment: a door with no success yet is not expired by a zero
        // lastSuccess — only by a week of actual attempts.
        let fresh = ["new-door": DoorHealth(consecutiveFails: 1, lastSuccess: 0, lastAttempt: now - 60)]
        XCTAssertEqual(
            CandidateRace.plan(ordered: ["new-door"], health: fresh, lighthouse: lighthouse, now: now)
                .map(\.host),
            ["new-door"]
        )
        let stale = [
            "new-door": DoorHealth(
                consecutiveFails: 900,
                lastSuccess: 0,
                lastAttempt: now - CandidateRace.expireAfterSeconds - 1
            )
        ]
        // The last attempt itself is older than the window — nothing has even
        // tried for a week; the door is forgotten.
        XCTAssertTrue(
            CandidateRace.plan(ordered: ["new-door"], health: stale, lighthouse: lighthouse, now: now)
                .isEmpty
        )
    }

    func testASuccessRevivesADemotedDoorOnTheSpot() {
        var health = ["lan": DoorHealth(consecutiveFails: 7, lastSuccess: now - 9999, lastAttempt: now - 5)]
        health = CandidateRace.settle(health, host: "lan", outcome: .success, now: now)
        XCTAssertEqual(health["lan"]?.consecutiveFails, 0)
        XCTAssertEqual(health["lan"]?.lastSuccess, now)
        let plan = CandidateRace.plan(
            ordered: ["lan", lighthouse],
            health: health,
            lighthouse: lighthouse,
            now: now
        )
        XCTAssertEqual(plan.first?.host, "lan", "one answer restores the doctrine rank")
    }

    func testFailuresAccumulateAndAnUnknownDoorStartsClean() {
        var health: [String: DoorHealth] = [:]
        for _ in 0..<3 {
            health = CandidateRace.settle(health, host: "lan", outcome: .failure, now: now)
        }
        XCTAssertEqual(health["lan"]?.consecutiveFails, 3)
        XCTAssertEqual(health["lan"]?.lastSuccess, 0)
        XCTAssertNil(health["other"], "settle touches only the attempted door")
    }

    // MARK: - codex re-verification 2026-09-07 (REJECT → repairs)

    private let poll: TimeInterval = 5
    private let week = CandidateRace.expireAfterSeconds

    /// Finding 1: a door that has NEVER answered, failing on every 5 s poll,
    /// must still expire after a week — the age is the first attempt, not the
    /// last miss (a corpse "recently touched" is not recently alive).
    func testADoorThatNeverAnswersExpiresAfterAWeekOfMisses() {
        var health: [String: DoorHealth] = [:]
        var t = now
        while t < now + week - poll {
            health = CandidateRace.settle(health, host: "dead-lan", outcome: .failure, now: t)
            t += poll
        }
        XCTAssertEqual(
            CandidateRace.plan(ordered: ["dead-lan", lighthouse], health: health, lighthouse: lighthouse, now: t)
                .map(\.host),
            [lighthouse, "dead-lan"], "still inside the window: demoted, not gone")
        health = CandidateRace.settle(health, host: "dead-lan", outcome: .failure, now: t)
        t += poll
        XCTAssertEqual(
            CandidateRace.plan(ordered: ["dead-lan", lighthouse], health: health, lighthouse: lighthouse, now: t)
                .map(\.host),
            [lighthouse], "a week of misses since the FIRST attempt: the door leaves the walk")
        XCTAssertGreaterThan(health["dead-lan"]!.lastAttempt, now + week - 2 * poll,
                             "lastAttempt was refreshed by every miss and did not save it")
    }

    /// Finding 1, the live shape: the dead LAN door's request outlives the
    /// stagger, the lighthouse wins, and the LAN lap is CANCELLED every round.
    /// An attempt cut short must still age the door toward expiry.
    func testADoorCancelledByAFasterWinnerEveryRoundStillExpires() {
        var health: [String: DoorHealth] = [:]
        var t = now
        while t <= now + week {
            health = CandidateRace.settle(health, host: "dead-lan", outcome: .attempted, now: t)
            health = CandidateRace.settle(health, host: lighthouse, outcome: .success, now: t)
            t += poll
        }
        XCTAssertEqual(health["dead-lan"]?.consecutiveFails, 0,
                       "a cancelled lap is never a miss — a slower live door is not demoted")
        XCTAssertEqual(
            CandidateRace.plan(ordered: ["dead-lan", lighthouse], health: health, lighthouse: lighthouse, now: t)
                .map(\.host),
            [lighthouse])
    }

    /// Distinct from the never-answered case: a door that answered once and then
    /// went quiet expires a week after its LAST ANSWER, and answering again resets
    /// that clock (wildhorse's ask: the two silences are different facts).
    func testAnAnsweredDoorExpiresAWeekAfterItsLastAnswerAndAnAnswerResetsTheClock() {
        var health = CandidateRace.settle([:], host: "lan", outcome: .success, now: now)
        var t = now + poll
        while t < now + week - poll {
            health = CandidateRace.settle(health, host: "lan", outcome: .failure, now: t)
            t += poll
        }
        XCTAssertFalse(
            CandidateRace.plan(ordered: ["lan"], health: health, lighthouse: lighthouse, now: t).isEmpty)
        health = CandidateRace.settle(health, host: "lan", outcome: .success, now: t)
        XCTAssertEqual(health["lan"]?.silentSince, t, "an answer restarts the silence")
        t += week - poll
        XCTAssertFalse(
            CandidateRace.plan(ordered: ["lan"], health: health, lighthouse: lighthouse, now: t).isEmpty,
            "measured from the last answer, not the first attempt")
        t += 2 * poll
        XCTAssertTrue(
            CandidateRace.plan(ordered: ["lan"], health: health, lighthouse: lighthouse, now: t).isEmpty)
    }

    /// Finding 2: expiry is one transition over BOTH stores — the expired door
    /// leaves the candidates and the health map; the lighthouse and the current
    /// door never leave, expired or not.
    func testForgetRemovesAnExpiredDoorFromBothStoresButNeverTheLighthouseOrTheCurrentDoor() {
        let stale = DoorHealth(consecutiveFails: 9, lastSuccess: now - week - 1, lastAttempt: now - 5,
                               silentSince: now - week - 1)
        let health = ["old-lan": stale, "current": stale, lighthouse: stale,
                      "fresh": DoorHealth(consecutiveFails: 0, lastSuccess: now - 60, lastAttempt: now - 60,
                                          silentSince: now - 60)]
        let out = CandidateRace.forget(
            hosts: ["current", "old-lan", lighthouse, "fresh"], health: health,
            lighthouse: lighthouse, keep: ["current"], now: now)
        XCTAssertEqual(out.hosts, ["current", lighthouse, "fresh"])
        XCTAssertEqual(out.forgotten, ["old-lan"])
        XCTAssertNil(out.health["old-lan"], "forgotten, not filtered forever")
        XCTAssertNotNil(out.health["current"])
        XCTAssertNotNil(out.health[lighthouse])
        XCTAssertNotNil(out.health["fresh"])
    }

    /// Finding 2: the remembered set is finite. Far more changing valid addresses
    /// than the bound: the stored host and health counts stay bounded and the
    /// oldest evidence goes first; a door with no row yet is the freshest.
    func testRememberedDoorsStayBoundedAsLeasesChangeForYears() {
        var hosts: [String] = ["current", lighthouse]
        var health: [String: DoorHealth] = [:]
        var t = now
        for i in 0..<200 {
            let door = "192.168.1.\(i)"
            hosts.append(door)
            // Each learned address answered once, then the lease moved on.
            health = CandidateRace.settle(health, host: door, outcome: .success, now: t)
            let out = CandidateRace.forget(hosts: hosts, health: health, lighthouse: lighthouse,
                                           keep: ["current"], now: t)
            hosts = out.hosts; health = out.health
            XCTAssertLessThanOrEqual(hosts.count, CandidateRace.maxRememberedDoors + 2)
            XCTAssertLessThanOrEqual(health.count, CandidateRace.maxRememberedDoors + 2)
            t += 3600
        }
        XCTAssertTrue(hosts.contains("current") && hosts.contains(lighthouse))
        XCTAssertTrue(hosts.contains("192.168.1.199"), "the newest evidence is kept")
        XCTAssertFalse(hosts.contains("192.168.1.0"), "the oldest evidence went first")
        // A rowless door (just learned) outranks every remembered one.
        let out = CandidateRace.forget(hosts: hosts + ["just-learned"], health: health,
                                       lighthouse: lighthouse, keep: ["current"], now: t)
        XCTAssertTrue(out.hosts.contains("just-learned"))
    }

    /// Finding 2: an expired address learned again later returns with fresh health.
    func testAForgottenDoorLearnedAgainStartsClean() {
        let stale = ["lan": DoorHealth(consecutiveFails: 40, lastSuccess: 0, lastAttempt: now - 5,
                                       silentSince: now - week - 1)]
        let out = CandidateRace.forget(hosts: ["lan", lighthouse], health: stale, lighthouse: lighthouse,
                                       keep: [], now: now)
        XCTAssertEqual(out.hosts, [lighthouse])
        // Re-learned: no row, so it takes its doctrine rank at the front again.
        let plan = CandidateRace.plan(ordered: ["lan", lighthouse], health: out.health,
                                      lighthouse: lighthouse, now: now)
        XCTAssertEqual(plan.map(\.host), ["lan", lighthouse])
        XCTAssertEqual(plan.first?.delayMs, 0)
    }

    /// Finding 3: many healthy polls against a live lighthouse must not write the
    /// store per poll — a fixed maximum number of writes, transitions and hourly
    /// checkpoints only.
    func testHealthyPollsCheckpointTheStoreHourlyNotPerPoll() {
        var live: [String: DoorHealth] = [:]
        var persisted: [String: DoorHealth] = [:]
        var writes = 0
        var t = now
        let hours: TimeInterval = 6
        while t < now + hours * 3600 {
            live = CandidateRace.settle(live, host: lighthouse, outcome: .success, now: t)
            if CandidateRace.needsCheckpoint(persisted: persisted, live: live) {
                persisted = live; writes += 1
            }
            t += poll
        }
        XCTAssertLessThanOrEqual(writes, Int(hours) + 1, "one write for the new row, then hourly")
        XCTAssertGreaterThanOrEqual(writes, Int(hours) - 1, "and it does checkpoint, so a relaunch is fresh")
    }

    /// Finding 3: a dead door failing on every poll writes on the way to demotion
    /// and then only at checkpoints — the streak past demotion is not policy.
    func testADeadDoorsEndlessStreakDoesNotChurnTheStore() {
        var live: [String: DoorHealth] = [:]
        var persisted: [String: DoorHealth] = [:]
        var writes = 0
        var t = now
        while t < now + 3600 {
            live = CandidateRace.settle(live, host: "dead", outcome: .failure, now: t)
            if CandidateRace.needsCheckpoint(persisted: persisted, live: live) {
                persisted = live; writes += 1
            }
            t += poll
        }
        XCTAssertLessThanOrEqual(writes, CandidateRace.demoteAfterFails + 1)
        XCTAssertEqual(persisted["dead"]?.consecutiveFails, CandidateRace.demoteAfterFails,
                       "the store knows the door is demoted")
    }

    /// Finding 3: the state that matters survives a relaunch through the store —
    /// what was checkpointed plans the same race as the live map.
    func testWhatIsPersistedPlansTheSameRaceAsTheLiveMap() {
        var live: [String: DoorHealth] = [:]
        var persisted: [String: DoorHealth] = [:]
        for i in 0..<10 {
            let t = now + Double(i) * poll
            live = CandidateRace.settle(live, host: "dead", outcome: .failure, now: t)
            live = CandidateRace.settle(live, host: lighthouse, outcome: .success, now: t)
            if CandidateRace.needsCheckpoint(persisted: persisted, live: live) { persisted = live }
        }
        let data = try! JSONEncoder().encode(persisted)
        let reloaded = try! JSONDecoder().decode([String: DoorHealth].self, from: data)
        let a = CandidateRace.plan(ordered: ["dead", lighthouse], health: live, lighthouse: lighthouse, now: now + 60)
        let b = CandidateRace.plan(ordered: ["dead", lighthouse], health: reloaded, lighthouse: lighthouse, now: now + 60)
        XCTAssertEqual(a, b)
        XCTAssertEqual(b.map(\.host), [lighthouse, "dead"])
    }

    /// Rows persisted before `silentSince` existed still decode, with the best
    /// age the old fields give (last answer, else last attempt).
    func testLegacyHealthRowsDecodeWithADerivedSilence() throws {
        let legacy = #"{"lan":{"consecutiveFails":4,"lastSuccess":100,"lastAttempt":900},"new":{"consecutiveFails":1,"lastSuccess":0,"lastAttempt":500}}"#
        let h = try JSONDecoder().decode([String: DoorHealth].self, from: Data(legacy.utf8))
        XCTAssertEqual(h["lan"]?.silentSince, 100)
        XCTAssertEqual(h["new"]?.silentSince, 500)
    }
}
