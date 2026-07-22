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
    /// Liveness as a word — "online" | "away" | "offline" (derived on the familiar's cadence).
    public var status: String?
    /// When the current continuous-online run began (unix secs); 0 when offline/unknown.
    public var session_start: Int64?
    /// Cumulative seconds online in the mesh, live session included.
    public var total_online_secs: Int64?
    /// A human can interact at this node's console.
    public var interactive: Bool?
    /// The human that node serves ("ian"), when shared/derivable.
    public var human: String?
    /// Where the node is (decimal degrees); 0/0 or absent = unknown.
    public var lat: Double?
    public var lon: Double?
    public var id: String { node_id }
}

/// A goal on the shared roadmap — mirrors the Rust `worldview::GoalView`. The mesh owns these and
/// burns them down together; a node whose capabilities fit claims one and drives it, and the whole
/// mesh sees the status. Deploy-class goals are claimed but parked for a human (Law III).
public struct GoalView: Codable, Equatable, Identifiable {
    public var id: String
    public var description: String
    public var needs: [String]
    /// "proposed" | "claimed" | "in_progress" | "awaiting_human" | "done" | "failed" | "blocked".
    public var status: String
    /// Short node id of the owner (empty while unclaimed).
    public var owner: String
    public var origin: String
    public var produced: String
    public var notes: String
    public var updated_at: Int64
    /// Lifecycle dates — whatever state the goal is in carries the date it got there.
    public var status_at: Int64?
    public var last_worked_at: Int64?
    public var completed_at: Int64?
    public var ended_at: Int64?
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
    /// Whatever the status is, it is dated: created / entered current status / last worked.
    public var created_at: Int64?
    public var status_at: Int64?
    public var last_worked_at: Int64?
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
    /// The shared roadmap — goals the mesh owns and burns down together. Optional for back-compat.
    public var goals: [GoalView]?
    /// Every address the familiar answers at, most-universal first (tailnet, then LAN). The model
    /// merges these into its candidate host list so a LAN-enrolled device learns the tailnet path.
    public var hosts: [String]?
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
    /// The device's position (decimal degrees) when GPS consent is on — refreshed every read so
    /// the mesh map is near-real-time. Omitted when unknown.
    var lat: Double?
    var lon: Double?
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
        osVersion: String = "",
        lat: Double = 0,
        lon: Double = 0
    ) async throws -> Worldview {
        let request = ViewRequest(
            node: session.node.identity,
            membership: session.membership,
            ts: now,
            nonce: nonce,
            client_version: clientVersion.isEmpty ? nil : clientVersion,
            os_version: osVersion.isEmpty ? nil : osVersion,
            lat: lat == 0 && lon == 0 ? nil : lat,
            lon: lat == 0 && lon == 0 ? nil : lon
        )
        guard let body = try? JSONEncoder().encode(request) else { throw ReadError.encoding }
        let sig = try session.node.sign(body)

        var req = URLRequest(url: session.url)
        req.httpMethod = "POST"
        req.timeoutInterval = 10   // fail fast so the caller can try the next candidate address
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
