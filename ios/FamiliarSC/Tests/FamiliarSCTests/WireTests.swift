import XCTest
@testable import FamiliarSC

/// The /v1 wire, decoded from captures of PROD (scrubbed) — the shapes are the exchange's,
/// not this package's reading of them.
final class WireTests: XCTestCase {
    func testStatusAndMe() throws {
        let s = try ExchangeWire.status(Fixtures.wire("status"))
        XCTAssertEqual(s.tick, 7532); XCTAssertEqual(s.tickDurationSec, 180); XCTAssertEqual(s.worldName, "PROD")
        let me = try ExchangeWire.me(Fixtures.wire("me"))
        XCTAssertEqual(me.credits, 7132); XCTAssertEqual(me.debt, 21400)
        XCTAssertNil(me.docked); XCTAssertEqual(me.enRouteTo, "foxys-diner"); XCTAssertTrue(me.underWay)
        XCTAssertTrue(me.leased, "not titled, lease principal outstanding")
        XCTAssertEqual(me.fittings, ["drive-tune"]); XCTAssertEqual(me.effectiveAccelMilliG, 205)
        XCTAssertEqual(me.contract?.loadId, "L3249"); XCTAssertEqual(me.contract?.status, "inTransit")
        XCTAssertNil(me.pendingActions, "the pending overlay is absent when nothing is in flight")
        let f = try XCTUnwrap(me.freight)
        XCTAssertEqual(f.count, 55)
        XCTAssertEqual(f[2].event, "booked"); XCTAssertEqual(f[2].loadId, "L3083"); XCTAssertEqual(f[2].freightPaid, 97)
        XCTAssertEqual(f[0].outcome, "serviced")
    }

    func testPendingOverlayWhenPresent() throws {
        let json = #"{"credits":1,"pendingActions":[{"loadId":"L9","verb":"book","resolvesAtTick":7533}]}"#
        let me = try ExchangeWire.me(Data(json.utf8))
        XCTAssertEqual(me.pendingActions?.first, PendingAction(verb: "book", loadId: "L9", resolvesAtTick: 7533))
    }

    func testLoadboardBothViews() throws {
        let open = try ExchangeWire.loads(Fixtures.wire("loadboard-open"))
        XCTAssertEqual(open.count, 8)
        XCTAssertEqual(open[0].loadId, "L3294"); XCTAssertEqual(open[0].serviceClass, "economy"); XCTAssertEqual(open[0].classBps, 5_000)
        XCTAssertEqual(open[0].pilotTicks, 18 + 23 + 16, "loading 7 floors to 8 at both ends")
        XCTAssertEqual(open[0].mine, false)
        let mine = try ExchangeWire.loads(Fixtures.wire("loadboard-mine"))
        XCTAssertEqual(mine[0].status, "delivered"); XCTAssertEqual(mine[0].mine, true)
    }

    func testQuotesGalaxyBothShapesReceipts() throws {
        let q = try ExchangeWire.quotes(Fixtures.wire("quotes"))
        XCTAssertEqual(q.station, "foxys-diner"); XCTAssertEqual(q.goods.count, 2)
        XCTAssertEqual(q.goods[0].ask, 145); XCTAssertEqual(q.goods[0].maxSellUnits, 52); XCTAssertEqual(q.goods[0].capacity, 260)
        let bare = try ExchangeWire.galaxy(Fixtures.wire("galaxy-prices"))
        XCTAssertEqual(bare.rows.count, 91); XCTAssertEqual(bare.unsurveyed, [])
        XCTAssertEqual(bare.rows[0], GalaxyPrice(good: "biscuit-substrate", station: "cannery-row", mid: 34, stock: 48))
        let wrapped = try ExchangeWire.galaxy(Fixtures.wire("galaxy-prices-wrapped"))
        XCTAssertEqual(wrapped.rows.count, 5); XCTAssertEqual(wrapped.unsurveyed, ["triton-outpost"])
        let r = try ExchangeWire.receipts(Fixtures.wire("receipts"))
        XCTAssertEqual(r.count, 1); XCTAssertEqual(r[0].side, "sell"); XCTAssertEqual(r[0].total, 5583); XCTAssertEqual(r[0].outcome, "filled")
    }

    func testRouteStationsProfileReferenceAck() throws {
        let route = try ExchangeWire.route(Fixtures.wire("route"))
        XCTAssertEqual(route.legs.count, 2); XCTAssertEqual(route.fuel, 165); XCTAssertEqual(route.ticks, 65)
        XCTAssertEqual(route.legs[0].assistBody, "jupiter"); XCTAssertEqual(route.driveAccelG, 0.189)
        let st = try ExchangeWire.stations(Fixtures.wire("stations"))
        XCTAssertEqual(st.count, 17); XCTAssertEqual(st[0].id, "cannery-row"); XCTAssertEqual(st[0].sellsFuel, false)
        let p = try ExchangeWire.profile(Fixtures.wire("profile"))
        XCTAssertEqual(p.traderName, "Luke SkyWhisker"); XCTAssertEqual(p.stats?.deliveries, 4)
        let ref = try ExchangeWire.reference(Fixtures.wire("reference"))
        XCTAssertEqual(ref.tickSeconds, 180); XCTAssertEqual(ref.ticksPerDay, 288)
        XCTAssertEqual(ref.recipes?.first?.inputs["grain"], 22); XCTAssertEqual(ref.recipes?.first?.ticksPerCycle, 8)
        XCTAssertEqual(ref.bodies?.first?.id, "sol"); XCTAssertEqual(ref.goods?.first?.decayBps, 4)
        XCTAssertNotNil(ref.params?["mortgagePrincipal"])
        let ack = try ExchangeWire.ack(Fixtures.wire("ack"))
        XCTAssertEqual(ack, ActionAck(actionId: "act-000001", receivedSeq: 42, resolvesAtTick: 7533))
    }

    func testClientRequestShapeAndNoWriteSurface() {
        let c = ExchangeClient(server: "https://exchange.example/", key: "ucfk_secret")!
        let r = c.request("/v1/me")
        XCTAssertEqual(r.url?.absoluteString, "https://exchange.example/v1/me")
        XCTAssertEqual(r.httpMethod, "GET")
        XCTAssertEqual(r.value(forHTTPHeaderField: "Authorization"), "Bearer ucfk_secret")
        XCTAssertEqual(r.value(forHTTPHeaderField: "X-UCF-App"), "familiar-sc")
        XCTAssertNil(ExchangeClient(server: "", key: "k"))
    }
}
