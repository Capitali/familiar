import XCTest
@testable import FamiliarMesh

/// The BLE survey's floor (T-228 Q3, closed): service-UUID class + coarse per-window
/// count, and structurally nothing else. What is pinned here is the policy every radio
/// after Bonjour inherits.
final class BLESurveyTests: XCTestCase {

    func testTheClassMapIsClosedAndWellFormed() {
        for (uuid, cls) in BLESurvey.serviceClasses {
            XCTAssertEqual(uuid.count, 4, "\(uuid) is not a 16-bit alias")
            XCTAssertEqual(uuid, uuid.lowercased(), "\(uuid) is not normalized")
            XCTAssertFalse(cls.isEmpty)
            XCTAssertFalse(cls.contains(" "), "\(cls) is not a class token")
        }
    }

    func testUUIDNormalizationCollapsesOnlyTheBluetoothBase() {
        // CoreBluetooth's 16-bit rendering, either case.
        XCTAssertEqual(BLESurvey.bleClass(forServiceUUID: "180D"), "heart-rate")
        XCTAssertEqual(BLESurvey.bleClass(forServiceUUID: "180d"), "heart-rate")
        // The same service in full 128-bit base form.
        XCTAssertEqual(
            BLESurvey.bleClass(forServiceUUID: "0000180D-0000-1000-8000-00805F9B34FB"),
            "heart-rate"
        )
        // A vendor's random 128-bit UUID is NEVER named — no repo-authored class, no row.
        XCTAssertNil(BLESurvey.bleClass(forServiceUUID: "E20A39F4-73F5-4BC4-A12F-17D1AD07A961"))
        // A 128-bit UUID that merely LOOKS near the base but is not it does not collapse.
        XCTAssertNil(BLESurvey.bleClass(forServiceUUID: "0000180D-0000-1000-8000-00805F9B34FC"))
        XCTAssertNil(BLESurvey.bleClass(forServiceUUID: "beef"))
        XCTAssertNil(BLESurvey.bleClass(forServiceUUID: ""))
    }

    func testTheWindowReportIsClassAndBucketOnlyAndDeterministic() {
        let report = BLESurvey.windowReport(classCounts: [
            "heart-rate": 1,
            "battery": 3,
            "vendor-serial": 9,
            "Bettys-Watch": 2, // a class the repo never authored cannot enter a report
            "hid": 0, // nothing seen, nothing said
        ])
        XCTAssertEqual(report.map(\.object), ["ble:battery", "ble:heart-rate", "ble:vendor-serial"])
        XCTAssertEqual(report.map(\.context), ["seen=few", "seen=one", "seen=many"])
        for entry in report {
            XCTAssertFalse(entry.object.contains("Betty"))
            XCTAssertFalse(entry.context.contains("9"), "a raw count leaked past the bucket")
        }
    }

    /// The BLE classes are speakable by App Intents — one closed vocabulary end to end —
    /// and every class survives the projection's own allowlist.
    func testBLEClassesJoinTheSpeakableVocabulary() {
        for cls in BLESurvey.classes {
            XCTAssertTrue(
                IntentProjection.speakableKinds.contains(cls),
                "\(cls) is a survey class the intent projection would silently drop"
            )
        }
    }
}
