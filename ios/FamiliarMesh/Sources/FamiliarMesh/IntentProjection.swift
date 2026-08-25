import Foundation

/// The kind-only projection served to EXTERNAL-INDEXED audiences (T-227 Q2, codex round 2).
///
/// Siri, Spotlight, lock-screen results, donated entities, and shortcut history are an
/// external-indexed audience — never proof that the current viewer is the enrolled human.
/// So what an App Intent may say is built HERE, from named fields only: counts, canonical
/// service kinds, the oracle's availability line, and the *fact* that a question is open —
/// never its text, never its owner, never a device or human name, never observation
/// context, never an entity identifier an index could grow a household graph from.
///
/// The fence is structural: this type has no field that could carry a name or free
/// observation text, and `project(from:)` reads only the fields it serves. Widening what
/// intents may say means adding a field to this type — a reviewable act, not a leak.
public struct IntentProjection: Codable, Equatable {
    /// The on-device oracle's availability line (ConsultRunner.state) — a statement about
    /// the model, carrying nothing about the household.
    public var oracleLine: String
    /// How many observations the familiar holds — a count, not their content.
    public var observationCount: Int
    /// Federated peers last seen — a count.
    public var peerCount: Int
    /// Canonical service kinds around the network (T-228's survey classes) — kinds only;
    /// the worldview's `ServiceView.name` is already served empty and is not read here.
    public var serviceKinds: [String]
    /// Whether the familiar is holding an open question for someone — the fact alone.
    public var openQuestion: Bool
    /// Unix seconds of the worldview read this was projected from.
    public var updatedAt: Int64

    public init(
        oracleLine: String,
        observationCount: Int,
        peerCount: Int,
        serviceKinds: [String],
        openQuestion: Bool,
        updatedAt: Int64
    ) {
        self.oracleLine = oracleLine
        self.observationCount = observationCount
        self.peerCount = peerCount
        self.serviceKinds = serviceKinds
        self.openQuestion = openQuestion
        self.updatedAt = updatedAt
    }

    /// Project a fresh worldview read down to what an external-indexed surface may hear.
    public static func project(from view: Worldview, oracleLine: String, now: Int64) -> IntentProjection {
        IntentProjection(
            oracleLine: oracleLine,
            observationCount: view.observation_count,
            peerCount: view.peers.count,
            serviceKinds: (view.services ?? []).map(\.kind).sorted(),
            openQuestion: !(view.question ?? "").isEmpty,
            updatedAt: now
        )
    }

    /// Where the cache lives. UserDefaults on purpose: small, in-container, reachable by
    /// an in-process intent whether or not the UI is up, and never synced anywhere.
    public static let defaultsKey = "intent.projection"

    public func store(in defaults: UserDefaults = .standard) {
        if let data = try? JSONEncoder().encode(self) {
            defaults.set(data, forKey: Self.defaultsKey)
        }
    }

    public static func stored(in defaults: UserDefaults = .standard) -> IntentProjection? {
        guard let data = defaults.data(forKey: defaultsKey) else { return nil }
        return try? JSONDecoder().decode(IntentProjection.self, from: data)
    }
}
