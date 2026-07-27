import Foundation

/// One mesh listed at a rendezvous (the lighthouse) — where a fresh device can go to ask to join,
/// without a QR (ADR-0012). Labels and addresses only; never a secret. Mirrors the Rust
/// `rendezvous::Entry`.
public struct MeshDoor: Codable, Equatable {
    public var group_ref: String
    public var group_label: String
    public var hosts: [String]
    public var port: Int
}

/// Reads the rendezvous directory from a public meeting node. The device asks "what meshes are
/// reachable?" and gets back doors to knock on — admission is still a human act at the familiar.
public struct RendezvousClient {
    /// Fetch the directory. Returns [] on any failure (unreachable rendezvous, empty directory) so
    /// the caller falls back to the QR/paste path cleanly.
    public static func directory(host: String, port: Int) async throws -> [MeshDoor] {
        guard let url = URL(string: "https://\(host):\(port)/mesh/rendezvous") else { return [] }
        var r = URLRequest(url: url)
        r.timeoutInterval = 8
        let (data, resp) = try await MeshTLS.session.data(for: r)
        guard (resp as? HTTPURLResponse)?.statusCode == 200 else { return [] }
        return (try? JSONDecoder().decode([MeshDoor].self, from: data)) ?? []
    }
}
