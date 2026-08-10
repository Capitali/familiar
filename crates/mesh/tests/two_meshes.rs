//! ADR-0033 end to end over real TLS loopback: two DIFFERENT meshes — separate group keys,
//! separate trust roots — federate through the door. A member of "river" mints a mesh
//! invite; "cedar" redeems it over the wire and adopts river as a sibling; river holds
//! cedar pending (reads fail closed) until the welcome tap; then cedar reads river's
//! worldview at the sibling rung — handle and declaration visible, names withheld.

use familiar_mesh::config::MeshConfig;
use familiar_mesh::federation;
use familiar_mesh::group::{self, DEFAULT_CERT_TTL_SECS};
use familiar_mesh::node::NodeKey;
use familiar_mesh::transport::{self, now_secs};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn tmp(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("familiar_fed_e2e_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

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

fn write_config(dir: &Path, port: u16, declared: &[&str]) {
    let cfg = MeshConfig {
        gossip_interval_secs: 3600, // no gossip needed — federation dials the door directly
        gossip_port: port,
        lan_discovery: false,
        declared_areas: declared.iter().map(|s| s.to_string()).collect(),
        ..MeshConfig::default()
    };
    fs::create_dir_all(dir.join("mesh")).unwrap();
    fs::write(
        dir.join(familiar_mesh::config::CONFIG_FILE),
        serde_json::to_string_pretty(&cfg).unwrap(),
    )
    .unwrap();
}

fn wait_for<F: Fn() -> bool>(secs: u64, f: F) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    false
}

#[test]
fn two_meshes_federate_through_the_door() {
    let pa = 48911u16;
    let dir_a = tmp("river");
    let dir_b = tmp("cedar");

    // Two meshes, two trust roots — cedar is NOT in river's group.
    let a = NodeKey::load_or_mint(&dir_a, "river-node").unwrap();
    let b = NodeKey::load_or_mint(&dir_b, "cedar-node").unwrap();
    let cred_a =
        group::create_group(&dir_a, &a, "river", now_secs(), DEFAULT_CERT_TTL_SECS).unwrap();
    let cred_b =
        group::create_group(&dir_b, &b, "cedar", now_secs(), DEFAULT_CERT_TTL_SECS).unwrap();

    write_boundary(&dir_a, true);
    write_boundary(&dir_b, true);
    write_config(&dir_a, pa, &["weather", "energy"]);
    write_config(&dir_b, pa + 1, &[]);

    // Only river's door needs to be up — cedar dials it as a client.
    let ha = transport::spawn(dir_a.clone());
    assert!(
        wait_for(10, || std::net::TcpStream::connect(("127.0.0.1", pa))
            .is_ok()),
        "river's door should bind"
    );

    // 1. A member of river mints the invite, naming river's door.
    let invite = federation::mint_mesh_invite(
        &a,
        &cred_a.membership,
        &cred_a,
        vec![format!("127.0.0.1:{pa}")],
        now_secs(),
    )
    .unwrap();
    let payload = invite.encode().unwrap();

    // 2. Cedar redeems over the wire: river answers with its introduction and stands as a
    //    sibling on cedar's side (pasting the invite was cedar's human's act).
    let river_on_b = federation::federate_with(&dir_b, &payload).unwrap();
    assert_eq!(river_on_b.handle, "river");
    assert_eq!(river_on_b.state, "sibling");
    assert_eq!(
        river_on_b.declared_areas,
        vec!["weather", "energy"],
        "the introduction carries river's declaration"
    );

    // 3. River holds cedar PENDING — cedar's sibling read fails closed.
    let cedar_on_a = federation::load_sibling(&dir_a, &cred_b.group_id).unwrap();
    assert_eq!(
        cedar_on_a.state, "pending",
        "consent is a human tap, never automatic"
    );
    assert!(
        federation::read_sibling_worldview(&dir_b, &cred_a.group_id).is_err(),
        "pending reads nothing"
    );

    // 4. The tap (a member of river welcomes) — and cedar reads at the sibling rung.
    federation::welcome_sibling(&dir_a, &cred_b.group_id, "ian", now_secs()).unwrap();
    let json = federation::read_sibling_worldview(&dir_b, &cred_a.group_id).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        v.get("group_label").and_then(|s| s.as_str()),
        Some("river"),
        "a sibling knows whose door it reads"
    );
    let declared: Vec<&str> = v
        .get("declared_areas")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().filter_map(|s| s.as_str()).collect())
        .unwrap_or_default();
    assert_eq!(declared, vec!["weather", "energy"]);
    // The ladder holds over the wire: no member carries a real human name.
    if let Some(members) = v.get("members").and_then(|m| m.as_array()) {
        for m in members {
            let human = m.get("human").and_then(|h| h.as_str()).unwrap_or("");
            assert!(
                human.is_empty() || human == "someone",
                "a sibling never sees a name — got {human:?}"
            );
        }
    }
    // Cedar sees itself standing in river's sibling list, as itself.
    let sibs = v
        .get("siblings")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(
        sibs.iter()
            .any(|s| s.get("handle").and_then(|h| h.as_str()) == Some("cedar")),
        "the reader sees itself as itself"
    );

    // 5. A replayed redemption of the spent invite is refused at the door.
    assert!(
        federation::federate_with(&dir_b, &payload).is_err(),
        "each token introduces once"
    );

    ha.shutdown();
    let _ = fs::remove_dir_all(&dir_a);
    let _ = fs::remove_dir_all(&dir_b);
}
