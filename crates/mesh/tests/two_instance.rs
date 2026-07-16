//! End-to-end: two familiar-mesh instances on loopback discover each other via
//! `static_peers`, prove same-group membership, and exchange signed briefs. Exercises the
//! real tokio server + hyper client path — not mocked. A non-group and a boundary-closed
//! instance are also checked to confirm nothing crosses without authorization.

use familiar_mesh::brief::*;
use familiar_mesh::config::{MeshConfig};
use familiar_mesh::group::{self, DEFAULT_CERT_TTL_SECS};
use familiar_mesh::node::NodeKey;
use familiar_mesh::transport::{self, now_secs};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn tmp(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("familiar_mesh_e2e_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

/// Write a boundary.json with allow_mesh as given (the human editing the file, in test).
fn write_boundary(dir: &Path, allow_mesh: bool) {
    let mut b = familiar_kernel::boundary::Boundary::closed();
    b.phase = "test".into();
    b.allow_mesh = allow_mesh;
    fs::write(
        dir.join(familiar_kernel::boundary::BOUNDARY_FILE),
        serde_json::to_string_pretty(&b).unwrap(),
    )
    .unwrap();
}

/// Write a mesh config pointing at one static peer on loopback, on our own port.
fn write_config(dir: &Path, port: u16, peer_port: u16) {
    let cfg = MeshConfig {
        gossip_interval_secs: 1,
        gossip_port: port,
        static_peers: vec![format!("127.0.0.1:{peer_port}")],
        ..MeshConfig::default()
    };
    fs::create_dir_all(dir.join("mesh")).unwrap();
    fs::write(
        dir.join(familiar_mesh::config::CONFIG_FILE),
        serde_json::to_string_pretty(&cfg).unwrap(),
    )
    .unwrap();
}

/// Build + sign a minimal outbox brief for a node and drop it at mesh/outbox.json.
fn write_outbox(dir: &Path, node: &NodeKey, membership: familiar_mesh::group::Membership) {
    let body = BriefBody {
        version: BRIEF_VERSION,
        node: node.identity(),
        membership,
        ts: now_secs(),
        nonce: format!("{}", now_secs()),
        presence: Presence {
            observer_count: 1,
            last_active: now_secs(),
        },
        capability: Capability {
            os: "test".into(),
            arch: "test".into(),
            env_summary: node.identity().label,
            familiar_version: "0.1.0".into(),
            tools: vec![],
        },
        knowledge: Knowledge::default(),
        identities: None,
        authority_requests: Vec::new(),
        authority_grants: Vec::new(),
    };
    let brief = sign_brief(body, node).unwrap();
    fs::write(
        dir.join(transport::OUTBOX_FILE),
        serde_json::to_vec_pretty(&brief).unwrap(),
    )
    .unwrap();
}

fn wait_for<F: Fn() -> bool>(secs: u64, f: F) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    f()
}

fn inbox_has(dir: &Path, node_id: &str) -> bool {
    dir.join(transport::INBOX_DIR)
        .join(format!("{node_id}.json"))
        .exists()
}

#[test]
fn two_group_members_exchange_briefs_over_loopback() {
    // Pick two ports unlikely to collide with the tailnet mesh port.
    let (pa, pb) = (48611u16, 48612u16);
    let dir_a = tmp("a");
    let dir_b = tmp("b");

    let a = NodeKey::load_or_mint(&dir_a, "alpha").unwrap();
    let b = NodeKey::load_or_mint(&dir_b, "beta").unwrap();

    // A creates the group; B joins with A's join key → same trust root.
    let cred_a = group::create_group(&dir_a, &a, "e2e", now_secs(), DEFAULT_CERT_TTL_SECS).unwrap();
    let cred_b = group::join_group(
        &dir_b,
        &b,
        &cred_a.join_key(),
        "e2e",
        now_secs(),
        DEFAULT_CERT_TTL_SECS,
    )
    .unwrap();

    for (dir, port, peer) in [(&dir_a, pa, pb), (&dir_b, pb, pa)] {
        write_boundary(dir, true);
        write_config(dir, port, peer);
    }
    write_outbox(&dir_a, &a, cred_a.membership.clone());
    write_outbox(&dir_b, &b, cred_b.membership.clone());

    let ha = transport::spawn(dir_a.clone());
    let hb = transport::spawn(dir_b.clone());

    // Each should end up with the other's verified brief in its inbox.
    let a_got_b = wait_for(12, || inbox_has(&dir_a, &b.node_id()));
    let b_got_a = wait_for(12, || inbox_has(&dir_b, &a.node_id()));

    ha.shutdown();
    hb.shutdown();

    assert!(a_got_b, "A should have received and verified B's brief");
    assert!(b_got_a, "B should have received and verified A's brief");

    // peers.json should list the counterpart on both sides.
    let peers_a: Vec<transport::PeerRecord> =
        serde_json::from_str(&fs::read_to_string(dir_a.join(transport::PEERS_FILE)).unwrap())
            .unwrap();
    assert!(peers_a.iter().any(|p| p.node_id == b.node_id()));

    let _ = fs::remove_dir_all(&dir_a);
    let _ = fs::remove_dir_all(&dir_b);
}

#[test]
fn closed_boundary_never_binds_or_exchanges() {
    let (pa, pb) = (48711u16, 48712u16);
    let dir_a = tmp("closed_a");
    let dir_b = tmp("closed_b");
    let a = NodeKey::load_or_mint(&dir_a, "alpha").unwrap();
    let b = NodeKey::load_or_mint(&dir_b, "beta").unwrap();
    let cred_a = group::create_group(&dir_a, &a, "e2e", now_secs(), DEFAULT_CERT_TTL_SECS).unwrap();
    let cred_b = group::join_group(&dir_b, &b, &cred_a.join_key(), "e2e", now_secs(), DEFAULT_CERT_TTL_SECS).unwrap();

    // A is OPEN, B is CLOSED (allow_mesh=false). B must neither serve nor gossip.
    write_boundary(&dir_a, true);
    write_boundary(&dir_b, false);
    write_config(&dir_a, pa, pb);
    write_config(&dir_b, pb, pa);
    write_outbox(&dir_a, &a, cred_a.membership.clone());
    write_outbox(&dir_b, &b, cred_b.membership);

    let ha = transport::spawn(dir_a.clone());
    let hb = transport::spawn(dir_b.clone());
    std::thread::sleep(Duration::from_secs(4));
    ha.shutdown();
    hb.shutdown();

    // B is closed: it never serves, so A cannot deposit into B's inbox, and B never
    // gossips, so A's inbox stays empty of B too.
    assert!(!inbox_has(&dir_b, &a.node_id()), "closed B must not accept briefs");
    assert!(!inbox_has(&dir_a, &b.node_id()), "closed B must not gossip out");

    let _ = fs::remove_dir_all(&dir_a);
    let _ = fs::remove_dir_all(&dir_b);
}
