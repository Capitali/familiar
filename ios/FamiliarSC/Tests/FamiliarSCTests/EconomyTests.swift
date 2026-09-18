import XCTest
@testable import FamiliarSC
@testable import FamiliarSCUI

/// T-241, the iPad half: the host's economy answer read as typed history — points, flows as
/// signed bars, the host's sentences verbatim — the route derived from the host's own
/// `captain_brief`, and the brief's summary on the fleet document Felix reads.
final class EconomyTests: XCTestCase {
    func testTheCaptainsEconomyReadsWholeFromTheHostsShape() throws {
        let v = try JSONDecoder().decode(JSONValue.self, from: Fixtures.wire("captain-economy"))
        let e = try XCTUnwrap(CaptainEconomy(json: v))
        XCTAssertEqual(e.captain, "Luke SkyWhisker"); XCTAssertEqual(e.captainID, "c_9f3")
        XCTAssertEqual(e.hulls.map(\.label), ["Kibble Klipper", "Kibble Klipper II"])
        XCTAssertEqual(e.hulls.map(\.id), ["world-kk", "world-kk2"]); XCTAssertEqual(e.pooled.id, "pooled")
        // The pool: every reading, the host's summary and sentences, the source word.
        XCTAssertEqual(e.pooled.points.map(\.credits), [1500, 900, 1513, 1743])
        XCTAssertEqual(e.pooled.points.first?.date, Date(timeIntervalSince1970: 1_789_000_000))
        XCTAssertEqual(e.pooled.summary.delta, 243); XCTAssertEqual(e.pooled.summary.readings, 4)
        XCTAssertEqual(e.pooled.summary.trendPerDay, 34.7, accuracy: 0.001)
        XCTAssertEqual(e.pooled.summary.best?.cause, "freight"); XCTAssertEqual(e.pooled.summary.worst?.delta, -600)
        XCTAssertEqual(e.pooled.analysis.count, 3)
        XCTAssertEqual(e.pooled.analysis[0], "+ℳ243 over 7.0 days: ℳ1500 → ℳ1743")
        XCTAssertEqual(e.pooled.sourceWords, "flows from the exchange's ledger where it answered, the journal elsewhere")
        XCTAssertEqual(e.hulls[0].sourceWords, "flows from the exchange's own ledger")
        // A hull with no worst move: nil, not a zero move.
        XCTAssertNil(e.hulls[1].summary.worst); XCTAssertEqual(e.hulls[1].summary.best?.delta, 300)
    }

    func testFlowsBecomeSignedBarsInTheHostsOrderOfTelling() throws {
        let v = try JSONDecoder().decode(JSONValue.self, from: Fixtures.wire("captain-economy"))
        let f = try XCTUnwrap(CaptainEconomy(json: v)).pooled.flows
        // In first, then out; zero buckets (outfit) are not bars; fills/settles are counts, not money.
        XCTAssertEqual(f.bars.map(\.cause), ["freight", "trade sold", "trade bought", "fuel", "repair", "debt paid", "dock", "other"])
        XCTAssertEqual(f.bars.map(\.amount), [776, 300, -105, -88, -20, -600, -12, -8])
        XCTAssertEqual(f.earned, 1076); XCTAssertEqual(f.spent, -833); XCTAssertEqual(f.tradeNet, 195)
        XCTAssertEqual(f.fills, 2); XCTAssertEqual(f.settles, 2)
        XCTAssertTrue(EconomyFlows().bars.isEmpty)
    }

    func testAnAnswerThatIsNotAnEconomyIsNil() throws {
        XCTAssertNil(CaptainEconomy(json: .object(["error": .string("no captain by that identity flies here")])))
        XCTAssertNil(EconomyHistory(json: .object(["points": .array([])])), "a summary is the least a history has")
        // The brief's summary shape: no points, still a history.
        let brief = try XCTUnwrap(EconomyHistory(json: .object(["flows": .object([:]), "flows_source": .string("journal"), "summary": .object(["readings": .number(0)]), "analysis": .array([.string("no readings in this window")])])))
        XCTAssertTrue(brief.points.isEmpty); XCTAssertEqual(brief.analysis, ["no readings in this window"])
    }

    func testTheRouteIsDerivedFromTheHostsOwnBriefPathAndOnlyForTheHostsWindows() {
        XCTAssertEqual(WireFeed.captainEconomyPath(briefPath: "captains/c_9f3/brief", window: "7d"), "captains/c_9f3/economy?window=7d")
        XCTAssertEqual(WireFeed.captainEconomyPath(briefPath: "captains/luke-skywhisker/brief", window: "24h"), "captains/luke-skywhisker/economy?window=24h")
        XCTAssertNil(WireFeed.captainEconomyPath(briefPath: "captains/c_9f3/brief", window: "1y"), "a window the host does not serve is not sent")
        XCTAssertNil(WireFeed.captainEconomyPath(briefPath: "ships/w/brief", window: "7d"), "not a captain brief")
    }

    func testTheBriefsSummaryRidesTheFleetDocumentFelixReads() throws {
        let v = try JSONDecoder().decode(JSONValue.self, from: Fixtures.wire("captain-economy"))
        let pooled = try XCTUnwrap(v["pooled"]?.object)
        var summary = pooled; summary["points"] = nil
        let brief: JSONValue = .object(["captain": .string("Luke SkyWhisker"), "computer": .string("Felix"), "ships": .array([]), "economy": .object(summary)])
        let t = Briefs.captain(brief)
        XCTAssertTrue(t.contains("The fleet's money this week: +ℳ243 over 7.0 days: ℳ1500 → ℳ1743; earned: freight +ℳ776"), t)
        XCTAssertTrue(t.contains("the trend is a straight line through the readings, ℳ35 a day"), t)
        XCTAssertTrue(t.contains("(flows from the exchange's ledger where it answered, the journal elsewhere)."), t)
        // A brief without the field says nothing about money it does not have.
        let without = Briefs.captain(.object(["captain": .string("Luke SkyWhisker"), "ships": .array([])]))
        XCTAssertFalse(without.contains("money"), without)
    }
}
