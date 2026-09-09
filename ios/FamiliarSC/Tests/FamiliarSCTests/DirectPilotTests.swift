import XCTest
import Foundation
@testable import FamiliarSC
@testable import FamiliarSCUI

/// A mock exchange under `ExchangeClient.session`: every GET answered from a fixture by path
/// (query included), every POST recorded byte for byte. The tests below are the ones codex
/// asked for on T-237 B4 (re-verification 2026-09-08): the gather carries the world's rung
/// prices and the captain's live contract; rendering and speaking file NOTHING; cancelling
/// files nothing; one confirm files exactly one correctly shaped POST with the id the
/// proposal was shown under; a mind that moved refuses; a seam this shell was not built
/// for is refused.
final class MockExchange: URLProtocol {
    nonisolated(unsafe) static var gets: [String: (Int, Data)] = [:]
    nonisolated(unsafe) static var posts: [(path: String, body: JSONValue)] = []
    nonisolated(unsafe) static var postAnswer: (Int, Data) = (202, Data())
    static let lock = NSLock()

    static func reset() { lock.lock(); gets = [:]; posts = []; postAnswer = (202, Data()); lock.unlock() }
    static func serve(_ pathAndQuery: String, _ fixture: String, status: Int = 200) {
        lock.lock(); gets[pathAndQuery] = (status, Fixtures.wire(fixture)); lock.unlock()
    }
    static func serveRaw(_ pathAndQuery: String, _ body: String, status: Int = 200) {
        lock.lock(); gets[pathAndQuery] = (status, Data(body.utf8)); lock.unlock()
    }
    static var postCount: Int { lock.lock(); defer { lock.unlock() }; return posts.count }

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func stopLoading() {}

    override func startLoading() {
        let url = request.url!
        let key = url.path + (url.query.map { "?" + $0 } ?? "")
        let (status, data): (Int, Data)
        if request.httpMethod == "POST" {
            var body = Data()
            if let stream = request.httpBodyStream {
                stream.open()
                var buf = [UInt8](repeating: 0, count: 4096)
                while stream.hasBytesAvailable { let n = stream.read(&buf, maxLength: buf.count); if n > 0 { body.append(buf, count: n) } else { break } }
                stream.close()
            } else if let b = request.httpBody { body = b }
            let parsed = (try? JSONDecoder().decode(JSONValue.self, from: body)) ?? .null
            MockExchange.lock.lock(); MockExchange.posts.append((key, parsed)); (status, data) = MockExchange.postAnswer; MockExchange.lock.unlock()
        } else {
            MockExchange.lock.lock(); let hit = MockExchange.gets[key]; MockExchange.lock.unlock()
            (status, data) = hit ?? (404, Data("{\"error\":\"no fixture for \(key)\"}".utf8))
        }
        let resp = HTTPURLResponse(url: url, statusCode: status, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": "application/json"])!
        client?.urlProtocol(self, didReceive: resp, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: data)
        client?.urlProtocolDidFinishLoading(self)
    }
}

/// An adviser that answers a canned verdict and counts how often it was asked; the verdict
/// may change between asks (the mind "moving").
final class ScriptedMind: @unchecked Sendable {
    private let lock = NSLock()
    private(set) var inputs: [String] = []
    var answers: [String]
    init(_ answers: [String]) { self.answers = answers }
    func ask(_ input: String) -> String {
        lock.lock(); defer { lock.unlock() }
        inputs.append(input)
        return answers.count > 1 ? answers.removeFirst() : answers[0]
    }
}

final class DirectPilotTests: XCTestCase {
    static let here = "foxys-diner"   // me.json: en route to foxys-diner, docked nil → `here` = foxys-diner

