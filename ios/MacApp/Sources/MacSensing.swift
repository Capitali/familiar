import Foundation
import AVFoundation
import Speech
import Network

/// Push-to-talk mic capture, mirroring iOS's `VoiceSensing.swift`: on-device transcription
/// only, no audio ever leaves the device. Gated by `allow_microphone` (SPEC.md R9/R1) — the
/// caller must check `MacBoundary.load().allow_microphone` before invoking `start()`.
/// Transcribed text is posted through the daemon's existing `local/answer` seam (the same one
/// the sphere console's text box uses), so this needs no new daemon-side API.
@MainActor
final class MacMicrophone: NSObject, ObservableObject {
    @Published var isListening = false
    @Published var lastTranscript = ""

    private let engine = AVAudioEngine()
    private var recognizer: SFSpeechRecognizer?
    private var request: SFSpeechAudioBufferRecognitionRequest?
    private var task: SFSpeechRecognitionTask?
    /// Called with the final transcript when the human stops talking — the caller (SphereBridge)
    /// posts it to local/answer.
    var onTranscript: ((String) -> Void)?

    func toggle() {
        if isListening { stop() } else { start() }
    }

    func start() {
        guard !isListening else { return }
        SFSpeechRecognizer.requestAuthorization { [weak self] status in
            guard status == .authorized else { return }
            AVCaptureDevice.requestAccess(for: .audio) { granted in
                guard granted else { return }
                Task { @MainActor in self?.beginCapture() }
            }
        }
    }

    private func beginCapture() {
        let recognizer = SFSpeechRecognizer(locale: Locale(identifier: "en-US"))
        self.recognizer = recognizer
        guard let recognizer, recognizer.isAvailable else { return }

        let req = SFSpeechAudioBufferRecognitionRequest()
        req.shouldReportPartialResults = true
        if recognizer.supportsOnDeviceRecognition { req.requiresOnDeviceRecognition = true }
        request = req

        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        input.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak req] buffer, _ in
            req?.append(buffer)
        }
        engine.prepare()
        guard (try? engine.start()) != nil else { return }
        isListening = true

        task = recognizer.recognitionTask(with: req) { [weak self] result, error in
            guard let self else { return }
            if let result {
                Task { @MainActor in self.lastTranscript = result.bestTranscription.formattedString }
                if result.isFinal {
                    Task { @MainActor in self.finish(text: result.bestTranscription.formattedString) }
                }
            }
            if error != nil {
                Task { @MainActor in self.stop() }
            }
        }
    }

    func stop() {
        guard isListening else { return }
        let text = lastTranscript
        finish(text: text)
    }

    private func finish(text: String) {
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        request?.endAudio()
        task?.cancel()
        request = nil
        task = nil
        isListening = false
        if !text.trimmingCharacters(in: .whitespaces).isEmpty { onTranscript?(text) }
        lastTranscript = ""
    }
}

/// Bonjour/mDNS service survey, mirroring iOS's `NetworkDiscovery.swift` (~25 service types).
/// Gated by `allow_network_discovery` (SPEC.md R9/R1). Derived-only by design: reports service
/// kind + advertised instance name, never a resolved address or payload.
///
/// Wired into the daemon via `POST /local/observe` (SphereBridge sets `onDiscovery`) — this app
/// has no NodeKey/membership cert of its own to sign a `/mesh/observe` push with (it talks to
/// the local daemon over plain loopback HTTP, never doing mesh crypto itself), so the loopback
/// seam is its only path to the mesh's observation log.
@MainActor
final class MacNetworkDiscovery: NSObject, ObservableObject {
    @Published var found: [String] = []
    private var browsers: [NWBrowser] = []
    /// Called once per genuinely new (type, instance name) finding — never re-fired for one
    /// already in `found`. The caller (SphereBridge) turns this into a `/local/observe` push.
    var onDiscovery: ((String, String) -> Void)?

    static let serviceTypes = [
        "_familiar-mesh._tcp", "_ssh._tcp", "_sftp-ssh._tcp", "_rfb._tcp", "_http._tcp",
        "_https._tcp", "_airplay._tcp", "_raop._tcp", "_airport._tcp", "_googlecast._tcp",
        "_spotify-connect._tcp", "_ipp._tcp", "_ipps._tcp", "_printer._tcp",
        "_pdl-datastream._tcp", "_homekit._tcp", "_hap._tcp", "_companion-link._tcp",
        "_apple-mobdev2._tcp", "_smb._tcp", "_afpovertcp._tcp", "_daap._tcp", "_dacp._tcp",
        "_mqtt._tcp", "_workstation._tcp", "_device-info._tcp",
    ]

    func start() {
        guard browsers.isEmpty else { return }
        for type in Self.serviceTypes {
            let browser = NWBrowser(for: .bonjour(type: type, domain: nil), using: .tcp)
            browser.browseResultsChangedHandler = { [weak self] results, _ in
                Task { @MainActor in
                    for r in results {
                        if case let .service(name, _, _, _) = r.endpoint {
                            let entry = "\(type):\(name)"
                            guard let self, !self.found.contains(entry) else { continue }
                            self.found.append(entry)
                            self.onDiscovery?(type, name)
                        }
                    }
                }
            }
            browser.start(queue: .main)
            browsers.append(browser)
        }
    }

    func stop() {
        browsers.forEach { $0.cancel() }
        browsers = []
        found = []
    }
}
