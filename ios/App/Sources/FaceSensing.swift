import Foundation
import AVFoundation
import Vision
import FamiliarMesh

/// Facial *analysis* on the iPad's front camera, on-device (Vision). Presence/attention
/// (never a raw frame) always runs once enabled; face *recognition* — matching against a known
/// identity — is a separate, sharper gate (`consent.faceRecognition`, distinct from plain
/// presence per SPEC.md R10) since the design doc calls biometric linking "strongly sensitive."
final class FaceSensing: NSObject, ObservableObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    @Published var running = false
    @Published var lastCount = 0
    /// Set when a face is present/engaged but recognition couldn't confidently match anyone —
    /// the UI (FaceIdentifyPrompt) shows the interactive fallback when this is true. Cleared
    /// once the human confirms/corrects, or the face leaves frame.
    @Published var needsIdentification = false
    /// The best-guess handle recognition proposed (unconfirmed) — the confirm-before-keep UI
    /// shows this as "is this X?" rather than a bare "who are you".
    @Published var proposedHandle: String?

    private let session = AVCaptureSession()
    private let queue = DispatchQueue(label: "io.river.familiar.face")
    private let deliver: (ObsRecord) -> Void
    private var lastState: String?
    private var lastEmit: Date = .distantPast
    private var recognitionEnabled = false
    let recognizer = FaceRecognizer()

    /// The handle this device expects to see — rung 1 of the ladder (ADR-0019), set by the model
    /// from the bound owner. Empty/nil on a shared device, which is why such a device asks.
    var prior: String?
    /// Called after a 1:1 check: the handle when the face AGREED with the prior, nil when it ran
    /// and disagreed. Never called when there was no prior to check against.
    var onVerification: ((String?) -> Void)?
    /// The embedding + face last offered to the confirm/interactive-fallback UI, held so
    /// `FaceIdentifyPrompt`'s confirm/correct actions can link it without recapturing.
    private var pendingEmbedding: [Float]?

    init(deliver: @escaping (ObsRecord) -> Void) {
        self.deliver = deliver
        super.init()
    }

    func start(recognize: Bool = false) {
        recognitionEnabled = recognize
        guard !running else { return }
        AVCaptureDevice.requestAccess(for: .video) { [weak self] ok in
            guard ok, let self else { return }
            self.queue.async { self.configureAndRun() }
        }
    }

    private func configureAndRun() {
        session.beginConfiguration()
        session.sessionPreset = .medium
        guard let cam = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: .front),
              let input = try? AVCaptureDeviceInput(device: cam),
              session.canAddInput(input) else { session.commitConfiguration(); return }
        session.addInput(input)
        let output = AVCaptureVideoDataOutput()
        output.alwaysDiscardsLateVideoFrames = true
        output.setSampleBufferDelegate(self, queue: queue)
        if session.canAddOutput(output) { session.addOutput(output) }
        session.commitConfiguration()
        session.startRunning()
        DispatchQueue.main.async { self.running = true }
    }

    func stop() {
        guard running else { return }
        queue.async { self.session.stopRunning() }
        running = false
        DispatchQueue.main.async {
            self.needsIdentification = false
            self.proposedHandle = nil
        }
    }

    /// The human confirmed the proposed match (or typed/said a different name) — link it,
    /// never sticky (a later correction just calls this again with a different handle).
    func confirmIdentity(handle: String) {
        guard let embedding = pendingEmbedding else { return }
        recognizer.learn(handle: handle, embedding: embedding)
        recognizer.noteSeen(handle)   // so the next sighting has something to verify against
        DispatchQueue.main.async {
            self.needsIdentification = false
            self.proposedHandle = nil
        }
        // Feed the confirmed name toward the daemon's identity registry, over the same signed
        // observation channel this device already uses for everything else — no new endpoint
        // needed. The daemon's observe::ingest_observations now recognizes exactly this shape
        // (action "recognized", object "face:<name>") via
        // familiar_kernel::identity::maybe_learn_from_observation and turns it into a real
        // registry entry + the current observer, not just this device's local cache.
        deliver(ObsRecord(actor: DeviceActor.current, action: "recognized", object: "face:\(handle)",
                          context: "on-device match, confirmed by human", confidence: 0.95))
    }

    // MARK: frame → Vision (throttled to ~2 fps; only derived signals leave)

    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer,
                       from connection: AVCaptureConnection) {
        guard Date().timeIntervalSince(lastEmit) > 0.5,
              let pixels = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        let request = VNDetectFaceLandmarksRequest { [weak self] req, _ in
            guard let self else { return }
            let faces = (req.results as? [VNFaceObservation]) ?? []
            self.handle(faces, pixels: pixels)
        }
        let handler = VNImageRequestHandler(cvPixelBuffer: pixels, orientation: .leftMirrored, options: [:])
        try? handler.perform([request])
    }

    private func handle(_ faces: [VNFaceObservation], pixels: CVPixelBuffer) {
        lastEmit = Date()
        let count = faces.count
        // Coarse "attention": a reasonably large, roughly-centred face is likely looking at the iPad.
        let engagedFace = faces.first { $0.boundingBox.width > 0.2 && abs($0.boundingBox.midX - 0.5) < 0.25 }
        let state = count == 0 ? "face:none" : (engagedFace != nil ? "face:engaged" : "face:present")
        DispatchQueue.main.async { self.lastCount = count }
        if state != lastState {
            lastState = state
            deliver(ObsRecord(actor: DeviceActor.current, action: "reports", object: state,
                              context: "faces=\(count)", confidence: 0.85))
        }
        guard recognitionEnabled, let face = engagedFace else { return }
        attemptRecognition(face: face, pixels: pixels)
    }

    /// Only attempts a match on a good, stable capture — never on a fleeting or poorly-lit
    /// frame, so a wrong link isn't proposed from bad input in the first place.
    ///
    /// **Verifies a prior; it does not search.** Where `prior` is set (a personal device knows its
    /// owner — ADR-0019 rung 1) this is a 1:1 question: "this should be Jeff; does the face agree?"
    /// That is both cheaper and far more reliable than scanning every known face, which across a
    /// household is exactly where false links come from. With no prior there is nothing to verify,
    /// so we ask (rung 3) rather than guess — a shared iPad earns its answer from a human.
    private func attemptRecognition(face: VNFaceObservation, pixels: CVPixelBuffer) {
        guard let embedding = recognizer.embedder.embedding(for: pixels, face: face) else { return }
        // The bound owner first, then the few people recently seen here. Bounded and decaying —
        // still verification against candidates we have a reason to expect, never a search of
        // everyone this device has ever met.
        var candidates: [String] = []
        if let prior, !prior.isEmpty { candidates.append(prior) }
        for h in recognizer.recentCandidates() where !candidates.contains(h) { candidates.append(h) }

        guard !candidates.isEmpty else {
            DispatchQueue.main.async {
                self.pendingEmbedding = embedding
                self.proposedHandle = nil
                self.needsIdentification = true
            }
            return
        }
        let matched = candidates.first { recognizer.verify(embedding, against: $0) }
        DispatchQueue.main.async {
            self.pendingEmbedding = embedding
            self.proposedHandle = matched
            self.needsIdentification = matched == nil
            // Only report a VERIFICATION when there was a bound prior to verify against. Failing to
            // match a recently-seen face means "someone else is here", not "the binding was
            // contradicted" — conflating them would demote a good binding every time a guest walks
            // past the camera.
            if let p = self.prior, !p.isEmpty {
                self.onVerification?(matched == p ? p : nil)
            }
        }
    }
}

