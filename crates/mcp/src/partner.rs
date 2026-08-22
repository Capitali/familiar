//! Stable identities for the first MCP rung that can carry authority.
//!
//! The original MCP seam accepts a caller-supplied `partner` label. That is enough for
//! constitution/hello/catalog speech and deliberately not enough for a grant. Rung 3 obtains
//! identity from a human-authored registry whose entries point at distinct credential files.
//! Secret bytes stay in those files; the registry and every audit record carry only a SHA-256
//! fingerprint. No MCP argument can select or rename the principal being authenticated.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub const PRINCIPALS_FILE: &str = "mcp/principals.json";
pub const PENDING_REGISTRATIONS_DIR: &str = "mcp/pending-registrations";
pub const MAX_PRINCIPALS: usize = 64;
pub const MAX_ALIAS_BYTES: usize = 80;
const MAX_PENDING_REGISTRATIONS: usize = 64;

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
    /// The established human who performed the registration ceremony. Missing on legacy
    /// records, which deliberately leaves rung 3 disabled until a new human-authenticated
    /// ceremony binds an addressee.
    #[serde(default)]
    pub registered_by: String,
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

/// The secret-free portion of a provisioning staged for one established human's console.
/// Credential paths and keys stay on the serving node and never enter this projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PendingRegistrationView {
    pub registration_id: String,
    pub partner_alias: String,
    pub credential_fingerprint: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingRegistration {
    version: u8,
    id: String,
    alias: String,
    credential_file: String,
    credential_key: String,
    credential_fingerprint: String,
    addressed_to: String,
    created_at: i64,
}

#[derive(Debug)]
struct LoadedPendingRegistration {
    record: PendingRegistration,
    path: PathBuf,
}

/// A human identity derived by the signed mesh door, never decoded from a decision payload.
///
/// The fields are private and this type is not deserializable. The mesh constructs it only
/// after certificate, node-key, freshness, standing, and effective-establishment checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanDecisionContext {
    device_node_id: String,
    human: String,
}

impl HumanDecisionContext {
    /// Cross-crate constructor for the verified mesh seam. Callers must have completed the
    /// checks documented on this type; ordinary request schemas cannot construct it.
    pub fn from_verified_mesh(device_node_id: String, human: String) -> Option<Self> {
        let device_node_id = device_node_id.trim();
        let human = human.trim();
        if device_node_id.is_empty() || human.is_empty() || human.chars().any(char::is_control) {
            return None;
        }
        Some(Self {
            device_node_id: device_node_id.to_string(),
            human: human.to_string(),
        })
    }

    pub fn device_node_id(&self) -> &str {
        &self.device_node_id
    }

    pub fn human(&self) -> &str {
        &self.human
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    BadAlias,
    BadCredentialReference,
    CredentialMissing,
    RegistryFull,
    DuplicateCredential,
    UnknownRegistration,
    WrongAddressee,
    CredentialChanged,
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
            Self::UnknownRegistration => write!(f, "that staged registration does not exist"),
            Self::WrongAddressee => {
                write!(f, "that staged registration is addressed to another human")
            }
            Self::CredentialChanged => write!(
                f,
                "the staged credential is missing or changed; provision a fresh ceremony"
            ),
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
    actor: &HumanDecisionContext,
    alias: &str,
    credential_file: &str,
    credential_key: &str,
) -> Result<PrincipalRecord, RegistrationError> {
    let _guard = registry_write_lock();
    register_unlocked(dir, actor, alias, credential_file, credential_key)
}

fn register_unlocked(
    dir: &Path,
    actor: &HumanDecisionContext,
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
        registered_by: actor.human().to_string(),
        enabled: true,
    };
    registry.principals.push(record.clone());
    write_registry(dir, &registry).map_err(|e| RegistrationError::Io(e.to_string()))?;
    Ok(record)
}

