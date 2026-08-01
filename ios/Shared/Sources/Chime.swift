import Foundation
import AVFoundation

/// The familiar's one sound: someone is at the door.
///
/// Deliberately a single, sparse cue rather than a notification vocabulary. It plays when a guest
/// *arrives* — an edge, not a state — so a guest who waits is announced once and then waits
/// quietly. A sound that repeats while nothing has changed teaches you to ignore it, which would
/// defeat the point of having one at all.
enum Chime {
    private static var player: AVAudioPlayer?

    /// Someone new is waiting to be recognised (ADR-0020).
    static func guestWaiting() {
        guard let url = Bundle.main.url(forResource: "guest-waiting", withExtension: "mp3") else {
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
