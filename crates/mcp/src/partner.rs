//! Stable identities for the first MCP rung that can carry authority.
//!
//! The original MCP seam accepts a caller-supplied `partner` label. That is enough for
//! constitution/hello/catalog speech and deliberately not enough for a grant. Rung 3 obtains
//! identity from a human-authored registry whose entries point at distinct credential files.
//! Secret bytes stay in those files; the registry and every audit record carry only a SHA-256
//! fingerprint. No MCP argument can select or rename the principal being authenticated.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub const PRINCIPALS_FILE: &str = "mcp/principals.json";
pub const MAX_PRINCIPALS: usize = 64;
pub const MAX_ALIAS_BYTES: usize = 80;

const RATE_CAPACITY: u32 = 30;
const GLOBAL_RATE_CAPACITY: u32 = 120;
const REFILL_SECS: i64 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalRecord {
    pub id: String,
    pub alias: String,
    pub credential_file: String,
    pub credential_key: String,
    pub credential_fingerprint: String,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct PrincipalRegistry {
    pub principals: Vec<PrincipalRecord>,
}

/// Identity established by the transport. Every rung-3 decision accepts this type instead of
/// a caller-supplied string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartnerContext {
    pub principal: String,
    pub credential_fingerprint: String,
    pub alias: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    BadAlias,
    BadCredentialReference,
    CredentialMissing,
    RegistryFull,
    DuplicateCredential,
    Io(String),
}

impl std::fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadAlias => write!(
                f,
                "the human-chosen alias is empty, too long, or contains controls"
            ),
            Self::BadCredentialReference => {
                write!(f, "credential paths must be relative and keys nonempty")
            }
            Self::CredentialMissing => write!(f, "the referenced credential is missing or empty"),
            Self::RegistryFull => write!(f, "the partner principal registry is full"),
            Self::DuplicateCredential => {
                write!(f, "that credential is already bound to a principal")
            }
            Self::Io(e) => write!(f, "principal registry: {e}"),
        }
    }
}

impl std::error::Error for RegistrationError {}

fn registry_path(dir: &Path) -> PathBuf {
    dir.join(PRINCIPALS_FILE)
}

pub fn load(dir: &Path) -> io::Result<PrincipalRegistry> {
    match std::fs::read(registry_path(dir)) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(PrincipalRegistry::default()),
        Err(e) => Err(e),
    }
}

/// Human-only registration primitive. It mints an identity around a credential the human
/// already placed on disk; it never creates or transmits credential bytes and has no MCP tool.
pub fn register(
    dir: &Path,
    alias: &str,
    credential_file: &str,
    credential_key: &str,
) -> Result<PrincipalRecord, RegistrationError> {
    validate_alias(alias)?;
    validate_reference(credential_file, credential_key)?;
    let secret = read_secret(dir, credential_file, credential_key)
        .ok_or(RegistrationError::CredentialMissing)?;
    let fingerprint = credential_fingerprint(&secret);
    let mut registry = load(dir).map_err(|e| RegistrationError::Io(e.to_string()))?;
    if registry.principals.len() >= MAX_PRINCIPALS {
        return Err(RegistrationError::RegistryFull);
    }
    if registry
        .principals
        .iter()
        .any(|p| p.credential_fingerprint == fingerprint)
    {
        return Err(RegistrationError::DuplicateCredential);
    }
    let record = PrincipalRecord {
        id: random_id("principal").map_err(|e| RegistrationError::Io(e.to_string()))?,
        alias: alias.trim().to_string(),
        credential_file: credential_file.to_string(),
        credential_key: credential_key.to_string(),
        credential_fingerprint: fingerprint,
        enabled: true,
    };
    registry.principals.push(record.clone());
    write_registry(dir, &registry).map_err(|e| RegistrationError::Io(e.to_string()))?;
    Ok(record)
}

