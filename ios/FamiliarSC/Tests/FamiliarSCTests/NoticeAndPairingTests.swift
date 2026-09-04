import XCTest
@testable import FamiliarSC

final class NoticeTests: XCTestCase {
    func testOnlyWhatDeservesTheCaptainsAttention() {
        let n = NoticePolicy.notices(for: Fixtures.journal().entries)
        XCTAssertEqual(n.map(\.kind), [.money, .money, .money, .distress, .distress, .distress, .advice, .needsTheCaptain, .needsTheCaptain, .needsTheCaptain, .hull, .distress, .distress])
        XCTAssertEqual(n.filter { $0.title == "Exchange unreachable" }.count, 1, "a run of unreachable lines is one notice")
        XCTAssertFalse(n.contains { $0.title.contains("holding") || $0.body.contains("waiting on the crane") })
        XCTAssertFalse(n.contains { $0.title == "Drive engaged" }, "a leg engaged is routine, not a notification")
        XCTAssertEqual(n[0].title, "Load L1 closed")
        XCTAssertEqual(n[1].title, "Bought 40 ore")
        XCTAssertEqual(n[2].body, "40 ore at ask 15, bound for io-slagworks; sellable from t411")
        XCTAssertEqual(n[7].title, "Your word, please")
        XCTAssertEqual(n[11].title, "Carry refused")
        XCTAssertEqual(n[12].kind, .distress)
        XCTAssertEqual(n[7].body, "book L3 — best rate on the board (until t205)")
    }

    func testLeaseNotice() {
        XCTAssertNil(NoticePolicy.leaseNotice(hoursLeft: 20, at: 1))
        XCTAssertEqual(NoticePolicy.leaseNotice(hoursLeft: 3, at: 1)?.title, "Lease lapses in 3h")
        XCTAssertEqual(NoticePolicy.leaseNotice(hoursLeft: -1, at: 1)?.title, "Lease expired")
    }
}

final class PairingTests: XCTestCase {
    func testKeyParseFromPasteURLAndQR() {
        let raw = "ucfk_0123abcdEFGHijkl_mnop-qrstuv"
        XCTAssertEqual(try? PairingKey.parse(raw).get().secret, raw)
        XCTAssertEqual(try? PairingKey.parse("  \(raw)\n").get().keyID, "0123abcd")
        XCTAssertEqual(try? PairingKey.parse("https://exchange.example/pair?key=\(raw)&ship=KKII").get().secret, raw)
        XCTAssertEqual(try? PairingKey.parse("UCF co-pilot key: \(raw). Keep it secret.").get().secret, raw)
        XCTAssertEqual(PairingKey.parse("nothing here"), .failure(.noKey))
        XCTAssertEqual(PairingKey.parse("ucfk_short"), .failure(.malformedKey("shorter than 16 characters after ucfk_")))
        XCTAssertEqual(try? PairingKey.parse(raw).get().redacted, "ucfk_0123abcd…")
    }

    func testRequestValidationAndArgvCarryNoSecret() {
        var r = PairingRequest(label: "KK II", captain: "Ian", server: "https://exchange.example", automations: [.freight, .trade], computerName: "Purr")
        XCTAssertNil(r.validate())
        let argv = r.fleetPairArguments(keyFile: "/tmp/key")
        XCTAssertEqual(argv, ["fleet", "pair", "--label", "KK II", "--captain", "Ian", "--server", "https://exchange.example", "--key-file", "/tmp/key", "--automations", "freight,trade", "--computer-name", "Purr"])
        XCTAssertFalse(argv.joined(separator: " ").contains("ucfk_"))
        let joins = PairingRequest(label: "KK", captain: "Ian", server: "https://exchange.example", automations: [.freight], computerName: nil)
        XCTAssertFalse(joins.fleetPairArguments(keyFile: "/k").contains("--computer-name"), "no name → the hull joins the captain's computer")
        r.server = "exchange.example"
        XCTAssertEqual(r.validate(), .badServer("exchange.example"))
        r.server = "http://127.0.0.1:7877"; r.label = " "
        XCTAssertEqual(r.validate(), .emptyLabel)
    }
}