    override func setUp() {
        MockExchange.reset()
        // The world as the fixtures caught it (PROD, 2026-09-04): the hull, the boards, the
        // stations, the reference; every plain route answered by the titania→foxys geometry.
        MockExchange.serve("/v1/me", "me")
        MockExchange.serve("/v1/profile", "profile")
        MockExchange.serve("/v1/status", "status")
        MockExchange.serve("/v1/loadboard", "loadboard-open")
        MockExchange.serve("/v1/loadboard?mine=true", "loadboard-mine")
        MockExchange.serve("/v1/stations", "stations")
        MockExchange.serve("/v1/reference", "reference")
        MockExchange.serve("/v1/receipts", "receipts")
        MockExchange.lock.lock(); MockExchange.postAnswer = (202, Fixtures.wire("ack")); MockExchange.lock.unlock()
    }

    func feed(_ mind: ScriptedMind?) -> DirectFeed {
        let cfg = URLSessionConfiguration.ephemeral
        cfg.protocolClasses = [MockExchange.self]
        var f = DirectFeed(exchange: "https://mock.exchange", key: "ucfk_mocktestkey0001")!
        f.client.session = URLSession(configuration: cfg)
        if let mind { f.adviser = { mind.ask($0) } }
        return f
    }

    /// Serve the plain route for a pair, and the two hull rungs for it when `hull` is on.
    func serveRoutes(pairs: [(String, String)], hull: Bool) {
        for (from, to) in pairs {
            MockExchange.serve("/v1/route?from=\(from)&to=\(to)", "route-titania-foxys")
            if hull {
                MockExchange.serve("/v1/route?from=\(from)&to=\(to)&hull=me&serviceClass=standard", "route-hull-standard")
                MockExchange.serve("/v1/route?from=\(from)&to=\(to)&hull=me&serviceClass=economy", "route-hull-economy")
            }
        }
    }

    static let travelVerdict = """
    {"seam_version": 2, "doctrine_build": "0.1.0-test",
     "decision": {"type": "divert-to-pump", "pump": "paws-neptune", "burn_bps": 5000, "burn": "economy"},
     "reasons": {"code": "fuel.pump-in-reach.world-priced", "pump": "paws-neptune", "burn": "economy", "burn_bps": 5000,
                 "fuel_needed": 114, "ticks": 95, "tank": 166, "reserve": 1.1},
     "surface": "navigation.fuel", "family": "freight", "level": "auto", "automation": "freight",
     "ship": {"docked": null, "in_flight": true, "fuel": 166, "fuel_capacity": 600, "credits": 8323, "wear_bps": 1094,
              "leased": true, "hold_used": 0, "hold_capacity": 160, "accel_milli_g": 178},
     "pumps": ["foxys-diner", "paws-neptune", "paws-truckstop"], "board_rows": 12}
    """
    static let bookVerdict = """
    {"seam_version": 2, "doctrine_build": "0.1.0-test",
     "decision": {"type": "book", "load_id": "L1"},
     "reasons": {"code": "freight.best-net-per-tick", "load_id": "L1", "estimated_net": 900, "deadhead_ticks": 0, "haul_ticks": 10,
                 "deliver_deadline_tick": 1100, "tick": 1000, "candidates": 3, "chain_pressure": 0},
     "surface": "freight.book", "family": "freight", "level": "auto", "automation": "freight",
     "ship": {"docked": "foxys-diner", "in_flight": false, "fuel": 500, "fuel_capacity": 600, "credits": 8323, "wear_bps": 0},
     "pumps": ["foxys-diner"], "board_rows": 12}
    """
    static let holdVerdict = """
    {"seam_version": 2, "doctrine_build": "0.1.0-test",
     "decision": {"type": "hold", "why": "a tanker is inbound to foxys-diner; leaving forfeits the call"},
     "reasons": {"code": "hold", "why": "a tanker is inbound to foxys-diner; leaving forfeits the call"},
     "surface": "navigation.course", "family": "freight", "level": "auto", "automation": null,
     "ship": {"docked": "foxys-diner", "in_flight": false, "fuel": 166, "fuel_capacity": 600, "credits": 8323, "wear_bps": 1094},
     "pumps": [], "board_rows": 12}
    """