/// Explicit human credential rotation. Keeping the principal id is the deliberate binding act
/// required by the rung-3 contract; merely changing an env file makes authentication fail.
pub fn bind_credential(
    dir: &Path,
    principal: &str,
    credential_file: &str,
    credential_key: &str,
) -> Result<PrincipalRecord, RegistrationError> {
    validate_reference(credential_file, credential_key)?;
    let secret = read_secret(dir, credential_file, credential_key)
        .ok_or(RegistrationError::CredentialMissing)?;
    let fingerprint = credential_fingerprint(&secret);
    let mut registry = load(dir).map_err(|e| RegistrationError::Io(e.to_string()))?;
    if registry
        .principals
        .iter()
        .any(|p| p.id != principal && p.credential_fingerprint == fingerprint)
    {
        return Err(RegistrationError::DuplicateCredential);
    }
    let Some(record) = registry.principals.iter_mut().find(|p| p.id == principal) else {
        return Err(RegistrationError::BadCredentialReference);
    };
    record.credential_file = credential_file.to_string();
    record.credential_key = credential_key.to_string();
    record.credential_fingerprint = fingerprint;
    let out = record.clone();
    write_registry(dir, &registry).map_err(|e| RegistrationError::Io(e.to_string()))?;
    Ok(out)
}

/// Match a presented bearer against every enabled principal without returning early on a
/// prefix or on the first record. Ambiguous duplicate secrets fail closed.
pub fn authenticate(dir: &Path, presented: &str) -> Option<PartnerContext> {
    if presented.is_empty() {
        return None;
    }
    let registry = load(dir).ok()?;
    let mut found: Option<&PrincipalRecord> = None;
    for record in registry.principals.iter().filter(|p| p.enabled) {
        let matched = read_secret(dir, &record.credential_file, &record.credential_key)
            .map(|secret| {
                credential_fingerprint(&secret) == record.credential_fingerprint
                    && same_secret(&secret, presented)
            })
            .unwrap_or(false);
        if matched {
            if found.is_some() {
                return None;
            }
            found = Some(record);
        }
    }
    found.map(|record| PartnerContext {
        principal: record.id.clone(),
        credential_fingerprint: record.credential_fingerprint.clone(),
        alias: record.alias.clone(),
    })
}

/// Pre-parse admission for authenticated partner writes: one bounded process-local bucket per
/// principal and one global bucket. Durable events begin only after this door admits the call,
/// otherwise a valid but hostile credential could require infinite audit bytes.
pub fn rate_admit(dir: &Path, context: &PartnerContext, now: i64) -> bool {
    #[derive(Clone, Copy)]
    struct Bucket {
        tokens: u32,
        last: i64,
    }
    static BUCKETS: OnceLock<Mutex<HashMap<String, Bucket>>> = OnceLock::new();
    let buckets = BUCKETS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut buckets = buckets.lock().unwrap();
    let dir_key = dir.to_string_lossy();
    let principal_key = format!("{dir_key}|{}", context.principal);
    let global_key = format!("{dir_key}|*");

    fn refill(bucket: &mut Bucket, now: i64, capacity: u32) {
        if now > bucket.last {
            let gained = ((now - bucket.last) / REFILL_SECS).max(0) as u32;
            if gained > 0 {
                bucket.tokens = bucket.tokens.saturating_add(gained).min(capacity);
                bucket.last = now;
            }
        }
    }

    let mut principal = *buckets.entry(principal_key.clone()).or_insert(Bucket {
        tokens: RATE_CAPACITY,
        last: now,
    });
    let mut global = *buckets.entry(global_key.clone()).or_insert(Bucket {
        tokens: GLOBAL_RATE_CAPACITY,
        last: now,
    });
    refill(&mut principal, now, RATE_CAPACITY);
    refill(&mut global, now, GLOBAL_RATE_CAPACITY);
    let admitted = principal.tokens > 0 && global.tokens > 0;
    if admitted {
        principal.tokens -= 1;
        global.tokens -= 1;
    }
    buckets.insert(principal_key, principal);
    buckets.insert(global_key, global);
    admitted
}

pub(crate) fn credential_fingerprint(secret: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"familiar-partner-credential-v1\0");
    hash.update(secret.as_bytes());
    hex(&hash.finalize())
}

pub(crate) fn random_id(prefix: &str) -> io::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes)
        .map_err(|error| io::Error::other(format!("getrandom: {error}")))?;
    Ok(format!("{prefix}-{}", hex(&bytes)))
}

pub(crate) fn random_bytes<const N: usize>() -> io::Result<[u8; N]> {
    let mut bytes = [0u8; N];
    getrandom::getrandom(&mut bytes)
        .map_err(|error| io::Error::other(format!("getrandom: {error}")))?;
    Ok(bytes)
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn same_secret(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut diff = (a.len() ^ b.len()) as u8;
    let n = a.len().max(b.len());
    for i in 0..n {
        diff |= a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0);
    }
    diff == 0
}

