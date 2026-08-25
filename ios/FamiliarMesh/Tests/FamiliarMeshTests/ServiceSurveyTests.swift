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

    /// **This test is expected to change when T-228's Q1 closes, and that is its job.**
    ///
    /// The naming discipline passes the advertised instance name through unchanged — which is
    /// today's shipping behaviour and precisely what Q1 is deciding (Bonjour instance names are
    /// overwhelmingly personal, and they are being written into observations that replicate
    /// mesh-wide). Pinning it means the policy cannot change by accident in one shell: whoever
    /// implements Q1's answer must come here and say so.
    func testTheNamingDisciplineIsStillPassthroughPendingQ1() {
        XCTAssertEqual(ServiceSurvey.context(forInstanceName: "Ian's MacBook Pro"), "Ian's MacBook Pro")
        XCTAssertEqual(ServiceSurvey.context(forInstanceName: ""), "")
    }
}
