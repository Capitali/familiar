import Foundation

/// A read-only mirror of the Rust core's `boundary.json` (`crates/kernel/src/boundary.rs`) —
/// the same human-owned capability gates the daemon enforces, read directly from disk the same
/// way `SphereBridge` already writes `mesh/geo.json` into the daemon's data dir. The Mac app
/// never widens these itself from background code; `MacConsentSettings` (a real, human-facing
/// preferences window) is the sanctioned "human edits the file" path, same as hand-editing
/// boundary.json — see `crates/kernel/src/boundary.rs`'s own doc comment.
enum MacBoundary {
    private static var url: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/Familiar/data/boundary.json")
    }

    struct Gates: Codable {
        var allow_camera = false
        var allow_microphone = false
        var allow_location = false
        var allow_motion = false
        var allow_network_discovery = false
        var allow_face_recognition = false
    }

    /// Fail-closed, exactly like the Rust side: a missing or unreadable file means every gate
    /// is off, never on.
    static func load() -> Gates {
        guard let data = try? Data(contentsOf: url),
              let gates = try? JSONDecoder().decode(Gates.self, from: data)
        else { return Gates() }
        return gates
    }

    /// The human explicitly flipping a gate from the settings window — reads the full boundary
    /// (preserving fields this app doesn't model, e.g. `allow_execute`), patches just the one
    /// key, writes it back. Never called from background sensing code, only a human's toggle.
    static func set(_ mutate: (inout Gates) -> Void) {
        var raw = (try? Data(contentsOf: url)).flatMap {
            try? JSONSerialization.jsonObject(with: $0) as? [String: Any]
        } ?? [:]
        var gates = load()
        mutate(&gates)
        raw["allow_camera"] = gates.allow_camera
        raw["allow_microphone"] = gates.allow_microphone
        raw["allow_location"] = gates.allow_location
        raw["allow_motion"] = gates.allow_motion
        raw["allow_network_discovery"] = gates.allow_network_discovery
        raw["allow_face_recognition"] = gates.allow_face_recognition
        guard let data = try? JSONSerialization.data(withJSONObject: raw, options: [.prettyPrinted]) else { return }
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? data.write(to: url)
    }
}