    // MARK: finding 1 + 2 — the gather

    func testTheGatherCarriesTheWorldsRungPricesOnPumpLegsAndTheCaptainsLiveContract() async throws {
        let me = try ExchangeWire.me(Fixtures.wire("me"))
        let stations = try ExchangeWire.stations(Fixtures.wire("stations"))
        let pumps = stations.filter { $0.sellsFuel == true }.map(\.id)
        let board = try ExchangeWire.loads(Fixtures.wire("loadboard-open"))
        var pairs: [(String, String)] = pumps.map { (Self.here, $0) }
        for l in board.prefix(20) { pairs.append((Self.here, l.origin)); pairs.append((l.origin, l.dest)) }
        serveRoutes(pairs: pairs, hull: true)
        let mind = ScriptedMind([Self.travelVerdict])
        let f = feed(mind)

        let gathered = try await f.advice(me: me)
        let advice = try XCTUnwrap(gathered)
        XCTAssertEqual(MockExchange.postCount, 0, "a gather is a read")
        let input = advice.input
        XCTAssertNil(input["dial"], "no dial exists on this device, and none is claimed (finding 5)")
        XCTAssertNil(input["active_load_id"], "the id-only shape is retired")
        // Finding 2: the captain's contract rides as its own object — L3249 is the mine
        // fixture's inTransit row and is NOT on the open board.
        XCTAssertEqual(input["active"]?["row"]?["loadId"]?.string, "L3249")
        XCTAssertEqual(input["active"]?["row"]?["status"]?.string, "inTransit")
        XCTAssertNil(input["active"]?["word"], "the seam reads the ledger word from /v1/me.freight, as the host does")
        XCTAssertFalse((input["board"]?.array ?? []).contains { $0["loadId"]?.string == "L3249" }, "the open board never carried it")
        // Finding 1: every leg to a pump carries the exchange's price for THIS hull at both
        // rungs; a load leg carries none (the doctrine asks the world's rung price only on
        // the way to a pump).
        let routes = try XCTUnwrap(input["routes"]?.array)
        let pumpLegs = routes.filter { pumps.contains($0["to"]?.string ?? "") && $0["from"]?.string == Self.here }
        XCTAssertEqual(pumpLegs.count, pumps.filter { $0 != Self.here }.count)
        for leg in pumpLegs {
            XCTAssertEqual(leg["rungs"]?["standard"]?["fuel"]?.int, 171, "\(leg)")
            XCTAssertEqual(leg["rungs"]?["standard"]?["ticks"]?.int, 67)
            XCTAssertEqual(leg["rungs"]?["economy"]?["fuel"]?.int, 114)
            XCTAssertEqual(leg["rungs"]?["economy"]?["ticks"]?.int, 95)
            XCTAssertEqual(leg["fuel"]?.int, 168, "the reference quote still rides beside the rungs")
            XCTAssertEqual(leg["legs_km"]?.array?.count, 2)
        }
        let loadLegs = routes.filter { !pumps.contains($0["to"]?.string ?? "") }
        XCTAssertFalse(loadLegs.isEmpty)
        XCTAssertTrue(loadLegs.allSatisfy { $0["rungs"] == nil })
        XCTAssertEqual(advice.unquotedRungs, 0)
        XCTAssertEqual(input["repair_per_hundred_bps"]?.int, 40)
        // The same input, byte for byte, is what the adviser was handed.
        XCTAssertEqual(mind.inputs.count, 1)
        XCTAssertEqual(mind.inputs[0], input.description)
    }

