//! Emit a deterministic golden test vector for the membership `CertBody` canonicalization, so the
//! Swift `FamiliarMesh` client can prove byte-for-byte agreement with this Rust implementation.
//! ed25519 is deterministic (RFC 8032), so from fixed secrets the cert is reproducible: Swift that
//! produces the same `cert` hex has matched our exact JSON canonicalization + signing.
//!
//! Run: cargo run -p familiar-mesh --example cert_vector

use familiar_mesh::{group, node::NodeKey};

// Fixed 32-byte secrets (hex) so the vector is stable across runs.
const GROUP_SECRET: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const NODE_SECRET: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const ISSUED: i64 = 1_700_000_000;
const TTL: i64 = 7_776_000; // 90 days

fn main() {
    let dir = std::env::temp_dir().join(format!("familiar_cert_vector_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("mesh")).unwrap();
    // Seed the node key file so load_or_mint loads our fixed key instead of minting a random one.
    std::fs::write(dir.join("mesh/node_key"), NODE_SECRET).unwrap();
    let node = NodeKey::load_or_mint(&dir, "vector").unwrap();

    // join_group derives the group from the secret and mints THIS node's membership at ISSUED.
    let cred = group::join_group(&dir, &node, GROUP_SECRET, "vector", ISSUED, TTL).unwrap();
    let m = &cred.membership;

    println!("GROUP_SECRET   {GROUP_SECRET}");
    println!("GROUP_PUBKEY   {}", cred.group_pubkey);
    println!("GROUP_ID       {}", cred.group_id);
    println!("NODE_SECRET    {NODE_SECRET}");
    println!("NODE_PUBKEY    {}", node.identity().pubkey);
    println!("NODE_ID        {}", m.node_id);
    println!("ISSUED         {}", m.issued);
    println!("EXPIRY         {}", m.expiry);
    println!("CERT           {}", m.cert);

    // Reconstruct the canonical CertBody by hand and prove it reproduces the real cert — so the
    // Swift client can assert byte-equality against a string we've *verified* is what serde emits.
    let body = format!(
        "{{\"node_id\":\"{}\",\"node_pubkey\":\"{}\",\"issued\":{},\"expiry\":{},\"group_id\":\"{}\"}}",
        m.node_id, m.node_pubkey, m.issued, m.expiry, m.group_id
    );
    use ed25519_dalek::{Signer, SigningKey};
    let mut secret = [0u8; 32];
    for (i, b) in secret.iter_mut().enumerate() {
        *b = u8::from_str_radix(&GROUP_SECRET[i * 2..i * 2 + 2], 16).unwrap();
    }
    let sig = SigningKey::from_bytes(&secret).sign(body.as_bytes());
    let sig_hex = sig.to_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>();
    assert_eq!(sig_hex, m.cert, "manual CertBody must reproduce the real cert");
    println!("CERT_BODY      {body}");

    let _ = std::fs::remove_dir_all(&dir);
}
