import XCTest
@testable import FamiliarSC
@testable import FamiliarSCUI

/// T-236 brick 1, codex round 2, the Swift findings: a broken persona is BROKEN in the fleet
/// row, never "unnamed" (7); and while a ship is being opened — or after that open fails —
/// nothing of the previous captain is readable or speakable under the new world (8).
final class T236SwiftTests: XCTestCase {
    func testABrokenPersonaOnTheShipsRowIsSaidAsBrokenNotUnnamed() throws {
        func row(_ persona: String) throws -> ShipSummary {
            let text = #"{"world":"w","label":"KK II","hull":"","captain":"Luke","server":"","automations":[],"persona":"# + persona + "}"
            let j = try JSONDecoder().decode(JSONValue.self, from: Data(text.utf8))
            return try XCTUnwrap(WireFeed.summary(from: j, tick: nil))
        }
        let broken = try row(#"{"error":"captain persona unreadable: style.mood is not a known mood"}"#)
        XCTAssertEqual(broken.personaState, .broken("captain persona unreadable: style.mood is not a known mood"))
        XCTAssertFalse(broken.named)
        XCTAssertTrue(broken.computer.hasPrefix("(persona broken — "), broken.computer)
        XCTAssertFalse(broken.computer.contains("unnamed"), "broken is not absent")
        let absent = try row("null")
        XCTAssertEqual(absent.personaState, .absent); XCTAssertFalse(absent.named)
        XCTAssertEqual(absent.computer, "(unnamed — `fleet rename` her)")
        let named = try row(#"{"name":"Felix","persona_version":1}"#)
        XCTAssertEqual(named.personaState, .named("Felix")); XCTAssertTrue(named.named); XCTAssertEqual(named.computer, "Felix")
        XCTAssertNotEqual(broken.personaState, absent.personaState)
    }

    /// Alice (Purr) is open. Bob's persona read SUSPENDS, then fails. While it is suspended
    /// and after it fails, nothing of Alice is readable or speakable under Bob's world.
    struct SlowBrokenFeed: ShipsFeed {
        let inner = FixtureFeed()
        let broken: String
        func ships() async throws -> [ShipSummary] { try await inner.ships() }
        func context(world: String, worldInstance: String?) async throws -> (frame: String?, documents: [ContextDocument]) { try await inner.context(world: world, worldInstance: worldInstance) }
        func persona(world: String) async throws -> Persona? {
            if world == broken {
                try await Task.sleep(nanoseconds: 400_000_000)
                throw FeedError.refused("captain persona unreadable: style.mood is not a known mood")
            }
            return try await inner.persona(world: world)
        }
        func journal(world: String, sinceTick: Int64?) async throws -> [JournalEntry] { try await inner.journal(world: world, sinceTick: sinceTick) }
        func window(world: String) async throws -> [MessageItem] { try await inner.window(world: world) }
        func dial(world: String) async throws -> DialSheet { try await inner.dial(world: world) }
        func book(world: String) async throws -> ShipBook { try await inner.book(world: world) }
    }

    func testWhileTheNextShipIsBeingReadThePreviousCaptainCannotSpeak() async throws {
        let model = BridgeModel(feed: SlowBrokenFeed(broken: "world-fixture-old"), acts: FixtureFeed())
        await model.refreshShips()
        await model.open(world: "world-fixture-purr")
        XCTAssertEqual(model.persona?.name, "Purr"); XCTAssertNotNil(model.conversation)
        await model.ask("status", spoken: false)
        XCTAssertEqual(model.turns.count, 1, "Alice answers for her own ship")

        let opening = Task { await model.open(world: "world-fixture-old") }
        try await Task.sleep(nanoseconds: 120_000_000)   // Bob's persona read is suspended now
        XCTAssertEqual(model.world, "world-fixture-old")
        XCTAssertTrue(model.loading)
        XCTAssertNil(model.persona); XCTAssertNil(model.conversation); XCTAssertTrue(model.turns.isEmpty)
        XCTAssertTrue(model.journal.isEmpty && model.window.isEmpty && model.reports.isEmpty)
        XCTAssertNotEqual(model.computerName, "Purr", "Alice's name must not stand under Bob's world while his reads run")
        await model.ask("where are we", spoken: false)
        XCTAssertTrue(model.turns.isEmpty, "nobody answers under a world still being read")
        await opening.value

        XCTAssertNotNil(model.error)
        XCTAssertNil(model.persona); XCTAssertNil(model.conversation); XCTAssertTrue(model.turns.isEmpty)
        XCTAssertNotEqual(model.computerName, "Purr")
        await model.ask("where are we", spoken: false)
        XCTAssertTrue(model.turns.isEmpty)
        // A refresh of the SAME good ship keeps its voice through the reads.
        await model.open(world: "world-fixture-purr")
        XCTAssertEqual(model.persona?.name, "Purr")
        await model.ask("status", spoken: false)
        XCTAssertEqual(model.turns.count, 1)
        let refresh = Task { await model.open(world: "world-fixture-purr") }
        await refresh.value
        XCTAssertEqual(model.turns.count, 1, "a refresh of the open ship does not clear her conversation")
    }
}
