import Foundation
import HealthKit
import CoreMotion
import FamiliarMesh

/// Turns the watch's on-wrist signals into *derived* observations — coarse activity and a bucketed
/// heart rate (elevated / normal) — and hands batches to a delivery closure. Nothing raw leaves the
/// wrist: no bpm stream, no motion vectors — only `motion:<activity>` and `heart_rate:<bucket>`.
final class WatchSensing {
    private let health = HKHealthStore()
    private let motion = CMMotionActivityManager()
    private let deliver: ([ObsRecord]) async -> Void

    private var lastActivity: String?
    private var lastHRBucket: String?
    /// Called with the raw bpm for the on-watch display only (never sent).
    var onHeartRate: ((Int) -> Void)?

    init(deliver: @escaping ([ObsRecord]) async -> Void) {
        self.deliver = deliver
    }

    func start(motionOn: Bool, heartOn: Bool) {
        if motionOn, CMMotionActivityManager.isActivityAvailable() {
            motion.startActivityUpdates(to: .main) { [weak self] activity in
                guard let self, let a = activity else { return }
                let label = Self.activityLabel(a)
                guard label != "unknown", a.confidence != .low, label != self.lastActivity else { return }
                self.lastActivity = label
                let obs = ObsRecord(actor: "watch:ian", action: "reports", object: "motion:\(label)",
                                    context: "confidence=\(a.confidence.rawValue)",
                                    confidence: a.confidence == .high ? 0.9 : 0.7)
                Task { await self.deliver([obs]) }
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
            let obs = ObsRecord(actor: "watch:ian", action: "reports", object: "heart_rate:\(bucket)",
                                context: "bpm~\(bpm)", confidence: 0.9)
            Task { await self.deliver([obs]) }
        }
        health.execute(q)
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
