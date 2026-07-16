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
        // updateApplicationContext only delivers the LATEST state and is dropped if unchanged — so we
        // also send via transferUserInfo, which queues and is delivered reliably even if the watch app
        // is backgrounded or launches later. Between the two, the address handoff (and thus the watch's
        // covenant enrollment) is robust. A fresh nonce keeps each context "changed" so it isn't coalesced.
        var payload = ctx
        payload["_n"] = UUID().uuidString
        do {
            try s.updateApplicationContext(payload)
        } catch {
            // benign: context unchanged / no watch — transferUserInfo below still delivers.
        }
        if s.isWatchAppInstalled {
            s.transferUserInfo(ctx)
        }
        DispatchQueue.main.async { self.lastSent = ctx["host"] as? String }
    }

    func session(_ s: WCSession, activationDidCompleteWith state: WCSessionActivationState, error: Error?) { flush() }
    func sessionReachabilityDidChange(_ s: WCSession) { flush() }
    func sessionWatchStateDidChange(_ s: WCSession) { flush() }
    func sessionDidBecomeInactive(_ s: WCSession) {}
    func sessionDidDeactivate(_ s: WCSession) { WCSession.default.activate() }
}
