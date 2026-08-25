import XCTest
@testable import FamiliarMesh

/// The gates a shell can SEE decide the gates a shell can OBEY (T-228 Q2, Ian 2026-08-24: the
/// clients follow the user's authorization). This mirror carried 7 of the door's 14 fields, and
/// `network_discovery` was one of the missing ones — which is the whole reason iOS gated its
/// network survey on a device-local toggle instead of the household boundary.
///
/// Two properties are pinned here because both are load-bearing and neither is obvious:
/// a door too old to send the sensor gates must still decode (or the console loses the entire
/// worldview over a field it does not need), and an absent gate must read SHUT (or an old door
/// silently authorizes everything).
final class GateStatesTests: XCTestCase {

    /// The seven fields this mirror has always modelled — what an older familiar answers.
    private let legacyJSON = #"""
    {"llm":true,"camera":false,"network":true,"mesh":true,
     "execute":false,"agent":false,"tool_install":false}
    """#

    /// What a current door answers (crates/mesh/src/worldview.rs), gates straight off the boundary.
    private let currentJSON = #"""
    {"llm":true,"camera":false,"network":true,"mesh":true,
     "execute":false,"agent":false,"tool_install":false,
     "microphone":false,"location":true,"motion":true,
     "network_discovery":true,"face_recognition":false,
     "outreach":false,"actuate":true}
    """#

    func testAnOlderDoorStillDecodes() throws {
        let gates = try JSONDecoder().decode(GateStates.self, from: Data(legacyJSON.utf8))
        XCTAssertTrue(gates.llm)
        XCTAssertTrue(gates.network)
        // It told us nothing about the sensor gates — and saying so is a distinct fact from
        // saying they are shut. A console must be able to tell "I did not hear" from "no".
        XCTAssertFalse(gates.reportsSensorGates)
    }

    func testAnUnheardGateIsShutNotOpen() throws {
        let gates = try JSONDecoder().decode(GateStates.self, from: Data(legacyJSON.utf8))
        XCTAssertFalse(gates.networkDiscoveryOpen)
        XCTAssertFalse(gates.microphoneOpen)
        XCTAssertFalse(gates.locationOpen)
        XCTAssertFalse(gates.motionOpen)
        XCTAssertFalse(gates.faceRecognitionOpen)
        XCTAssertFalse(gates.outreachOpen)
        XCTAssertFalse(gates.actuateOpen)
    }

    func testACurrentDoorIsReadFaithfully() throws {
        let gates = try JSONDecoder().decode(GateStates.self, from: Data(currentJSON.utf8))
        XCTAssertTrue(gates.reportsSensorGates)
        XCTAssertTrue(gates.networkDiscoveryOpen)
        XCTAssertTrue(gates.locationOpen)
        XCTAssertTrue(gates.motionOpen)
        XCTAssertTrue(gates.actuateOpen)
        // Shut is shut: an explicit false must never round-trip into an open reading.
        XCTAssertFalse(gates.microphoneOpen)
        XCTAssertFalse(gates.faceRecognitionOpen)
        XCTAssertFalse(gates.outreachOpen)
    }

    /// A gate the human just shut must read shut immediately — the same decoder, the same field,
    /// no cached optimism. This is what makes it safe to re-evaluate the survey on every read.
    func testAShutGateReadsShutOnTheNextRead() throws {
        let open = try JSONDecoder().decode(GateStates.self, from: Data(currentJSON.utf8))
        XCTAssertTrue(open.networkDiscoveryOpen)
        let shutJSON = currentJSON.replacingOccurrences(
            of: #""network_discovery":true"#, with: #""network_discovery":false"#
        )
        let shut = try JSONDecoder().decode(GateStates.self, from: Data(shutJSON.utf8))
        XCTAssertTrue(shut.reportsSensorGates)
        XCTAssertFalse(shut.networkDiscoveryOpen)
    }
}
