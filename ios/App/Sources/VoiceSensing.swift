import Foundation
import Speech
import AVFoundation
import FamiliarMesh

/// Push-to-talk voice, on-device. The human taps to speak (so it's consent-driven, not ambient
/// eavesdropping); `SFSpeechRecognizer` transcribes on-device (`requiresOnDeviceRecognition`) and the
/// final transcript becomes an observation the familiar can act on. No audio leaves the device.
final class VoiceSensing: NSObject, ObservableObject {
    @Published var listening = false
    @Published var partial = ""
    @Published var available = false

    private let recognizer = SFSpeechRecognizer()
    private let engine = AVAudioEngine()
    private var request: SFSpeechAudioBufferRecognitionRequest?
    private var task: SFSpeechRecognitionTask?
    private let deliver: (ObsRecord) -> Void

    init(deliver: @escaping (ObsRecord) -> Void) {
        self.deliver = deliver
        super.init()
        available = recognizer?.isAvailable ?? false
    }

    /// Ask for speech + microphone permission (once). `granted` on the main queue.
    func requestAccess(_ granted: @escaping (Bool) -> Void) {
        SFSpeechRecognizer.requestAuthorization { status in
            AVAudioApplication.requestRecordPermission { mic in
                DispatchQueue.main.async { granted(status == .authorized && mic) }
            }
        }
    }

    func start() {
        guard !listening, let recognizer, recognizer.isAvailable else { return }
        let req = SFSpeechAudioBufferRecognitionRequest()
        req.shouldReportPartialResults = true
        if recognizer.supportsOnDeviceRecognition { req.requiresOnDeviceRecognition = true }
        request = req

        let session = AVAudioSession.sharedInstance()
        try? session.setCategory(.record, mode: .measurement, options: .duckOthers)
        try? session.setActive(true, options: .notifyOthersOnDeactivation)

        let input = engine.inputNode
        input.installTap(onBus: 0, bufferSize: 1024, format: input.outputFormat(forBus: 0)) { buffer, _ in
            req.append(buffer)
        }
        engine.prepare()
        do { try engine.start() } catch { return }

        listening = true
        partial = ""
        task = recognizer.recognitionTask(with: req) { [weak self] result, error in
            guard let self else { return }
            DispatchQueue.main.async {
                if let result {
                    self.partial = result.bestTranscription.formattedString
                    if result.isFinal { self.finish(self.partial) }
                }
                if error != nil { self.stop() }
            }
        }
    }

    /// End the utterance and emit it as an observation.
    func stop() {
        guard listening else { return }
        engine.stop()
        engine.inputNode.removeTap(onBus: 0)
        request?.endAudio()
        task?.cancel()
        task = nil
        listening = false
        let text = partial
        if !text.trimmingCharacters(in: .whitespaces).isEmpty { finish(text) }
    }

    private func finish(_ text: String) {
        let trimmed = String(text.prefix(200))
        deliver(ObsRecord(actor: "ipad:ian", action: "said",
                          object: "voice:\(trimmed)", context: "on-device speech", confidence: 0.9))
        partial = ""
    }
}
