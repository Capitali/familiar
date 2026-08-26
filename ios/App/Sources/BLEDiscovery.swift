import CoreBluetooth
import FamiliarMesh
import Foundation

/// Surveys nearby BLE advertisements and reports CLASSES — the phone is the household's
/// always-present radio (T-228, Ian: "every client is an observatory"), and this is the
/// radio the shells never had.
///
/// **What may be said** (Q3, closed): the service-UUID class plus a coarse per-window
/// count — `ble:heart-rate / seen=few`. Never a peripheral name, manufacturer bytes,
/// advertisement payload, or platform identifier; never a cross-window token. The only
/// per-device state is `seenThisWindow`, an in-memory set of CoreBluetooth's (already
/// per-host-randomized) peripheral ids used to avoid double-counting one device inside
/// one window — it is cleared every window and never leaves this object.
///
/// **Authorization**: armed and stood down by `AppModel.startDiscoveryIfAuthorized`
/// under exactly the gates the Bonjour survey rides — the household boundary's
/// `allow_network_discovery` ∧ the device's narrowing preference — and the platform's
/// own Bluetooth permission is the second half of the same authorization: without it
/// this object reports an honest state and scans nothing.
///
/// **Actuation is not this file.** Driving a BLE device (the lights witness) needs a
/// declared surface, a pairing ceremony, and `allow_actuate` — a survey only looks.
final class BLEDiscovery: NSObject, CBCentralManagerDelegate {
    /// One survey window: sightings accumulate per class, then one bounded report leaves.
    static let windowSecs: TimeInterval = 60

    private let deliver: ([ObsRecord]) async -> Void
    private var central: CBCentralManager?
    private var classCounts: [String: Int] = [:]
    private var seenThisWindow = Set<UUID>()
    private var windowTimer: Timer?

    /// The honest state string surfaced beside the survey toggle.
    private(set) var state = "not started"

    init(deliver: @escaping ([ObsRecord]) async -> Void) {
        self.deliver = deliver
        super.init()
    }

    func start() {
        stop()
        // The manager prompts for Bluetooth permission on first creation; from then on the
        // authorization is the human's standing answer, reported honestly either way.
        central = CBCentralManager(delegate: self, queue: nil)
    }

    func stop() {
        windowTimer?.invalidate()
        windowTimer = nil
        central?.stopScan()
        central = nil
        classCounts.removeAll()
        seenThisWindow.removeAll()
        state = "not started"
    }

    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        switch central.state {
        case .poweredOn:
            guard CBCentralManager.authorization == .allowedAlways else {
                state = "Bluetooth permission not granted"
                return
            }
            state = "surveying BLE classes"
            central.scanForPeripherals(withServices: nil, options: nil)
            windowTimer = Timer.scheduledTimer(
                withTimeInterval: Self.windowSecs, repeats: true
            ) { [weak self] _ in self?.closeWindow() }
        case .unauthorized: state = "Bluetooth permission not granted"
        case .poweredOff: state = "Bluetooth is off"
        case .unsupported: state = "no BLE radio"
        default: state = "waiting for Bluetooth"
        }
    }

    func centralManager(
        _ central: CBCentralManager,
        didDiscover peripheral: CBPeripheral,
        advertisementData: [String: Any],
        rssi RSSI: NSNumber
    ) {
        // One device counts once per window, keyed on the platform's own (per-host
        // randomized) peripheral id — window-local memory only.
        guard seenThisWindow.insert(peripheral.identifier).inserted else { return }
        let advertised =
            (advertisementData[CBAdvertisementDataServiceUUIDsKey] as? [CBUUID]) ?? []
        // Each advertised service maps to a repo-authored class or to nothing at all —
        // an unknown vendor UUID is never named, and nothing else in the advertisement
        // (name, manufacturer data, payload) is even read.
        for uuid in advertised {
            if let cls = BLESurvey.bleClass(forServiceUUID: uuid.uuidString) {
                classCounts[cls, default: 0] += 1
            }
        }
    }

    /// End of a window: one bounded, class-only report leaves; every per-window memory dies.
    private func closeWindow() {
        let report = BLESurvey.windowReport(classCounts: classCounts)
        classCounts.removeAll()
        seenThisWindow.removeAll()
        guard !report.isEmpty else { return }
        let actor = DeviceActor.current
        let batch = report.map { entry in
            ObsRecord(
                actor: actor, action: "discovered",
                object: entry.object, context: entry.context, confidence: 0.9
            )
        }
        Task { await deliver(batch) }
    }
}
