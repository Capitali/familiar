import SwiftUI
#if os(iOS)
import AVFoundation
import UIKit

/// A camera view that calls back with the first QR payload it reads. iOS only; the package's
/// PairingView takes it through `PairingScanner`.
public struct QRScannerView: UIViewControllerRepresentable {
    public var onScan: (String) -> Void
    public init(onScan: @escaping (String) -> Void) { self.onScan = onScan }

    public func makeUIViewController(context: Context) -> ScannerVC { let vc = ScannerVC(); vc.onScan = onScan; return vc }
    public func updateUIViewController(_ vc: ScannerVC, context: Context) {}

    public final class ScannerVC: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
        var onScan: ((String) -> Void)?
        private let session = AVCaptureSession()
        private var fired = false

        public override func viewDidLoad() {
            super.viewDidLoad()
            view.backgroundColor = .black
            guard let device = AVCaptureDevice.default(for: .video), let input = try? AVCaptureDeviceInput(device: device), session.canAddInput(input) else { return }
            session.addInput(input)
            let output = AVCaptureMetadataOutput()
            guard session.canAddOutput(output) else { return }
            session.addOutput(output)
            output.setMetadataObjectsDelegate(self, queue: .main)
            output.metadataObjectTypes = [.qr]
            let preview = AVCaptureVideoPreviewLayer(session: session)
            preview.videoGravity = .resizeAspectFill
            preview.frame = view.bounds
            view.layer.addSublayer(preview)
            DispatchQueue.global(qos: .userInitiated).async { [session] in session.startRunning() }
        }

        public override func viewWillDisappear(_ animated: Bool) { super.viewWillDisappear(animated); session.stopRunning() }

        public func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from connection: AVCaptureConnection) {
            guard !fired, let code = (objects.first as? AVMetadataMachineReadableCodeObject)?.stringValue else { return }
            fired = true
            onScan?(code)
        }
    }
}

/// The scanner as the pairing sheet wants it: a sheet with a cancel button.
public struct QRScanSheet: View {
    public let onScan: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    public init(onScan: @escaping (String) -> Void) { self.onScan = onScan }
    public var body: some View {
        NavigationStack {
            QRScannerView { code in onScan(code); dismiss() }
                .ignoresSafeArea()
                .navigationTitle("Scan the key")
                .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        }
    }
}

public extension PairingScanner {
    /// The package's own camera scanner.
    static var camera: PairingScanner { PairingScanner { done in AnyView(QRScanSheet(onScan: done)) } }
}
#endif
