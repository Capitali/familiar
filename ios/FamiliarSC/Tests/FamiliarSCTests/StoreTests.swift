import XCTest
@testable import FamiliarSC

/// The ship-store reader against the Rust contract — every file the pilot and the fleet
/// write, read back typed; malformed persona/dial files heard about, never half-honoured.
final class StoreTests: XCTestCase {
    func testPersonaV2LoadsWithItsStyle() throws {
        let p = try XCTUnwrap(try Fixtures.store.persona())
        XCTAssertEqual(p.personaVersion, 2)
        XCTAssertEqual(p.name, "Purr")
        XCTAssertEqual(p.voice.vocabulary, "feline")
        XCTAssertEqual(p.voice.humor, 8)
        XCTAssertEqual(p.voice.greeting, "Mrrp.")
        XCTAssertEqual(Fixtures.store.computerName(), "Purr")
        XCTAssertEqual(Fixtures.store.namings().map(\.name), ["Purr"])
    }

    func testAShipPairedBeforeT236HasNoPersonaAndSaysSo() throws {
        let s = try Fixtures.scratchStore { try FileManager.default.removeItem(at: $0.appendingPathComponent("persona.json")) }
        XCTAssertNil(try s.persona())
        XCTAssertEqual(s.computerName(), "(unnamed — `fleet rename` her)")
        XCTAssertNotEqual(s.computerName(), Persona.householdDefaultName, "a ship never borrows the household's default name")
    }

    func testPersonaRefusalsMirrorTheRustLoader() {
        func refused(_ json: String) -> String? {
            do { _ = try Persona.decode(Data(json.utf8)); return nil } catch let e as StoreError { return e.description } catch { return "\(error)" }
        }
        XCTAssertNotNil(refused(#"{"persona_version":1,"name":"Purr","style":{}}"#), "v1 cannot carry a style")
        XCTAssertNotNil(refused(#"{"persona_version":3,"name":"Purr"}"#), "unknown version")
        XCTAssertNotNil(refused(#"{"persona_version":2,"name":"Purr","style":{"warmth":30}}"#), "out of bounds is refused, not clamped")
        XCTAssertNotNil(refused(#"{"persona_version":2,"name":"Purr","style":{"vocabulary":"pirate"}}"#))
        XCTAssertNotNil(refused(#"{"persona_version":2,"name":"Purr","mood":"grumpy"}"#), "unknown field")
        XCTAssertNotNil(refused(#"{"persona_version":2,"name":"Purr","style":{"sarcasm":9}}"#), "unknown style field")
        XCTAssertNotNil(refused(#"{"persona_version":2,"name":""}"#), "a persona must have a name")
        XCTAssertNil(refused(#"{"persona_version":2,"name":"Purr"}"#), "v2 without a style is fine")
        XCTAssertNil(refused(#"{"name":"Purr"}"#), "a bare v1 record parses")
    }

    func testCaptainAutomationsHoldingsDeliveries() throws {
        let c = try Fixtures.store.captain()
        XCTAssertEqual(c.keyID, "0123abcd")
        XCTAssertEqual(c.hullName, "Fixture Freighter")
        XCTAssertEqual(c.pilotArgs, ["--allow-paws"])
        XCTAssertEqual(try Fixtures.store.automations(), ["freight", "trade", "outfit"])
        let h = Fixtures.store.holdings()
        XCTAssertEqual(h.count, 1)
        XCTAssertEqual(h[0].sellableAt, 411)
        let d = Fixtures.store.deliveries()
        XCTAssertEqual(d.map(\.loadID), ["L1", "L0"])
        XCTAssertTrue(d[0].perishable)
        XCTAssertEqual(d[1].paid, 170)
    }

    func testJournalParsesEveryLineAndKeepsUnknownEvents() throws {
        let j = Fixtures.journal()
        XCTAssertEqual(j.malformed, 0)
        XCTAssertEqual(j.entries.count, 36)
        XCTAssertEqual(j.entries.first?.event, "watch-begins")
        XCTAssertNil(j.entries.first?.tick, "a tick-less line keeps its nil")
        let comet = try XCTUnwrap(j.entries.last)
        XCTAssertEqual(comet.event, "sighted-comet")
        XCTAssertEqual(comet.int("tail_km"), 123456)
        XCTAssertEqual(j.lastTick, 251)
        let acted = j.entries.first { $0.event == "acted" }!
        XCTAssertEqual(acted.string("decision"), "Collect { load_id: \"L1\" }")
        XCTAssertEqual(acted.int("resolves"), 104)
    }

    func testJournalSinceTickKeepsTicklessLinesByWallClock() {
        let j = Fixtures.journal()
        let w = j.since(tick: 181)
        XCTAssertEqual(w.first?.tick, 181)
        XCTAssertTrue(w.contains { $0.event == "exchange-unreachable" }, "the unreachable lines after t181 ride along")
        XCTAssertFalse(w.contains { $0.event == "watch-begins" }, "the watch beginning is before the window")
    }

    func testMalformedJournalLinesAreCountedNotHidden() {
        let j = Journal.parse("{\"at\":1,\"event\":\"holding\"}\nnot json\n{\"at\":2}\n")
        XCTAssertEqual(j.entries.count, 1)
        XCTAssertEqual(j.malformed, 2, "a line without an event word is malformed too")
    }

    func testMissingFilesAreNamed() {
        let s = ShipStore(directory: URL(fileURLWithPath: "/nonexistent/world-x"))
        XCTAssertThrowsError(try s.captain()) { XCTAssertEqual($0 as? StoreError, .missing("captain.json")) }
        XCTAssertEqual(s.holdings(), [])
        XCTAssertEqual(s.dial(), .absent)
    }
}
