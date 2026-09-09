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
        // Every mark of the journal cites at least one tick, and the story carries every citation.
        for m in h.marks where m.kind != .name { XCTAssertFalse(m.ticks.isEmpty, m.id) }
        XCTAssertTrue(h.story.contains("Routes flown: whisker-hollow → foxys-diner, flown 1 time [t105]"), h.story)
        XCTAssertTrue(h.story.contains("Deliveries: 2 loads delivered, ℳ444 freight paid (bluefin-reserve, tinplate) [t120]"))
        XCTAssertTrue(h.story.hasSuffix("it is what she did, and the names are not forgotten."))
    }

    /// Names are lineage (Ian, 2026-09-08): the store's naming trail and the host's ledger both
    /// become marks, dated, never forgotten — and a rename's refusal is the host's sentence.
    func testNamesAreLineageAndARefusedRenameIsSaidNotRetried() async throws {
        let names = Fixtures.store.namings().map(NameLine.init(naming:))
        XCTAssertEqual(names, [NameLine(kind: "computer", name: "Purr", act: "paired", by: "pairing", at: 1700000000)])
        func ledger(_ json: String) throws -> NameLine { try XCTUnwrap(NameLine(ledger: try JSONDecoder().decode(JSONValue.self, from: Data(json.utf8)))) }
        let rows = [
            try ledger(#"{"at":1700003000,"kind":"computer","name":"Felix","holder":"cpt-1","act":"renamed","from":"Purr","by":"ian"}"#),
            try ledger(#"{"at":1700001000,"kind":"hull","name":"Kibble Klipper II","holder":"world-1","act":"paired","by":"ian"}"#),
        ]
        XCTAssertNil(NameLine(ledger: .object(["act": .string("named")])), "a row without a name is no line")
        let h = ShipHistory.from(journal: Fixtures.journal().entries, book: ShipBook(holdings: [], deliveries: []), names: names + rows)
        XCTAssertEqual(h.marks(of: .name).map(\.text), [
            "the computer was Purr from the pairing on 2023-11-14",
            "the hull was Kibble Klipper II from the pairing (by ian) on 2023-11-14",
            "the computer became Felix, was Purr (by ian) on 2023-11-14",
        ])
        XCTAssertTrue(h.story.hasPrefix("Names: the computer was Purr from the pairing on 2023-11-14; "), h.story)
        // The store feed remembers its trail.
        let remembered = try await StoreFeed(worlds: Fixtures.root).names(world: "ship")
        XCTAssertEqual(remembered.map(\.name), ["Purr"])
        // A refused rename is the host's sentence on the sheet, never a retry.
        let model = BridgeModel(feed: FixtureFeed(), acts: RefusingActs())
        await model.refreshShips(); await model.open(world: "world-fixture-purr")
        let out = await model.rename(computer: "Felix")
        XCTAssertFalse(out.ok)
        XCTAssertEqual(out.text, "\"Felix\" is Luke SkyWhisker's computer's name; two ships' computers cannot have the same name (HTTP 400)")
    }

    /// The host's captain brief carries her ledger rows (`names`), verbatim and in order; a brief
    /// without the field remembers nothing.
    func testTheWireFeedReadsHerNamesOffTheCaptainBrief() throws {
        let brief = try JSONDecoder().decode(JSONValue.self, from: Data(#"{"captain":"Luke SkyWhisker","captain_id":"cpt-1","names":[{"at":1700001000,"kind":"hull","name":"Kibble Klipper II","holder":"world-1","act":"paired","by":"ian"},{"at":1700003000,"kind":"computer","name":"Felix","holder":"cpt-1","act":"renamed","from":"Purr","by":"ian"},{"kind":"captain"}]}"#.utf8))
        let names = WireFeed.names(fromBrief: brief)
        XCTAssertEqual(names.map(\.name), ["Kibble Klipper II", "Felix"], "verbatim, in the host's order; a row without a name is dropped")
        XCTAssertEqual(names[1], NameLine(kind: "computer", name: "Felix", holder: "cpt-1", act: "renamed", from: "Purr", by: "ian", at: 1700003000))
        XCTAssertTrue(WireFeed.names(fromBrief: .object(["captain": .string("x")])).isEmpty)
    }

    struct RefusingActs: CaptainActs {
        func approve(world: String, proposalID: String, approved: Bool) async throws {}
        func setDial(world: String, dial: AutonomyDial) async throws {}
        func pair(_ request: PairingRequest, key: PairingKey) async throws {}
        func unpair(world: String) async throws {}
        func rename(world: String, computer: String) async throws -> String? {
            throw FeedError.refused("\"Felix\" is Luke SkyWhisker's computer's name; two ships' computers cannot have the same name (HTTP 400)")
        }
        func setAutomations(world: String, automations: [Automation]) async throws -> String? { nil }
        func setCaptain(world: String, captain: String) async throws -> String? { nil }
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
