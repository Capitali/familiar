import XCTest
@testable import FamiliarSC
@testable import FamiliarSCUI

/// The cross-runtime contracts codex found drifting on T-237 B2 (re-verification 2026-09-08,
/// findings 2–4), each pinned against ONE shared fixture the Rust suite pins too
/// (`Fixtures/contract/*.json`): the dial's surfaces and defaults, the captain's durable
/// identity, and the runner's journal vocabulary through the notice and voice policies.
final class ContractDriftTests: XCTestCase {
    func contract(_ name: String) throws -> JSONValue {
        let url = Fixtures.root.appendingPathComponent("contract/\(name).json")
        return try JSONDecoder().decode(JSONValue.self, from: try Data(contentsOf: url))
    }

    // MARK: finding 2 — the dial's vocabulary

    func testEverySurfaceAndItsDefaultMatchTheSharedContract() throws {
        let c = try contract("autonomy-surfaces")
        let surfaces = try XCTUnwrap(c["surfaces"]?.object)
        XCTAssertEqual(surfaces.count, ControlSurface.allCases.count, "a surface one side lacks")
        let dial = AutonomyDial()
        for s in ControlSurface.allCases {
            let want = try XCTUnwrap(surfaces[s.key]?.string, "\(s.key) is not in the shared contract")
            XCTAssertEqual(dial.level(for: s).rawValue, want, "default for \(s.key)")
        }
        XCTAssertEqual(Set(c["families"]?.array?.compactMap(\.string) ?? []), Set(ControlSurface.families))
        // The one that drifted: a valid host file carrying the credit-line dial decodes, shows,
        // and sets — and an unconfigured captain is OFFERED the borrow, not committed to it.
        let hostFile = Data(#"{"market.margin":"confirm","navigation.rescue":"auto"}"#.utf8)
        let d = try AutonomyDial.decode(hostFile)
        XCTAssertEqual(d.level(for: .marketMargin), .confirm)
        XCTAssertEqual(d.level(for: .navigationRescue), .auto)
        XCTAssertEqual(AutonomyDial().level(for: .marketMargin), .advise)
        XCTAssertEqual(AutonomyDial(settings: ["*": .auto]).level(for: .marketMargin), .auto, "a captain who set * has spoken")
        XCTAssertEqual(AutonomyDial(settings: ["market": .confirm]).level(for: .marketMargin), .confirm)
        var setDial = AutonomyDial()
        XCTAssertNil(setDial.set("market.margin", .auto))
        XCTAssertEqual(ControlSurface.marketMargin.automation, "trade")
    }

    // MARK: finding 3 — the captain's identity

    func testTheCaptainIdRidesTheRecordAndIsEmptyOnlyForALegacyOne() throws {
        let modern = Data(#"{"captain_id":"c-7f3a9","captain":"A/B","key_id":"0123abcd","server":"http://x","automations":[],"paired_at":1}"#.utf8)
        let m = try JSONCoding.decode(Captain.self, from: modern)
        XCTAssertEqual(m.captainID, "c-7f3a9"); XCTAssertEqual(m.captain, "A/B")
        let legacy = try Fixtures.store.captain()   // the fixture store predates the field
        XCTAssertEqual(legacy.captainID, "")
        XCTAssertEqual(legacy.captain, "A. Captain")
    }

    func testCaptainScopedJoinsKeyOnTheIdNeverTheLabel() {
        func ship(_ world: String, captain: String, id: String) -> ShipSummary {
            var s = ShipSummary(world: world, label: world, computer: "Felix", named: true, hull: "", captain: captain, server: "", automations: [], pilotAlive: false, reachable: true, mood: .steady, openProposals: 0)
            s.captainID = id
            return s
        }
        // Two captains who share a display name are two captains once either has an id.
        let a = ship("w1", captain: "A B", id: "c-1"), b = ship("w2", captain: "A/B", id: "c-2"), c = ship("w3", captain: "A B", id: "c-1")
        XCTAssertNotEqual(a.captainIdentity, b.captainIdentity)
        XCTAssertEqual(a.captainIdentity, c.captainIdentity, "one captain, two hulls")
        // A rename does not move the captain: same id, new label, same identity.
        let renamed = ship("w1", captain: "Luke SkyWhisker", id: "c-1")
        XCTAssertEqual(renamed.captainIdentity, a.captainIdentity)
        // Legacy records (no id) still group, by label, and are marked as such.
        let l1 = ship("w4", captain: "Old Salt", id: ""), l2 = ship("w5", captain: "Old Salt", id: "")
        XCTAssertEqual(l1.captainIdentity, l2.captainIdentity)
        XCTAssertTrue(l1.captainIdentity.hasPrefix("label:"))
        XCTAssertNotEqual(l1.captainIdentity, ship("w6", captain: "Old Salt", id: "c-9").captainIdentity, "an id outranks a matching label")
    }

    func testTheWireRowCarriesTheCaptainId() throws {
        let row: JSONValue = .object(["world": .string("kk2"), "label": .string("KK II"), "captain": .string("Luke SkyWhisker"), "captain_id": .string("c-42"),
                                      "captain_brief": .string("/captains/c-42/brief"), "server": .string("https://x"), "automations": .array([])])
        let s = try XCTUnwrap(WireFeed.summary(from: row, tick: 100))
        XCTAssertEqual(s.captainID, "c-42")
        XCTAssertEqual(s.captainIdentity, "id:c-42")
        XCTAssertEqual(WireFeed.captainBriefPath(row: row, captainName: "Luke SkyWhisker"), "captains/c-42/brief", "the host-built route, root-relative as the client joins it")
    }

    // MARK: finding 4 — the journal vocabulary

    func testEveryRunnerEventIsClassifiedDeliberately() throws {
        let events = try XCTUnwrap(try contract("journal-events")["events"]?.array?.compactMap(\.string))
        XCTAssertGreaterThan(events.count, 30)
        for name in events {
            let e = JournalEntry(at: 0, tick: 1, event: name, fields: [:])
            let sev = TemplatedVoice.severity(e)
            // Every word the runner writes has a deliberate rank: chatter (0) or a listed
            // tier — never the "unknown" fallback of 2 (a word nobody classified).
            XCTAssertTrue(sev == 0 || sev >= 3, "\(name) falls through to the unknown rank (\(sev)) — classify it")
            XCTAssertEqual(sev == 0, TemplatedVoice.chatter.contains(name), "\(name): chatter and severity 0 must agree")
            // And every word that is TOLD renders as a fact, not as the neutral key-sorted
            // payload dump; chatter is folded into a count and never told one by one.
            if sev > 0 {
                let fact = TemplatedVoice(persona: Persona(name: "Felix", style: nil)).fact(for: e)
                XCTAssertFalse(fact.hasSuffix(": \(name)") || fact.contains("\(name) {"), "\(name) has no fact renderer: \(fact)")
            }
        }
    }

    func testMoneyAndRefusalEventsReachTheCaptain() {
        let paid = JournalEntry(at: 10, tick: 500, event: "paid-down", fields: ["amount": .number(1200), "owed_before": .number(21400), "credits": .number(7000), "at_station": .string("cannery-row"), "resolves": .number(501)])
        let payRefused = JournalEntry(at: 11, tick: 501, event: "pay-down-refused", fields: ["amount": .number(1200), "why": .string("action refused: HTTP 409 nothing owed")])
        let tradeRefused = JournalEntry(at: 12, tick: 502, event: "trade-refused", fields: ["side": .string("buy"), "good": .string("catnip"), "why": .string("action refused: HTTP 402 insufficient credits")])
        let forecast = JournalEntry(at: 13, tick: 503, event: "forecast", fields: ["horizon_ticks": .number(120), "starving": .array([.string("tinplate@tranquility")])])
        let routine = JournalEntry(at: 14, tick: 504, event: "acted", fields: ["decision": .string("Travel"), "credits": .number(7000), "fuel": .number(400), "resolves": .number(510)])

        let notices = NoticePolicy.notices(for: [paid, payRefused, tradeRefused, forecast, routine])
        XCTAssertEqual(notices.map(\.kind), [.money, .distress, .distress], "paid-down is money; both refusals are distress; a forecast and a routine act are not notices")
        XCTAssertEqual(notices[0].title, "Paid down ℳ1200 on the lease")
        XCTAssertTrue(notices[0].body.contains("owed ℳ21400 before") && notices[0].body.contains("t500"))
        XCTAssertEqual(notices[1].title, "Lease payment refused")
        XCTAssertEqual(notices[2].title, "Trade refused at the door")
        XCTAssertTrue(notices[2].body.contains("buy catnip") && notices[2].body.contains("402"))

        // Ranking: a refusal at the door outranks routine and money; money outranks routine.
        XCTAssertEqual(TemplatedVoice.severity(tradeRefused), TemplatedVoice.severity(JournalEntry(at: 0, tick: 0, event: "refused-at-the-door", fields: [:])))
        XCTAssertGreaterThan(TemplatedVoice.severity(payRefused), TemplatedVoice.severity(paid))
        XCTAssertGreaterThan(TemplatedVoice.severity(paid), TemplatedVoice.severity(routine))
        XCTAssertTrue(TemplatedVoice.isDanger(tradeRefused) && TemplatedVoice.isDanger(payRefused))
        XCTAssertFalse(TemplatedVoice.isDanger(paid))

        // The voice says each from its own numbers.
        let v = TemplatedVoice(persona: Persona(name: "Felix", style: nil))
        XCTAssertEqual(v.fact(for: paid), "t500: paid ℳ1200 down on the lease at cannery-row (owed ℳ21400 before) — ℳ7000")
        XCTAssertEqual(v.fact(for: payRefused), "t501: lease payment of ℳ1200 refused — action refused: HTTP 409 nothing owed")
        XCTAssertEqual(v.fact(for: tradeRefused), "t502: buy catnip refused at the door — action refused: HTTP 402 insufficient credits")
        XCTAssertEqual(v.fact(for: forecast), "t503: forecast over 120 ticks — running dry: tinplate@tranquility")

        // Under a full window the refusals and the payment survive; routine is what gets folded.
        var window: [JournalEntry] = [paid, payRefused, tradeRefused]
        for i in 0..<30 { window.append(JournalEntry(at: Int64(100 + i), tick: Int64(600 + i), event: "acted", fields: ["decision": .string("Travel"), "credits": .number(1), "fuel": .number(1), "resolves": .number(1)])) }
        let told = v.report(entries: window).facts.joined(separator: "\n")
        XCTAssertTrue(told.contains("lease payment of ℳ1200 refused") && told.contains("buy catnip refused") && told.contains("paid ℳ1200 down"), told)
    }

    func testAFutureRefusalIsDistressAndAFutureRoutineWordIsNeutral() {
        let future = JournalEntry(at: 1, tick: 9, event: "escort-refused", fields: ["why": .string("no escort berthed here")])
        XCTAssertEqual(TemplatedVoice.severity(future), 8)
        XCTAssertTrue(TemplatedVoice.isDanger(future))
        let n = NoticePolicy.notices(for: [future])
        XCTAssertEqual(n.map(\.kind), [.distress]); XCTAssertEqual(n[0].title, "Escort Refused"); XCTAssertEqual(n[0].body, "no escort berthed here")
        let mild = JournalEntry(at: 2, tick: 10, event: "sighted-comet", fields: ["name": .string("Halley")])
        XCTAssertEqual(TemplatedVoice.severity(mild), 2)
        XCTAssertFalse(TemplatedVoice.isDanger(mild))
        XCTAssertTrue(NoticePolicy.notices(for: [mild]).isEmpty, "an unknown routine word is never a buzz")
    }
}
