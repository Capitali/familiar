import Foundation

/// The v1 secret import bundle written by tools/provision-envoy-credential.sh — the only
/// artifact that ever carries the Envoy's bearer. Importing it moves the secret into this
/// app's own Keychain item and the non-secret configuration into preferences; the caller
/// is told to delete the file afterwards (and offered that deletion at import).
struct ImportBundle: Codable, Equatable {
    enum BundleError: Error, CustomStringConvertible {
        case unsupportedVersion(Int)
        case insecureOrigin(String)
        case emptySecret

        var description: String {
            switch self {
            case .unsupportedVersion(let v):
                "import bundle version \(v) is not understood (this build speaks v1)"
            case .insecureOrigin(let origin):
                "the bundle's origin must be an https:// URL ending in /mcp — got \(origin)"
            case .emptySecret: "the bundle carries no bearer token"
            }
        }
    }

    let version: Int
    let registrationId: String
    let alias: String
    let mcpOrigin: String
    let bearerToken: String

    enum CodingKeys: String, CodingKey {
        case version
        case registrationId = "registration_id"
        case alias
        case mcpOrigin = "mcp_origin"
        case bearerToken = "bearer_token"
    }

    /// Parse and validate. Loopback http is tolerated for hermetic fixtures only —
    /// the provisioning script itself refuses to write a non-https origin.
    static func parse(_ data: Data) throws -> ImportBundle {
        let bundle = try JSONDecoder().decode(ImportBundle.self, from: data)
        guard bundle.version == 1 else { throw BundleError.unsupportedVersion(bundle.version) }
        guard !bundle.bearerToken.isEmpty else { throw BundleError.emptySecret }
        let url = URL(string: bundle.mcpOrigin)
        let scheme = url?.scheme?.lowercased() ?? ""
        let host = url?.host ?? ""
        let loopback = host == "127.0.0.1" || host == "localhost"
        guard let url, url.path.hasSuffix("/mcp"),
            scheme == "https" || (scheme == "http" && loopback)
        else { throw BundleError.insecureOrigin(bundle.mcpOrigin) }
        return bundle
    }
}