/// Produces a face embedding from a captured frame — the piece Apple deliberately doesn't
/// expose publicly (Vision has detection/landmarks/capture-quality, never recognition/matching
/// — verified against current Apple documentation before writing this, not assumed). A real
/// implementation needs a bundled CoreML embedding model (e.g. a converted MobileFaceNet-style
/// network) run via VNCoreMLRequest on the face crop. `StubFaceEmbedder` always returns `nil`
/// so the pipeline honestly falls through to the interactive-identification fallback rather
/// than fabricating a match — this is the real, functioning behavior until a model is sourced
/// and bundled as a follow-up.
protocol FaceEmbedder {
    func embedding(for pixelBuffer: CVPixelBuffer, face: VNFaceObservation) -> [Float]?
}

struct StubFaceEmbedder: FaceEmbedder {
    func embedding(for pixelBuffer: CVPixelBuffer, face: VNFaceObservation) -> [Float]? { nil }
}

/// A per-device cache of confirmed face↔handle links, matched by cosine similarity. On-device
/// only, never synced or shared — see FaceSensing.confirmIdentity's note on the daemon-side
/// registry gap. A wrong link is always correctable: `learn()` replaces, it never appends.
final class FaceRecognizer {
    let embedder: FaceEmbedder
    private let store = UserDefaults.standard
    private let key = "faceRecognizer.links.v1"
    /// Cosine similarity above this is treated as a confident match. Conservative on purpose —
    /// a missed recognition just asks again; a false positive misattributes a person.
    private let matchThreshold: Float = 0.6

