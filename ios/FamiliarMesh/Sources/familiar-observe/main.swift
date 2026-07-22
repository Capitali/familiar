import Foundation
import FamiliarMesh

// A macOS stand-in for the phone that exercises the WHOLE covenant flow against a live familiar:
// generate keys → attest & request to join → poll until the human approves → then POST one signed
// derived observation using the granted cert. The real Swift→Rust proof of Brick 1 + ingestion.
//
// Usage: familiar-observe <host> <port> [object]
//   Approve on the familiar with:  familiar mesh approve <node_id>  (printed below), or
//   open a window first with:       familiar mesh invite

func fail(_ msg: String, _ code: Int32) -> Never {
    FileHandle.standardError.write(Data((msg + "\n").utf8)); exit(code)
}

let args = CommandLine.arguments
guard args.count >= 3, let port = Int(args[2]) else { fail("usage: familiar-observe <host> <port> [object]", 2) }
let host = args[1]
let object = args.count >= 4 ? args[3] : "location:home"

let node = NodeKey(label: "swift-e2e")
FileHandle.standardError.write(Data("node_id=\(node.nodeId)  (approve with: familiar mesh approve \(node.nodeId))\n".utf8))

let enroller = EnrollmentClient(host: host, port: port)

func obtainGrant() async throws -> Grant {
    if let g = try await enroller.requestJoin(node: node) { return g } // auto-approved (invite window)
    // Pending: poll until the human approves (up to ~2 min).
    for _ in 0..<60 {
        try await Task.sleep(nanoseconds: 2_000_000_000)
        if let g = try await enroller.pollGrant(nodeId: node.nodeId) { return g }
        FileHandle.standardError.write(Data("… waiting for approval\n".utf8))
    }
    fail("timed out waiting for approval", 6)
}

do {
    let grant = try await obtainGrant()
    let url = URL(string: "https://\(host):\(port)/mesh/observe")!
    let client = ObservationClient(session: .init(node: node, membership: grant.membership, url: url))
    let n = try await client.send([ObsRecord(actor: "phone:swift", action: "reports", object: object,
                                             context: "post-covenant observe", confidence: 0.95)])
    print("admitted to \(grant.group_label); recorded=\(n)")
} catch {
    fail("error: \(error)", 5)
}
