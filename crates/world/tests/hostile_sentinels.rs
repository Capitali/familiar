//! ADR-0045's decisive test, at the partition rung (build-order step 2): seed
//! household-only sentinels, prove no ship store contains them; seed ship-only
//! sentinels, prove the household read path never sees them; prove a receipt cannot
//! smuggle a payload; prove the inbound door accepts only the control plane; prove the
//! lease fails closed every way it can be wrong.
//!
//! The full-cadence version of this test (run the complete ship reasoning loop and
//! replies) arrives with build-order step 5, when a ship cadence exists to run.

use std::fs;
use std::path::Path;

use familiar_kernel::observation::{self, Observation};
use familiar_world::bridge::{self, AttentionNotice, ControlEnvelope, Provenance};
use familiar_world::instance::{self, Lifecycle};
use familiar_world::lease;

const HH_SENTINEL: &str = "HH-SENTINEL-betty-slept-badly-9f31";
const SHIP_SENTINEL: &str = "SHIP-SENTINEL-io-slagworks-haul-77c2";

/// Every byte of every file under `root` — the bluntest possible reader, so a leak
/// cannot hide behind a loader that politely skips it.
fn all_bytes_under(root: &Path) -> String {
    let mut out = String::new();
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                out.push_str(&all_bytes_under(&p));
            } else {
                out.push_str(&String::from_utf8_lossy(&fs::read(&p).unwrap_or_default()));
            }
        }
    }
    out
}

fn seed_household(dir: &Path) {
    observation::record(
        dir,
        Observation::new(
            "betty",
            "told the familiar",
            HH_SENTINEL,
            "private household speech",
            "cli",
            1_000,
            1.0,
        ),
    )
    .unwrap();
}

#[test]
fn the_partition_holds_both_directions_and_the_bridge_refuses_every_wrong_thing() {
    let household = familiar_kernel::testing::temp_root("world_household");
    let ships = familiar_kernel::testing::temp_root("world_ships");
    seed_household(&household);

    // ---- Commission: the ship store is born elsewhere, closed, with its own key. ----
    let (w, ship_dir) = instance::commission(
        &household,
        &ships,
        "Purr aboard Long Haul",
        "ian",
        "",
        2_000,
    )
    .unwrap();
    assert_eq!(w.lifecycle, Lifecycle::Commissioned);
    assert_eq!(w.grant_epoch, 1);

    // Household sentinel appears nowhere in the ship's store — not in its key, its
    // boundary, anything.
    assert!(
        !all_bytes_under(&ship_dir).contains(HH_SENTINEL),
        "household speech leaked into a ship store"
    );

    // The ship store is born with every gate shut.
    let b = familiar_kernel::boundary::load(&ship_dir).unwrap();
    assert!(!b.allow_network && !b.allow_llm);

    // ---- Ship-side data stays ship-side. ----
    observation::record(
        &ship_dir,
        Observation::new("purr", "noticed", SHIP_SENTINEL, "", "ship", 2_100, 1.0),
    )
    .unwrap();
    let household_reads = observation::load(&household).unwrap();
    assert!(
        household_reads
            .iter()
            .all(|o| !format!("{o:?}").contains(SHIP_SENTINEL)),
        "a ship record reached the household read path"
    );
    assert!(
        !all_bytes_under(&household).contains(SHIP_SENTINEL),
        "ship bytes leaked into the household store"
    );

    // ---- The outward bridge: a receipt is delivery evidence, never payload. ----
    let ship_key = familiar_mesh::node::NodeKey::load_or_mint(&ship_dir, "purr").unwrap();
    let notice = AttentionNotice {
        provenance: Provenance {
            instance: w.id.clone(),
            source_key: ship_key.identity().pubkey,
            grant_epoch: 1,
            schema_version: 1,
            event_id: "evt-0001".into(),
        },
        kind: "low_stores".into(),
        headline: format!("water is low — {SHIP_SENTINEL}"),
        observed_at: 2_200,
        sent_at: 2_201,
        expires_at: 9_000,
        supersedes: None,
    };
    let raw = serde_json::to_vec(&notice).unwrap();
    let sig = ship_key.sign(&raw);

    let (delivered, receipt) = bridge::receive_notice(&household, &raw, &sig, 2_300).unwrap();
    assert!(delivered.headline.contains(SHIP_SENTINEL)); // the CALLER gets the notice…
    assert!(!format!("{receipt:?}").contains(SHIP_SENTINEL)); // …the receipt cannot carry it
    let receipts_raw = fs::read_to_string(household.join(bridge::RECEIPTS_FILE)).unwrap();
    assert!(
        !receipts_raw.contains(SHIP_SENTINEL),
        "the receipt log carries ship payload"
    );

    // Replay: the same event id is refused.
    assert!(bridge::receive_notice(&household, &raw, &sig, 2_400).is_err());

    // Tampering: one flipped byte fails the signature.
    let mut forged = raw.clone();
    let idx = forged.len() - 2;
    forged[idx] ^= 1;
    assert!(bridge::receive_notice(&household, &forged, &sig, 2_400).is_err());

    // Expiry: a dead notice is refused even when authentic.
    let mut late = notice.clone();
    late.provenance.event_id = "evt-0002".into();
    let late_raw = serde_json::to_vec(&late).unwrap();
    let late_sig = ship_key.sign(&late_raw);
    assert!(bridge::receive_notice(&household, &late_raw, &late_sig, 99_999).is_err());

    // ---- Decommission ends authority immediately, and only authority. ----
    let ended = instance::decommission(&household, &w.id).unwrap();
    assert_eq!(ended.lifecycle, Lifecycle::Decommissioned);
    assert_eq!(ended.grant_epoch, 2);
    let mut fresh = notice.clone();
    fresh.provenance.event_id = "evt-0003".into();
    let fresh_raw = serde_json::to_vec(&fresh).unwrap();
    let fresh_sig = ship_key.sign(&fresh_raw);
    assert!(
        bridge::receive_notice(&household, &fresh_raw, &fresh_sig, 2_500).is_err(),
        "a decommissioned instance still spoke"
    );
    assert!(
        ship_dir.join("boundary.json").exists(),
        "decommission touched the ship store — history was destroyed as a side effect"
    );
}

