import CoreBluetooth
import FamiliarMesh
import Foundation

/// The CoreBluetooth adapter over [`BLEWindowMachine`] — the machine owns the survey's
/// entire state (scan lifecycle, the one window clock, window-local memory, refusal
/// semantics); this object only translates: delegate callbacks in, real scan/timer
/// commands out. See the machine's doc for the two CoreBluetooth contracts that make
/// this split load-bearing rather than taste (discovery coalescing and clock stacking).
///
/// **What may be said** (T-228 Q3, closed): the service-UUID class plus a coarse
/// per-window count. Nothing else in an advertisement is even read. Actuation is not
/// this file — a survey only looks.
final class BLEDiscovery: NSObject, CBCentralManagerDelegate {
    /// One survey window.
    static let windowSecs: TimeInterval = 60

    private var central: CBCentralManager?
    private var machine: BLEWindowMachine?
    private var windowTimer: Timer?

    /// The honest state string surfaced beside the survey toggle.
    var state: String { machine?.state ?? "not started" }

    init(deliver: @escaping ([ObsRecord]) async -> Void) {
        super.init()
        // The machine drives; these closures are its only hands.
        machine = BLEWindowMachine(
            startScan: { [weak self] allowDuplicates in
                // Duplicates ON, deliberately: the default scan coalesces a stationary
                // peripheral into one event per scan, which would empty every window
                // after the first. The machine's window-local dedup bounds the cost,
                // and the surveyor is foreground-only.
                self?.central?.scanForPeripherals(
                    withServices: nil,
                    options: [CBCentralManagerScanOptionAllowDuplicatesKey: allowDuplicates]
                )
            },
            stopScan: { [weak self] in self?.central?.stopScan() },
            armTimer: { [weak self] in
                guard let self else { return }
                self.windowTimer?.invalidate() // belt to the machine's braces
                self.windowTimer = Timer.scheduledTimer(
                    withTimeInterval: Self.windowSecs, repeats: true
                ) { [weak self] _ in self?.machine?.tick() }
            },
            disarmTimer: { [weak self] in
                self?.windowTimer?.invalidate()
                self?.windowTimer = nil
            },
            report: { entries in
                let actor = DeviceActor.current
                let batch = entries.map { entry in
                    ObsRecord(
                        actor: actor, action: "discovered",
                        object: entry.object, context: entry.context, confidence: 0.9
                    )
                }
                Task { await deliver(batch) }
            }
        )
    }

    func start() {
        // The manager prompts for Bluetooth permission on first creation; every state
        // after that reaches the machine through the delegate below.
        central = CBCentralManager(delegate: self, queue: nil)
    }

    func stop() {
        machine?.radio(.unknown) // stands everything down: scan, clock, pending window
        central = nil
    }

    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        switch central.state {
        case .poweredOn:
            machine?.radio(.poweredOn(authorized: CBCentralManager.authorization == .allowedAlways))
        case .unauthorized: machine?.radio(.unauthorized)
        case .poweredOff: machine?.radio(.poweredOff)
        case .unsupported: machine?.radio(.unsupported)
        default: machine?.radio(.unknown)
        }
    }

    func centralManager(
        _ central: CBCentralManager,
        didDiscover peripheral: CBPeripheral,
        advertisementData: [String: Any],
        rssi RSSI: NSNumber
    ) {
        let advertised =
            (advertisementData[CBAdvertisementDataServiceUUIDsKey] as? [CBUUID]) ?? []
        machine?.sighting(
            peripheral: peripheral.identifier,
            serviceUUIDs: advertised.map(\.uuidString)
        )
    }
}
