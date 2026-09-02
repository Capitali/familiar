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
}
