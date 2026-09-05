//! The familiar's core, exported for device shells (ADR-0009 Phase 0).
//!
//! A capable phone or tablet embeds this library and IS a full peer: it founds its own
//! mesh on first launch (no host, no server, no introducer), or joins one by covenant,
//! and serves the same worldview its console renders. The shell (Swift/Kotlin) owns
//! presentation and platform sensors; everything constitutional lives here.
//!
//! First API surface — deliberately small, all JSON-at-the-seam (the sphere web layer
//! consumes worldview JSON identically whether it came from a remote node or this
//! embedded one):
//!   found / join_payload / is_founded — lifecycle
//!   worldview_json — the read seam, in-process
//!   answer — the human speaks
//!   mesh_start / mesh_stop — the gossip transport
//!   invite_payload — mint an invitation (QR/share-link body)
//!   whisker_advise — the ship pilot's doctrine over wire JSON the shell fetched (T-237 B4)

use std::path::PathBuf;
use std::sync::Mutex;

uniffi::setup_scaffolding!();

static MESH: Mutex<Option<familiar_mesh::transport::MeshHandle>> = Mutex::new(None);

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Whether a familiar already lives in this data dir (found or joined).
#[uniffi::export]
pub fn is_founded(data_dir: String) -> bool {
    let dir = PathBuf::from(&data_dir);
    matches!(familiar_mesh::group::load(&dir), Ok(Some(_)))
}

/// Found a new familiar: mint the node key and create its own group. The first person
/// anywhere begins here — node #1, population 1, nothing else required. Returns the
/// group id, or an error string.
#[uniffi::export]
pub fn found(data_dir: String, label: String) -> String {
    let dir = PathBuf::from(&data_dir);
    let _ = std::fs::create_dir_all(&dir);
    let node = match familiar_mesh::node::NodeKey::load_or_mint(&dir, &label) {
        Ok(n) => n,
        Err(e) => return format!("error: {e}"),
    };
    match familiar_mesh::group::create_group(
        &dir,
        &node,
        &label,
        now(),
        familiar_mesh::group::DEFAULT_CERT_TTL_SECS,
    ) {
        Ok(cred) => cred.group_id,
        Err(e) => format!("error: {e}"),
    }
}

/// Join an existing familiar from an enrollment payload (the QR / share-link body).
/// Returns the group id, or an error string.
#[uniffi::export]
pub fn join(data_dir: String, label: String, secret: String, group_label: String) -> String {
    let dir = PathBuf::from(&data_dir);
    let _ = std::fs::create_dir_all(&dir);
    let node = match familiar_mesh::node::NodeKey::load_or_mint(&dir, &label) {
        Ok(n) => n,
        Err(e) => return format!("error: {e}"),
    };
    match familiar_mesh::group::join_group(
        &dir,
        &node,
        &secret,
        &group_label,
        now(),
        familiar_mesh::group::DEFAULT_CERT_TTL_SECS,
    ) {
        Ok(cred) => cred.group_id,
        Err(e) => format!("error: {e}"),
    }
}

/// The worldview, exactly as the read seam serves it — the sphere renders this JSON
/// whether it came over TLS from a peer or from right here.
#[uniffi::export]
pub fn worldview_json(data_dir: String) -> String {
    let dir = PathBuf::from(&data_dir);
    let Ok(Some(cred)) = familiar_mesh::group::load(&dir) else {
        return String::new();
    };
    match familiar_mesh::worldview::assemble_worldview(&dir, &cred, now()) {
        Ok(view) => serde_json::to_string(&view).unwrap_or_default(),
        Err(_) => String::new(),
    }
}

/// The human speaks to their familiar (the console answer path).
#[uniffi::export]
pub fn answer(data_dir: String, human: String, text: String) {
    let dir = PathBuf::from(&data_dir);
    let t = text.trim();
    if t.is_empty() {
        return;
    }
    let obs = familiar_kernel::observation::Observation::new(
        &human,
        "told the familiar",
        t,
        "console",
        "local",
        now(),
        1.0,
    );
    let _ = familiar_kernel::observation::record(&dir, obs);
    let _ = std::fs::write(dir.join("question.txt"), "");
    let _ = std::fs::write(dir.join("active_question.txt"), "");
}

/// Start the gossip transport (TLS mesh port + LAN discovery) inside this process.
#[uniffi::export]
pub fn mesh_start(data_dir: String) {
    let mut guard = MESH.lock().unwrap_or_else(|p| p.into_inner());
    if guard.is_none() {
        *guard = Some(familiar_mesh::transport::spawn(PathBuf::from(data_dir)));
    }
}

/// Stop the gossip transport.
#[uniffi::export]
pub fn mesh_stop() {
    let mut guard = MESH.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(handle) = guard.take() {
        handle.shutdown();
    }
}

/// Mint an invitation for a new device/peer: the enrollment payload as JSON (group
/// secret included — trusted screens only).
#[uniffi::export]
pub fn invite_payload(data_dir: String) -> String {
    let dir = PathBuf::from(&data_dir);
    let Ok(Some(cred)) = familiar_mesh::group::load(&dir) else {
        return String::new();
    };
    let port = familiar_mesh::config::load(&dir)
        .map(|c| c.gossip_port)
        .unwrap_or(47_100);
    let hosts = familiar_mesh::transport::reachable_hosts();
    serde_json::json!({
        "v": 1,
        "secret": cred.join_key(),
        "group": cred.group_id,
        "label": cred.label,
        "host": hosts.first().cloned().unwrap_or_default(),
        "hosts": hosts,
        "port": port,
        "tlspin": familiar_mesh::transport::tls_spki_pin(&dir).unwrap_or_default(),
    })
    .to_string()
}

/// The ship's computer's MIND, in the shell's hand (T-237 B4, "one doctrine, two
/// runtimes"). The shell fetches `/v1/me`, `/v1/loadboard`, `/v1/stations` and the
/// routes it wants priced with the captain's own key, hands them over as one JSON
/// object (see `familiar_whisker::wire::advise`), and gets back what the pilot would
/// do now: the decision, the dial surface it spends, the captain's level on it, the
/// automation it needs. Byte-for-byte the doctrine the host runner flies; this never
/// acts — acting is the shell's, under the captain's tap and the act scope.
#[uniffi::export]
pub fn whisker_advise(input_json: String) -> String {
    let input: serde_json::Value = match serde_json::from_str(&input_json) {
        Ok(v) => v,
        Err(e) => {
            return serde_json::json!({"error": format!("input is not JSON: {e}")}).to_string()
        }
    };
    familiar_whisker::wire::advise(&input).to_string()
}
