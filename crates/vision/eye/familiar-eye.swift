// familiar-eye — the familiar's gated still-camera helper.
//
// Captures a single frame from a camera and writes it to a file as JPEG, then exits. It is
// the *watching* act the `vision` crate reserves for the gated reach layer: the Rust side
// only ever invokes this after confirming the boundary's `allow_camera`. Keeping the actual
// AVFoundation call in a tiny bundled helper means the macOS camera permission (TCC) attaches
// to the signed `Familiar.app` that contains it — not to whatever terminal launched a build.
//
// Usage:  familiar-eye <output.jpg> [camera-name-substring]
// Exit:   0 ok · 2 usage · 3 permission denied · 4 no device · 5 session error · 6 timeout
//
// Deliberately dependency-free (system frameworks only) and fail-fast: a hard timeout means
// it can never hang the always-on daemon that calls it.

import AVFoundation
import CoreGraphics
import CoreImage
import CoreMedia
import CoreVideo
import Foundation

let args = CommandLine.arguments
guard args.count >= 2 else {
    FileHandle.standardError.write(Data("usage: familiar-eye <output.jpg> [camera-name]\n".utf8))
    exit(2)
}
let outPath = args[1]
let wantName: String? = args.count >= 3 ? args[2] : nil

func fail(_ code: Int32, _ msg: String) -> Never {
    FileHandle.standardError.write(Data("familiar-eye: \(msg)\n".utf8))
    exit(code)
}

// 1. Authorization. Blocks until the user answers the system prompt the first time; after
//    that the stored TCC decision returns immediately.
func ensureAuthorized() -> Bool {
    switch AVCaptureDevice.authorizationStatus(for: .video) {
    case .authorized:
        return true
    case .notDetermined:
        let sem = DispatchSemaphore(value: 0)
        var granted = false
        AVCaptureDevice.requestAccess(for: .video) { ok in
            granted = ok
            sem.signal()
        }
        sem.wait()
        return granted
    default:
        return false
    }
}
guard ensureAuthorized() else { fail(3, "camera access denied") }

// 2. Pick a device — the named one if asked, else the system default video camera (the
//    built-in FaceTime HD on this Mac).
func pickDevice() -> AVCaptureDevice? {
    var types: [AVCaptureDevice.DeviceType] = [.builtInWideAngleCamera]
    if #available(macOS 14.0, *) {
        types.append(.external)
        types.append(.continuityCamera)
    }
    let discovery = AVCaptureDevice.DiscoverySession(
        deviceTypes: types, mediaType: .video, position: .unspecified)
    if let want = wantName,
        let match = discovery.devices.first(where: {
            $0.localizedName.localizedCaseInsensitiveContains(want)
        })
    {
        return match
    }
    return AVCaptureDevice.default(for: .video) ?? discovery.devices.first
}
guard let device = pickDevice() else { fail(4, "no camera device") }

// 3. Build a minimal capture session feeding a video-data output.
let session = AVCaptureSession()
session.sessionPreset = .photo
guard let input = try? AVCaptureDeviceInput(device: device), session.canAddInput(input) else {
    fail(5, "cannot open camera input")
}
session.addInput(input)

let output = AVCaptureVideoDataOutput()
output.videoSettings = [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA]

final class Grabber: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    let dest: String
    let done = DispatchSemaphore(value: 0)
    var ok = false
    private var seen = 0

    init(dest: String) { self.dest = dest }

    func captureOutput(
        _ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        seen += 1
        // Drop the first few frames so auto-exposure/white-balance settle — otherwise the
        // saved still is often black or badly lit.
        guard seen >= 5 else { return }
        guard let pixel = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        let image = CIImage(cvImageBuffer: pixel)
        let context = CIContext()
        do {
            try context.writeJPEGRepresentation(
                of: image, to: URL(fileURLWithPath: dest),
                colorSpace: CGColorSpaceCreateDeviceRGB(), options: [:])
            ok = true
        } catch {
            ok = false
        }
        done.signal()
    }
}

let grabber = Grabber(dest: outPath)
let queue = DispatchQueue(label: "io.river.familiar.eye")
output.setSampleBufferDelegate(grabber, queue: queue)
guard session.canAddOutput(output) else { fail(5, "cannot add camera output") }
session.addOutput(output)

session.startRunning()
let result = grabber.done.wait(timeout: .now() + 8.0)
session.stopRunning()

switch result {
case .success:
    exit(grabber.ok ? 0 : 5)
case .timedOut:
    fail(6, "timed out waiting for a frame")
}
