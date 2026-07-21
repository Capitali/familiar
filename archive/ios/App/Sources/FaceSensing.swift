import Foundation
import AVFoundation
import Vision
import FamiliarMesh

/// Facial *analysis* on the iPad's front camera, on-device (Vision). It derives only presence and
/// coarse attention — how many faces, and whether someone is roughly facing the device — never a
/// raw frame, never an identity (recognising *who* is a later, separately-consented brick). Feeds
/// the familiar's sense of who is present (Law II). Consent-gated by the camera permission.
final class FaceSensing: NSObject, ObservableObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    @Published var running = false
    @Published var lastCount = 0

    private let session = AVCaptureSession()
    private let queue = DispatchQueue(label: "io.river.familiar.face")
    private let deliver: (ObsRecord) -> Void
    private var lastState: String?
    private var lastEmit: Date = .distantPast

    init(deliver: @escaping (ObsRecord) -> Void) {
        self.deliver = deliver
        super.init()
    }

    func start() {
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
    }

    // MARK: frame → Vision (throttled to ~2 fps; only derived signals leave)

    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer,
                       from connection: AVCaptureConnection) {
        guard Date().timeIntervalSince(lastEmit) > 0.5,
              let pixels = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        let request = VNDetectFaceRectanglesRequest { [weak self] req, _ in
            guard let self else { return }
            let faces = (req.results as? [VNFaceObservation]) ?? []
            self.handle(faces)
        }
        let handler = VNImageRequestHandler(cvPixelBuffer: pixels, orientation: .leftMirrored, options: [:])
        try? handler.perform([request])
    }

    private func handle(_ faces: [VNFaceObservation]) {
        lastEmit = Date()
        let count = faces.count
        // Coarse "attention": a reasonably large, roughly-centred face is likely looking at the iPad.
        let engaged = faces.contains { $0.boundingBox.width > 0.2 && abs($0.boundingBox.midX - 0.5) < 0.25 }
        let state = count == 0 ? "face:none" : (engaged ? "face:engaged" : "face:present")
        DispatchQueue.main.async { self.lastCount = count }
        guard state != lastState else { return }
        lastState = state
        deliver(ObsRecord(actor: DeviceActor.current, action: "reports", object: state,
                          context: "faces=\(count)", confidence: 0.85))
    }
}
