//! APNs — the ember reaches a pocket even when the app sleeps.
//!
//! iOS suspends the console within seconds of the screen locking, so a turn passed at the
//! fire used to wait until the holder happened to look at their phone. A door that holds an
//! APNs sender config pushes a **visible, time-sensitive alert** to every registered device
//! of the human whose turn it became — the OS shows it whether or not the app is running.
//!
//! Three pieces, all best-effort (a door without the config, a device without a token, or an
//! unreachable APNs simply mean no push — the game itself never depends on this):
//!
//! - **Registration** (`POST /mesh/push-token`, transport.rs): a member device posts its APNs
//!   device token + environment, signed like any other member write. Stored per node in
//!   `mesh/push_tokens.json` (atomic replace, same law as the peer roster).
//! - **The sender config** (`mesh/apns.json`): `{key_path, key_id, team_id, topic}` pointing
//!   at an Apple-issued `.p8` ES256 key. Only doors that should push carry the file.
//! - **The send**: a provider JWT (ES256 over the P-256 key, signed with ring — already in
//!   the tree under rustls) and an HTTP/2 POST to APNs — shelled to `curl --http2`, which
//!   both doors ship, so hyper stays http1-only.
//!
//! Environments matter: development-provisioned installs (Xcode, `devicectl`) register
//! **sandbox** tokens; TestFlight/App Store installs register **production**. The device
//! reports which it is; the door routes to the matching gateway. A key can be restricted to
//! sandbox-only at creation — production sends then fail with `BadEnvironmentKeyInToken`
//! until the key is re-issued for both (observed live, 2026-08-07).

use crate::{Error, Result};
use base64::Engine as _;
use std::path::Path;

