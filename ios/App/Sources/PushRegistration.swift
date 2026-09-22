import SwiftUI
import UserNotifications
import FamiliarMesh

/// APNs registration (iOS only): ask once, register with the OS, relay the device token to
/// this device's door — so "the ember is yours" reaches a locked phone (ADR-0028's chime
/// path, extended past the app's own lifetime). The token callback lands on the app
/// delegate, so the SwiftUI app installs this adaptor.
final class PushDelegate: NSObject, UIApplicationDelegate, UNUserNotificationCenterDelegate {
    /// The model the token should flow to — set by the app at scene build.
    static weak var model: AppModel?

    func application(
        _ application: UIApplication,
        didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data
    ) {
        let hex = deviceToken.map { String(format: "%02x", $0) }.joined()
        Self.model?.apnsTokenArrived(hex)
    }

    func application(
        _ application: UIApplication,
        didFailToRegisterForRemoteNotificationsWithError error: Error
    ) {
        // No note: simulator and denied-permission runs land here routinely; the games
        // screen's door line reports registration success, absence means "no push".
    }

    /// Show the banner even when the app is foregrounded — in-app the chime already rings,
    /// but a visible banner keeps the two paths indistinguishable to the player.
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        [.banner, .sound]
    }
}

enum PushRegistration {
    /// Request permission and register. Safe to call repeatedly (the OS dedupes); called
    /// when the console appears and again whenever enrollment completes. `delegate` is the
    /// app's `@UIApplicationDelegateAdaptor` instance — the same object receives the token
    /// callback and foreground-presentation asks.
    static func request(_ model: AppModel, delegate: PushDelegate) {
        PushDelegate.model = model
        UNUserNotificationCenter.current().delegate = delegate
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) {
            granted, _ in
            guard granted else { return }
            DispatchQueue.main.async {
                UIApplication.shared.registerForRemoteNotifications()
            }
        }
    }
}
