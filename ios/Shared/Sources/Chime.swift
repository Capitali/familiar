import Foundation
import AVFoundation

/// The familiar's one sound: someone has arrived.
///
/// Deliberately a single, sparse cue rather than a notification vocabulary. It plays on an
/// *arrival* — an edge, not a state — and under ADR-0026 it is a greeting, not an ask: nothing
/// waits on anyone, so the sound announces "someone new has joined" and then the mesh simply
/// carries on. A sound that repeats while nothing has changed teaches you to ignore it, which
/// would defeat the point of having one at all.
enum Chime {
    private static var player: AVAudioPlayer?

    /// Someone new has arrived — a guest looking around, a member admitted (ADR-0026: the
    /// welcome is a greeting, not a gate).
    static func guestWaiting() { play("guest-waiting", "mp3") }

    /// An admission completed. Plays on every member console greeting the arrival **and** on
    /// the device that was just admitted — the second is the point: being accepted should be
    /// felt on the accepted device, not only announced elsewhere.
    static func accepted() { play("acceptance", "wav") }

    /// A riddle was solved (B13) — the fanfare. It rides the acceptance cue (a bright, earned
    /// sound; no new asset) and is edge-triggered on the win, so it rings once, not on every
    /// poll of a finished game.
    static func fanfare() { play("acceptance", "wav") }

    private static func play(_ name: String, _ ext: String) {
        guard let url = Bundle.main.url(forResource: name, withExtension: ext) else {
            return
        }
        #if os(iOS)
        // Respect the ring/silent switch and duck rather than interrupt: this is an ambient
        // notice, not media the human asked for, and it must never stop their music.
        try? AVAudioSession.sharedInstance().setCategory(.ambient, mode: .default)
        try? AVAudioSession.sharedInstance().setActive(true)
        #endif
        // Held in a static so the player is not deallocated mid-playback — the classic reason a
        // short sound plays as a click or not at all.
        player = try? AVAudioPlayer(contentsOf: url)
        player?.prepareToPlay()
        player?.play()
    }
}