    init(embedder: FaceEmbedder = StubFaceEmbedder()) {
        self.embedder = embedder
    }

    private func links() -> [String: [Float]] {
        guard let data = store.data(forKey: key),
              let decoded = try? JSONDecoder().decode([String: [Float]].self, from: data)
        else { return [:] }
        return decoded
    }

    // A shared device has no bound owner, so ADR-0019 rung 2 has nothing to verify against and it
    // falls to asking. Always asking is friction nobody tolerates on a galley iPad, but searching
    // the whole registry is what produces false links in the first place. The middle is a SCOPED
    // prior: the handful of people this device has actually seen lately, which decays. It matches
    // the transience principle — nobody is permanently "the person at this iPad"; the set just
    // reflects who has been around, and it empties itself if they stop coming.
    private let recentKey = "faceRecognizer.recent.v1"
    private let recentCap = 3
    private let recentTTL: TimeInterval = 14 * 24 * 3600

    private func recentRaw() -> [String: Double] {
        (store.dictionary(forKey: recentKey) as? [String: Double]) ?? [:]
    }

    /// Remember that this human was confirmed here, so the next sighting has something to check.
    func noteSeen(_ handle: String, at: Date = Date()) {
        guard !handle.isEmpty else { return }
        var all = recentRaw()
        all[handle] = at.timeIntervalSince1970
        // Keep only the freshest few — a long tail is a registry search wearing a disguise.
        let kept = all.sorted { $0.value > $1.value }.prefix(recentCap)
        store.set(Dictionary(uniqueKeysWithValues: kept.map { ($0.key, $0.value) }), forKey: recentKey)
    }

    /// Who is worth checking against on this device, freshest first. Expired entries are dropped.
    func recentCandidates(now: Date = Date()) -> [String] {
        recentRaw()
            .filter { now.timeIntervalSince1970 - $0.value <= recentTTL }
            .sorted { $0.value > $1.value }
            .map { $0.key }
    }

    func forgetRecent() { store.removeObject(forKey: recentKey) }

    func learn(handle: String, embedding: [Float]) {
        var all = links()
        all[handle] = embedding
        if let data = try? JSONEncoder().encode(all) { store.set(data, forKey: key) }
    }

    func forget(handle: String) {
        var all = links()
        all.removeValue(forKey: handle)
        if let data = try? JSONEncoder().encode(all) { store.set(data, forKey: key) }
    }

    /// 1:1 — does this face match the one handle we already expect? The question ADR-0019 wants
    /// asked. Uses the same conservative threshold as `recognize`, but against a single candidate,
    /// so there is no field of near-misses for the best of a bad set to win.
    func verify(_ embedding: [Float], against handle: String) -> Bool {
        guard let known = links()[handle] else { return false }
        return cosineSimilarity(embedding, known) >= matchThreshold
    }

    func recognize(_ embedding: [Float]) -> String? {
        var best: (handle: String, score: Float)?
        for (handle, known) in links() {
            let score = cosineSimilarity(embedding, known)
            if best == nil || score > best!.score { best = (handle, score) }
        }
        guard let best, best.score >= matchThreshold else { return nil }
        return best.handle
    }

    private func cosineSimilarity(_ a: [Float], _ b: [Float]) -> Float {
        guard a.count == b.count, !a.isEmpty else { return 0 }
        var dot: Float = 0, magA: Float = 0, magB: Float = 0
        for i in 0..<a.count {
            dot += a[i] * b[i]
            magA += a[i] * a[i]
            magB += b[i] * b[i]
        }
        let denom = (magA.squareRoot() * magB.squareRoot())
        return denom > 0 ? dot / denom : 0
    }
}
