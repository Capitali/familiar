import Foundation
import WatchConnectivity

/// The iPhone side of the watch link: hands the paired Apple Watch the familiar's address so the
/// watch can enrol itself by covenant (the watch has no good text entry). Address only — never a
/// secret or a cert; the watch mints its own key and gets its own grant.
///
/// Robustness matters: the watch may connect *after* the phone enrolled, so we remember the latest
/// address and (re)deliver it whenever the session activates or the watch's state/reachability
/// changes. `updateApplicationContext` keeps only the newest value and delivers it when the watch
/// app next activates.
final class PhoneWatchLink: NSObject, WCSessionDelegate, ObservableObject {
    static let shared = PhoneWatchLink()

    @Published var paired = false
    @Published var appInstalled = false
    @Published var lastSent: String?

    private var latest: [String: Any]?

    private override init() {
        super.init()
        if WCSession.isSupported() {
            WCSession.default.delegate = self
            WCSession.default.activate()
        }
    }

    /// Remember + deliver the address. Safe to call repeatedly / before the watch is present.
    func sendAddress(host: String, port: Int, label: String) {
        latest = ["host": host, "port": port, "label": label]
        flush()
    }

    private func flush() {
        guard WCSession.isSupported() else { return }
        let s = WCSession.default
        DispatchQueue.main.async {
            self.paired = s.isPaired
            self.appInstalled = s.isWatchAppInstalled
        }
        guard s.activationState == .activated, let ctx = latest else { return }
        do {
            try s.updateApplicationContext(ctx)
            DispatchQueue.main.async { self.lastSent = ctx["host"] as? String }
        } catch {
            // benign: no watch paired yet, or context unchanged — retried on the next state change
        }
    }

    func session(_ s: WCSession, activationDidCompleteWith state: WCSessionActivationState, error: Error?) { flush() }
    func sessionReachabilityDidChange(_ s: WCSession) { flush() }
    func sessionWatchStateDidChange(_ s: WCSession) { flush() }
    func sessionDidBecomeInactive(_ s: WCSession) {}
    func sessionDidDeactivate(_ s: WCSession) { WCSession.default.activate() }
}
