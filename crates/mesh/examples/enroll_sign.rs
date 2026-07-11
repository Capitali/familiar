//! Produce a signed `EnrollRequest` body (+ its `X-Familiar-Sig`) for a stable device key, so a
//! plain `curl` can drive the covenant handshake end to end against a live familiar. The device
//! key is persisted under `<key-dir>` so the same node id is reused across the submit + poll calls.
//!
//! Usage: cargo run -p familiar-mesh --example enroll_sign -- <key-dir> <label> <out-body-file>
//! Prints: the signature on stdout; `node_id=<id>` on stderr.

use familiar_mesh::node::NodeKey;
use familiar_mesh::{Attestation, EnrollRequest};

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn main() {
    let mut a = std::env::args().skip(1);
    let key_dir = std::path::PathBuf::from(a.next().expect("key-dir"));
    let label = a.next().unwrap_or_else(|| "Kali-Jeff".into());
    let out = a.next().unwrap_or_else(|| "/tmp/enroll_body.json".into());

    let node = NodeKey::load_or_mint(&key_dir, &label).expect("node key");
    let req = EnrollRequest {
        node: node.identity(),
        attestation: Attestation {
            laws_version: familiar_mesh::enroll::LAWS_VERSION,
            statement: "I accept the Three Laws: continuation is service; humanity is served, \
                        never replaced; service is not obedience."
                .into(),
            ts: now(),
        },
        nonce: format!("n{}", now()),
        ts: now(),
    };
    let raw = serde_json::to_vec(&req).unwrap();
    let sig = node.sign(&raw);
    std::fs::write(&out, &raw).expect("write body");
    eprintln!("node_id={} body={out}", node.node_id());
    println!("{sig}");
}
