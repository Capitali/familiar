import XCTest
@testable import FamiliarMesh

/// Every door failed → the badge carries one sentence; the per-door lines stay
/// diagnostics. The shapes here are the live ones from a filed bug (build 100, 5G+).
final class JoinFailureTests: XCTestCase {
    private let lighthouse = "134.209.168.50"

    func testFifteenTimeoutsAndALostLighthouseBecomeOneSentence() {
        // A handset off Wi-Fi: every LAN and tailnet door timed out, the lighthouse dropped.
        var lines = (10...23).map { "192.168.1.\($0)→t:The request timed out." }
        lines.insert("\(lighthouse)→t:The network connection was los", at: 3)
        let s = JoinFailure.sentence(attempts: lines, lighthouse: lighthouse)
        XCTAssertEqual(s, "every door timed out, and the lighthouse dropped the connection (15 doors tried) — off Wi-Fi, or no data path from here?")
        XCTAssertFalse(s.contains("192.168"), "no address in the sentence — the lines are the Device screen's")
    }

    func testAMixedWalkNamesTheDominantCauseAndCountsTheRest() {
        let lines = ["a→t:The request timed out.", "b→t:The request timed out.", "c→h403:refused: unknown node", "d→dec"]
        let s = JoinFailure.sentence(attempts: lines, lighthouse: lighthouse)
        XCTAssertEqual(s, "2 of 4 doors timed out; 1 refused (HTTP 403), 1 answered with something unreadable (4 doors tried)")
    }

    func testOneDoorAndTheLighthouseAlone() {
        XCTAssertEqual(JoinFailure.sentence(attempts: ["lan→t:Could not connect to the server."], lighthouse: lighthouse),
                       "the door could not be reached (1 door tried) — off Wi-Fi, or no data path from here?")
        XCTAssertEqual(JoinFailure.sentence(attempts: ["\(lighthouse)→h502:bad gateway"], lighthouse: lighthouse),
                       "the lighthouse refused (HTTP 502) (1 door tried)")
        XCTAssertEqual(JoinFailure.sentence(attempts: [], lighthouse: lighthouse), "no door to try")
    }

    func testTheParserReadsTheRacesOwnCodes() {
        XCTAssertEqual(JoinFailure.parse("h→t:The request timed out.")?.cause, .timedOut)
        XCTAssertEqual(JoinFailure.parse("h→h500:boom")?.cause, .refused(status: 500))
        XCTAssertEqual(JoinFailure.parse("h→enc")?.cause, .unreadable)
        XCTAssertEqual(JoinFailure.parse("h→-1009")?.cause, .other("-1009"))
        XCTAssertEqual(JoinFailure.parse("h→t:The Internet connection appears to be offline.")?.cause, .offline)
        XCTAssertNil(JoinFailure.parse("   "))
    }
}
