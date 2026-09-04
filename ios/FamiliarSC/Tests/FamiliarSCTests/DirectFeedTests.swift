import XCTest
@testable import FamiliarSC
@testable import FamiliarSCUI

/// Direct mode, pure parts, plus one live pass against a dev world when `UCF_LOCAL` names it
/// (e.g. `UCF_LOCAL=http://127.0.0.1:7877 swift test`): enrol a pilot, read the hull, compute
/// the fuel picture. Skipped, loudly, when no dev world is offered.
final class DirectFeedTests: XCTestCase {
    func testJournalFromTheWireKeepsTheLedgerVerbatim() throws {
        let me = try ExchangeWire.me(Fixtures.wire("me"))
        let receipts = try ExchangeWire.receipts(Fixtures.wire("receipts"))
        let j = DirectFeed.journal(from: me, receipts: receipts)
        XCTAssertEqual(j.count, 55 + 1)
        XCTAssertEqual(j.map { $0.tick ?? 0 }, j.map { $0.tick ?? 0 }.sorted())
        let booked = try XCTUnwrap(j.first { $0.string("load") == "L3083" && $0.string("why") == "booked" })
        XCTAssertEqual(booked.event, "freight"); XCTAssertEqual(booked.int("credits_paid"), 97)
        let v = TemplatedVoice(persona: Persona(name: "Felix", style: nil))
        XCTAssertEqual(v.fact(for: booked), "t7249: booked — ℳ97 paid [L3083]")
        let sale = try XCTUnwrap(j.first { $0.event == "trade-outcome" })
        XCTAssertEqual(v.fact(for: sale), "t7434: sell 53 bluefin-reserve: filled")
        XCTAssertEqual(KnownExchange.name(for: KnownExchange.prod), "PROD")
        XCTAssertEqual(KnownExchange.name(for: "http://127.0.0.1:7877"), "LOCAL")
        XCTAssertNil(DirectFeed(exchange: "", key: "ucfk_x"))
    }

    func testBurnRungsMatchThePilotAndTheFold() throws {
        // titania-cold-store → foxys-diner as PROD quoted it 2026-09-04: 168 fuel at the reference drive.
        let route = try ExchangeWire.route(Fixtures.wire("route-titania-foxys"))
        let legs = route.legs.compactMap { $0.distanceKm.map { Int64($0) } }
        XCTAssertEqual(legs.count, 2); XCTAssertEqual(route.fuel, 168)
        XCTAssertTrue(BurnRungs.modelAgrees(legsKm: legs, quotedAtReference: 168), "the shipped constants describe this world")
        // The day KK flew out (2026-09-04): legs 3,491,917,000 + 1,774,626 km, hull 189 mG (wear 0,
        // effectiveAccelMilliG straight off /v1/me — the one number with everything applied).
        // Standard 168 = the exchange's own quote; economy 112 = what the fold charged, leg one
        // 106 to the unit (the tank went 135 → 29). Wildhorse's arithmetic and this one agree.
        let day: [Int64] = [3_491_917_000, 1_774_626]
        XCTAssertEqual(BurnRungs.routeFuel(legsKm: day, hullAccelMilliG: 189, burnBps: BurnRungs.standardBps), 168)
        XCTAssertEqual(BurnRungs.routeFuel(legsKm: day, hullAccelMilliG: 189, burnBps: BurnRungs.economyBps), 112)
        XCTAssertEqual(BurnRungs.legFuel(distanceKm: day[0], accelMilliG: 94), 106)
        let plan = BurnRungs.plan(legsKm: day, quotedAtReference: 168, hullAccelMilliG: 189, tank: 135)
        XCTAssertEqual(plan, BurnRungs.Plan(burn: "economy", fuel: 112, reaches: true))
        // A derated hull (178 mG, wear 1094 bps) is priced off ITS drive while the reference check stays at 189.
        XCTAssertEqual(BurnRungs.routeFuel(legsKm: day, hullAccelMilliG: 178, burnBps: BurnRungs.economyBps), 109)
        XCTAssertTrue(BurnRungs.modelAgrees(legsKm: day, quotedAtReference: 168))
        let full = BurnRungs.plan(legsKm: legs, quotedAtReference: 168, hullAccelMilliG: 189, tank: 600)
        XCTAssertEqual(full.burn, "standard", "a healthy tank flies the throttle it always flew"); XCTAssertTrue(full.reaches)
        let dry = BurnRungs.plan(legsKm: legs, quotedAtReference: 168, hullAccelMilliG: 189, tank: 20)
        XCTAssertEqual(dry.burn, "economy"); XCTAssertFalse(dry.reaches)
        let foreign = BurnRungs.plan(legsKm: legs, quotedAtReference: 1000, hullAccelMilliG: 189, tank: 135)
        XCTAssertEqual(foreign, BurnRungs.Plan(burn: "standard", fuel: 1000, reaches: false), "a world the model does not describe gets only the quote")
    }

    func testDevicePersonaRoundTrip() throws {
        let store = DevicePersonaStore(defaults: UserDefaults(suiteName: "sc-direct-\(UUID().uuidString)")!)
        XCTAssertNil(store.load(keyID: "abcd1234"))
        store.save(Persona(name: "Felix", style: Style()), keyID: "abcd1234")
        XCTAssertEqual(store.load(keyID: "abcd1234")?.name, "Felix")
    }

    func testLiveDevWorldEnrolReadAndFuel() async throws {
        guard let world = ProcessInfo.processInfo.environment["UCF_LOCAL"], !world.isEmpty else {
            throw XCTSkip("no dev world offered — set UCF_LOCAL=http://127.0.0.1:7877 to run this pass")
        }
        let r = try await DirectFeed.enrol(exchange: world, traderName: "Felix Test \(Int(Date().timeIntervalSince1970) % 10000)", deviceID: "test-\(UUID().uuidString)")
        XCTAssertTrue(r.key.hasPrefix("ucfk_"))
        let feed = try XCTUnwrap(DirectFeed(exchange: world, key: r.key))
        let ships = try await feed.ships()
        XCTAssertEqual(ships.count, 1)
        let s = ships[0]
        XCTAssertEqual(s.worldName, "LOCAL"); XCTAssertFalse(s.pilotAlive); XCTAssertEqual(s.computer, Persona.rootName)
        XCTAssertNotNil(s.credits); XCTAssertNotNil(s.fuel)
        let (frame, docs) = try await feed.context(world: s.world, worldInstance: s.worldInstance)
        XCTAssertTrue(frame?.contains("direct to the exchange") ?? false)
        let fuel = try XCTUnwrap(docs.first { $0.name == "fuel" })
        XCTAssertTrue(fuel.text.hasPrefix("Fuel aboard:"))
        XCTAssertTrue(fuel.text.contains("Pump "), fuel.text)
        _ = try await feed.rename(world: s.world, computer: "Felix")
        let named = try await feed.persona(world: s.world)?.name
        XCTAssertEqual(named, "Felix")
        print("LIVE direct mode on \(world): \(s.hull) · \(s.sentence)\n\(fuel.text)")
    }
}