/// Return only ceremonies addressed to the verified human. Every staged file is validated as
/// a whole before filtering; malformed or changed provisioning makes the private view fail
/// rather than presenting a card whose credential no longer has the stated fingerprint.
pub fn pending_for(
    dir: &Path,
    actor: &HumanDecisionContext,
) -> io::Result<Vec<PendingRegistrationView>> {
    let pending = load_pending(dir)?;
    if pending
        .iter()
        .any(|pending| !pending_secret_is_current(dir, &pending.record))
    {
        return Err(invalid_pending(
            "a pending registration credential is missing or changed",
        ));
    }
    let registered: HashSet<String> = load(dir)?
        .principals
        .into_iter()
        .map(|record| record.credential_fingerprint)
        .collect();
    Ok(pending
        .into_iter()
        .filter(|pending| {
            pending.record.addressed_to == actor.human()
                && !registered.contains(&pending.record.credential_fingerprint)
        })
        .map(|pending| PendingRegistrationView {
            registration_id: pending.record.id,
            partner_alias: pending.record.alias,
            credential_fingerprint: pending.record.credential_fingerprint,
            created_at: pending.record.created_at,
        })
        .collect())
}

/// Bind one pre-provisioned credential to the human derived by the signed console door.
/// The wire carries only the random staging id: alias, credential reference, fingerprint, and
/// addressee are re-read from the serving node after signature/freshness/standing checks.
pub fn register_staged(
    dir: &Path,
    actor: &HumanDecisionContext,
    registration_id: &str,
) -> Result<PrincipalRecord, RegistrationError> {
    let registration_id = registration_id.trim();
    if !valid_registration_id(registration_id) {
        return Err(RegistrationError::UnknownRegistration);
    }
    let _guard = registry_write_lock();
    let mut pending = load_pending(dir).map_err(|e| RegistrationError::Io(e.to_string()))?;
    let Some(staged) = pending
        .drain(..)
        .find(|pending| pending.record.id == registration_id)
    else {
        return Err(RegistrationError::UnknownRegistration);
    };
    if staged.record.addressed_to != actor.human() {
        return Err(RegistrationError::WrongAddressee);
    }

    let registry = load(dir).map_err(|e| RegistrationError::Io(e.to_string()))?;
    if let Some(existing) = registry
        .principals
        .iter()
        .find(|record| record.credential_fingerprint == staged.record.credential_fingerprint)
    {
        if existing.registered_by == actor.human() && existing.alias == staged.record.alias {
            let out = existing.clone();
            let _ = std::fs::remove_file(&staged.path);
            return Ok(out);
        }
        return Err(RegistrationError::DuplicateCredential);
    }

    if !pending_secret_is_current(dir, &staged.record) {
        return Err(RegistrationError::CredentialChanged);
    }
    let registered = register_unlocked(
        dir,
        actor,
        &staged.record.alias,
        &staged.record.credential_file,
        &staged.record.credential_key,
    )?;
    // The registry write is the authority transition and is already durable. A failed cleanup
    // cannot roll it back; projections filter registered fingerprints, and a retry is idempotent.
    let _ = std::fs::remove_file(staged.path);
    Ok(registered)
}

/// Return the rung-3 addressee for an enabled principal. Legacy records have no addressee and
/// therefore fail closed. Registry corruption is also an absence, never guessed authority.
pub fn registered_by(dir: &Path, principal: &str) -> Option<String> {
    load(dir)
        .ok()?
        .principals
        .into_iter()
        .find(|record| record.enabled && record.id == principal)
        .and_then(|record| {
            let human = record.registered_by.trim();
            (!human.is_empty() && !human.chars().any(char::is_control)).then(|| human.to_string())
        })
}

pub fn is_registered_for(dir: &Path, principal: &str, actor: &HumanDecisionContext) -> bool {
    registered_by(dir, principal).as_deref() == Some(actor.human())
}

