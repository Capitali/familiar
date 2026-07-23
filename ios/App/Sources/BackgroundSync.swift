import Foundation
import BackgroundTasks
import FamiliarMesh

/// Opportunistic background sync (SPEC.md R12) — the honest version of "always-on" on iOS.
/// The platform never lets a backgrounded app hold an open, listening socket, so this is a
/// periodic wake-and-dial-out, not a persistent server. API verified against Apple's current
/// documentation before writing this (BGAppRefreshTaskRequest for short work, ~30s budget;
/// BGProcessingTaskRequest is for longer/heavier jobs — a worldview read is short, so refresh
/// is the right request type here).
///
/// Deliberately standalone, not routed through the live `AppModel` instance: iOS can launch the
/// app specifically to run a background task without the normal SwiftUI scene (and therefore
/// without `@StateObject var model = AppModel()`) ever being created, so the handler reads the
/// same Keychain-stored enrollment state `AppModel` uses directly, rather than assuming a live
/// model exists to capture.
enum BackgroundSync {
    static let refreshTaskID = "io.river.familiar.ios.refresh"

    /// Register the task handler — must happen before app launch finishes, so this is called
    /// from `FamiliarAgentApp.init()`, not from a view's `onAppear`.
    static func register() {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: refreshTaskID, using: nil) { task in
            handle(task as! BGAppRefreshTask)
        }
    }

    /// Ask the scheduler for another run — call this on app backgrounding and again at the end
    /// of each run, so there's always a next one pending. Timing after that is entirely the
    /// OS's call (battery, usage patterns, ...) — this is a request, never a guarantee.
    static func scheduleNext() {
        let request = BGAppRefreshTaskRequest(identifier: refreshTaskID)
        request.earliestBeginDate = Date(timeIntervalSinceNow: 15 * 60)
        try? BGTaskScheduler.shared.submit(request)
    }

    private static func handle(_ task: BGAppRefreshTask) {
        scheduleNext()  // always keep one queued, whether this run succeeds or not
        let work = Task {
            await syncOnce()
            task.setTaskCompleted(success: true)
        }
        // The OS's ~30s budget can end early (low battery, task starvation) — bail cleanly.
        task.expirationHandler = { work.cancel() }
    }

    /// One bounded sync pass: pull the worldview (learns new reachable hosts, e.g. a lighthouse,
    /// same failover walk AppModel.refreshWorldview() does) using whichever enrollment is
    /// already on disk. A no-op, not an error, when the device isn't enrolled.
    private static func syncOnce() async {
        guard let seed = KeychainStore.load(account: "node.seed"),
              let node = try? NodeKey(seed: seed, label: "background"),
              let grantData = KeychainStore.load(account: "grant.json"),
              let grant = try? JSONDecoder().decode(Grant.self, from: grantData),
              let enrollData = KeychainStore.load(account: "enroll.info"),
              let enrollJSON = try? JSONSerialization.jsonObject(with: enrollData) as? [String: Any],
              let hosts = (enrollJSON["hosts"] as? [String])?.filter({ !$0.isEmpty }),
              !hosts.isEmpty
        else { return }
        let port = (enrollJSON["port"] as? Int) ?? 47100

        for host in hosts {
            guard !Task.isCancelled,
                  let url = WorldviewClient.worldviewURL(host: host, port: port)
            else { continue }
            let session = ObservationClient.Session(node: node, membership: grant.membership, url: url)
            if (try? await WorldviewClient(session: session).fetchWithRaw(
                clientVersion: "background", osVersion: "background", lat: 0, lon: 0
            )) != nil {
                return  // reached one host — the point of this pass (stay a known-live peer)
            }
        }
    }
}