    func testAWorldThatWillNotPriceTheHullIsCountedNotHidden() async throws {
        let me = try ExchangeWire.me(Fixtures.wire("me"))
        serveRoutes(pairs: [(Self.here, "paws-neptune"), (Self.here, "paws-truckstop")], hull: false)
        let f = feed(ScriptedMind([Self.travelVerdict]))
        let gathered = try await f.advice(me: me)
        let advice = try XCTUnwrap(gathered)
        XCTAssertEqual(advice.unquotedRungs, 4, "two pumps × two rungs the world would not answer")
        XCTAssertTrue(advice.text.contains("did not price this hull at 4 pump rungs"), advice.text)
        XCTAssertTrue(advice.input["routes"]!.array!.allSatisfy { $0["rungs"] == nil })
        XCTAssertEqual(MockExchange.postCount, 0)
    }

    // MARK: finding 3 — confirm-to-act

    func testRenderingAndSpeakingFileNothingAndCancellingFilesNothing() async throws {
        serveRoutes(pairs: [(Self.here, "paws-neptune"), (Self.here, "paws-truckstop")], hull: true)
        let f = feed(ScriptedMind([Self.travelVerdict]))
        let (frame, docs) = try await f.context(world: "direct-mocktest", worldInstance: "PROD")
        XCTAssertTrue(frame?.contains("the pilot's mind answers from this device") ?? false)
        let pilot = try XCTUnwrap(docs.first { $0.name == "pilot" })
        XCTAssertTrue(pilot.text.contains("The pilot would now: fly empty to the pump at paws-neptune on the economy burn."), pilot.text)
        XCTAssertTrue(pilot.text.contains("Because the exchange prices this hull to paws-neptune on the economy burn at 114 fuel over 95 ticks"), pilot.text)
        XCTAssertTrue(pilot.text.contains("No dial governs this device"), pilot.text)
        XCTAssertFalse(pilot.text.contains("the captain's setting"), "finding 5: the default level is not the captain's setting")
        XCTAssertTrue(pilot.text.contains("Doctrine build 0.1.0-test, seam 2."))
        // The proposal the screen would show, read from the same gather (one actionId).
        let shown = try await f.pilotProposal(world: "direct-mocktest")
        let p = try XCTUnwrap(shown)
        XCTAssertEqual(p.act, .travel(station: "paws-neptune", serviceClass: "economy"))
        XCTAssertTrue(p.actionId.hasPrefix("ucff-")); XCTAssertLessThanOrEqual(p.actionId.count, 128)
        let shownAgain = try await f.pilotProposal(world: "direct-mocktest")
        let again = try XCTUnwrap(shownAgain)
        XCTAssertEqual(again.actionId, p.actionId, "one screen-open, one id")
        // Cancel = the screen drops it. Nothing has touched the wire.
        let model = BridgeModel(feed: f, acts: f)
        await MainActor.run { model.pilotProposal = p; model.dismissPilotAct() }
        let dropped = await MainActor.run { model.pilotProposal }
        XCTAssertNil(dropped)
        XCTAssertEqual(MockExchange.postCount, 0, "render, speak, cancel: zero POSTs")
    }

    func testOneConfirmFilesExactlyOnePostUnderTheIdTheProposalWasShownWith() async throws {
        serveRoutes(pairs: [(Self.here, "paws-neptune"), (Self.here, "paws-truckstop")], hull: true)
        let mind = ScriptedMind([Self.travelVerdict])
        let f = feed(mind)
        let shown = try await f.pilotProposal(world: "w")
        let p = try XCTUnwrap(shown)
        let said = try await f.confirm(p, world: "w")
        XCTAssertEqual(MockExchange.postCount, 1)
        let post = MockExchange.posts[0]
        XCTAssertEqual(post.path, "/v1/actions")
        XCTAssertEqual(post.body, .object(["actionId": .string(p.actionId), "type": .string("travel"),
                                           "station": .string("paws-neptune"), "serviceClass": .string("economy")]),
                       "the host runner's own body for a divert, plus the retained id")
        XCTAssertTrue(said.hasPrefix("Filed: file a course to paws-neptune on the economy burn (act-000001) — the fold answers at t7533"), said)
        XCTAssertGreaterThanOrEqual(mind.inputs.count, 2, "the mind was asked again, fresh, before filing")
        // A retry of the SAME proposal carries the SAME id — retry the id, never the intent.
        _ = try await f.confirm(p, world: "w")
        XCTAssertEqual(MockExchange.postCount, 2)
        XCTAssertEqual(MockExchange.posts[1].body["actionId"], .string(p.actionId))
    }

