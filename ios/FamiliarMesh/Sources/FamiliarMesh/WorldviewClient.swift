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

/// A classified mesh participant — mirrors the Rust `members::Member`. Every participant sits at one
/// layer: self / gossip peer / device peer / device agent. `The Familiar` is the whole collective,
/// not any one node — none is privileged.
public struct Member: Codable, Equatable, Identifiable {
    public enum Kind: String, Codable { case self_node = "self_node", gossip_peer, device_peer, device_agent }
    public var node_id: String
    public var label: String
    public var kind: Kind
    public var os: String
    public var os_version: String?
    public var actor: String
    public var detail: String
    public var first_seen: Int64
    public var last_seen: Int64
    public var online: Bool
    public var familiar_version: String?
    public var tools: Int?
    public var patterns: Int?
    public var addr: String?
    public var relationship: String?
    /// This node has direct local / context AI access (badged in the roster + mesh map).
    public var ai: Bool?
    /// Graduated trust tier: "trusted" (normal), "throttled", "marginalized", or "severed".
    /// Absent/"trusted" ⇒ full standing; anything else is badged on the roster + map.
    public var trust: String?
    public var id: String { node_id }
}

/// A real relationship between two members — mirrors the Rust `worldview::EdgeView`. Lets the map
/// draw a mesh (peers linked to peers) instead of a star centered on self.
public struct EdgeView: Codable, Equatable, Identifiable {
    public var from: String
    public var to: String
    /// "gossip", "delegation", or "attribution".
    public var kind: String
    public var id: String { from + ">" + to + ":" + kind }
}

/// A reachable-but-unenrolled device — mirrors the Rust `worldview::FrontierView`. Drawn as a faded
/// branch on the mesh map, dimmed by reach class.
public struct FrontierView: Codable, Equatable, Identifiable {
    public var label: String
    public var ip: String
    /// "agent-capable", "protocol-controllable", or "observable-only".
    public var reach: String
    public var open: [String]
    public var last_seen: Int64
    public var id: String { label + "/" + ip }
}

/// A discovered network service / data stream — mirrors the Rust `worldview::ServiceView`. The second
/// roster tab (networks / services / data-streams), aggregated from Bonjour surveys shared over the mesh.
public struct ServiceView: Codable, Equatable, Identifiable {
    public var kind: String
    public var name: String
    public var seen_by: String
    public var last_seen: Int64
    public var id: String { "\(kind)/\(name)/\(seen_by)" }
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

/// One of the familiar's theories — mirrors the Rust `worldview::TheoryView`.
public struct TheoryView: Codable, Equatable, Identifiable {
    public var id: String
    public var question: String
    public var theory: String
    public var direction: String
    public var status: String
}

/// One reflection on humanity — mirrors the Rust `worldview::ReflectionView`.
public struct ReflectionView: Codable, Equatable, Identifiable {
    public var id: String
    public var reflection: String
    public var grounded_in: String
    public var created_at: Int64
}

/// The boundary gates (Law III) — mirrors the Rust `worldview::GateStates`.
public struct GateStates: Codable, Equatable {
    public var llm: Bool
    public var camera: Bool
    public var network: Bool
    public var mesh: Bool
    public var execute: Bool
    public var agent: Bool
    public var tool_install: Bool
}

/// A snapshot of what the familiar knows — mirrors the Rust `worldview::Worldview`. Enough to render
/// a Glass-like console: the three constitutional meters, the peer roster, the recent feed, the
/// familiar's own theories, the boundary gates, and a coarse tick/uptime. The later fields are
/// optional so an older familiar that predates them still decodes.
public struct Worldview: Codable, Equatable {
    public var group_label: String
    public var node_id: String
    public var question: String?
    public var presence: Double
    public var withdrawn: Bool
    public var service: Double
    public var capacity: Double
    public var observation_count: Int
    public var peers: [PeerView]
    public var recent: [ObsView]
    public var theories: [TheoryView]?
    public var theory_quality: Double?
    public var gates: GateStates?
    public var tick: Int?
    public var uptime_secs: Int64?
    public var humanity: [ReflectionView]?
    public var members: [Member]?
    public var services: [ServiceView]?
    public var frontier: [FrontierView]?
    public var edges: [EdgeView]?
}

/// The signed read request — mirrors the Rust `worldview::ViewRequest` (an observe envelope minus
/// the observations). Reuses the same signer, so there's nothing new to reconcile.
struct ViewRequest: Codable {
    var node: NodeIdentity
    var membership: Membership
    var ts: Int64
    var nonce: String
    /// This device's app build + OS release, so the familiar it reads can show them in the roster.
    /// Optional — omitted when empty so the signed bytes stay minimal and older familiars ignore them.
    var client_version: String?
    var os_version: String?
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
        nonce: String = ObservationClient.freshNonce(),
        clientVersion: String = "",
        osVersion: String = ""
    ) async throws -> Worldview {
        let request = ViewRequest(
            node: session.node.identity,
            membership: session.membership,
            ts: now,
            nonce: nonce,
            client_version: clientVersion.isEmpty ? nil : clientVersion,
            os_version: osVersion.isEmpty ? nil : osVersion
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