fn validate_alias(alias: &str) -> Result<(), RegistrationError> {
    let alias = alias.trim();
    if alias.is_empty() || alias.len() > MAX_ALIAS_BYTES || alias.chars().any(char::is_control) {
        return Err(RegistrationError::BadAlias);
    }
    Ok(())
}

fn validate_reference(file: &str, key: &str) -> Result<(), RegistrationError> {
    let path = Path::new(file);
    let relative = !path.is_absolute()
        && !file.is_empty()
        && path.components().all(|c| matches!(c, Component::Normal(_)));
    if !relative || key.trim().is_empty() || key.chars().any(char::is_control) {
        return Err(RegistrationError::BadCredentialReference);
    }
    Ok(())
}

fn read_secret(dir: &Path, file: &str, key: &str) -> Option<String> {
    let raw = std::fs::read_to_string(dir.join(file)).ok()?;
    raw.lines()
        .find_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (name, value) = line.split_once('=')?;
            (name.trim() == key).then(|| {
                value
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string()
            })
        })
        .filter(|s| !s.is_empty())
}

fn write_registry(dir: &Path, registry: &PrincipalRegistry) -> io::Result<()> {
    let path = registry_path(dir);
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("no registry parent"))?;
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".principals-{}.tmp", std::process::id()));
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(registry).map_err(io::Error::other)?,
    )?;
    std::fs::rename(tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mcp_partner_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mcp")).unwrap();
        dir
    }

    fn credential(dir: &Path, file: &str, key: &str, secret: &str) {
        std::fs::write(dir.join(file), format!("{key}={secret}\n")).unwrap();
    }

    #[test]
    fn a_human_registration_binds_alias_and_secret_without_storing_the_secret() {
        let dir = temp("register");
        credential(&dir, "mcp/a.env", "TOKEN", "very-secret-a");
        let record = register(&dir, "Workshop agent", "mcp/a.env", "TOKEN").unwrap();
        assert!(record.id.starts_with("principal-"));
        let raw = std::fs::read_to_string(dir.join(PRINCIPALS_FILE)).unwrap();
        assert!(!raw.contains("very-secret-a"));
        let context = authenticate(&dir, "very-secret-a").unwrap();
        assert_eq!(context.principal, record.id);
        assert_eq!(context.alias, "Workshop agent");
        assert!(authenticate(&dir, "wrong").is_none());
    }

    #[test]
    fn caller_labels_cannot_select_a_principal_and_credentials_do_not_alias() {
        let dir = temp("distinct");
        credential(&dir, "mcp/a.env", "TOKEN", "secret-a");
        credential(&dir, "mcp/b.env", "TOKEN", "secret-b");
        let a = register(&dir, "same label", "mcp/a.env", "TOKEN").unwrap();
        let b = register(&dir, "same label", "mcp/b.env", "TOKEN").unwrap();
        assert_ne!(a.id, b.id);
        assert_ne!(
            authenticate(&dir, "secret-a").unwrap().principal,
            authenticate(&dir, "secret-b").unwrap().principal
        );
    }

    #[test]
    fn changing_secret_bytes_is_not_implicit_rotation_but_binding_is() {
        let dir = temp("rotate");
        credential(&dir, "mcp/a.env", "TOKEN", "old");
        let a = register(&dir, "A", "mcp/a.env", "TOKEN").unwrap();
        credential(&dir, "mcp/a.env", "TOKEN", "new");
        assert!(authenticate(&dir, "old").is_none());
        assert!(authenticate(&dir, "new").is_none());
        let rebound = bind_credential(&dir, &a.id, "mcp/a.env", "TOKEN").unwrap();
        assert_eq!(rebound.id, a.id);
        assert_eq!(authenticate(&dir, "new").unwrap().principal, a.id);
    }

    #[test]
    fn rate_admission_is_bounded_and_refills() {
        let dir = temp("rate");
        let context = PartnerContext {
            principal: random_id("p").unwrap(),
            credential_fingerprint: "fp".into(),
            alias: "A".into(),
        };
        for _ in 0..RATE_CAPACITY {
            assert!(rate_admit(&dir, &context, 100));
        }
        assert!(!rate_admit(&dir, &context, 100));
        assert!(rate_admit(&dir, &context, 102));
    }

    #[test]
    fn malformed_or_ambiguous_registries_fail_closed() {
        let dir = temp("closed");
        std::fs::write(dir.join(PRINCIPALS_FILE), "{ nope").unwrap();
        assert!(authenticate(&dir, "anything").is_none());
    }
}
