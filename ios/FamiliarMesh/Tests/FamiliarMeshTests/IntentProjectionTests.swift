import XCTest
@testable import FamiliarMesh

/// The external-indexed projection (T-227 Q2): what Siri/Spotlight/lock screen may hear is
/// counts, canonical kinds, the oracle line, and the FACT of an open question — proven here
/// against a worldview deliberately full of things that must not pass.
final class IntentProjectionTests: XCTestCase {

    private func worldview() throws -> Worldview {
        let json = """
        {
          "group_label": "river",
          "node_id": "node-abc",
          "question": "Has Betty's iPad been seen since Tuesday?",
          "question_owner": "betty",
          "presence": 0.8, "withdrawn": false, "service": 0.5, "capacity": 0.9,
          "observation_count": 1234,
          "peers": [
            {"node_id": "peer-1", "label": "GIIWEO lighthouse", "last_seen": 100,
             "tools_offered": 0, "patterns_offered": 0},
            {"node_id": "peer-2", "label": "Wildhorse", "last_seen": 90,
             "tools_offered": 0, "patterns_offered": 0}
          ],
          "recent": [
            {"actor": "phone:ian", "action": "reports", "object": "motion:walking",
             "context": "near Betty", "source": "mesh:node", "ts": 100, "confidence": 0.9}
          ],
          "services": [
            {"kind": "airplay", "name": "", "seen_by": "phone:ian", "last_seen": 100},
            {"kind": "mqtt", "name": "", "seen_by": "host", "last_seen": 90}
          ]
        }
        """
        return try JSONDecoder().decode(Worldview.self, from: Data(json.utf8))
    }

    func testProjectionCarriesKindsCountsAndFactsOnly() throws {
        let p = IntentProjection.project(
            from: try worldview(),
            oracleLine: "on-device model ready",
            now: 200
        )
        XCTAssertEqual(p.observationCount, 1234)
        XCTAssertEqual(p.peerCount, 2)
        XCTAssertEqual(p.serviceKinds, ["airplay", "mqtt"])
        XCTAssertTrue(p.openQuestion, "the FACT of a question is served")

        // Nothing personal survives into the serialized projection: not the question text,
        // not its owner, not peer labels, not observation actors/objects/contexts.
        let raw = String(data: try JSONEncoder().encode(p), encoding: .utf8)!
        for leak in ["Betty", "betty", "GIIWEO", "Wildhorse", "phone:ian", "walking", "Tuesday"] {
            XCTAssertFalse(raw.contains(leak), "external-indexed projection leaked: \(leak)")
        }
    }

    func testStoreAndReadBackRoundTrips() throws {
        let suite = UserDefaults(suiteName: "intent-projection-tests")!
        suite.removePersistentDomain(forName: "intent-projection-tests")
        XCTAssertNil(IntentProjection.stored(in: suite))
        let p = IntentProjection.project(from: try worldview(), oracleLine: "x", now: 5)
        p.store(in: suite)
        XCTAssertEqual(IntentProjection.stored(in: suite), p)
    }
}
