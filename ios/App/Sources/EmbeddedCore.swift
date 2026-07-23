import Foundation

/// ADR-0009 Phase 0: the Rust core embedded in-process via UniFFI (`crates/core-ffi`, linked as
/// FamiliarCore.xcframework — SPEC.md R11). A capable phone can run the same mesh transport a
/// headless peer runs (`meshStart`), becoming a true `GossipPeer` instead of a console-only
/// `DevicePeer` that merely reads the worldview.
///
/// Deliberately NOT wired into `AppModel`'s existing enrollment flow yet. That flow is live on
/// real, currently-enrolled devices (Aphelion, Codex) — silently swapping its founding/joining
/// behavior for the embedded-core path is a bigger product decision (should this be automatic?
/// a per-device opt-in? does founding-first change what "enrolled" even means for the existing
/// DevicePeer UX?) than "link the framework and call it," and risks breaking devices that
/// already work. This class makes the capability real, linkable, and independently testable;
/// choosing how it replaces or augments the existing flow is a deliberate follow-up.
enum EmbeddedCore {
    /// This device's own data directory for the embedded core — separate from anything the
    /// existing DevicePeer/AppModel flow uses (Keychain-held NodeKey, UserDefaults enrollment
    /// info). The embedded core is a from-scratch peer identity if/when actually adopted.
    static var dataDir: String {
        let dir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("EmbeddedCore", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.path
    }

    // Named distinctly from the generated free functions (foundGroup vs. found, etc.) — Swift
    // resolves an unqualified call by base name within the enclosing type's own scope first,
    // so a same-named static method here would shadow (and appear to recurse into) the global
    // FFI function rather than calling it.
    static var hasFounded: Bool { isFounded(dataDir: dataDir) }

    /// First launch, no familiar reachable: found a new node + group of its own (ADR-0009
    /// Phase 0's "founding-first" design). Returns the invite payload for the group it created.
    static func foundGroup(label: String) -> String {
        found(dataDir: dataDir, label: label)
    }

    /// Join an existing group by secret (the same secret `familiar mesh key` prints, or an
    /// invite payload's `secret` field) — mints this device as a full peer in that group.
    static func joinGroup(label: String, secret: String, groupLabel: String) -> String {
        join(dataDir: dataDir, label: label, secret: secret, groupLabel: groupLabel)
    }

    /// Start the in-process gossip transport — the moment this device becomes a real
    /// `GossipPeer` (serves the worldview seam, exchanges signed briefs) rather than a console.
    static func startMesh() { meshStart(dataDir: dataDir) }
    static func stopMesh() { meshStop() }

    static func worldviewJSON() -> String { worldviewJson(dataDir: dataDir) }
    static func fetchInvitePayload() -> String { invitePayload(dataDir: dataDir) }
}
