import XCTest
@testable import FamiliarSC
@testable import FamiliarSCUI

/// The bridge's model and feeds, headless: the store feed's summary over the fixture
/// store, the fold windows, the fixture feed's captain acts, and the notifier's dedupe.
final class UITests: XCTestCase {
    func testStoreFeedSummarisesAPairedShip() async throws {
        let feed = StoreFeed(worlds: Fixtures.ship.deletingLastPathComponent())
        let ships = try await feed.ships()
        let s = try XCTUnwrap(ships.first { $0.world == "ship" })
        XCTAssertEqual(s.computer, "Purr"); XCTAssertTrue(s.named)
        XCTAssertEqual(s.hull, "Fixture Freighter"); XCTAssertEqual(s.captain, "A. Captain")
        XCTAssertEqual(s.credits, 1200, "the last acted/holding line's credits")
        XCTAssertEqual(s.mood, .concerned); XCTAssertEqual(s.moodWord, "worried")
        XCTAssertFalse(s.sentence.isEmpty)
        XCTAssertEqual(s.automations, ["freight", "trade", "outfit"])
        let w = try await feed.window(world: "ship")
        XCTAssertEqual(w.count, 3)
        let d = try await feed.dial(world: "ship")
        XCTAssertEqual(d.loaded.dial.level(for: .marketBuy), .confirm)
        let b = try await feed.book(world: "ship")
        XCTAssertEqual(b.hauls, 2); XCTAssertEqual(b.freightPaid, 444); XCTAssertEqual(b.inventoryAtCost, 600)
    }

    func testFoldWindowsAreNewestFirstAndChronologicalInside() {
        let j = Fixtures.journal().entries
        let folds = BridgeModel.fold(journal: j, persona: nil, windowTicks: 50, count: 10, openProposals: 1)
        XCTAssertEqual(folds.first?.toTick, 251)
        XCTAssertEqual(folds.map(\.toTick), folds.map(\.toTick).sorted(by: >))
        XCTAssertEqual(folds.first?.report.mood, .concerned)
        XCTAssertTrue(folds.first!.report.headline.contains("1 proposal waiting") == false, "the first window's mood is concerned: distress outranks the proposal")
        XCTAssertTrue(folds.allSatisfy { !$0.report.facts.isEmpty })
    }

    func testFixtureFeedCaptainActsRoundTrip() async throws {
        let feed = FixtureFeed()
        let model = BridgeModel(feed: feed, acts: feed)
        await model.refreshShips()
        XCTAssertEqual(model.ships.map(\.computer), ["Purr", "(unnamed — `fleet rename` her)"])
        XCTAssertEqual(model.ships[0].openProposals, 1)
        await model.open(world: "world-fixture-purr")
        XCTAssertEqual(model.openProposals, 1)
        XCTAssertEqual(model.persona?.name, "Purr")
        XCTAssertFalse(model.reports.isEmpty)
        await model.approve(id: "p-fedcba9876543210", approved: true)
        XCTAssertEqual(model.openProposals, 0)
        guard case .proposal(_, _, _, _, let st) = model.window.last!.kind, case .approved = st else { return XCTFail("approved") }
        var d = model.dial!.loaded.dial
        XCTAssertNil(d.set("market", .advise))
        await model.save(dial: d)
        XCTAssertEqual(model.dial?.loaded.dial.level(for: .marketSell), .advise)
        XCTAssertEqual(model.dial?.loaded.dial.level(for: .marketBuy), .confirm, "the category still wins")
        let err = await model.pair(PairingRequest(label: "x", captain: "y", server: "https://e.example", automations: [.freight]), key: PairingKey(secret: "ucfk_0123abcdEFGHijkl_mnop"))
        XCTAssertTrue(err?.contains("needs the ship's host") ?? false, "a fixture cannot pair; it says what the host must run")
    }

    func testShipSettingsActsRoundTripOnTheFixture() async {
        let feed = FixtureFeed()
        let model = BridgeModel(feed: feed, acts: feed)
        await model.refreshShips()
        await model.open(world: "world-fixture-old")
        let e1 = await model.rename(computer: "Felix"); XCTAssertTrue(e1.ok)
        XCTAssertEqual(model.summary?.computer, "Felix"); XCTAssertTrue(model.summary?.named ?? false)
        let e2 = await model.setAutomations([.freight, .trade]); XCTAssertTrue(e2.ok)
        XCTAssertEqual(e2.text, "granted; she picks it up on her next start", "a grant is not live until the pilot restarts")
        XCTAssertEqual(Set(model.summary?.automations ?? []), ["freight", "trade"])
        XCTAssertEqual(Set(model.dial?.bought ?? []), ["freight", "trade"], "the dial's bought set follows")
        let e3 = await model.setCaptain("Luke SkyWhisker"); XCTAssertTrue(e3.ok)
        XCTAssertEqual(model.summary?.captain, "Luke SkyWhisker")
    }

