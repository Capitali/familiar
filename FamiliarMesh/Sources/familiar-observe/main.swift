import Foundation
import FamiliarMesh

// A macOS stand-in for the phone: parse an enrollment payload (from `familiar mesh qr`), mint a
// membership cert, and POST one signed derived observation to the familiar's /mesh/observe. Prints
// `node_id=<id> recorded=<n>` on success. This is the real Swift→Rust interop check.
//
// Usage: familiar-observe '<payload-json>'|@file [object]

func fail(_ msg: String, _ code: Int32) -> Never {
    FileHandle.standardError.write(Data((msg + "\n").utf8))
    exit(code)
}

let args = CommandLine.arguments
guard args.count >= 2 else { fail("usage: familiar-observe <payload-json|@file> [object]", 2) }

var payloadArg = args[1]
if payloadArg.hasPrefix("@") {
    payloadArg = (try? String(contentsOfFile: String(payloadArg.dropFirst()), encoding: .utf8)) ?? ""
}
let object = args.count >= 3 ? args[2] : "location:home"

guard let p = EnrollmentPayload.parse(payloadArg), let secret = p.secretData, let url = p.observeURL else {
    fail("bad enrollment payload", 3)
}

let node = NodeKey(label: "swift-e2e")
guard let membership = try? Cert.mint(
    groupSecret: secret, node: node.identity,
    issued: Int64(Date().timeIntervalSince1970), ttlSecs: defaultCertTTLSecs,
    expectedGroupId: p.group
) else { fail("cert mint failed", 4) }

let client = ObservationClient(session: .init(node: node, membership: membership, url: url))
let obs = ObsRecord(actor: "phone:swift", action: "reports", object: object,
                    context: "swift e2e batch", confidence: 0.95)

do {
    let n = try await client.send([obs])
    print("node_id=\(node.nodeId) recorded=\(n)")
} catch {
    fail("send error: \(error)", 5)
}
