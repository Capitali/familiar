import CryptoKit
import Foundation

/// The identity filter's wire (ADR-0026). A knock lands a device as a **guest** — a real cert,
/// reads succeed, projected. Membership follows automatically the moment identity is
/// **established by evidence** at `POST /mesh/introduce`. Nothing here is approved by anybody:
/// the 403 text on a refusal is the door naming the path to admission, and it is shown to the
/// human verbatim.

/// What the device *says* about who it serves. A claim addresses; it admits nothing.
public struct IdentityClaim: Codable, Equatable {
    public var handle: String
    public var ts: Int64
    public init(handle: String, ts: Int64 = Int64(Date().timeIntervalSince1970)) {
        self.handle = handle
        self.ts = ts
    }
}

/// A member-signed, single-use, ten-minute invite (evidence class E3). Carries the minting
/// member's cert so ANY door can verify it — and no group secret, ever. `expected_handle`
/// empty means the newcomer introduces their own (new) handle; naming a handle is the
/// inviter's deliberate act and covers that handle, existing or new — which is exactly how a
/// device handoff works: the old device invites the new one as its own human.
public struct InviteToken: Codable, Equatable {
    public var token_id: String
    public var group_id: String
    public var minted_by: Membership
    public var expected_handle: String
    public var issued: Int64
    public var expires: Int64
    public var sig: String

    public static let ttlSecs: Int64 = 10 * 60

    /// Mint on this (member) device. The signature covers the **canonical body** — the exact
    /// byte sequence the Rust door reconstructs (`record::InviteBody`, serde_json, declared
    /// field order). Built by hand here because JSONEncoder does not promise key order; the
    /// Rust side pins this format with a conformance test so the two can never drift apart.
    public static func mint(node: NodeKey, membership: Membership, expectedHandle: String,
                            now: Int64 = Int64(Date().timeIntervalSince1970)) throws -> InviteToken {
        let raw = SymmetricKey(size: .bits128).withUnsafeBytes { Data($0) }
        let tokenId = raw.map { String(format: "%02x", $0) }.joined()
        let issued = now
        let expires = now + ttlSecs
        let body = canonicalBody(tokenId: tokenId, groupId: membership.group_id,
                                 mintedByNode: membership.node_id,
                                 expectedHandle: expectedHandle, issued: issued, expires: expires)
        let sig = try node.sign(Data(body.utf8))
        return InviteToken(token_id: tokenId, group_id: membership.group_id,
                           minted_by: membership, expected_handle: expectedHandle,
                           issued: issued, expires: expires, sig: sig)
    }

    /// serde_json of Rust's `InviteBody`, byte for byte: declared field order, no spaces,
    /// strings minimally escaped. Inputs are hex ids and slug handles, but escape anyway —
    /// a body that silently diverged from the door's reconstruction would just read "invite:
    /// signature did not verify" with nothing to debug.
    static func canonicalBody(tokenId: String, groupId: String, mintedByNode: String,
                              expectedHandle: String, issued: Int64, expires: Int64) -> String {
        "{\"token_id\":\(jsonString(tokenId)),\"group_id\":\(jsonString(groupId)),"
            + "\"minted_by_node\":\(jsonString(mintedByNode)),"
            + "\"expected_handle\":\(jsonString(expectedHandle)),"
            + "\"issued\":\(issued),\"expires\":\(expires)}"
    }

    /// Minimal JSON string encoding, matching serde_json: `"` and `\` escaped, control
    /// characters as \u00XX (serde_json uses short escapes for \n\r\t — match those too).
    static func jsonString(_ s: String) -> String {
        var out = "\""
        for scalar in s.unicodeScalars {
            switch scalar {
            case "\"": out += "\\\""
            case "\\": out += "\\\\"
            case "\n": out += "\\n"
            case "\r": out += "\\r"
            case "\t": out += "\\t"
            default:
                if scalar.value < 0x20 {
                    out += String(format: "\\u%04x", scalar.value)
                } else {
                    out.unicodeScalars.append(scalar)
                }
            }
        }
        return out + "\""
    }
}

/// The evidence a device presents for the identity filter. Mirrors the Rust `Evidence` enum's
/// internally-tagged wire (`{"class": "…", …}`). Rotation proofs and device vouchers are minted
/// device-to-device (the watch link) and ride the same shapes when that path lands.
public enum Evidence {
    /// E4 — introduce yourself: a name and your own words. The DOOR decides the provenance
    /// from what it actually observed about the connection; nothing this device claims about
    /// where it is carries any weight, so `remote` is sent and honesty costs nothing.
    case introduction(handle: String, statement: String, ts: Int64)
    /// E3 — a member's deliberate act, displaced in time.
    case invite(InviteToken)

