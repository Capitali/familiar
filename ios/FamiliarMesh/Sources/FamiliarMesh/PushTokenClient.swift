import Foundation

/// `POST /mesh/push-token` — hand this device's APNs token to its door, signed like every
/// member write, so the ember can reach a locked phone (the door pushes "the ember is
/// yours" when the turn arrives). The `env` names which APNs gateway the token belongs to:
/// development-provisioned installs (Xcode, devicectl) live in "sandbox"; TestFlight and
/// the App Store live in "production". Registering is idempotent — the door keeps one row
/// per node and a re-registration simply replaces it.
public struct PushTokenClient {
    public var node: NodeKey
    public var urlSession: URLSession

    public init(node: NodeKey, urlSession: URLSession = MeshTLS.session) {
        self.node = node
        self.urlSession = urlSession
    }

    /// The APNs environment this build's token belongs to. TestFlight/App Store builds
    /// carry no development provisioning profile → "production"; anything with an embedded
    /// development profile (Xcode, devicectl direct installs) → "sandbox".
    public static func apnsEnvironment() -> String {
        guard let path = Bundle.main.path(forResource: "embedded", ofType: "mobileprovision"),
              let data = try? Data(contentsOf: URL(fileURLWithPath: path)),
              let text = String(data: data, encoding: .isoLatin1)
        else { return "production" }
        // The profile embeds a plist; a development profile declares
        // <key>aps-environment</key><string>development</string>.
        if text.contains("<key>aps-environment</key>"),
           text.range(of: "aps-environment</key>\\s*<string>development",
                      options: .regularExpression) != nil {
            return "sandbox"
        }
        // An embedded profile without a development aps-environment (ad-hoc/enterprise)
        // still pushes through production.
        return "production"
    }

    /// Returns the door's words on success, or throws/returns nil-equivalent via the words.
    @discardableResult
    public func register(
        token: String,
        env: String = PushTokenClient.apnsEnvironment(),
        host: String, port: Int,
        now: Int64 = Int64(Date().timeIntervalSince1970),
        nonce: String = ObservationClient.freshNonce()
    ) async throws -> String {
        let obj: [String: Any] = [
            "node": ["node_id": node.nodeId, "pubkey": node.pubkeyHex, "label": node.label],
            "token": token,
            "env": env,
            "ts": now,
            "nonce": nonce,
        ]
        guard JSONSerialization.isValidJSONObject(obj),
              let body = try? JSONSerialization.data(withJSONObject: obj) else {
            throw URLError(.cannotParseResponse)
        }
        let sig = try node.sign(body)
        guard let url = URL(string: "https://\(host):\(port)/mesh/push-token") else {
            throw URLError(.badURL)
        }
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.timeoutInterval = 8
        req.setValue(sig, forHTTPHeaderField: "X-Familiar-Sig")
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = body
        let (data, resp) = try await urlSession.data(for: req)
        let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
        let said = String(data: data, encoding: .utf8) ?? ""
        if code != 200 {
            throw NSError(domain: "PushTokenClient", code: code,
                          userInfo: [NSLocalizedDescriptionKey: said.isEmpty ? "door said \(code)" : said])
        }
        return said
    }
}
