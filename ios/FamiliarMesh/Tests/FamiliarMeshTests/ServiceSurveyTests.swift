import XCTest
@testable import FamiliarMesh

/// The survey vocabulary both shells now speak (T-228 brick 2). What is pinned here is not the
/// *policy* — Q1 is still open — but the properties that let one policy govern two shells.
final class ServiceSurveyTests: XCTestCase {

    func testTheListIsOneListAndWellFormed() {
        let types = ServiceSurvey.serviceTypes
        XCTAssertFalse(types.isEmpty)
        // A duplicate would start a second browser for the same type and double-report everything
        // it finds, since dedup keys on (type, name) per browser run.
        XCTAssertEqual(Set(types).count, types.count, "duplicate service type in the shared list")
        for t in types {
            XCTAssertTrue(t.hasPrefix("_"), "\(t) is not a Bonjour service type")
            XCTAssertTrue(t.hasSuffix("._tcp"), "\(t) is not a Bonjour service type")
        }
    }

    func testKindIsTheClassTheKernelSees() {
        XCTAssertEqual(ServiceSurvey.kind("_airplay._tcp"), "airplay")
        XCTAssertEqual(ServiceSurvey.kind("_familiar-mesh._tcp"), "familiar-mesh")
        XCTAssertEqual(ServiceSurvey.kind("_pdl-datastream._tcp"), "pdl-datastream")
        // Something that is not a service type comes back unchanged rather than mangled — a
        // surveyor should never invent a class it cannot derive.
        XCTAssertEqual(ServiceSurvey.kind("airplay"), "airplay")
        XCTAssertEqual(ServiceSurvey.kind(""), "")
    }

    /// Every type the shells browse yields a distinct class. If two types collapsed to one kind,
    /// observations from different things would share an `obs_class` and recurrence analysis would
    /// quietly conflate them.
    func testEveryTypeYieldsADistinctClass() {
        let kinds = ServiceSurvey.serviceTypes.map(ServiceSurvey.kind)
        XCTAssertEqual(Set(kinds).count, kinds.count, "two service types collapse to the same kind")
        XCTAssertFalse(kinds.contains(""), "a service type produced an empty class")
    }

    /// **Q1 CLOSED (codex round 2): no advertised name reaches an observation. Ever.**
    ///
    /// This is the flip of the passthrough pin that stood while Q1 was open — that test's job
    /// was to force whoever landed the answer to come here and say so. Said: the name drops,
    /// with no exception for the familiar's own service and no salted stand-in (a stable
    /// pseudonym is still a tracking token). Changing this back is a design reversal, not a
    /// refactor — it reopens the dialogue.
    func testNoAdvertisedNameSurvivesIntoContext() {
        XCTAssertEqual(ServiceSurvey.context(forInstanceName: "Ian's MacBook Pro"), "")
        XCTAssertEqual(ServiceSurvey.context(forInstanceName: "Betty's AirPods"), "")
        XCTAssertEqual(ServiceSurvey.context(forInstanceName: "GIIWEO._familiar-mesh._tcp"), "")
        XCTAssertEqual(ServiceSurvey.context(forInstanceName: ""), "")
    }

    /// codex round 2's acceptance gap, closed: the shared list is only the whole truth if BOTH
    /// app plists declare exactly the same `NSBonjourServices` — Apple resolves only declared
    /// types, and a drifted plist fails SILENTLY (the browse returns nothing for the missing
    /// type). This structural check makes that drift loud from `swift test`.
    func testBothInfoPlistsDeclareExactlyTheSharedList() throws {
        let root = URL(fileURLWithPath: #filePath) // …/FamiliarMesh/Tests/FamiliarMeshTests/…
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent() // …/ios
        for plist in ["App/Support/Info.plist", "MacApp/Support/Info.plist"] {
            let url = root.appendingPathComponent(plist)
            let data = try Data(contentsOf: url)
            let parsed = try PropertyListSerialization.propertyList(from: data, format: nil)
            let declared = (parsed as? [String: Any])?["NSBonjourServices"] as? [String]
            XCTAssertEqual(
                declared, ServiceSurvey.serviceTypes,
                "\(plist) NSBonjourServices has drifted from ServiceSurvey.serviceTypes"
            )
        }
    }
}
