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
        DispatchQueue.main.async {
            self.needsIdentification = false
            self.proposedHandle = nil
        }
        // Feed the confirmed name toward the daemon's identity registry. NOTE: as of this
        // change, familiar_kernel::identity::remember() has no production trigger anywhere in
        // the daemon (only called from tests) — the general "human introduces themselves"
        // pipeline (docs/UI-DESIGN-BRIEF.md's "next phase") isn't built yet, and /local/*
        // endpoints are loopback-only (unreachable from a phone). Wiring this to the daemon's
        // registry needs a new signed mesh endpoint — left for a follow-up rather than
        // inventing that endpoint under this task's scope. For now the link is real and
        // useful on-device (this phone recognizes this face next time) even though it doesn't
        // yet reach the daemon.
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
    private func attemptRecognition(face: VNFaceObservation, pixels: CVPixelBuffer) {
        guard let embedding = recognizer.embedder.embedding(for: pixels, face: face) else { return }
        if let handle = recognizer.recognize(embedding) {
            DispatchQueue.main.async {
                self.pendingEmbedding = embedding
                self.proposedHandle = handle
                self.needsIdentification = false
            }
        } else {
            DispatchQueue.main.async {
                self.pendingEmbedding = embedding
                self.proposedHandle = nil
                self.needsIdentification = true
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
