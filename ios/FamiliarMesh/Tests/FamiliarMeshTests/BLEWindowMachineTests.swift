import XCTest
@testable import FamiliarMesh

/// The scan-window state machine (codex's BLE review, finding 1): the two regressions the
/// return named, plus the cross-language vocabulary drift pin from finding 2.
final class BLEWindowMachineTests: XCTestCase {

    final class Harness {
        var scans: [Bool] = []
        var stops = 0
        var arms = 0
        var disarms = 0
        var reports: [[(object: String, context: String)]] = []
        lazy var machine = BLEWindowMachine(
            startScan: { self.scans.append($0) },
            stopScan: { self.stops += 1 },
            armTimer: { self.arms += 1 },
            disarmTimer: { self.disarms += 1 },
            report: { self.reports.append($0) }
        )
    }

    /// codex (a): the same stationary peripheral contributes once in EACH of two
    /// consecutive windows — which is only possible because the machine requests
    /// duplicate callbacks; a default coalescing scan would empty every window after
    /// the first.
    func testAStationaryPeripheralCountsInConsecutiveWindows() {
        let h = Harness()
        let p = UUID()
        h.machine.radio(.poweredOn(authorized: true))
        XCTAssertEqual(h.scans, [true], "the scan must request duplicate callbacks")
        h.machine.sighting(peripheral: p, serviceUUIDs: ["180D"])
        h.machine.sighting(peripheral: p, serviceUUIDs: ["180D"]) // same window: dedup
        h.machine.tick()
        h.machine.sighting(peripheral: p, serviceUUIDs: ["180D"]) // NEXT window: counts again
        h.machine.tick()
        XCTAssertEqual(h.reports.count, 2, "the second window went silently empty")
        for window in h.reports {
            XCTAssertEqual(window.map(\.object), ["ble:heart-rate"])
            XCTAssertEqual(window.map(\.context), ["seen=one"])
        }
    }

    /// codex (b): poweredOn → refused/off → poweredOn leaves exactly ONE armed clock, and
    /// nothing collected before the refusal is ever flushed after it.
    func testAPowerCycleLeavesOneClockAndBurnsTheRefusedWindow() {
        let h = Harness()
        h.machine.radio(.poweredOn(authorized: true))
        h.machine.sighting(peripheral: UUID(), serviceUUIDs: ["180F"]) // gathered pre-refusal
        h.machine.radio(.poweredOff)
        XCTAssertEqual(h.stops, 1)
        XCTAssertEqual(h.disarms, 1)
        XCTAssertEqual(h.machine.state, "Bluetooth is off")
        h.machine.tick() // a straggler tick from a dying clock emits nothing
        h.machine.radio(.poweredOn(authorized: true))
        h.machine.radio(.poweredOn(authorized: true)) // a replayed callback stacks nothing
        XCTAssertEqual(h.arms, 2, "one arm per powered-on interval")
        XCTAssertTrue(h.machine.timerArmed)
        XCTAssertEqual(h.arms - h.disarms, 1, "exactly one clock is live")
        h.machine.tick()
        XCTAssertTrue(
            h.reports.isEmpty,
            "an observation gathered before the refusal leaked out after it"
        )
    }

    /// A refusal of authorization behaves like a radio loss: nothing collects, honest state.
    func testAnUnauthorizedRadioCollectsNothing() {
        let h = Harness()
        h.machine.radio(.poweredOn(authorized: false))
        XCTAssertEqual(h.machine.state, "Bluetooth permission not granted")
        h.machine.sighting(peripheral: UUID(), serviceUUIDs: ["180D"])
        h.machine.tick()
        XCTAssertTrue(h.reports.isEmpty)
        XCTAssertTrue(h.scans.isEmpty, "a refused radio must not scan")
    }

    /// codex finding 2's drift pin, Swift half: the Swift survey classes and the daemon's
    /// `ble_classes.txt` manifest are ONE vocabulary, exactly. (The Rust side builds its
    /// ingest/viewer set from the same file, so equality here closes the loop.)
    func testTheSwiftClassesMatchTheSharedManifestExactly() throws {
        let manifest = URL(fileURLWithPath: #filePath) // …/Tests/FamiliarMeshTests/…
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent() // …/ios
            .deletingLastPathComponent() // repo root
            .appendingPathComponent("crates/mesh/src/ble_classes.txt")
        let lines = try String(contentsOf: manifest, encoding: .utf8)
            .split(separator: "\n")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
        XCTAssertEqual(
            Set(lines), BLESurvey.classes,
            "the Swift survey vocabulary drifted from crates/mesh/src/ble_classes.txt"
        )
    }
}
