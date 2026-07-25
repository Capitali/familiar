import Foundation
import CryptoKit
import Security

// Transport security for the mesh (ADR-0009 Phase 1): every node serves TLS with a
// self-signed certificate over a persistent P-256 key. Authenticity still comes from the
// covenant signatures on the payloads — TLS adds confidentiality on any path.
//
// A connection is accepted when the server cert's SPKI SHA-256 is one of the pins the device
// trusts. That set is the GROUP's node keys, not one node (ADR-0012): a device pins its
// enrolling familiar AND every sibling that familiar vouches for (the lighthouse, peers), so
// it can fail over to whichever member it can reach — the enrolling node on the LAN, the
// lighthouse on cellular — without a pin mismatch. With no pins at all (older enrollments)
// any self-signed cert is accepted, which still ends passive observation.
public enum MeshTLS {
    /// The set of trusted SPKI SHA-256 pins (hex): the enrolling node's plus every sibling's
    /// the mesh advertised. A cert is accepted iff its SPKI is in here (empty ⇒ accept any).
    public static var pins: Set<String> = []

    /// Back-compat: the single-pin accessor older code sets. Writing it seeds the set; reading
    /// returns any member (the set is what the delegate checks).
    public static var pin: String? {
        get { pins.first }
        set {
            if let p = newValue, !p.isEmpty { pins.insert(p) }
        }
    }

    /// Adopt additional trusted pins (from a worldview read that advertised the group's set).
    public static func trust(_ more: [String]) {
        for p in more where !p.isEmpty { pins.insert(p) }
    }

    /// The URLSession every mesh client uses.
    public static let session: URLSession = {
        URLSession(configuration: .default, delegate: Delegate(), delegateQueue: nil)
    }()

    static func spkiHex(for key: SecKey) -> String? {
        guard let rep = SecKeyCopyExternalRepresentation(key, nil) as Data? else { return nil }
        // A P-256 public key exports as the X9.63 point (04‖X‖Y, 65 bytes). SPKI DER is a
        // fixed 26-byte header + that point — reconstruct it so the hash matches the
        // server's SHA-256(SubjectPublicKeyInfo).
        guard rep.count == 65, rep.first == 0x04 else { return nil }
        let header: [UInt8] = [
            0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01,
            0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
        ]
        var spki = Data(header)
        spki.append(rep)
        return SHA256.hash(data: spki).map { String(format: "%02x", $0) }.joined()
    }

    final class Delegate: NSObject, URLSessionDelegate {
        func urlSession(
            _ session: URLSession,
            didReceive challenge: URLAuthenticationChallenge,
            completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void
        ) {
            guard challenge.protectionSpace.authenticationMethod == NSURLAuthenticationMethodServerTrust,
                  let trust = challenge.protectionSpace.serverTrust
            else { return completionHandler(.performDefaultHandling, nil) }

            if !MeshTLS.pins.isEmpty {
                guard let chain = SecTrustCopyCertificateChain(trust) as? [SecCertificate],
                      let leaf = chain.first,
                      let key = SecCertificateCopyKey(leaf),
                      let got = MeshTLS.spkiHex(for: key),
                      MeshTLS.pins.contains(got)
                else { return completionHandler(.cancelAuthenticationChallenge, nil) }
            }
            // SPKI is a trusted group pin (or no pins yet): accept the self-signed cert. The
            // covenant signatures on every payload remain the authenticity floor either way.
            completionHandler(.useCredential, URLCredential(trust: trust))
        }
    }
}
