import Foundation
import AVFoundation
import Vision

/// macOS equivalent of the iOS app's face presence + recognition pipeline (SPEC.md R9/R10/A4) —
/// lives here, in the GUI app, per A2's decision that headless peers (the daemon) never touch
/// the camera. Same duplicated-per-platform pattern the rest of this codebase's sensing already
/// follows (no shared sensing abstraction exists between iOS/macOS today).
///
/// FaceEmbedder/StubFaceEmbedder here duplicate iOS App/Sources/FaceSensing.swift's — see that
/// file's doc comment for why: Vision has no public recognition/embedding API (verified against
/// Apple's current documentation), so this is honestly stubbed until a bundled CoreML model
/// exists. The stub always returns nil, so the pipeline falls through to the interactive
/// fallback rather than fabricating a match.
protocol MacFaceEmbedder {
    func embedding(for pixelBuffer: CVPixelBuffer, face: VNFaceObservation) -> [Float]?
}

struct MacStubFaceEmbedder: MacFaceEmbedder {
    func embedding(for pixelBuffer: CVPixelBuffer, face: VNFaceObservation) -> [Float]? { nil }
}

/// Per-device cache of confirmed face↔handle links — same cosine-similarity match as iOS's
/// FaceRecognizer, same "never synced, never shared" scope, same correctable-not-sticky discipline.
final class MacFaceRecognizer {
    let embedder: MacFaceEmbedder
    private let store = UserDefaults.standard
    private let key = "macFaceRecognizer.links.v1"
    private let matchThreshold: Float = 0.6

    init(embedder: MacFaceEmbedder = MacStubFaceEmbedder()) {
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
            dot += a[i] * b[i]; magA += a[i] * a[i]; magB += b[i] * b[i]
        }
        let denom = magA.squareRoot() * magB.squareRoot()
        return denom > 0 ? dot / denom : 0
    }
}

@MainActor
final class MacFaceSensing: NSObject, ObservableObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    @Published var running = false
    @Published var lastCount = 0
    @Published var needsIdentification = false
    @Published var proposedHandle: String?

    private let session = AVCaptureSession()
    private let queue = DispatchQueue(label: "io.river.familiar.mac.face")
    private var lastEmit: Date = .distantPast
    private var recognitionOn = false
    let recognizer = MacFaceRecognizer()
    private var pendingEmbedding: [Float]?
    /// Called with the confirmed handle — the caller (SphereBridge) turns this into a
    /// `POST /local/observe {"action":"recognized","object":"face:<handle>"}` push, which
    /// `identity::maybe_learn_from_observation` on the daemon side turns into a real registry
    /// entry, not just this on-device cache.
    var onIdentityConfirmed: ((String) -> Void)?

    func start(recognize: Bool) {
        recognitionOn = recognize
        guard !running else { return }
        guard let cam = AVCaptureDevice.default(for: .video),
              let input = try? AVCaptureDeviceInput(device: cam) else { return }
        session.beginConfiguration()
        session.sessionPreset = .medium
        guard session.canAddInput(input) else { session.commitConfiguration(); return }
        session.addInput(input)
        let output = AVCaptureVideoDataOutput()
        output.alwaysDiscardsLateVideoFrames = true
        output.setSampleBufferDelegate(self, queue: queue)
        if session.canAddOutput(output) { session.addOutput(output) }
        session.commitConfiguration()
        queue.async { self.session.startRunning() }
        running = true
    }

    func stop() {
        guard running else { return }
        queue.async { self.session.stopRunning() }
        running = false
        needsIdentification = false
        proposedHandle = nil
    }

    func setRecognition(_ on: Bool) { recognitionOn = on }

    func confirmIdentity(handle: String) {
        guard let embedding = pendingEmbedding else { return }
        recognizer.learn(handle: handle, embedding: embedding)
        needsIdentification = false
        proposedHandle = nil
        onIdentityConfirmed?(handle)
    }

    nonisolated func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer,
                                    from connection: AVCaptureConnection) {
        guard let pixels = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        let request = VNDetectFaceLandmarksRequest { [weak self] req, _ in
            let faces = (req.results as? [VNFaceObservation]) ?? []
            Task { @MainActor in self?.handle(faces, pixels: pixels) }
        }
        let handler = VNImageRequestHandler(cvPixelBuffer: pixels, orientation: .up, options: [:])
        try? handler.perform([request])
    }

    private func handle(_ faces: [VNFaceObservation], pixels: CVPixelBuffer) {
        guard Date().timeIntervalSince(lastEmit) > 0.5 else { return }
        lastEmit = Date()
        lastCount = faces.count
        guard recognitionOn, let face = faces.first(where: { $0.boundingBox.width > 0.15 }) else { return }
        guard let embedding = recognizer.embedder.embedding(for: pixels, face: face) else { return }
        pendingEmbedding = embedding
        if let handle = recognizer.recognize(embedding) {
            proposedHandle = handle
            needsIdentification = false
        } else {
            proposedHandle = nil
            needsIdentification = true
        }
    }
}