/// Explicit human credential rotation. Keeping the principal id is the deliberate binding act
/// required by the rung-3 contract; merely changing an env file makes authentication fail.
pub fn bind_credential(
    dir: &Path,
    principal: &str,
    credential_file: &str,
    credential_key: &str,
) -> Result<PrincipalRecord, RegistrationError> {
    let _guard = registry_write_lock();
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

fn registry_write_lock() -> std::sync::MutexGuard<'static, ()> {
    static WRITES: OnceLock<Mutex<()>> = OnceLock::new();
    WRITES
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn load_pending(dir: &Path) -> io::Result<Vec<LoadedPendingRegistration>> {
    let pending_dir = dir.join(PENDING_REGISTRATIONS_DIR);
    let entries = match std::fs::read_dir(&pending_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.path().extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        if !entry.file_type()?.is_file() {
            return Err(invalid_pending(
                "a pending registration is not a regular file",
            ));
        }
        paths.push(entry.path());
    }
    paths.sort();
    if paths.len() > MAX_PENDING_REGISTRATIONS {
        return Err(invalid_pending("too many pending partner registrations"));
    }

    let mut ids = HashSet::new();
    let mut fingerprints = HashSet::new();
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = std::fs::read(&path)?;
        let record: PendingRegistration = serde_json::from_slice(&bytes)
            .map_err(|_| invalid_pending("a pending registration is malformed"))?;
        validate_pending(&record)?;
        if !ids.insert(record.id.clone()) {
            return Err(invalid_pending("duplicate pending registration id"));
        }
        if !fingerprints.insert(record.credential_fingerprint.clone()) {
            return Err(invalid_pending("duplicate pending registration credential"));
        }
        out.push(LoadedPendingRegistration { record, path });
    }
    Ok(out)
}

fn validate_pending(record: &PendingRegistration) -> io::Result<()> {
    if record.version != 1
        || !valid_registration_id(&record.id)
        || record.created_at <= 0
        || record.addressed_to.trim().is_empty()
        || record.addressed_to.len() > MAX_ALIAS_BYTES
        || record.addressed_to.chars().any(char::is_control)
        || record.credential_fingerprint.len() != 64
        || !record
            .credential_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid_pending("a pending registration is invalid"));
    }
    validate_alias(&record.alias)
        .and_then(|_| validate_reference(&record.credential_file, &record.credential_key))
        .map_err(|_| invalid_pending("a pending registration is invalid"))
}

