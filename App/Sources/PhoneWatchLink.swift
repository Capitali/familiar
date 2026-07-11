import Foundation
import WatchConnectivity

/// The iPhone side of the watch link: hands the paired Apple Watch the familiar's address so the
/// watch can enrol itself by covenant (the watch has no good text entry). Address only — never a
/// secret or a cert; the watch mints its own key and gets its own grant.
final class PhoneWatchLink: NSObject, WCSessionDelegate {
    static let shared = PhoneWatchLink()

    private override init() {
        super.init()
        if WCSession.isSupported() {
            WCSession.default.delegate = self
            WCSession.default.activate()
        }
    }

    /// Push the current familiar address to the watch (idempotent — the latest context wins).
    func sendAddress(host: String, port: Int, label: String) {
        guard WCSession.isSupported() else { return }
        try? WCSession.default.updateApplicationContext(["host": host, "port": port, "label": label])
    }

    func session(_ s: WCSession, activationDidCompleteWith state: WCSessionActivationState, error: Error?) {}
    func sessionDidBecomeInactive(_ s: WCSession) {}
    func sessionDidDeactivate(_ s: WCSession) { WCSession.default.activate() }
}