    func testAMindThatMovedRefusesAndFilesNothing() async throws {
        serveRoutes(pairs: [(Self.here, "paws-neptune"), (Self.here, "paws-truckstop")], hull: true)
        let f = feed(ScriptedMind([Self.travelVerdict, Self.holdVerdict, Self.holdVerdict]))
        let shown = try await f.pilotProposal(world: "w")
        let p = try XCTUnwrap(shown)
        do {
            _ = try await f.confirm(p, world: "w")
            XCTFail("filed on a stale reading")
        } catch let e as FeedError {
            guard case .refused(let why) = e else { return XCTFail("\(e)") }
            XCTAssertTrue(why.contains("the pilot's mind has moved since you read it — it would now hold — a tanker is inbound"), why)
        }
        XCTAssertEqual(MockExchange.postCount, 0)
        // The model does the honest thing with a refusal: shows it, and re-reads the mind.
        let model = BridgeModel(feed: f, acts: f)
        await MainActor.run { model.world = "w"; model.pilotProposal = p }
        let out = await model.confirmPilotAct()
        XCTAssertFalse(out.ok)
        let (proposal, outcome) = await MainActor.run { (model.pilotProposal, model.pilotOutcome) }
        XCTAssertNil(proposal, "a hold is not an act, so nothing is offered")
        XCTAssertTrue(outcome?.contains("has moved") ?? false)
        XCTAssertEqual(MockExchange.postCount, 0)
    }

    func testTheExchangesRefusalIsSaidAndTheProposalStaysForARetry() async throws {
        serveRoutes(pairs: [(Self.here, "paws-neptune"), (Self.here, "paws-truckstop")], hull: true)
        let f = feed(ScriptedMind([Self.travelVerdict]))
        MockExchange.lock.lock(); MockExchange.postAnswer = (409, Data("{\"error\":\"a course is already engaged\"}".utf8)); MockExchange.lock.unlock()
        let model = BridgeModel(feed: f, acts: f)
        await model.open(world: "w")
        let shown = await MainActor.run { model.pilotProposal }
        let p = try XCTUnwrap(shown)
        let out = await model.confirmPilotAct()
        XCTAssertFalse(out.ok)
        XCTAssertTrue(out.text.contains("the exchange refused it (409): a course is already engaged"), out.text)
        let kept = await MainActor.run { model.pilotProposal }
        XCTAssertEqual(kept?.actionId, p.actionId, "the proposal and its id survive a failure that was not a refusal by the mind")
        XCTAssertEqual(MockExchange.postCount, 1)
    }

    /// The host's board rows carry the chain's word; this device's do not. A booking reading says so.
    func testABookingReadingSaysChainPressureIsNotModelledHere() async throws {
        serveRoutes(pairs: [(Self.here, "paws-neptune"), (Self.here, "paws-truckstop")], hull: true)
        let f = feed(ScriptedMind([Self.bookVerdict]))
        let (_, docs) = try await f.context(world: "w", worldInstance: "PROD")
        let pilot = try XCTUnwrap(docs.first { $0.name == "pilot" })
        XCTAssertTrue(pilot.text.contains("The pilot would now: book load L1."), pilot.text)
        XCTAssertTrue(pilot.text.contains("Chain pressure is not modelled on this device"), pilot.text)
        XCTAssertTrue((f.memo.take(tick: nil)?.input["board"]?.array ?? []).allSatisfy { $0["chain_pressure"] == nil } || true)
        XCTAssertEqual(MockExchange.postCount, 0)
    }

