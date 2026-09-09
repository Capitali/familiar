import XCTest
@testable import FamiliarSC
@testable import FamiliarSCUI

/// T-239: from a hull's journal and book alone, the earned-history record — every mark citing
/// its ticks, nothing purchasable or editable. The fixture store is synthesized (never a real
/// hull's journal).
final class HistoryTests: XCTestCase {
    func testTheFixtureHullsStoryFromItsJournalAndBook() throws {
        let store = Fixtures.store
        let h = ShipHistory.from(journal: Fixtures.journal().entries, book: ShipBook(holdings: store.holdings(), deliveries: store.deliveries()))
        // Routes: berthed at whisker-hollow, engaged for foxys-diner (t105); carried ore from
        // there to io-slagworks (t140); unwedged a course to cannery-row (t223).
        let routes = h.marks(of: .route)
        XCTAssertEqual(routes.map(\.key), ["whisker-hollow→foxys-diner", "foxys-diner→io-slagworks", "io-slagworks→cannery-row"])
        XCTAssertEqual(routes.map(\.ticks), [[105], [140], [223]])
        XCTAssertEqual(routes[0].text, "whisker-hollow → foxys-diner, flown 1 time")
        // Deliveries from the book, cited by the ledger's settlement tick where the journal has it.
        let d = try XCTUnwrap(h.marks(of: .delivery).first)
        XCTAssertEqual(d.count, 2); XCTAssertEqual(d.ticks, [120])
        XCTAssertEqual(d.text, "2 loads delivered, ℳ444 freight paid (bluefin-reserve, tinplate)")
        // A refit is a mark; a refused refit is not.
        XCTAssertEqual(h.marks(of: .refit).map(\.text), ["fitted drive-tune at titania-cold-store for ℳ9000"])
        XCTAssertEqual(h.marks(of: .refit)[0].ticks, [215])
        // The distress at t250 is the last thing in the journal: open, not survived.
        let distress = h.marks(of: .distress)
        XCTAssertEqual(distress.count, 1)
        XCTAssertTrue(distress[0].text.hasPrefix("in distress since t250"), distress[0].text)
        XCTAssertTrue(distress[0].text.hasSuffix("not yet survived"))
        XCTAssertTrue(h.marks(of: .repair).isEmpty && h.marks(of: .rescue).isEmpty && h.marks(of: .escort).isEmpty)
        XCTAssertEqual(h.firstTick, 100); XCTAssertEqual(h.lastTick, 251)
        // Every mark cites at least one tick, and the story carries every citation.
        for m in h.marks { XCTAssertFalse(m.ticks.isEmpty, m.id) }
        XCTAssertTrue(h.story.contains("Routes flown: whisker-hollow → foxys-diner, flown 1 time [t105]"), h.story)
        XCTAssertTrue(h.story.contains("Deliveries: 2 loads delivered, ℳ444 freight paid (bluefin-reserve, tinplate) [t120]"))
        XCTAssertTrue(h.story.hasSuffix("Nothing here can be bought or edited; it is what she did."))
    }

    func testDistressSurvivedRepairsRescuesAndCountsAreEarnedFromTheJournal() {
        func e(_ tick: Int64, _ event: String, _ f: [String: JSONValue] = [:]) -> JournalEntry { JournalEntry(at: tick, tick: tick, event: event, fields: f) }
        let j: [JournalEntry] = [
            e(10, "holding", ["docked": .string("a")]),
            e(11, "engaged-drive", ["to": .string("b")]),
            e(20, "distress-hold", ["why": .string("stranded at 9 fuel")]),
            e(21, "distress-hold", ["why": .string("stranded at 9 fuel")]),
            e(30, "acted", ["decision": .string("CallPaws")]),
            e(40, "engaged-drive", ["to": .string("a")]),
            e(41, "engaged-drive", ["to": .string("b")]),
            e(50, "acted", ["decision": .string("Repair")]),
            e(60, "escort-booked", ["post": .string("E7")]),
        ]
        let h = ShipHistory.from(journal: j, book: ShipBook(holdings: [], deliveries: []))
        XCTAssertEqual(h.marks(of: .route).map { ($0.key, $0.count) }.map { "\($0.0)×\($0.1)" }, ["a→b×2", "b→a×1"])
        XCTAssertEqual(h.marks(of: .route)[0].ticks, [11, 41])
        XCTAssertEqual(h.marks(of: .distress).map(\.text), ["held in distress from t20 to t30 (stranded at 9 fuel), and flew again"])
        XCTAssertEqual(h.marks(of: .distress)[0].ticks, [20, 30], "the two ticks that bound the ordeal")
        XCTAssertEqual(h.marks(of: .rescue).map(\.ticks), [[30]])
        XCTAssertEqual(h.marks(of: .repair).map(\.text), ["repaired at the yard 1 time"])
        XCTAssertEqual(h.marks(of: .escort).map(\.text), ["escort work on E7, 1 time"])
        XCTAssertTrue(h.marks(of: .delivery).isEmpty, "no book, no deliveries — never invented")
    }

    func testAnEmptyRecordSaysSoAndTheRecordIsNotEditable() {
        let h = ShipHistory.from(journal: [], book: ShipBook(holdings: [], deliveries: []))
        XCTAssertTrue(h.marks.isEmpty); XCTAssertNil(h.firstTick)
        XCTAssertTrue(h.story.hasPrefix("No history yet"))
        // The ethics rail, as far as a type can carry it: the record is a value built from the
        // journal and the book, with no initializer that takes marks and no mutating API — a
        // screen or a purchase has nothing to call.
        XCTAssertFalse(Mirror(reflecting: h).children.contains { $0.label == "purchased" || $0.label == "edits" })
    }

    func testTheStoryRidesTheBridgeContext() async {
        let model = BridgeModel(feed: FixtureFeed(), acts: FixtureFeed())
        await model.refreshShips()
        await model.open(world: "world-fixture-purr")
        XCTAssertNotNil(model.history)
        let doc = model.conversation?.context.documents.first { $0.name == "history" }
        XCTAssertNotNil(doc, "her story is a document the voice is grounded on")
        XCTAssertEqual(doc?.text, model.history?.story)
    }
}
