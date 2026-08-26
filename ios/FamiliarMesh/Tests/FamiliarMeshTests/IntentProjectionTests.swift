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
        XCTAssertNil(IntentProjection.stored(in: suite, now: 5))
        let p = IntentProjection.project(from: try worldview(), oracleLine: "x", now: 5)
        p.store(in: suite)
        XCTAssertEqual(IntentProjection.stored(in: suite, now: 6), p)
    }

    /// codex's brick-1 return §1: `ServiceView.kind` upstream is NOT an allowlist — a
    /// validly-signed but defective client can submit free/personal text as a "kind". The
    /// projection speaks only the repo-authored vocabulary; anything else is OMITTED, never
    /// normalized into something speakable.
    func testAHostileKindNeverBecomesSpeakable() throws {
        let json = """
        {
          "group_label": "river", "node_id": "n",
          "presence": 0, "withdrawn": false, "service": 0, "capacity": 0,
          "observation_count": 1, "peers": [], "recent": [],
          "services": [
            {"kind": "Bettys-iPhone", "name": "", "seen_by": "x", "last_seen": 1},
            {"kind": "airplay._tcp evil", "name": "", "seen_by": "x", "last_seen": 1},
            {"kind": "mqtt", "name": "", "seen_by": "x", "last_seen": 1}
          ]
        }
        """
        let view = try JSONDecoder().decode(Worldview.self, from: Data(json.utf8))
        let p = IntentProjection.project(from: view, oracleLine: "x", now: 2)
        XCTAssertEqual(p.serviceKinds, ["mqtt"], "only the closed vocabulary is speakable")
        let raw = String(data: try JSONEncoder().encode(p), encoding: .utf8)!
        XCTAssertFalse(raw.contains("Bettys"), "a hostile kind leaked into the projection")
    }

    /// codex's brick-1 return §2: a timestamp without enforcement is not a freshness fence.
    /// Past the horizon the read seam itself answers nil — the intent then says "open the
    /// app to refresh" instead of fabricating a current reading from stale state.
    func testAStaleProjectionIsRefusedAtTheReadSeam() throws {
        let suite = UserDefaults(suiteName: "intent-projection-stale")!
        suite.removePersistentDomain(forName: "intent-projection-stale")
        let p = IntentProjection.project(from: try worldview(), oracleLine: "x", now: 1000)
        p.store(in: suite)
        let horizon = IntentProjection.freshnessHorizonSecs
        XCTAssertNotNil(IntentProjection.stored(in: suite, now: 1000 + horizon))
        XCTAssertNil(
            IntentProjection.stored(in: suite, now: 1000 + horizon + 1),
            "an expired projection was served as current"
        )
    }

    /// Severance forgets the projection: an unenrolled device holds no cached claim about a
    /// familiar it no longer belongs to (AppModel.unenroll calls this).
    func testClearForgetsTheProjection() throws {
        let suite = UserDefaults(suiteName: "intent-projection-clear")!
        suite.removePersistentDomain(forName: "intent-projection-clear")
        IntentProjection.project(from: try worldview(), oracleLine: "x", now: 5).store(in: suite)
        XCTAssertNotNil(IntentProjection.stored(in: suite, now: 6))
        IntentProjection.clear(in: suite)
        XCTAssertNil(IntentProjection.stored(in: suite, now: 6))
    }
}