fn valid_registration_id(id: &str) -> bool {
    id.starts_with("registration-")
        && id.len() > "registration-".len()
        && id.len() <= 80
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn pending_secret_is_current(dir: &Path, pending: &PendingRegistration) -> bool {
    read_secret(dir, &pending.credential_file, &pending.credential_key)
        .is_some_and(|secret| credential_fingerprint(&secret) == pending.credential_fingerprint)
}

fn invalid_pending(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
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

    fn human(name: &str) -> HumanDecisionContext {
        HumanDecisionContext::from_verified_mesh("device-test".into(), name.into()).unwrap()
    }

    fn stage(dir: &Path, id: &str, addressed_to: &str, secret: &str) {
        let credential_file = format!("mcp/{id}.env");
        credential(dir, &credential_file, "TOKEN", secret);
        let pending_dir = dir.join(PENDING_REGISTRATIONS_DIR);
        std::fs::create_dir_all(&pending_dir).unwrap();
        std::fs::write(
            pending_dir.join(format!("{id}.json")),
            serde_json::to_vec_pretty(&serde_json::json!({
                "version": 1,
                "id": id,
                "alias": "Envoy (on-device)",
                "credential_file": credential_file,
                "credential_key": "TOKEN",
                "credential_fingerprint": credential_fingerprint(secret),
                "addressed_to": addressed_to,
                "created_at": 1_780_000_000_i64
            }))
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn a_human_registration_binds_alias_and_secret_without_storing_the_secret() {
        let dir = temp("register");
        credential(&dir, "mcp/a.env", "TOKEN", "very-secret-a");
        let record = register(&dir, &human("ian"), "Workshop agent", "mcp/a.env", "TOKEN").unwrap();
        assert!(record.id.starts_with("principal-"));
        let raw = std::fs::read_to_string(dir.join(PRINCIPALS_FILE)).unwrap();
        assert!(!raw.contains("very-secret-a"));
        let context = authenticate(&dir, "very-secret-a").unwrap();
        assert_eq!(context.principal, record.id);
        assert_eq!(context.alias, "Workshop agent");
        assert_eq!(registered_by(&dir, &record.id).as_deref(), Some("ian"));
        assert!(authenticate(&dir, "wrong").is_none());
    }

    #[test]
    fn caller_labels_cannot_select_a_principal_and_credentials_do_not_alias() {
        let dir = temp("distinct");
        credential(&dir, "mcp/a.env", "TOKEN", "secret-a");
        credential(&dir, "mcp/b.env", "TOKEN", "secret-b");
        let a = register(&dir, &human("ian"), "same label", "mcp/a.env", "TOKEN").unwrap();
        let b = register(&dir, &human("ian"), "same label", "mcp/b.env", "TOKEN").unwrap();
        assert_ne!(a.id, b.id);
        assert_ne!(
            authenticate(&dir, "secret-a").unwrap().principal,
            authenticate(&dir, "secret-b").unwrap().principal
        );
    }

    #[test]
    fn staged_registration_projects_no_secret_and_binds_the_verified_human() {
        let dir = temp("staged");
        stage(&dir, "registration-0123456789abcdef", "ian", "envoy-secret");
        assert!(pending_for(&dir, &human("betty")).unwrap().is_empty());
        let view = pending_for(&dir, &human("ian")).unwrap();
        assert_eq!(view.len(), 1);
        let raw = serde_json::to_string(&view).unwrap();
        assert!(!raw.contains("envoy-secret"));
        assert!(!raw.contains("credential_file"));
        assert!(!raw.contains("credential_key"));

        let record = register_staged(&dir, &human("ian"), "registration-0123456789abcdef").unwrap();
        assert_eq!(record.registered_by, "ian");
        assert_eq!(record.alias, "Envoy (on-device)");
        assert_eq!(
            authenticate(&dir, "envoy-secret").unwrap().principal,
            record.id
        );
        assert!(pending_for(&dir, &human("ian")).unwrap().is_empty());
    }

    #[test]
    fn staged_registration_rechecks_addressee_and_credential_at_the_act() {
        let dir = temp("staged_refusal");
        let id = "registration-fedcba9876543210";
        stage(&dir, id, "ian", "original-secret");
        assert_eq!(
            register_staged(&dir, &human("betty"), id),
            Err(RegistrationError::WrongAddressee)
        );
        credential(&dir, &format!("mcp/{id}.env"), "TOKEN", "changed-secret");
        assert_eq!(
            register_staged(&dir, &human("ian"), id),
            Err(RegistrationError::CredentialChanged)
        );
        assert!(load(&dir).unwrap().principals.is_empty());
        assert!(pending_for(&dir, &human("ian")).is_err());
    }

    #[test]
    fn changing_secret_bytes_is_not_implicit_rotation_but_binding_is() {
        let dir = temp("rotate");
        credential(&dir, "mcp/a.env", "TOKEN", "old");
        let a = register(&dir, &human("ian"), "A", "mcp/a.env", "TOKEN").unwrap();
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

    #[test]
    fn legacy_principal_has_no_rung_three_addressee() {
        let dir = temp("legacy");
        std::fs::write(
            dir.join(PRINCIPALS_FILE),
            r#"{"principals":[{"id":"old","alias":"Old","credential_file":"mcp/a.env","credential_key":"TOKEN","credential_fingerprint":"fp","enabled":true}]}"#,
        )
        .unwrap();
        assert_eq!(registered_by(&dir, "old"), None);
        assert!(!is_registered_for(&dir, "old", &human("ian")));
    }
}
