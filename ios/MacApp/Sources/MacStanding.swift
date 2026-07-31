import Foundation

/// The standing roll (ADR-0020) as this Mac writes it — the same "a human at this machine edits
/// the daemon's own file" path `MacBoundary` uses for capability gates.
///
/// Membership decides whether a node may READ; standing decides what it SEES. Recognising someone
/// is a human act taken at a console, and ADR-0020 lets **any active member** take it rather than
/// only a steward — so it belongs here, next to the gates, not behind an admin tool.
///
/// Written directly rather than posted to the daemon: the Mac console is a peer in its own right
/// and may have no local daemon at all, so an HTTP call to loopback would silently do nothing on
/// exactly the machines that most need this to work.
///
/// **Known limit:** the roll is per-node until it federates from the minting door, so a decision
/// taken here applies to what THIS node serves. The console reads its own node's roll back on the
/// next poll, which is why the welcome list updates locally but a sibling's may not.
enum MacStanding {
    private static var url: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/Familiar/data/standing.json")
    }

    /// Fail-closed like the Rust side: an unreadable roll means nobody stands, never everybody.
    private static func load() -> (full: [String], notes: [String: String]) {
        guard let data = try? Data(contentsOf: url),
              let raw = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return ([], [:]) }
        return (raw["full"] as? [String] ?? [], raw["notes"] as? [String: String] ?? [:])
    }

    private static func save(full: [String], notes: [String: String]) {
        let obj: [String: Any] = ["full": full, "notes": notes]
        guard let data = try? JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted])
        else { return }
        try? data.write(to: url, options: .atomic)
    }

    /// Recognise a member: they read the real worldview from here on.
    @discardableResult
    static func grant(_ nodeID: String, note: String = "recognised from the console") -> Bool {
        let id = nodeID.trimmingCharacters(in: .whitespaces)
        guard !id.isEmpty else { return false }
        var (full, notes) = load()
        guard !full.contains(id) else { return false }
        full.append(id)
        notes[id] = note
        save(full: full, notes: notes)
        return true
    }

    /// Return a member to guest. This narrows what they SEE — it does not remove them from the
    /// mesh, which is `mesh abandon`, a heavier and separate act.
    @discardableResult
    static func revoke(_ nodeID: String) -> Bool {
        let id = nodeID.trimmingCharacters(in: .whitespaces)
        var (full, notes) = load()
        let before = full.count
        full.removeAll { $0 == id }
        guard full.count != before else { return false }
        notes.removeValue(forKey: id)
        save(full: full, notes: notes)
        return true
    }
}
