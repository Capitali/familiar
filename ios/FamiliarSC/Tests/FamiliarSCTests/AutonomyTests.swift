import XCTest
@testable import FamiliarSC

/// The dial, in lockstep with crates/whisker/src/autonomy.rs — the same four pins the
/// Rust tests hold, plus the file-level truths the app must tell the captain.
final class AutonomyTests: XCTestCase {
    func testAbsentDialIsAutoForEverythingButTheTanker() {
        let d = AutonomyDial()
        XCTAssertEqual(d.level(for: .freightBook), .auto)
        XCTAssertEqual(d.level(for: .marketBuy), .auto)
        XCTAssertEqual(d.level(for: .navigationRescue), .advise)
    }

    func testMostSpecificSettingWins() {
        var d = AutonomyDial()
        XCTAssertNil(d.set("*", .advise))
        XCTAssertNil(d.set("market", .confirm))
        XCTAssertNil(d.set("market.sell", .auto))
        XCTAssertEqual(d.level(for: .freightBook), .advise)
        XCTAssertEqual(d.level(for: .marketBuy), .confirm)
        XCTAssertEqual(d.level(for: .marketSell), .auto)
        XCTAssertEqual(d.level(for: .navigationRescue), .advise, "`*` covers the tanker too")
        XCTAssertEqual(d.set("kitchen.sink", .auto), "unknown control surface `kitchen.sink`")
        let back = try! AutonomyDial.decode(d.encoded())
        XCTAssertEqual(back, d)
    }

    func testTheFixtureDialReadsAsWhiskerReadsIt() {
        guard case .dial(let d) = Fixtures.store.dial() else { return XCTFail("dial present") }
        XCTAssertEqual(d.level(for: .marketBuy), .confirm)
        XCTAssertEqual(d.level(for: .marketSell), .auto, "`*` = auto")
        XCTAssertEqual(d.level(for: .navigationCourse), .advise, "family default")
        XCTAssertEqual(d.level(for: .navigationRescue), .advise)
    }

    func testAMalformedDialIsLoudBecauseWhiskerReadsItAsAbsent() throws {
        let s = try Fixtures.scratchStore { try Data(#"{"market.buy":"maybe"}"#.utf8).write(to: $0.appendingPathComponent("autonomy.json")) }
        guard case .malformed(let why) = s.dial() else { return XCTFail("malformed") }
        XCTAssertTrue(why.contains("maybe"), why)
        XCTAssertEqual(s.dial().dial.level(for: .marketBuy), .auto, "what whisker will actually do")
        let s2 = try Fixtures.scratchStore { try Data(#"{"kitchen.sink":"auto"}"#.utf8).write(to: $0.appendingPathComponent("autonomy.json")) }
        guard case .malformed = s2.dial() else { return XCTFail("unknown surface is malformed") }
    }

    func testLevelAliasesAndSurfaceVocabulary() {
        XCTAssertEqual(AutonomyLevel.parse("Advisory"), .advise)
        XCTAssertEqual(AutonomyLevel.parse("ask"), .confirm)
        XCTAssertEqual(AutonomyLevel.parse("autonomous"), .auto)
        XCTAssertNil(AutonomyLevel.parse("yes"))
        XCTAssertEqual(ControlSurface.allCases.count, 17)
        XCTAssertEqual(ControlSurface.parse(" racing.refusal "), .racingRefusal)
        XCTAssertEqual(ControlSurface.marketCarry.family, "market")
        XCTAssertEqual(ControlSurface.marketCarry.category, "carry")
        XCTAssertEqual(ControlSurface.shipLease.automation, "outfit")
        XCTAssertNil(ControlSurface.racingPlot.automation, "racing is not a grant yet")
    }
}
