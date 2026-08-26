import Foundation

/// The BLE survey's one owned state machine (codex's BLE review, finding 1).
///
/// CoreBluetooth's contract makes the naive shape wrong twice: a default scan COALESCES
/// repeated discoveries of one peripheral into a single event per scan (so clearing a
/// local set does not make later windows see a stationary device again), and a delegate
/// that reacts to `.poweredOn` by scheduling a timer can stack clocks across power
/// cycles while refusal states leak observations collected before the refusal.
///
/// So the machine owns everything and the CoreBluetooth adapter owns nothing: radio
/// transitions and sightings come in; scan commands, timer intent, and window reports go
/// out through injected closures. Scanning is requested WITH duplicate callbacks (the
/// window-local dedup here makes that safe, and the surveyor is foreground-only so the
/// battery cost is bounded and deliberate). Any state but authorized-powered-on stops
/// the scan, disarms the timer, and burns the pending window — nothing collected before
/// a refusal ever leaves after it.
public final class BLEWindowMachine {

    public enum Radio: Equatable {
        /// Powered on; `authorized` is the platform permission's answer.
        case poweredOn(authorized: Bool)
        case poweredOff
        case unauthorized
        case unsupported
        case unknown
    }

    /// `startScan(allowDuplicates:)` — begin scanning (always with duplicates, see above).
    private let startScan: (_ allowDuplicates: Bool) -> Void
    private let stopScan: () -> Void
    /// Arm/disarm the ONE window clock. The adapter maps this to a real repeating timer;
    /// the machine guarantees arm is never called twice without a disarm between.
    private let armTimer: () -> Void
    private let disarmTimer: () -> Void
    private let report: ([(object: String, context: String)]) -> Void

    private var scanning = false
    /// Exposed for the adapter and the regression that proves clocks never stack.
    public private(set) var timerArmed = false
    /// The honest state string surfaced beside the survey toggle.
    public private(set) var state = "not started"

    private var classCounts: [String: Int] = [:]
    private var seenThisWindow = Set<UUID>()

    public init(
        startScan: @escaping (_ allowDuplicates: Bool) -> Void,
        stopScan: @escaping () -> Void,
        armTimer: @escaping () -> Void,
        disarmTimer: @escaping () -> Void,
        report: @escaping ([(object: String, context: String)]) -> Void
    ) {
        self.startScan = startScan
        self.stopScan = stopScan
        self.armTimer = armTimer
        self.disarmTimer = disarmTimer
        self.report = report
    }

    /// A radio/permission transition. Idempotent: repeated `.poweredOn(authorized: true)`
    /// callbacks (CoreBluetooth restores can replay them) never stack a second clock.
    public func radio(_ radio: Radio) {
        switch radio {
        case .poweredOn(authorized: true):
            state = "surveying BLE classes"
            guard !scanning else { return }
            scanning = true
            startScan(true)
            if !timerArmed {
                timerArmed = true
                armTimer()
            }
        case .poweredOn(authorized: false), .unauthorized:
            standDown(to: "Bluetooth permission not granted")
        case .poweredOff:
            standDown(to: "Bluetooth is off")
        case .unsupported:
            standDown(to: "no BLE radio")
        case .unknown:
            standDown(to: "waiting for Bluetooth")
        }
    }

    /// One sighting: the platform's (already per-host-randomized) peripheral id for
    /// window-local dedup, and the advertised service UUIDs — nothing else exists here.
    public func sighting(peripheral: UUID, serviceUUIDs: [String]) {
        guard scanning else { return } // a refused interval collects nothing
        guard seenThisWindow.insert(peripheral).inserted else { return }
        for uuid in serviceUUIDs {
            if let cls = BLESurvey.bleClass(forServiceUUID: uuid) {
                classCounts[cls, default: 0] += 1
            }
        }
    }

    /// The window clock fired: one bounded class-only report leaves, the memory dies.
    public func tick() {
        guard scanning else { return }
        let out = BLESurvey.windowReport(classCounts: classCounts)
        classCounts.removeAll()
        seenThisWindow.removeAll()
        if !out.isEmpty { report(out) }
    }

    /// Every not-authorized-powered-on state: stop, disarm, and BURN the pending window —
    /// observations gathered before a refusal are never flushed after it.
    private func standDown(to newState: String) {
        state = newState
        if scanning {
            scanning = false
            stopScan()
        }
        if timerArmed {
            timerArmed = false
            disarmTimer()
        }
        classCounts.removeAll()
        seenThisWindow.removeAll()
    }
}
