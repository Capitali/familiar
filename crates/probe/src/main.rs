//! Scratch diagnostic: build a SIGNED worldview ViewRequest exactly as a console would,
//! from this machine's member identity, and print body + sig for curl.
use std::path::PathBuf;

fn main() {
    let dir = PathBuf::from(std::env::var("HOME").unwrap())
        .join("Library/Application Support/Familiar/data");
    let key = familiar_mesh::node::NodeKey::load_or_mint(&dir, "probe").expect("node key");
    let cred = familiar_mesh::group::load(&dir)
        .expect("group load")
        .expect("enrolled");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let req = serde_json::json!({
        "node": key.identity(),
        "membership": cred.membership,
        "ts": now,
        "nonce": format!("probe-{now}"),
    });
    let raw = serde_json::to_vec(&req).unwrap();
    let sig = key.sign(&raw);
    std::fs::write("/tmp/probe-body.json", &raw).unwrap();
    println!("{sig}");
}
