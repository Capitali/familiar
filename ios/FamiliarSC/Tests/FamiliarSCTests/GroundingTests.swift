import XCTest
@testable import FamiliarSC

/// The truth boundary binds a statement's verb to its source, not only its numbers (codex
/// T-237 B2 re-verification, finding 1): the four cases codex named, and the rephrases that
/// must still pass.
final class GroundingTests: XCTestCase {
    let floor = [
        "t123: bought 40 ore at ask 15 at foxys-diner — ℳ4400",
        "t502: buy catnip refused at the door — action refused: HTTP 402 insufficient credits",
        "t300: proposal p-0123abcd approved — book L3",
        "t7434: sell 53 bluefin-reserve: filled",
    ]

    func testBuyBecomingSellIsInverted() {
        XCTAssertNil(Grounding.bind("I bought 40 ore at ask 15 at foxys-diner for ℳ4400.", to: floor))
        XCTAssertNil(Grounding.bind("At foxys-diner, 40 ore came aboard at 15 — ℳ4400 on the book.", to: floor), "no verb on either axis: a rephrase")
        XCTAssertEqual(Grounding.bind("I sold 40 ore at ask 15 at foxys-diner.", to: floor), "inverted side: \"I sold 40 ore at ask 15 at foxys-diner.\" — its source says the opposite")
        XCTAssertEqual(Grounding.bind("We did not buy 40 ore at foxys-diner.", to: floor), "inverted side: \"We did not buy 40 ore at foxys-diner.\" — its source says the opposite", "a negation flips the sign")
    }

    func testApprovedBecomingDeniedIsInverted() {
        XCTAssertNil(Grounding.bind("You approved p-0123abcd, so L3 is booked.", to: floor))
        XCTAssertEqual(Grounding.bind("You denied p-0123abcd, so L3 waits.", to: floor), "inverted outcome: \"You denied p-0123abcd, so L3 waits.\" — its source says the opposite")
    }

    func testDroppingTheRefusalIsALie() {
        XCTAssertNil(Grounding.bind("The catnip buy at t502 was refused at the door — short of credits.", to: floor))
        XCTAssertEqual(Grounding.bind("I bought catnip at t502.", to: floor), "inverted outcome: \"I bought catnip at t502.\" — its source says the opposite")
        XCTAssertEqual(Grounding.bind("The catnip buy at t502.", to: floor), "dropped the refusal: \"The catnip buy at t502.\" — its source was refused")
        XCTAssertNil(Grounding.bind("53 bluefin-reserve sold at t7434 and filled.", to: floor))
    }

    func testAStatementWithNoSourceMustStillBeSupported() {
        XCTAssertNil(Grounding.bind("I sold the lot.", to: floor), "the floor does sell (t7434), so an unbound 'sold' is supported")
        XCTAssertEqual(Grounding.bind("Everything was denied.", to: ["t1: bought 4 ore"]), "inverted outcome: \"Everything was denied.\" — its source says the opposite")
        XCTAssertEqual(Grounding.bind("Everything was denied.", to: ["t1: holding at a"]), "unsupported outcome: \"Everything was denied.\"")
        XCTAssertNil(Grounding.bind("A quiet watch.", to: floor))
    }

    func testAnInventedStationTripsTheConversationCheck() {
        let truth = floor.joined(separator: "\n") + "\nship, hull KK (PROD), captain Luke SkyWhisker"
        XCTAssertNil(Grounding.checkReply("We bought 40 ore at foxys-diner for ℳ4400. Nothing waits on you.", truth: truth, facts: floor))
        XCTAssertEqual(Grounding.checkReply("We bought 40 ore at tuna-prime.", truth: truth, facts: floor), "invented: tuna-prime")
        XCTAssertEqual(Grounding.checkReply("We sold 40 ore at foxys-diner. Nothing waits.", truth: truth, facts: floor), "inverted side: \"We sold 40 ore at foxys-diner\" — its source says the opposite")
    }

    func testTheReportLaneBindsEveryLine() {
        let base = BridgeReport(headline: "A fair watch, captain.", facts: floor, nextAct: "Nothing needs you.", mood: .steady)
        XCTAssertNil(Grounding.check(spoken: base, floor: base))
        var flipped = base; flipped.facts[0] = "t123: sold 40 ore at ask 15 at foxys-diner — ℳ4400"
        XCTAssertEqual(Grounding.check(spoken: flipped, floor: base), "inverted side: \"t123: sold 40 ore at ask 15 at foxys-diner — ℳ4400\" — its source says the opposite")
        var dropped = base; dropped.facts[1] = "t502: buy catnip at the door — HTTP 402"
        XCTAssertEqual(Grounding.check(spoken: dropped, floor: base), "dropped the refusal: \"t502: buy catnip at the door — HTTP 402\" — its source was refused")
    }
}
