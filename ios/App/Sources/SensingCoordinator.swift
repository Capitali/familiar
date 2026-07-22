import Foundation
import CoreLocation
import CoreMotion
import FamiliarMesh

/// Turns the phone's location + motion into *derived* observations — home/away and coarse activity —
/// and hands batches to a delivery closure. Nothing raw leaves the device: no coordinates are sent,
/// only `location:home|away` and `motion:<activity>`. Phase 1's single sensor pair; HealthKit/audio
/// come later.
final class SensingCoordinator: NSObject, CLLocationManagerDelegate {
    private let location = CLLocationManager()
    private let motion = CMMotionActivityManager()
    private let deliver: ([ObsRecord]) async -> Void

    private var homeRegionKey = "home.region"
    private var lastPlace: String?
    private var lastActivity: String?
    private var current: CLLocation?

    /// The most recent fix, for consent-gated reporting (worldview reads carry it so the mesh
    /// can place this device). Nil until location sensing has produced one.
    var lastCoordinate: (lat: Double, lon: Double)? {
        current.map { ($0.coordinate.latitude, $0.coordinate.longitude) }
    }

    init(deliver: @escaping ([ObsRecord]) async -> Void) {
        self.deliver = deliver
        super.init()
        location.delegate = self
        location.desiredAccuracy = kCLLocationAccuracyHundredMeters
        location.allowsBackgroundLocationUpdates = true
        location.pausesLocationUpdatesAutomatically = false
    }

    /// The covenant baseline: every device with GPS provides its position to the mesh —
    /// location matters to the familiar's understanding of patterns and locality. This
    /// starts position updates only; the richer derived sensing (home/away observations,
    /// motion) remains behind its own consent toggles via `start`.
    func startFixBaseline() {
        location.requestAlwaysAuthorization()
        location.startMonitoringSignificantLocationChanges()
    }

    func start(location wantLocation: Bool, motion wantMotion: Bool) {
        derivedLocationObs = wantLocation
        if wantLocation {
            location.requestAlwaysAuthorization()
            location.startMonitoringSignificantLocationChanges()
        }
        if wantMotion, CMMotionActivityManager.isActivityAvailable() {
            motion.startActivityUpdates(to: .main) { [weak self] activity in
                guard let self, let a = activity else { return }
                let label = Self.activityLabel(a)
                // Ignore "unknown" (the classifier's "don't know") and low-confidence flaps —
                // otherwise a stationary phone oscillates still↔unknown and floods the familiar.
                // Only a *changed*, confident activity is worth an observation.
                guard label != "unknown", a.confidence != .low else { return }
                guard label != self.lastActivity else { return }
                self.lastActivity = label
                let obs = ObsRecord(actor: DeviceActor.current, action: "reports",
                                    object: "motion:\(label)", context: "confidence=\(a.confidence.rawValue)",
                                    confidence: a.confidence == .high ? 0.9 : 0.7)
                Task { await self.deliver([obs]) }
            }
        }
    }

    func stop() {
        // The fix baseline continues (covenant contract); only derived sensing stops.
        derivedLocationObs = false
        motion.stopActivityUpdates()
    }

    /// Whether location updates should also emit derived home/away observations (consent).
    private var derivedLocationObs = false

    /// Anchor "home" at the current location (a user gesture). Home/away is derived from this.
    func markHomeAtCurrent() {
        guard let c = current else { return }
        UserDefaults.standard.set([c.coordinate.latitude, c.coordinate.longitude], forKey: homeRegionKey)
    }

    // MARK: CLLocationManagerDelegate

    func locationManager(_ m: CLLocationManager, didUpdateLocations locs: [CLLocation]) {
        guard let loc = locs.last else { return }
        current = loc
        guard derivedLocationObs else { return }   // baseline mode: hold the fix, no observations
        let place = placeLabel(for: loc)
        guard place != lastPlace else { return }
        lastPlace = place
        let obs = ObsRecord(actor: DeviceActor.current, action: "reports",
                            object: "location:\(place)", context: "acc=\(Int(loc.horizontalAccuracy))m",
                            confidence: 0.9)
        Task { await deliver([obs]) }
    }

    func locationManagerDidChangeAuthorization(_ m: CLLocationManager) {
        if m.authorizationStatus == .authorizedAlways || m.authorizationStatus == .authorizedWhenInUse {
            m.startMonitoringSignificantLocationChanges()
        }
    }

    // MARK: derivation

    private func placeLabel(for loc: CLLocation) -> String {
        guard let home = UserDefaults.standard.array(forKey: homeRegionKey) as? [Double], home.count == 2 else {
            return "unknown"
        }
        let homeLoc = CLLocation(latitude: home[0], longitude: home[1])
        return loc.distance(from: homeLoc) < 150 ? "home" : "away"
    }

    private static func activityLabel(_ a: CMMotionActivity) -> String {
        if a.automotive { return "driving" }
        if a.cycling { return "cycling" }
        if a.running { return "running" }
        if a.walking { return "walking" }
        if a.stationary { return "still" }
        return "unknown"
    }
}
