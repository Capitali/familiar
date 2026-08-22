import CryptoKit
import Foundation

/// The familiar's mesh serves a self-signed certificate (CN=familiar-mesh); partners are
/// expected to pin its TLS key rather than chase a CA. This mirrors the console's pin
/// algorithm (MeshTLS.spkiHex) WITHOUT linking FamiliarMesh — the Envoy stays outside:
/// SHA-256 over the DER SubjectPublicKeyInfo, reconstructed from the P-256 X9.63 point.
///
/// Behavior: with a pin configured, ONLY a server whose SPKI matches is accepted — for
/// any host, since the door origin is the only endpoint this app has. With no pin
/// configured, system trust applies unchanged (a CA-signed door needs nothing).
enum DoorPinning {
    static func spkiHex(for key: SecKey) -> String? {
        guard let rep = SecKeyCopyExternalRepresentation(key, nil) as Data? else { return nil }
        guard rep.count == 65, rep.first == 0x04 else { return nil }
        let header: [UInt8] = [
            0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01,
            0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
        ]
        var spki = Data(header)
        spki.append(rep)
        return SHA256.hash(data: spki).map { String(format: "%02x", $0) }.joined()
    }

    /// A URLSession that pins the door's SPKI when `pin` is set; plain system trust otherwise.
    static func session(pin: String?) -> URLSession {
        guard let pin, !pin.isEmpty else { return .shared }
        return URLSession(
            configuration: .ephemeral, delegate: PinDelegate(pin: pin.lowercased()),
            delegateQueue: nil)
    }

    final class PinDelegate: NSObject, URLSessionDelegate {
        let pin: String
        init(pin: String) { self.pin = pin }

        func urlSession(
            _ session: URLSession,
            didReceive challenge: URLAuthenticationChallenge,
            completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?)
                -> Void
        ) {
            guard
                challenge.protectionSpace.authenticationMethod
                    == NSURLAuthenticationMethodServerTrust,
                let trust = challenge.protectionSpace.serverTrust,
                let key = SecTrustCopyKey(trust),
                let spki = DoorPinning.spkiHex(for: key),
                spki == pin
            else { return completionHandler(.cancelAuthenticationChallenge, nil) }
            completionHandler(.useCredential, URLCredential(trust: trust))
        }
    }
}
