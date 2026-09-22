import Foundation
import HealthKit
import CoreMotion
import CoreLocation
import FamiliarMesh

/// Turns the watch's on-wrist signals into *derived* observations — coarse activity, a bucketed
/// heart rate (elevated / normal), and the wearer's position — and hands batches to a delivery
/// closure. Nothing raw beyond a coarse fix leaves the wrist: no bpm stream, no motion vectors —
/// only `motion:<activity>`, `heart_rate:<bucket>`, and a rounded `location`. The watch is the
/// wearer, so its fix is the wearer's presence in the world (the mesh map + geo, ADR-0008 geo).
final class WatchSensing: NSObject, CLLocationManagerDelegate {
    private let health = HKHealthStore()
    private let motion = CMMotionActivityManager()
    private let deviceMotion = CMMotionManager()
    private let locator = CLLocationManager()
    private let deliver: ([ObsRecord]) async -> Void

    /// Who this watch is serving — the paired phone's served human (ADR-0016). Reports are tagged
    /// `watch:<servedHuman>` so they attribute to the same person as `phone:<servedHuman>`, not a
    /// baked "ian". Set by WatchModel from the phone hand-off before `start`.
    var servedHuman = "observer"

    private var lastActivity: String?
    private var lastHRBucket: String?
    private var lastGyroBucket: String?
    private var lastFix: (lat: Double, lon: Double)?
    /// Called with the raw bpm for the on-watch display only (never sent).
    var onHeartRate: ((Int) -> Void)?
    /// The freshest coarse fix (lat, lon) for the mesh geo — read by the model on each observe push.
    private(set) var coordinate: (lat: Double, lon: Double)?

    init(deliver: @escaping ([ObsRecord]) async -> Void) {
        self.deliver = deliver
        super.init()
        locator.delegate = self
    }

    func start(motionOn: Bool, heartOn: Bool, locationOn: Bool = false) {
        if locationOn {
            locator.requestWhenInUseAuthorization()
            locator.desiredAccuracy = kCLLocationAccuracyHundredMeters   // coarse; the wrist isn't a survey tool
            locator.distanceFilter = 100                                  // only meaningful moves
            locator.startUpdatingLocation()
        }
        if motionOn, CMMotionActivityManager.isActivityAvailable() {
            motion.startActivityUpdates(to: .main) { [weak self] activity in
                guard let self, let a = activity else { return }
                let label = Self.activityLabel(a)
                guard label != "unknown", a.confidence != .low, label != self.lastActivity else { return }
                self.lastActivity = label
                let obs = ObsRecord(actor: "watch:\(servedHuman)", action: "reports", object: "motion:\(label)",
                                    context: "confidence=\(a.confidence.rawValue)",
                                    confidence: a.confidence == .high ? 0.9 : 0.7)
                Task { await self.deliver([obs]) }
            }
            // Gyroscope: a coarse rotation state (still / turning / spinning) from the rotation-rate
            // magnitude — same privacy posture as the rest (a bucket, never the raw vector), sampled
            // at 1 Hz and reported only when the bucket changes. Shares the motion consent.
            if deviceMotion.isDeviceMotionAvailable {
                deviceMotion.deviceMotionUpdateInterval = 1.0
                deviceMotion.startDeviceMotionUpdates(to: .main) { [weak self] dm, _ in
                    guard let self, let dm else { return }
                    let r = dm.rotationRate
                    let mag = (r.x * r.x + r.y * r.y + r.z * r.z).squareRoot()   // rad/s
                    let bucket = mag > 2.0 ? "spinning" : (mag > 0.5 ? "turning" : "still")
                    guard bucket != self.lastGyroBucket else { return }
                    self.lastGyroBucket = bucket
                    let obs = ObsRecord(actor: "watch:\(servedHuman)", action: "reports", object: "gyro:\(bucket)",
                                        context: "rate~\(String(format: "%.1f", mag))", confidence: 0.8)
                    Task { await self.deliver([obs]) }
                }
            }
        }
        if heartOn, HKHealthStore.isHealthDataAvailable() {
            let hr = HKQuantityType(.heartRate)
            health.requestAuthorization(toShare: [], read: [hr]) { [weak self] ok, _ in
                if ok { self?.observeHeartRate(hr) }
            }
        }
    }

    private func observeHeartRate(_ hr: HKQuantityType) {
        let observer = HKObserverQuery(sampleType: hr, predicate: nil) { [weak self] _, done, _ in
            self?.readLatestHeartRate(hr)
            done()
        }
        health.execute(observer)
        // Prime once so we have a value without waiting for the first change.
        readLatestHeartRate(hr)
    }

    private func readLatestHeartRate(_ hr: HKQuantityType) {
        let sort = [NSSortDescriptor(key: HKSampleSortIdentifierEndDate, ascending: false)]
        let q = HKSampleQuery(sampleType: hr, predicate: nil, limit: 1, sortDescriptors: sort) {
            [weak self] _, samples, _ in
            guard let self, let s = samples?.first as? HKQuantitySample else { return }
            let bpm = Int(s.quantity.doubleValue(for: HKUnit.count().unitDivided(by: .minute())))
            self.onHeartRate?(bpm)
            let bucket = bpm > 100 ? "elevated" : (bpm < 50 ? "low" : "normal")
            guard bucket != self.lastHRBucket else { return }
            self.lastHRBucket = bucket
            let obs = ObsRecord(actor: "watch:\(servedHuman)", action: "reports", object: "heart_rate:\(bucket)",
                                context: "bpm~\(bpm)", confidence: 0.9)
            Task { await self.deliver([obs]) }
        }
        health.execute(q)
    }

    // A coarse fix — rounded to ~100m (3 decimals) so it locates the wearer for the mesh map and
    // geo without pinpointing them. Emitted only on a meaningful move (distanceFilter) and dedup'd.
    func locationManager(_ m: CLLocationManager, didUpdateLocations locs: [CLLocation]) {
        guard let l = locs.last else { return }
        let lat = (l.coordinate.latitude * 1000).rounded() / 1000
        let lon = (l.coordinate.longitude * 1000).rounded() / 1000
        coordinate = (lat, lon)
        if let f = lastFix, abs(f.lat - lat) < 0.0005, abs(f.lon - lon) < 0.0005 { return }
        lastFix = (lat, lon)
        let obs = ObsRecord(actor: "watch:\(servedHuman)", action: "reports",
                            object: "location:\(lat),\(lon)",
                            context: "accuracy=coarse", confidence: 0.85)
        Task { await self.deliver([obs]) }
    }

    private static func activityLabel(_ a: CMMotionActivity) -> String {
        if a.running { return "running" }
        if a.walking { return "walking" }
        if a.cycling { return "cycling" }
        if a.automotive { return "driving" }
        if a.stationary { return "still" }
        return "unknown"
    }
}
