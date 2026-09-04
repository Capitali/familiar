import Foundation
import FamiliarSC
#if canImport(UserNotifications)
import UserNotifications
#endif

/// Screen 6 — notifications: the notice policy scheduled onto the captain's device, each
/// notice once (a dedupe ledger in UserDefaults keyed by kind+tick+title). Local
/// notifications only: nothing here talks to a push service.
public final class CaptainNotifier {
    public let defaults: UserDefaults
    let ledgerKey = "sc.notices.delivered"
    public var ledgerCap = 400

    public init(defaults: UserDefaults = .standard) { self.defaults = defaults }

    public static func key(for n: CaptainNotice, world: String) -> String {
        "\(world)|\(n.kind.rawValue)|\(n.tick ?? n.at)|\(n.title)"
    }

    var delivered: [String] {
        get { defaults.stringArray(forKey: ledgerKey) ?? [] }
        set { defaults.set(Array(newValue.suffix(ledgerCap)), forKey: ledgerKey) }
    }

    /// The notices not yet delivered for this world, oldest first; marks them delivered.
    public func fresh(_ notices: [CaptainNotice], world: String) -> [CaptainNotice] {
        var seen = Set(delivered)
        var out: [CaptainNotice] = []
        var ledger = delivered
        for n in notices {
            let k = CaptainNotifier.key(for: n, world: world)
            if seen.insert(k).inserted { out.append(n); ledger.append(k) }
        }
        delivered = ledger
        return out
    }

    #if canImport(UserNotifications)
    public func requestAuthorization() async -> Bool {
        (try? await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge])) ?? false
    }

    /// Schedule the fresh notices for one ship; returns how many were scheduled.
    @discardableResult
    public func deliver(_ notices: [CaptainNotice], world: String, computer: String) async -> Int {
        let batch = fresh(notices, world: world)
        let center = UNUserNotificationCenter.current()
        for n in batch {
            let content = UNMutableNotificationContent()
            content.title = "\(computer): \(n.title)"
            content.body = n.body
            content.threadIdentifier = world
            content.categoryIdentifier = n.kind.rawValue
            content.sound = n.kind == .distress || n.kind == .needsTheCaptain ? .defaultCritical : .default
            let req = UNNotificationRequest(identifier: CaptainNotifier.key(for: n, world: world), content: content, trigger: nil)
            try? await center.add(req)
        }
        return batch.count
    }
    #endif
}
