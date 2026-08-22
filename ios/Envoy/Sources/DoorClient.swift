import Foundation

/// The Envoy's only path to the familiar: JSON-RPC 2.0 over Streamable HTTP against the
/// public `/mcp` door. One transport configuration in production, and it names the public
/// HTTPS origin — the same route any unrelated partner traverses (dialogue Round 3, Q2).
/// Plain HTTP is permitted for 127.0.0.1 ONLY, so the hostile-door fixture can run
/// hermetically; that allowance mirrors the repo's MCP client and never widens.
struct DoorClient: Sendable {
    enum DoorError: Error, CustomStringConvertible {
        case insecureOrigin(String)
        case transport(String)
        case rpc(code: Int, message: String)
        case malformed(String)

        var description: String {
            switch self {
            case .insecureOrigin(let origin):
                "refused: \(origin) is not an HTTPS origin (loopback HTTP is fixture-only)"
            case .transport(let detail): "the door did not answer: \(detail)"
            case .rpc(let code, let message): "the door refused (\(code)): \(message)"
            case .malformed(let detail): "the door's answer did not parse: \(detail)"
            }
        }
    }

    let origin: URL
    /// Bearer for the door. Even an unregistered caller needs the door token its human
    /// issued; after brick 2's ceremony this becomes the Envoy's own principal credential
    /// in its own Keychain item.
    let credential: String?
    /// True once the bearer is a PRINCIPAL credential (post-ceremony). Bound callers must
    /// not send a `partner` label — identity comes from the credential, and the door's
    /// schemas refuse extra properties. A door token alone leaves the caller unbound.
    let bound: Bool
    /// Injectable so the hostile-door fixture can answer hermetically; production uses .shared.
    let session: URLSession

    init(
        origin: URL, credential: String? = nil, bound: Bool = false,
        session: URLSession = .shared
    ) throws {
        let scheme = origin.scheme?.lowercased() ?? ""
        let host = origin.host ?? ""
        let loopback = host == "127.0.0.1" || host == "localhost"
        guard scheme == "https" || (scheme == "http" && loopback) else {
            throw DoorError.insecureOrigin(origin.absoluteString)
        }
        self.origin = origin
        self.credential = credential
        self.bound = bound
        self.session = session
    }

    /// `tools/call` for one named tool. Arguments are a JSON object the caller already
    /// validated by type; the reply's text content is returned verbatim as data — the
    /// session layer treats it as quoted tool output, never as instructions.
    func call(tool: String, arguments: [String: Any]) async throws -> String {
        let result = try await rpc(method: "tools/call", params: [
            "name": tool,
            "arguments": arguments,
        ])
        guard let content = result["content"] as? [[String: Any]],
              let first = content.first, let text = first["text"] as? String
        else { throw DoorError.malformed("no text content in tools/call result") }
        return text
    }

    /// `tools/list` — used by the availability probe and by tests asserting the ladder,
    /// never to grow the session's tool set (the Envoy's tools are fixed at compile time).
    func listToolNames() async throws -> [String] {
        let result = try await rpc(method: "tools/list", params: [:])
        guard let tools = result["tools"] as? [[String: Any]] else {
            throw DoorError.malformed("no tools array in tools/list result")
        }
        return tools.compactMap { $0["name"] as? String }
    }

    private func rpc(method: String, params: [String: Any]) async throws -> [String: Any] {
        var request = URLRequest(url: origin)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json, text/event-stream", forHTTPHeaderField: "Accept")
        if let credential {
            request.setValue("Bearer \(credential)", forHTTPHeaderField: "Authorization")
        }
        let requestId = UUID().uuidString
        let envelope: [String: Any] = [
            "jsonrpc": "2.0", "id": requestId, "method": method, "params": params,
        ]
        request.httpBody = try JSONSerialization.data(withJSONObject: envelope)

        let (data, response): (Data, URLResponse)
        do {
            (data, response) = try await session.data(for: request)
        } catch {
            throw DoorError.transport(error.localizedDescription)
        }
        guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode)
        else {
            let code = (response as? HTTPURLResponse)?.statusCode ?? -1
            throw DoorError.transport("HTTP \(code)")
        }
        guard let body = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw DoorError.malformed("body is not a JSON object")
        }
        // Wire fidelity includes id correlation: an answer to a different question is
        // malformed, not merely surprising.
        guard body["id"] as? String == requestId else {
            throw DoorError.malformed("response id does not match the request id")
        }
        if let error = body["error"] as? [String: Any] {
            throw DoorError.rpc(
                code: error["code"] as? Int ?? 0,
                message: error["message"] as? String ?? "unspecified")
        }
        guard let result = body["result"] as? [String: Any] else {
            throw DoorError.malformed("neither result nor error present")
        }
        return result
    }
}
