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

    /// The host's typed `computer_state` (additive, 2026-09-08) is preferred over the persona read.
    func testTheHostsTypedComputerStateIsPreferred() throws {
        func row(_ extra: String) throws -> ShipSummary {
            let text = #"{"world":"w","label":"KK II","hull":"","captain":"Luke","server":"","automations":[],"persona":{"name":"Felix"},"# + extra + "}"
            return try XCTUnwrap(WireFeed.summary(from: try JSONDecoder().decode(JSONValue.self, from: Data(text.utf8)), tick: nil))
        }
        let broken = try row(#""computer_state":{"state":"broken","error":"style.mood is not a known mood"}"#)
        XCTAssertEqual(broken.personaState, .broken("style.mood is not a known mood")); XCTAssertFalse(broken.named)
        let named = try row(#""computer_state":{"state":"named","name":"Sprocket"}"#)
        XCTAssertEqual(named.personaState, .named("Sprocket")); XCTAssertEqual(named.computer, "Sprocket"); XCTAssertTrue(named.named)
        let absent = try row(#""computer_state":{"state":"absent"}"#)
        XCTAssertEqual(absent.personaState, .absent); XCTAssertFalse(absent.named)
        let legacy = try row(#""world_name":"LOCAL""#)
        XCTAssertEqual(legacy.personaState, .named("Felix"), "a host without the field is read as before")
    }

    /// The row's `computer_state.pronouns` (where the host carries them while it strips the
    /// persona for older readers) and `contracts[]` (T-243) are read; a host without them
    /// serves nothing and nothing is claimed.
    func testPronounsAndTheBayAreReadOffTheRow() throws {
        func row(_ extra: String) throws -> ShipSummary {
            let text = #"{"world":"w","label":"KK II","hull":"","captain":"Luke","server":"","automations":[],"persona":{"name":"Felix"},"# + extra + "}"
            return try XCTUnwrap(WireFeed.summary(from: try JSONDecoder().decode(JSONValue.self, from: Data(text.utf8)), tick: nil))
        }
        let he = try row(#""computer_state":{"state":"named","name":"Felix","pronouns":{"label":"he/him","subject":"he","object":"him","possessive":"his"}},"contracts":[{"load":"L1","word":"pickedUp"},{"loadId":"L2","status":"delivered"},{"word":"orphan"}]"#)
        XCTAssertEqual(he.pronouns?.label, "he/him"); XCTAssertEqual(he.spokenOf.possessiveTitle, "His")
        XCTAssertEqual(he.heldContracts, [.init(loadId: "L1", word: "pickedUp"), .init(loadId: "L2", word: "delivered")])
        let onPersonaText = #"{"world":"w","label":"KK II","hull":"","captain":"Luke","server":"","automations":[],"persona":{"name":"Felix","pronouns":{"label":"she/her","subject":"she","object":"her","possessive":"her"}}}"#
        let onPersona = try XCTUnwrap(WireFeed.summary(from: try JSONDecoder().decode(JSONValue.self, from: Data(onPersonaText.utf8)), tick: nil))
        XCTAssertEqual(onPersona.pronouns?.subject, "she", "a host that no longer strips them: read off the persona")
        let older = try row(#""world_name":"LOCAL""#)
        XCTAssertNil(older.pronouns); XCTAssertTrue(older.heldContracts.isEmpty)
        XCTAssertEqual(older.spokenOf.possessive, "Felix\u{2019}s", "named, no choice served: the name")
        let unnamed = try row(#""computer_state":{"state":"absent"},"persona":null"#)
        XCTAssertEqual(unnamed.spokenOf.subject, "it")
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

    /// A feed whose persona read for one world is SLOW and succeeds — the overlapping-open race.
    struct SlowGoodFeed: ShipsFeed {
        let inner = FixtureFeed()
        let slow: String
        func ships() async throws -> [ShipSummary] { try await inner.ships() }
        func context(world: String, worldInstance: String?) async throws -> (frame: String?, documents: [ContextDocument]) { try await inner.context(world: world, worldInstance: worldInstance) }
        func persona(world: String) async throws -> Persona? {
            if world == slow { try await Task.sleep(nanoseconds: 400_000_000) }
            return try await inner.persona(world: world)
        }
        func journal(world: String, sinceTick: Int64?) async throws -> [JournalEntry] { try await inner.journal(world: world, sinceTick: sinceTick) }
        func window(world: String) async throws -> [MessageItem] { try await inner.window(world: world) }
        func dial(world: String) async throws -> DialSheet { try await inner.dial(world: world) }
        func book(world: String) async throws -> ShipBook { try await inner.book(world: world) }
    }

    /// codex T-236 r3, finding 8 (the reentrancy half): an answer that was in flight when the
    /// captain switched ships is neither appended nor spoken under the new ship; an answer
    /// that lands with no switch still is.
    func testAnAnswerInFlightAcrossAShipSwitchIsDropped() async throws {
        // The next ship reads FAST and well, so a stale answer would have a live conversation
        // to land in (a broken next ship clears the voice again afterwards and would hide it).
        let model = BridgeModel(feed: FixtureFeed(), acts: FixtureFeed())
        model.speakAnswers = false
        await model.refreshShips()
        await model.open(world: "world-fixture-purr")
        XCTAssertEqual(model.persona?.name, "Purr")
        // Hold every answer for 300 ms before the conversation gives it.
        model.asker = { c, q in try? await Task.sleep(nanoseconds: 300_000_000); return await c.ask(q) }
        let asking = Task { await model.ask("status", spoken: false) }
        try await Task.sleep(nanoseconds: 50_000_000)   // the old world's answer is in flight
        XCTAssertTrue(model.asking)
        await model.open(world: "world-fixture-old")    // the switch clears the voice
        await asking.value
        XCTAssertEqual(model.world, "world-fixture-old")
        XCTAssertEqual(model.conversationWorld, "world-fixture-old")
        XCTAssertTrue(model.turns.isEmpty, "Alice's answer must not land under Bob's ship")
        // Back on her own ship, a held answer that meets no switch is appended as before.
        await model.open(world: "world-fixture-purr")
        await model.ask("status", spoken: false)
        XCTAssertEqual(model.turns.count, 1)
        XCTAssertEqual(model.turns.first?.question, "status")
    }

    /// codex T-236 r3, finding 8 (the overlapping-open half): two opens race; the one that
    /// resumes last must not publish its persona under the later selection. Only the newest
    /// open publishes, and `loading` reflects it.
    func testAnOlderOpenThatResumesLastPublishesNothing() async throws {
        let model = BridgeModel(feed: SlowGoodFeed(slow: "world-fixture-purr"), acts: FixtureFeed())
        await model.refreshShips()
        let first = Task { await model.open(world: "world-fixture-purr") }   // suspends 400 ms in its persona read
        try await Task.sleep(nanoseconds: 50_000_000)
        await model.open(world: "world-fixture-old")                            // the newer selection, fast
        XCTAssertEqual(model.world, "world-fixture-old")
        XCTAssertNil(model.persona, "the old hull has no persona")
        XCTAssertFalse(model.loading)
        await first.value                                                        // Purr's read resumes now
        XCTAssertEqual(model.world, "world-fixture-old")
        XCTAssertNil(model.persona, "Purr must not be published under the old hull")
        XCTAssertEqual(model.conversationWorld, "world-fixture-old")
        XCTAssertNotEqual(model.computerName, "Purr")
        XCTAssertFalse(model.loading)
        // The reverse order — the slow one is the NEWEST — still publishes the slow one.
        let again = Task { await model.open(world: "world-fixture-purr") }
        await again.value
        XCTAssertEqual(model.persona?.name, "Purr"); XCTAssertEqual(model.conversationWorld, "world-fixture-purr")
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
