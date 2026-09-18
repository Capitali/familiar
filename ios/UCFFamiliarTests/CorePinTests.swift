import XCTest
@testable import UCF_Familiar

/// The checked-in FamiliarCore archive — the pilot's mind as UCF Familiar actually ships it —
/// says what current doctrine says (codex T-237 B4 r3, finding 2). The Swift package tests
/// use canned verdicts and never link the archive; the Rust tests exercise source. Both were
/// green while the archive, built before T-243, ignored `denied` and offered `repair` to a key
/// that could not file it. This test runs the ARCHIVE over the fixtures the Rust side pins
/// (`wire::seam_parity_tests`, the same two files, read at compile time there).
final class CorePinTests: XCTestCase {
    func testTheCheckedInCoreSaysWhatCurrentSourceSays() throws {
        for name in ["seam-denied-repair", "seam-bay-full"] {
            let url = try XCTUnwrap(Bundle(for: CorePinTests.self).url(forResource: name, withExtension: "json"), "fixture \(name) in the test bundle")
            let fixture = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
            let input = try JSONSerialization.data(withJSONObject: try XCTUnwrap(fixture["input"]), options: [.sortedKeys])
            let out = whiskerAdvise(inputJson: String(decoding: input, as: UTF8.self))
            let verdict = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(out.utf8)) as? [String: Any], "\(name): the archive answered \(out.prefix(200))")
            let expect = try XCTUnwrap(fixture["expect"] as? [String: Any])
            XCTAssertEqual(verdict["seam_version"] as? Int, expect["seam_version"] as? Int, "\(name): the archive's seam is not the shell's — rebuild it (tools/build-core.sh)")
            XCTAssertEqual(verdict["decision"] as? NSDictionary, expect["decision"] as? NSDictionary, "\(name): the archive decides differently from current source: \(out)")
        }
    }
}