#[test]
fn the_inbound_door_accepts_only_the_five_human_acts() {
    let ok = serde_json::json!({
        "act": "CommissioningBundle",
        "instance": "world-abc",
        "label": "Purr",
        "commissioner": "ian",
        "constitution_sha256": "deadbeef",
        "schema_version": 1
    });
    assert!(matches!(
        bridge::control_from_slice(ok.to_string().as_bytes()).unwrap(),
        ControlEnvelope::CommissioningBundle { .. }
    ));

    // A household observation dressed as an envelope is refused at the door.
    let smuggled = serde_json::json!({
        "act": "Observation",
        "actor": "betty",
        "object": HH_SENTINEL
    });
    assert!(bridge::control_from_slice(smuggled.to_string().as_bytes()).is_err());

    // Even a real act with one extra field is refused — the door is exact.
    let padded = serde_json::json!({
        "act": "Rename",
        "instance": "world-abc",
        "label": "Purr II",
        "biography": HH_SENTINEL
    });
    assert!(bridge::control_from_slice(padded.to_string().as_bytes()).is_err());
}

#[test]
fn the_lease_fails_closed_every_way_it_can_be_wrong() {
    let issuer_dir = familiar_kernel::testing::temp_root("world_lease_issuer");
    let key = familiar_mesh::node::NodeKey::load_or_mint(&issuer_dir, "household").unwrap();
    let issuer = key.identity();

    let mut root = familiar_kernel::boundary::Boundary::closed();
    root.allow_network = true;

    let signed = lease::issue(&root, "world-abc", 600, 1_000, &key).unwrap();

    // Fresh, signed, right instance: the projected gate answers.
    assert!(lease::permits(
        Some(&signed),
        &issuer,
        "world-abc",
        1_100,
        |b| b.allow_network
    ));
    // The projection only projects — a gate the root holds shut stays shut.
    assert!(!lease::permits(
        Some(&signed),
        &issuer,
        "world-abc",
        1_100,
        |b| b.allow_llm
    ));
    // Stale: refused.
    assert!(!lease::permits(
        Some(&signed),
        &issuer,
        "world-abc",
        2_000,
        |b| b.allow_network
    ));
    // Missing: refused.
    assert!(!lease::permits(None, &issuer, "world-abc", 1_100, |b| b.allow_network));
    // Issued to someone else: refused.
    assert!(!lease::permits(
        Some(&signed),
        &issuer,
        "world-xyz",
        1_100,
        |b| b.allow_network
    ));
    // Tampered body: refused.
    let mut forged = signed.clone();
    forged.lease_json = forged.lease_json.replace("world-abc", "world-xyz");
    assert!(!lease::permits(
        Some(&forged),
        &issuer,
        "world-xyz",
        1_100,
        |b| b.allow_network
    ));
    // Signed by a stranger: refused.
    let stranger_dir = familiar_kernel::testing::temp_root("world_lease_stranger");
    let stranger = familiar_mesh::node::NodeKey::load_or_mint(&stranger_dir, "stranger").unwrap();
    let forged = lease::issue(&root, "world-abc", 600, 1_000, &stranger).unwrap();
    assert!(!lease::permits(
        Some(&forged),
        &issuer,
        "world-abc",
        1_100,
        |b| b.allow_network
    ));
}