    func testCancellationIsNotAnErrorAndErrorsRead() {
        XCTAssertTrue(BridgeModel.isCancellation(CancellationError()))
        XCTAssertTrue(BridgeModel.isCancellation(URLError(.cancelled)))
        XCTAssertTrue(BridgeModel.isCancellation(NSError(domain: NSURLErrorDomain, code: NSURLErrorCancelled)))
        XCTAssertFalse(BridgeModel.isCancellation(URLError(.notConnectedToInternet)))
        XCTAssertFalse(BridgeModel.isCancellation(FeedError.refused("HTTP 401")))
        XCTAssertEqual(BridgeModel.describe(FeedError.refused("no bearer (HTTP 401)")), "no bearer (HTTP 401)")
        // A URLSession error carries its own sentence; a bare URLError() in a test does not, so
        // pin only that the dump form ("Error Domain=… Code=… UserInfo={…}") never appears.
        XCTAssertFalse(BridgeModel.describe(URLError(.cannotConnectToHost)).hasPrefix("Error Domain="), "a sentence, not an NSError dump")
    }

    func testFuelBriefRendersWhatSheCanSay() throws {
        let f = try JSONDecoder().decode(JSONValue.self, from: Data(#"{"fuel":135,"capacity":600,"docked":"titania-cold-store","credits":0,"fill_price_here":930,"stranded":true,"can_reach":[],"pumps":[{"station":"foxys-diner","ticks":66,"fuel_cost":167,"reachable":false,"short_by":65,"fill_price":930,"affordable":false,"here":false}],"saleable_here":[{"good":"bluefin-reserve","units":114,"bid":58,"will_take":0,"worth":0}],"tanker":{"available":true,"pilot_will_call":false,"why":"a PAWS call-out   pins the hull"},"if_stranded":"sell what this berth will take,  ask another captain"}"#.utf8))
        let text = Briefs.fuel(f)
        XCTAssertTrue(text.contains("Fuel aboard: 135 of 600. Berthed at titania-cold-store."))
        XCTAssertTrue(text.contains("She is STRANDED"))
        XCTAssertTrue(text.contains("Pump foxys-diner: 66 ticks away, burns 167 fuel to reach, NOT reachable — short by 65 fuel; a fill there costs ℳ930 (not affordable)."))
        XCTAssertTrue(text.contains("This berth will not take the 114 bluefin-reserve aboard (bid 58, takes 0)."))
        XCTAssertTrue(text.contains("Tanker: available; the pilot will NOT call it on her own. Why: a PAWS call-out pins the hull"))
        XCTAssertTrue(text.contains("Ways out when stranded: sell what this berth will take, ask another captain."))
        let b = try JSONDecoder().decode(JSONValue.self, from: Data(#"{"context":{"kind":"ship","hull":"Kibble Klipper","captain":"Luke SkyWhisker","computer":"Felix","world":"w"},"aboard":{"units":{"bluefin-reserve":114},"cost":15960},"dial":{"*":"confirm","freight":"confirm"},"open_proposals":[],"standing_advice":[{"event":"distress-hold","what":"low fuel, no affordable pump","since_tick":7717,"times":17}],"recent":[{"tick":7717,"event":"book-corrected","why":"adopted"}]}"#.utf8))
        XCTAssertEqual(Briefs.frame(fromBrief: b, worldInstance: "PROD"), "ship, hull Kibble Klipper (PROD), captain Luke SkyWhisker, computer Felix")
        let brief = Briefs.brief(b)
        XCTAssertTrue(brief.contains("Aboard: 114 bluefin-reserve (cost ℳ15960)."))
        XCTAssertTrue(brief.contains("Autonomy dial: everything on confirm."))
        XCTAssertTrue(brief.contains("Standing (distress hold): low fuel, no affordable pump (since t7717, said 17 times)."), brief)
        // The floor answer to "how do I refuel?" comes from the fuel document, not a shrug.
        let ctx = BridgeContext(entries: [], openProposals: 0, frame: "ship", documents: [ContextDocument(name: "fuel", title: "fuel", text: text)])
        let conv = Conversation(voice: BridgeVoice(persona: Persona(name: "Felix", style: nil)), context: ctx)
        let answer = conv.floorAnswer("how do I refuel")
        XCTAssertTrue(answer.contains("Fuel aboard: 135 of 600"), answer)
        XCTAssertTrue(answer.contains("Pump foxys-diner"), answer)
        XCTAssertTrue(Grounding.tokens(in: ctx.truth(floor: BridgeVoice(persona: Persona(name: "Felix", style: nil)).floor(ctx))).contains("foxys-diner"))
    }

    func testNotifierDeliversEachNoticeOnce() {
        let defaults = UserDefaults(suiteName: "sc-tests-\(UUID().uuidString)")!
        let n = CaptainNotifier(defaults: defaults)
        let notices = NoticePolicy.notices(for: Fixtures.journal().entries)
        XCTAssertEqual(n.fresh(notices, world: "w").count, notices.count)
        XCTAssertEqual(n.fresh(notices, world: "w").count, 0)
        XCTAssertEqual(n.fresh(notices, world: "w2").count, notices.count, "another ship's notices are their own")
    }

    func testTradeBookCaveatsFromTheRow() throws {
        let row = try JSONDecoder().decode(JSONValue.self, from: Data(#"{"world":"w","computer":"Felix","hull":"","captain":"","server":"","automations":[],"trades":{"filled":3,"rejected":1,"realized":5583,"cost_of_sold":0,"margin_pct":0,"inventory_cost":860,"inventory":[],"unmatched_units":116,"unmatched_proceeds":1453,"quoted_basis_lots":1,"closed_positions":2,"expected_margin":3937,"realized_on_closed":5318}}"#.utf8))
        let s = try XCTUnwrap(WireFeed.summary(from: row, tick: nil))
        let t = try XCTUnwrap(s.trades)
        XCTAssertEqual(t.realized, 5583); XCTAssertEqual(t.filled, 3); XCTAssertEqual(t.inventoryCost, 860)
        XCTAssertEqual(t.caveat, "ℳ1453 from 116 unmatched units set aside; 1 lot at a quoted basis, so the profit is a ceiling")
        XCTAssertEqual(t.estimatesLine, "estimates: 2 closed, promised ℳ3937, returned ℳ5318")
        XCTAssertNil(TradeBook().caveat); XCTAssertNil(TradeBook().estimatesLine)
        let clean = try JSONDecoder().decode(JSONValue.self, from: Data(#"{"world":"w","computer":"Felix","hull":"","captain":"","server":"","automations":[]}"#.utf8))
        XCTAssertNil(try XCTUnwrap(WireFeed.summary(from: clean, tick: nil)).trades, "no trades block on the row means no card")
    }

    func testWireSummaryReadsAFleetStatusRow() throws {
        let row = try JSONDecoder().decode(JSONValue.self, from: Data(#"{"world":"world-1","label":"KK II","computer":"Purr","hull":"Kibble Klipper II","captain":"ian","server":"https://x","automations":["freight","trade"],"pilot_pid":123,"lease_expires_in_h":20,"credits":7132,"debt":21400,"fuel":166,"wearBps":1104,"docked":null,"reachable":true,"last_event":"holding","last_at":1}"#.utf8))
        let s = try XCTUnwrap(WireFeed.summary(from: row, tick: 7532))
        XCTAssertEqual(s.computer, "Purr"); XCTAssertTrue(s.named); XCTAssertTrue(s.pilotAlive)
        XCTAssertEqual(s.leaseHoursLeft, 20); XCTAssertEqual(s.credits, 7132); XCTAssertNil(s.docked)
        XCTAssertEqual(s.shipName, "Kibble Klipper II", "the hull's name, never the world's")
        XCTAssertEqual(s.worldInstance, "x", "no world_name served → the exchange host")
        var local = s; local.server = "http://127.0.0.1:7877"; XCTAssertEqual(local.worldInstance, "LOCAL")
        var named = s; named.worldName = "PROD"; XCTAssertEqual(named.worldInstance, "PROD")
        let old = try JSONDecoder().decode(JSONValue.self, from: Data(#"{"world":"w2","computer":"(unnamed — `fleet rename` her)","hull":"","captain":"","server":"","automations":[]}"#.utf8))
        XCTAssertFalse(try XCTUnwrap(WireFeed.summary(from: old, tick: nil)).named)
        // The live rows carry the name in `persona` (null until named) and no `computer` field.
        let felix = try JSONDecoder().decode(JSONValue.self, from: Data(#"{"world":"w3","label":"KK II (PROD)","hull":"","captain":"","server":"","automations":[],"persona":{"persona_version":2,"name":"Felix","style":{}}}"#.utf8))
        let f = try XCTUnwrap(WireFeed.summary(from: felix, tick: nil))
        XCTAssertEqual(f.computer, "Felix"); XCTAssertTrue(f.named)
        let unnamed = try JSONDecoder().decode(JSONValue.self, from: Data(#"{"world":"w4","label":"soak","hull":"","captain":"","server":"","automations":[],"persona":null}"#.utf8))
        let u = try XCTUnwrap(WireFeed.summary(from: unnamed, tick: nil))
        XCTAssertFalse(u.named); XCTAssertEqual(u.computer, "(unnamed — `fleet rename` her)")
    }
}