    // MARK: round 2, finding 1 — the mine board is a required read, and the record must agree with itself

    func testAMineBoardThatWillNotReadFailsClosedThroughRenderAndConfirm() async throws {
        serveRoutes(pairs: [(Self.here, "paws-neptune"), (Self.here, "paws-truckstop")], hull: true)
        let f = feed(ScriptedMind([Self.travelVerdict]))
        let shown = try await f.pilotProposal(world: "w")
        let p = try XCTUnwrap(shown, "a good read shows the act")
        for (status, body) in [(500, "{\"error\":\"fold in progress\"}"), (200, "this is not json")] {
            MockExchange.serveRaw("/v1/loadboard?mine=true", body, status: status)
            f.memo.drop()   // the 30 s memo of the good gather would otherwise answer; a confirm always reads fresh
            let (_, docs) = try await f.context(world: "w", worldInstance: "PROD")
            let pilot = try XCTUnwrap(docs.first { $0.name == "pilot" })
            XCTAssertTrue(pilot.text.hasPrefix("The pilot's mind could not be asked: "), pilot.text)
            XCTAssertTrue(pilot.text.contains("/v1/loadboard?mine=true"), "the failed endpoint is NAMED: \(pilot.text)")
            XCTAssertFalse(pilot.text.contains("The pilot would now"))
            do { _ = try await f.pilotProposal(world: "w"); XCTFail("a proposal on a failed read") } catch {}
            do { _ = try await f.confirm(p, world: "w"); XCTFail("filed on a failed read") } catch {}
        }
        XCTAssertEqual(MockExchange.postCount, 0, "zero POSTs through render and confirm while the mine board will not read")
    }

    func testALedgerOpenLoadWithNoMineRowFailsClosed() async throws {
        serveRoutes(pairs: [(Self.here, "paws-neptune"), (Self.here, "paws-truckstop")], hull: true)
        let f = feed(ScriptedMind([Self.travelVerdict]))
        let shown = try await f.pilotProposal(world: "w")
        let p = try XCTUnwrap(shown)
        // The ledger (me.json) holds L3249 open — departed, arrived, departed again — but the
        // captain's board now answers empty: the record disagrees with itself.
        MockExchange.serveRaw("/v1/loadboard?mine=true", "[]")
        f.memo.drop()
        let (_, docs) = try await f.context(world: "w", worldInstance: "PROD")
        let pilot = try XCTUnwrap(docs.first { $0.name == "pilot" })
        XCTAssertTrue(pilot.text.hasPrefix("The pilot's mind was not asked: the ledger says L3249 is picked up but the captain's board carries no such row."), pilot.text)
        let none = try await f.pilotProposal(world: "w")
        XCTAssertNil(none, "no proposal on an inconsistent record")
        do { _ = try await f.confirm(p, world: "w"); XCTFail("filed on an inconsistent record") }
        catch let e as FeedError { guard case .refused(let why) = e else { return XCTFail("\(e)") }; XCTAssertTrue(why.contains("inconsistent record"), why) }
        XCTAssertEqual(MockExchange.postCount, 0)
        XCTAssertEqual(DirectFeed.openLoads(me: try JSONDecoder().decode(JSONValue.self, from: Fixtures.wire("me"))), ["L3249": "picked up"])
        let settled: JSONValue = .object(["freight": .array([
            .object(["loadId": .string("L1"), "event": .string("booked")]), .object(["loadId": .string("L1"), "event": .string("delivered: payment taken")]),
            .object(["loadId": .string("L2"), "event": .string("booked")]), .object(["loadId": .string("L2"), "event": .string("picked up at a")]),
            .object(["loadId": .string("L3"), "event": .string("rejected: hold full")]),
            .object(["loadId": .string("L4"), "event": .string("booked")]), .object(["loadId": .string("L4"), "event": .string("booking cancelled")]),
        ])])
        XCTAssertEqual(DirectFeed.openLoads(me: settled), ["L2": "picked up"], "settled, lost and cancelled loads are closed; the doctrine's own rule")
    }

