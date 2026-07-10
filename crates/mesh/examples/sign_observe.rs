//! A tiny device-agent stand-in: mint a membership cert against a data dir's group, build a
//! signed observation batch, write the body to a file and print the `X-Familiar-Sig` value —
//! so a plain `curl` can exercise the live `/mesh/observe` endpoint end-to-end. This is the
//! Rust preview of what the Swift `FamiliarMesh` module will do on the phone.
//!
//! Usage: cargo run -p familiar-mesh --example sign_observe -- <data-dir> <object> <nonce> <out-body-file>

use familiar_mesh::{group, node::NodeKey, ObsRecord, ObserveEnvelope};
use std::path::PathBuf;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn main() {
    let mut a = std::env::args().skip(1);
    let data_dir = PathBuf::from(a.next().expect("data-dir"));
    let object = a.next().unwrap_or_else(|| "location:home".into());
    let nonce = a.next().unwrap_or_else(|| format!("n{}", now()));
    let out = a.next().unwrap_or_else(|| "/tmp/observe_body.json".into());

    let cred = group::load(&data_dir)
        .expect("load group")
        .expect("no group enrolled in that data dir");
    let devdir = std::env::temp_dir().join(format!("familiar_e2e_dev_{}", std::process::id()));
    let device = NodeKey::load_or_mint(&devdir, "iPhone-e2e").expect("device key");
    let id = device.identity();
    let membership = cred
        .mint_membership(&id.node_id, &id.pubkey, now(), group::DEFAULT_CERT_TTL_SECS)
        .expect("mint cert");

    let env = ObserveEnvelope {
        node: id,
        membership,
        ts: now(),
        nonce,
        observations: vec![ObsRecord {
            actor: "phone:ian".into(),
            action: "reports".into(),
            object,
            context: "e2e signed batch".into(),
            confidence: 0.95,
        }],
    };
    let raw = serde_json::to_vec(&env).unwrap();
    let sig = device.sign(&raw);
    std::fs::write(&out, &raw).expect("write body");
    eprintln!("node_id={} body={out}", device.node_id());
    println!("{sig}"); // stdout = the signature, for the X-Familiar-Sig header
}
