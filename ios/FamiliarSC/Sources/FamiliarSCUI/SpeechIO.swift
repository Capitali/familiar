import Foundation
import Observation
import AVFoundation
#if canImport(Speech)
import Speech
#endif

// The speech interface: the captain talks, Felix listens (on-device SpeechAnalyzer, iOS 26),
// answers through Conversation, and speaks (AVSpeechSynthesizer). Nothing here leaves the
// device. Permission strings (NSMicrophoneUsageDescription, NSSpeechRecognitionUsageDescription)
// are the app's; the package only asks.

/// Text to voice. One synthesizer, the best English voice the device has.
public final class Speaker: NSObject, AVSpeechSynthesizerDelegate, @unchecked Sendable {
    private let synth = AVSpeechSynthesizer()
    public private(set) var speaking = false
    public var onFinish: (() -> Void)?

    public override init() { super.init(); synth.delegate = self }

    /// The captain's chosen voice (UserDefaults `sc.voiceIdentifier`), else the best installed:
    /// premium, then enhanced, matching the device's language first. Premium voices are a
    /// download (iOS Settings → Accessibility → Spoken Content → Voices); the compact default is
    /// the "terrible" one (Ian, 2026-09-04).
    public static let chosenVoiceKey = "sc.voiceIdentifier"

    public static func candidates() -> [AVSpeechSynthesisVoice] {
        let lang = Locale.current.language.languageCode?.identifier ?? "en"
        let all = AVSpeechSynthesisVoice.speechVoices()
        let mine = all.filter { $0.language.hasPrefix(lang) }
        let rank: (AVSpeechSynthesisVoice) -> Int = { v in v.quality == .premium ? 0 : v.quality == .enhanced ? 1 : 2 }
        return (mine.isEmpty ? all : mine).sorted { rank($0) != rank($1) ? rank($0) < rank($1) : $0.name < $1.name }
    }

    public static func bestVoice() -> AVSpeechSynthesisVoice? {
        if let id = UserDefaults.standard.string(forKey: chosenVoiceKey), let v = AVSpeechSynthesisVoice(identifier: id) { return v }
        return candidates().first ?? AVSpeechSynthesisVoice(language: "en-US")
    }

    public static func qualityWord(_ v: AVSpeechSynthesisVoice) -> String {
        switch v.quality { case .premium: return "premium"; case .enhanced: return "enhanced"; default: return "compact" }
    }

    public func speak(_ text: String, rate: Float = AVSpeechUtteranceDefaultSpeechRate * 0.92) {
        stop()
        #if os(iOS) || os(visionOS)
        try? AVAudioSession.sharedInstance().setCategory(.playback, mode: .spokenAudio, options: [.duckOthers])
        try? AVAudioSession.sharedInstance().setActive(true)
        #endif
        let u = AVSpeechUtterance(string: text)
        u.voice = Speaker.bestVoice()
        u.rate = rate
        u.preUtteranceDelay = 0.15
        u.pitchMultiplier = 0.98
        speaking = true
        synth.speak(u)
    }

    public func stop() { if synth.isSpeaking { synth.stopSpeaking(at: .immediate) }; speaking = false }

    public func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didFinish utterance: AVSpeechUtterance) { speaking = false; onFinish?() }
    public func speechSynthesizer(_ synthesizer: AVSpeechSynthesizer, didCancel utterance: AVSpeechUtterance) { speaking = false }
}

/// Voice to text, on device. `start()` opens the microphone and streams into a
/// SpeechTranscriber; `text` updates as the captain speaks (volatile, then final);
/// `stop()` finalizes and returns what was said.
@Observable
public final class Dictation: @unchecked Sendable {
    public private(set) var listening = false
    public private(set) var text = ""
    public private(set) var status = ""
    public var available: Bool {
        #if canImport(Speech)
        if #available(iOS 26.0, macOS 26.0, visionOS 26.0, *) { return true }
        #endif
        return false
    }

    private let engine = AVAudioEngine()
    private var stopper: (() async -> String)?

    public init() {}

    public static func requestPermission() async -> Bool {
        #if os(iOS) || os(visionOS)
        return await AVAudioApplication.requestRecordPermission()
        #else
        return true
        #endif
    }

    @MainActor
    public func start() async {
        guard !listening else { return }
        text = ""; status = ""
        #if canImport(Speech)
        if #available(iOS 26.0, macOS 26.0, visionOS 26.0, *) {
            do {
                guard await Dictation.requestPermission() else { status = "microphone not allowed"; return }
                let transcriber = SpeechTranscriber(locale: Locale.current, transcriptionOptions: [], reportingOptions: [.volatileResults], attributeOptions: [])
                if let req = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
                    status = "fetching the speech model…"
                    try await req.downloadAndInstall()
                }
                let analyzer = SpeechAnalyzer(modules: [transcriber])
                guard let format = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [transcriber]) else { status = "no audio format for the transcriber"; return }
                let (stream, continuation) = AsyncStream<AnalyzerInput>.makeStream()
                #if os(iOS) || os(visionOS)
                let session = AVAudioSession.sharedInstance()
                try session.setCategory(.playAndRecord, mode: .spokenAudio, options: [.duckOthers, .defaultToSpeaker])
                try session.setActive(true)
                #endif
                let input = engine.inputNode
                let inFormat = input.outputFormat(forBus: 0)
                guard let converter = AVAudioConverter(from: inFormat, to: format) else { status = "cannot convert microphone audio"; return }
                input.removeTap(onBus: 0)
                input.installTap(onBus: 0, bufferSize: 4096, format: inFormat) { buffer, _ in
                    let ratio = format.sampleRate / inFormat.sampleRate
                    let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 16
                    guard let out = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: capacity) else { return }
                    var consumed = false
                    var err: NSError?
                    converter.convert(to: out, error: &err) { _, status in
                        if consumed { status.pointee = .noDataNow; return nil }
                        consumed = true; status.pointee = .haveData; return buffer
                    }
                    if err == nil, out.frameLength > 0 { continuation.yield(AnalyzerInput(buffer: out)) }
                }
                engine.prepare()
                try engine.start()
                try await analyzer.start(inputSequence: stream)
                listening = true
                status = "listening"
                let results = Task { [weak self] in
                    do {
                        for try await r in transcriber.results {
                            let s = String(r.text.characters)
                            await MainActor.run { self?.text = s }
                        }
                    } catch { await MainActor.run { self?.status = "\(error)" } }
                }
                stopper = { [weak self] in
                    self?.engine.stop()
                    self?.engine.inputNode.removeTap(onBus: 0)
                    continuation.finish()
                    try? await analyzer.finalizeAndFinishThroughEndOfInput()
                    _ = await results.result
                    return await MainActor.run { self?.text ?? "" }
                }
            } catch {
                status = "\(error)"
                engine.stop(); engine.inputNode.removeTap(onBus: 0)
            }
            return
        }
        #endif
        status = "on-device dictation needs iOS 26"
    }

    /// Stop listening; returns the final transcript.
    @MainActor
    public func stop() async -> String {
        guard listening, let s = stopper else { return text }
        listening = false
        let final = await s()
        stopper = nil
        status = ""
        text = final
        return final
    }
}