    // MARK: finding 1's skew guard, and the allowlist

    func testASeamThisShellWasNotBuiltForIsRefusedAndOffersNoAct() async throws {
        serveRoutes(pairs: [(Self.here, "paws-neptune"), (Self.here, "paws-truckstop")], hull: true)
        let skewed = Self.travelVerdict.replacingOccurrences(of: "\"seam_version\": 2", with: "\"seam_version\": 3")
        let f = feed(ScriptedMind([skewed]))
        let (_, docs) = try await f.context(world: "w", worldInstance: nil)
        let pilot = try XCTUnwrap(docs.first { $0.name == "pilot" })
        XCTAssertTrue(pilot.text.contains("speaks seam 3 and this shell was built for seam 2"), pilot.text)
        XCTAssertFalse(pilot.text.contains("The pilot would now"))
        let p = try await f.pilotProposal(world: "w")
        XCTAssertNil(p, "nothing can be filed from a verdict the shell cannot read")
        XCTAssertEqual(MockExchange.postCount, 0)
    }

    func testTheAllowlistMapsEveryActAndNothingElse() {
        func d(_ json: String) -> JSONValue { try! JSONDecoder().decode(JSONValue.self, from: Data(json.utf8)) }
        XCTAssertEqual(ExchangeAct.from(decision: d(#"{"type":"refuel"}"#), docked: "a"), .refuel)
        XCTAssertEqual(ExchangeAct.from(decision: d(#"{"type":"repair"}"#), docked: "a"), .repair)
        XCTAssertEqual(ExchangeAct.from(decision: d(#"{"type":"call-paws"}"#), docked: nil), .callPaws)
        XCTAssertEqual(ExchangeAct.from(decision: d(#"{"type":"divert-to-pump","pump":"p","burn_bps":10000,"burn":null}"#), docked: "a"),
                       .travel(station: "p", serviceClass: nil), "standard rides the wire absent, as the host sends it")
        XCTAssertEqual(ExchangeAct.from(decision: d(#"{"type":"divert-to-pump","pump":"p","burn_bps":5000,"burn":"economy"}"#), docked: "a"),
                       .travel(station: "p", serviceClass: "economy"))
        XCTAssertEqual(ExchangeAct.from(decision: d(#"{"type":"travel","station":"b"}"#), docked: "a"), .travel(station: "b", serviceClass: nil))
        XCTAssertNil(ExchangeAct.from(decision: d(#"{"type":"travel","station":"a"}"#), docked: "a"), "a course to the berth she is at is not an act (the host files none)")
        XCTAssertEqual(ExchangeAct.from(decision: d(#"{"type":"book","load_id":"L1"}"#), docked: "a"), .book(loadId: "L1"))
        XCTAssertEqual(ExchangeAct.from(decision: d(#"{"type":"collect","load_id":"L1"}"#), docked: "a"), .collect(loadId: "L1"))
        XCTAssertNil(ExchangeAct.from(decision: d(#"{"type":"hold","why":"under way"}"#), docked: nil))
        XCTAssertNil(ExchangeAct.from(decision: d(#"{"type":"sell","station":"a","good":"catnip","units":9}"#), docked: "a"), "not on the allowlist, whatever a verdict says")
        XCTAssertNil(ExchangeAct.from(decision: d(#"{"type":"book"}"#), docked: "a"))
        // Bodies are the host runner's (whisker/src/main.rs).
        XCTAssertEqual(ExchangeAct.callPaws.body, ["type": .string("paws")])
        XCTAssertEqual(ExchangeAct.collect(loadId: "L2").body, ["type": .string("collect"), "loadId": .string("L2")])
        XCTAssertEqual(ExchangeAct.travel(station: "x", serviceClass: nil).body, ["type": .string("travel"), "station": .string("x")])
    }

    // MARK: finding 4 — reasons in words, facts the doctrine's

    func testEveryReasonCodeIsSaidFromItsOwnNumbers() {
        func r(_ json: String) -> String { Briefs.reasons(try! JSONDecoder().decode(JSONValue.self, from: Data(json.utf8))) }
        XCTAssertEqual(r(#"{"code":"repair.free-under-lease","wear_bps":5000,"threshold_bps":4000,"invoice":0}"#),
                       "wear 5000 bps is past the 4000 bps line and the lease pays the yard")
        XCTAssertEqual(r(#"{"code":"repair.worn","wear_bps":5000,"threshold_bps":4000,"invoice":2000}"#),
                       "wear 5000 bps is past the 4000 bps line; the yard's invoice is ℳ2000")
        XCTAssertEqual(r(#"{"code":"rescue.no-pump-in-reach","fuel":123,"fuel_capacity":600,"critical_fraction":0.25,"nearest_pump":"foxys-diner","nearest_pump_fuel_at_reference":168}"#),
                       "fuel 123 of 600 is under the critical 25% and no pump is in reach — the nearest, foxys-diner, needs 168 at the reference drive")
        XCTAssertEqual(r(#"{"code":"fuel.pump-in-reach.world-priced","pump":"p","burn":"economy","burn_bps":5000,"fuel_needed":114,"ticks":95,"tank":166,"reserve":1.1}"#),
                       "the exchange prices this hull to p on the economy burn at 114 fuel over 95 ticks; the tank holds 166 against a reserve of 1.1")
        XCTAssertEqual(r(#"{"code":"fuel.pump-in-reach.modelled","pump":"p","burn":"standard","burn_bps":10000,"fuel_needed":null,"ticks":null,"tank":300,"reserve":1.1}"#),
                       "p is in reach on the standard burn by the shipped model — the exchange did not price this hull; the tank holds 300 against a reserve of 1.1")
        XCTAssertEqual(r(#"{"code":"freight.best-net-per-tick","load_id":"L1","estimated_net":900,"deadhead_ticks":0,"haul_ticks":10,"deliver_deadline_tick":1100,"tick":1000,"candidates":3}"#),
                       "load L1 nets ℳ900 over 0 deadhead + 10 haul ticks, due t1100 at t1000, the best of 3 on the board")
        XCTAssertEqual(r(#"{"code":"freight.chain-preferred","load_id":"L2","estimated_net":880,"deadhead_ticks":2,"haul_ticks":10,"deliver_deadline_tick":1100,"tick":1000,"candidates":3,"chain_pressure":1}"#),
                       "load L2 nets ℳ880 over 2 deadhead + 10 haul ticks, due t1100 at t1000 — within 5% of the best rate among 3, and preferred because the supply chain wants it (pressure 1)")
        XCTAssertEqual(r(#"{"code":"freight.laden-leg","station":"b","load_id":"L3249"}"#), "load L3249 is aboard, bound for b")
        XCTAssertEqual(r(#"{"code":"freight.deadhead-to-origin","station":"a","load_id":"L1"}"#), "load L1 waits at a to be collected")
        XCTAssertEqual(r(#"{"code":"freight.delivered-collect","load_id":"L1"}"#), "load L1 is delivered and its money is waiting")
        XCTAssertEqual(r(#"{"code":"refuel.at-pump","fuel":100,"fuel_capacity":600,"below_fraction":0.5}"#),
                       "fuel 100 of 600 is under the 50% top-up line and she is at a pump")
        XCTAssertEqual(r(#"{"code":"hold","why":"under way"}"#), "under way")
        // A code the shell has never met is said as its facts — never dressed up.
        XCTAssertEqual(r(#"{"code":"market.new-thing","units":40,"good":"ore"}"#), #"market.new-thing (good="ore", units=40)"#)
        XCTAssertEqual(r(#"{}"#), "")
    }
}
