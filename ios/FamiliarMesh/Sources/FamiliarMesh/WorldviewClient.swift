import Foundation

/// One observation as the familiar's console shows it — mirrors the Rust `worldview::ObsView`.
public struct ObsView: Codable, Equatable, Identifiable {
    public var actor: String
    public var action: String
    public var object: String
    public var context: String
    public var source: String
    public var ts: Int64
    public var confidence: Double
    /// Stable-enough identity for SwiftUI lists (no id on the wire).
    public var id: String { "\(ts)|\(source)|\(actor)|\(object)" }
}

/// A federated peer as last seen — mirrors the Rust `worldview::PeerView`.
public struct PeerView: Codable, Equatable, Identifiable {
    public var node_id: String
    public var label: String
    public var last_seen: Int64
    public var tools_offered: Int
    public var patterns_offered: Int
    public var id: String { node_id }
}

/// A snapshot of what the familiar knows — mirrors the Rust `worldview::Worldview`. Enough to render
/// a Glass-like console: the three constitutional meters, the peer roster, and the recent feed.
public struct Worldview: Codable, Equatable {
    public var group_label: String
    public var node_id: String
    public var presence: Double
    public var withdrawn: Bool
    public var service: Double
    public var capacity: Double
    public var observation_count: Int
    public var peers: [PeerView]
    public var recent: [ObsView]
}

/// The signed read request — mirrors the Rust `worldview::ViewRequest` (an observe envelope minus
/// the observations). Reuses the same signer, so there's nothing new to reconcile.
struct ViewRequest: Codable {
    var node: NodeIdentity
    var membership: Membership
    var ts: Int64
    var nonce: String
}

/// Reads a familiar's worldview over `POST /mesh/worldview`. The device is a member peer: it signs
/// an identity+freshness envelope (same trust path as posting observations) and gets back the
/// snapshot only if the familiar verifies it as an in-group node with the mesh open.
public struct WorldviewClient {
    public enum ReadError: Error, Equatable {
        case encoding
        case http(status: Int, body: String)
        case transport(String)
        case decoding
    }

    public var session: ObservationClient.Session
    public var urlSession: URLSession

    /// `session.url` should point at the familiar's `/mesh/worldview` (see `worldviewURL`).
    public init(session: ObservationClient.Session, urlSession: URLSession = .shared) {
        self.session = session
        self.urlSession = urlSession
    }

    /// Sign and POST a read request; decode the returned snapshot.
    public func fetch(
        now: Int64 = Int64(Date().timeIntervalSince1970),
        nonce: String = ObservationClient.freshNonce()
    ) async throws -> Worldview {
        let request = ViewRequest(
            node: session.node.identity,
            membership: session.membership,
            ts: now,
            nonce: nonce
        )
        guard let body = try? JSONEncoder().encode(request) else { throw ReadError.encoding }
        let sig = try session.node.sign(body)

        var req = URLRequest(url: session.url)
        req.httpMethod = "POST"
        req.setValue(sig, forHTTPHeaderField: "X-Familiar-Sig")
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = body

        let (data, response): (Data, URLResponse)
        do {
            (data, response) = try await urlSession.data(for: req)
        } catch {
            throw ReadError.transport(error.localizedDescription)
        }
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        guard status == 200 else {
            throw ReadError.http(status: status, body: String(data: data, encoding: .utf8) ?? "")
        }
        guard let view = try? JSONDecoder().decode(Worldview.self, from: data) else {
            throw ReadError.decoding
        }
        return view
    }

    /// Turn an enrollment host+port into the worldview endpoint URL.
    public static func worldviewURL(host: String, port: Int) -> URL? {
        URL(string: "http://\(host):\(port)/mesh/worldview")
    }
}