/// Door-side sender config. Absent file = this door does not push.
pub const APNS_FILE: &str = "mesh/apns.json";
/// The per-device token registry: a JSON array of [`PushToken`].
pub const TOKENS_FILE: &str = "mesh/push_tokens.json";

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ApnsConfig {
    /// Path to the Apple `.p8` ES256 auth key (PKCS#8 PEM).
    pub key_path: String,
    /// The key's 10-char id (shown at creation).
    pub key_id: String,
    /// The Apple developer team id.
    pub team_id: String,
    /// The app's bundle id — APNs calls it the topic.
    pub topic: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PushToken {
    pub node_id: String,
    /// Hex APNs device token.
    pub token: String,
    /// "sandbox" | "production" — which gateway this token belongs to.
    pub env: String,
    pub updated: i64,
}

pub fn load_config(dir: &Path) -> Option<ApnsConfig> {
    let s = std::fs::read_to_string(dir.join(APNS_FILE)).ok()?;
    serde_json::from_str(&s).ok()
}

pub fn load_tokens(dir: &Path) -> Vec<PushToken> {
    std::fs::read_to_string(dir.join(TOKENS_FILE))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Atomic replace — same law as the peer roster (ADR-0029 §2): a torn read must never be
/// mistaken for an empty registry and saved back.
fn save_tokens(dir: &Path, tokens: &[PushToken]) -> Result<()> {
    let path = dir.join(TOKENS_FILE);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_file_name(format!("push_tokens.json.tmp-{}", std::process::id()));
    std::fs::write(&tmp, serde_json::to_vec_pretty(tokens)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Remember (or refresh) a device's token. One row per node — a device re-registering with a
/// new token (reinstall, OS refresh) simply replaces its old one.
pub fn upsert_token(dir: &Path, node_id: &str, token: &str, env: &str, now: i64) -> Result<()> {
    let env = if env == "production" { "production" } else { "sandbox" };
    let mut tokens = load_tokens(dir);
    tokens.retain(|t| t.node_id != node_id);
    tokens.push(PushToken {
        node_id: node_id.to_string(),
        token: token.to_string(),
        env: env.to_string(),
        updated: now,
    });
    save_tokens(dir, &tokens)
}

fn b64url(data: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// Read the `.p8` (PKCS#8 PEM) into DER for ring.
fn read_p8_der(path: &str) -> Result<Vec<u8>> {
    let pem = std::fs::read_to_string(path)?;
    let body: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");
    base64::engine::general_purpose::STANDARD
        .decode(body.trim())
        .map_err(|e| Error::Untrusted(format!("apns key: not valid PEM base64: {e}")))
}

/// Mint an ES256 provider token: `{"alg":"ES256","kid":…}` / `{"iss":team,"iat":now}`.
/// APNs accepts a token for up to an hour; we mint per send — pushes are rare events.
pub fn provider_jwt(cfg: &ApnsConfig, now: i64) -> Result<String> {
    let der = read_p8_der(&cfg.key_path)?;
    let rng = ring::rand::SystemRandom::new();
    let key = ring::signature::EcdsaKeyPair::from_pkcs8(
        &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
        &der,
        &rng,
    )
    .map_err(|e| Error::Untrusted(format!("apns key rejected: {e}")))?;
    let header = b64url(format!(r#"{{"alg":"ES256","kid":"{}"}}"#, cfg.key_id).as_bytes());
    let claims = b64url(format!(r#"{{"iss":"{}","iat":{}}}"#, cfg.team_id, now).as_bytes());
    let signing_input = format!("{header}.{claims}");
    let sig = key
        .sign(&rng, signing_input.as_bytes())
        .map_err(|e| Error::Untrusted(format!("apns sign failed: {e}")))?;
    Ok(format!("{signing_input}.{}", b64url(sig.as_ref())))
}

/// Send one alert push. Returns curl's HTTP status line + body for the caller's log.
async fn send_one(cfg: &ApnsConfig, jwt: &str, t: &PushToken, payload: &str) -> String {
    let host = if t.env == "production" {
        "api.push.apple.com"
    } else {
        "api.sandbox.push.apple.com"
    };
    let url = format!("https://{host}/3/device/{}", t.token);
    let out = tokio::process::Command::new("curl")
        .args([
            "-s",
            "-w",
            "\n%{http_code}",
            "--http2",
            "-m",
            "15",
            "-X",
            "POST",
            &url,
            "-H",
            &format!("authorization: bearer {jwt}"),
            "-H",
            &format!("apns-topic: {}", cfg.topic),
            "-H",
            "apns-push-type: alert",
            "-H",
            "apns-priority: 10",
            "-d",
            payload,
        ])
        .output()
        .await;
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().replace('\n', " | "),
        Err(e) => format!("curl failed: {e}"),
    }
}

/// The turn changed hands: push "the ember is yours" to every registered device of the
/// holder. Spawned off the reply path — best-effort, never blocks a judge's answer.
/// No-op unless this door carries `mesh/apns.json`.
pub fn spawn_notify_turn(dir: &Path, holder: &str, kind: &str) {
    let Some(cfg) = load_config(dir) else { return };
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    // The holder is a HUMAN handle (ADR-0028); their devices are the member records whose
    // established handle matches. Tokens exist only for devices that registered.
    let holder_lc = holder.to_lowercase();
    let device_ids: Vec<String> = crate::record::load_all(dir)
        .into_iter()
        .filter(|r| {
            crate::record::derive_state(r) == crate::record::RecordState::Member
                && r.identity
                    .established
                    .as_ref()
                    .is_some_and(|e| e.handle.to_lowercase() == holder_lc)
        })
        .map(|r| r.device_id)
        .collect();
    let tokens: Vec<PushToken> = load_tokens(dir)
        .into_iter()
        .filter(|t| device_ids.iter().any(|d| d == &t.node_id))
        .collect();
    if tokens.is_empty() {
        return;
    }
    let body = match kind {
        "campfire" => "the campfire — your turn at the fire",
        "changeling" => "the changeling — three lines, one human truth. Come look",
        "pact" => "the pact — the constitution has dealt; come rule",
        _ => "riddle of the mesh — your turn at the fire",
    };
    let payload = format!(
        r#"{{"aps":{{"alert":{{"title":"🔥 the ember is yours","body":"{body}"}},"sound":"default","interruption-level":"time-sensitive"}},"ember":"{kind}"}}"#
    );
    let now = crate::transport::now_secs();
    handle.spawn(async move {
        let jwt = match provider_jwt(&cfg, now) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("apns: no provider token: {e}");
                return;
            }
        };
        for t in tokens {
            let r = send_one(&cfg, &jwt, &t, &payload).await;
            if !r.ends_with("200") {
                eprintln!("apns: push to {}({}) -> {}", &t.node_id[..8.min(t.node_id.len())], t.env, r);
            }
        }
    });
}

/// Announce a riddle WIN to every member device — the fanfare reaches the phones in pockets
/// too (B13), not only the winner's. Best-effort; needs registered tokens and an APNs config.
pub fn spawn_notify_win(dir: &Path, winner: &str, kind: &str) {
    let Some(cfg) = load_config(dir) else { return };
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let member_ids: Vec<String> = crate::record::load_all(dir)
        .into_iter()
        .filter(|r| crate::record::derive_state(r) == crate::record::RecordState::Member)
        .map(|r| r.device_id)
        .collect();
    let tokens: Vec<PushToken> = load_tokens(dir)
        .into_iter()
        .filter(|t| member_ids.iter().any(|d| d == &t.node_id))
        .collect();
    if tokens.is_empty() {
        return;
    }
    let title = match kind {
        "changeling" => "✦ the changeling is done",
        "pact" => "✦ the pact is settled",
        _ => "✦ the riddle is solved",
    };
    let body = if winner.is_empty() {
        "someone took it".to_string()
    } else {
        format!("{winner} took it")
    };
    let payload = format!(
        r#"{{"aps":{{"alert":{{"title":"{title}","body":"{body}"}},"sound":"default"}},"win":"{kind}"}}"#
    );
    let now = crate::transport::now_secs();
    handle.spawn(async move {
        let jwt = match provider_jwt(&cfg, now) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("apns: no provider token: {e}");
                return;
            }
        };
        for t in tokens {
            let r = send_one(&cfg, &jwt, &t, &payload).await;
            if !r.ends_with("200") {
                eprintln!("apns: win push to {}({}) -> {}", &t.node_id[..8.min(t.node_id.len())], t.env, r);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_registry_upserts_one_row_per_node() {
        let d = std::env::temp_dir().join("familiar_push_test_registry");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        upsert_token(&d, "node-a", "tok1", "sandbox", 100).unwrap();
        upsert_token(&d, "node-b", "tok2", "production", 101).unwrap();
        upsert_token(&d, "node-a", "tok3", "weird-env", 102).unwrap();
        let ts = load_tokens(&d);
        assert_eq!(ts.len(), 2);
        let a = ts.iter().find(|t| t.node_id == "node-a").unwrap();
        assert_eq!(a.token, "tok3");
        assert_eq!(a.env, "sandbox"); // unknown env falls back to sandbox, never production
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn provider_jwt_signs_with_a_p256_key() {
        // A throwaway P-256 key in PKCS#8, generated for this test only.
        let d = std::env::temp_dir().join("familiar_push_test_jwt");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let rng = ring::rand::SystemRandom::new();
        let doc = ring::signature::EcdsaKeyPair::generate_pkcs8(
            &ring::signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            &rng,
        )
        .unwrap();
        let pem = format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            base64::engine::general_purpose::STANDARD.encode(doc.as_ref())
        );
        let key_path = d.join("k.p8");
        std::fs::write(&key_path, pem).unwrap();
        let cfg = ApnsConfig {
            key_path: key_path.to_string_lossy().into_owned(),
            key_id: "TESTKEY123".into(),
            team_id: "TEAMID1234".into(),
            topic: "io.river.familiar.ios".into(),
        };
        let jwt = provider_jwt(&cfg, 1_786_000_000).unwrap();
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(parts[0])
            .unwrap();
        assert!(String::from_utf8(header).unwrap().contains("TESTKEY123"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