    var jsonObject: [String: Any] {
        switch self {
        case .introduction(let handle, let statement, let ts):
            return [
                "class": "introduction",
                "intro": ["handle": handle, "statement": statement, "ts": ts],
                "provenance": ["kind": "remote"],
            ]
        case .invite(let t):
            return [
                "class": "invite",
                "token_id": t.token_id,
                "group_id": t.group_id,
                "minted_by": [
                    "node_id": t.minted_by.node_id, "node_pubkey": t.minted_by.node_pubkey,
                    "issued": t.minted_by.issued, "expiry": t.minted_by.expiry,
                    "group_id": t.minted_by.group_id, "cert": t.minted_by.cert,
                ],
                "expected_handle": t.expected_handle,
                "issued": t.issued, "expires": t.expires, "sig": t.sig,
            ]
        }
    }
}

/// `POST /mesh/introduce` — run the identity filter. Signed over the raw body with the node
/// key, like every mesh write; the knock must already have happened (the contract filter).
public struct AdmissionClient {
    public enum Outcome: Equatable {
        /// Both filters hold — the device is a member, serving `handle` (may be empty when the
        /// establishment named no one, e.g. a migrated household device).
        case member(handle: String)
        /// Not yet — `path` is the door's own words for what would work. Show it verbatim.
        case notYet(path: String)
        /// Held (a correction's cool-off). Try again after `retryIn` seconds.
        case held(retryIn: Int64)
        case error(String)
    }

    public var node: NodeKey
    public var urlSession: URLSession

    public init(node: NodeKey, urlSession: URLSession = MeshTLS.session) {
        self.node = node
        self.urlSession = urlSession
    }

    public func introduce(
        claim: IdentityClaim?,
        evidence: Evidence,
        host: String,
        port: Int,
        now: Int64 = Int64(Date().timeIntervalSince1970),
        nonce: String = ObservationClient.freshNonce()
    ) async throws -> Outcome {
        var obj: [String: Any] = [
            "node": ["node_id": node.nodeId, "pubkey": node.pubkeyHex, "label": node.label],
            "evidence": evidence.jsonObject,
            "nonce": nonce,
            "ts": now,
        ]
        if let c = claim { obj["claim"] = ["handle": c.handle, "ts": c.ts] }
        guard JSONSerialization.isValidJSONObject(obj),
              let body = try? JSONSerialization.data(withJSONObject: obj) else {
            return .error("could not encode the introduction")
        }
        let sig = try node.sign(body)
        guard let url = URL(string: "https://\(host):\(port)/mesh/introduce") else {
            return .error("bad host")
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
        switch code {
        case 200:
            let obj2 = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
            return .member(handle: obj2?["handle"] as? String ?? "")
        case 403: return .notYet(path: said)
        case 429:
            let after = (resp as? HTTPURLResponse)?.value(forHTTPHeaderField: "Retry-After")
                .flatMap { Int64($0) } ?? 60
            return .held(retryIn: after)
        default: return .error(said.isEmpty ? "host said \(code)" : said)
        }
    }
}

/// A member's deliberate reversal, traveling (ADR-0026 §5): sever, disestablish ("that's not
/// Betty"), hold, restore. Replaces the standing vote — there is nothing to vote on any more;
/// a correction is one member's attributable act, and it goes on the record.
public struct CorrectionClient {
    public enum Outcome: Equatable {
        case applied(state: String)
        case refused(String)
    }

    public var node: NodeKey
    public var membership: Membership
    public var groupPubkey: String
    public var urlSession: URLSession

    public init(node: NodeKey, membership: Membership, groupPubkey: String,
                urlSession: URLSession = MeshTLS.session) {
        self.node = node
        self.membership = membership
        self.groupPubkey = groupPubkey
        self.urlSession = urlSession
    }

    /// `act`: "sever" | "disestablish" | "hold" | "restore". `reason` travels with the act —
    /// "that's not Betty" is half the record's value a year later.
    public func correct(
        subject: String,
        act: String,
        reason: String,
        host: String,
        port: Int,
        now: Int64 = Int64(Date().timeIntervalSince1970),
        nonce: String = ObservationClient.freshNonce()
    ) async throws -> Outcome {
        let obj: [String: Any] = [
            "membership": [
                "node_id": membership.node_id, "node_pubkey": membership.node_pubkey,
                "issued": membership.issued, "expiry": membership.expiry,
                "group_id": membership.group_id, "cert": membership.cert,
            ],
            "group_pubkey": groupPubkey,
            "correction": [
                "act": act,
                "subject_device": subject,
                "corrected_by": node.nodeId,
                "reason": reason,
                "ts": now,
                "nonce": nonce,
                // The envelope's raw-body signature (X-Familiar-Sig) is what the door
                // verifies; the inner field exists for records that travel onward.
                "sig": "",
            ],
        ]
        guard let body = try? JSONSerialization.data(withJSONObject: obj) else {
            return .refused("could not encode the correction")
        }
        let sig = try node.sign(body)
        guard let url = URL(string: "https://\(host):\(port)/mesh/correct") else {
            return .refused("bad host")
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
        if code == 200 {
            let obj2 = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
            return .applied(state: obj2?["state"] as? String ?? said)
        }
        return .refused(said.isEmpty ? "host said \(code)" : said)
    }
}
