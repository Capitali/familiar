//! The refusal ledger — Layer 4 of The Service Charter.
//!
//! Every consequential refusal, escalation or execution the familiar makes is one structured
//! record, appended to `ledger.jsonl` in the data dir. The file is append-only and
//! tamper-evident: each record's hash covers its own fields and the previous record's hash,
//! so a record cannot be altered, removed or inserted without every later hash changing.
//! `verify` walks the chain from genesis and names the first broken link.
//!
//! **Redaction, not opacity.** Three fields are free text about a person's command —
//! `command_summary`, `affected_population`, `reasoning`. The hash does not commit to their
//! plain text; it commits to the SHA-256 of each. So a redacted copy, in which each of those
//! fields has been replaced by its own hash, verifies against exactly the same chain. The
//! public can see that something was refused, when, under which trigger, and that the record
//! is intact — without seeing whose command it was. That is the charter's answer to "is a
//! self-governance layer just self-certification": the evidence is left where an auditor
//! would find it, checkable without trusting the system that wrote it.
//!
//! **What this is not.** It is not the corruption ledger (`corruption.rs`), which marks an
//! ACTOR who keeps pushing the familiar past its boundary and has no expunge mechanism. This
//! ledger marks the familiar's own decisions, never a person's standing. The owner's 2026-08-17
//! ruling — no reputational mark on a human for a machine's bad sentence — is untouched: a
//! record here says "the familiar refused", and who asked is behind a hash.
//!
//! This file lives BESIDE the SQLite store rather than in it, on purpose: a chain the public
//! is invited to verify should be a file anyone can read with `cat`, copy with `cp`, and
//! hash with `shasum`, and `familiar export` carries it verbatim.

use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The ledger, in the data dir. Plain JSON lines; never rewritten.
pub const LEDGER_FILE: &str = "ledger.jsonl";
/// The `prev_hash` of the first record.
pub const GENESIS: &str = "genesis";
/// The lock taken while a writer reads the tail and appends, so two writers cannot both
/// chain onto the same predecessor.
const LOCK_FILE: &str = "ledger.lock";

/// What the record is about.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Refusal,
    Execution,
    Escalation,
}

/// One line of the ledger. Field names are the charter's, verbatim.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// RFC 3339, UTC.
    pub timestamp: String,
    /// The instance's stable public id (the mesh node id), or `unknown`.
    pub service_id: String,
    pub action_type: ActionType,
    /// `C1`..`C5`, `RT-1`..`RT-5`, or `none`.
    pub trigger: String,
    /// Redactable.
    pub command_summary: String,
    /// Redactable.
    pub affected_population: String,
    /// Redactable.
    pub reasoning: String,
    pub human_oversight_notified: bool,
    /// True when the three redactable fields already hold their hashes rather than their
    /// text. A redacted record verifies against the same chain as the original.
    #[serde(default)]
    pub redacted: bool,
    /// The previous record's `hash`, or [`GENESIS`].
    pub prev_hash: String,
    /// SHA-256 over the canonical commitment (see [`commitment`]).
    pub hash: String,
}

/// What a caller supplies; the ledger adds the time, the chain and the hash.
#[derive(Clone, Debug)]
pub struct Draft {
    pub service_id: String,
    pub action_type: ActionType,
    pub trigger: String,
    pub command_summary: String,
    pub affected_population: String,
    pub reasoning: String,
    pub human_oversight_notified: bool,
}

/// Why a chain failed to verify: the index of the first bad record and what was wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Broken {
    pub index: usize,
    pub why: String,
}

impl std::fmt::Display for Broken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "record {}: {}", self.index, self.why)
    }
}

/// Lowercase hex SHA-256.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Unix seconds as RFC 3339 UTC (`2026-09-18T07:25:26Z`). No calendar crate: the
/// civil-from-days arithmetic is short and the workspace keeps time as `i64` seconds.
pub fn rfc3339(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60
    )
}

/// The bytes the hash commits to: the record as canonical JSON (keys sorted, no spaces)
/// with `hash` removed and each redactable field replaced by the SHA-256 of its text. A
/// record that is already redacted carries those hashes in place of the text, so it commits
/// to exactly the same bytes as its original.
pub fn commitment(rec: &Record) -> String {
    let mut v = serde_json::to_value(rec).unwrap_or(Value::Null);
    if let Value::Object(map) = &mut v {
        map.remove("hash");
        map.remove("redacted");
        if !rec.redacted {
            for k in ["command_summary", "affected_population", "reasoning"] {
                if let Some(Value::String(s)) = map.get(k) {
                    let h = sha256_hex(s.as_bytes());
                    map.insert(k.to_string(), Value::String(h));
                }
            }
        }
    }
    // serde_json's Map is a BTreeMap here (no `preserve_order`), so keys come out sorted;
    // `to_string` is the compact form. That is the canonical JSON the charter names.
    serde_json::to_string(&v).unwrap_or_default()
}

/// The hash a record should carry.
pub fn hash_of(rec: &Record) -> String {
    sha256_hex(commitment(rec).as_bytes())
}

/// The redacted copy: the three free-text fields replaced by their hashes, `redacted`
/// set, everything else — chain included — untouched.
pub fn redacted(rec: &Record) -> Record {
    if rec.redacted {
        return rec.clone();
    }
    Record {
        command_summary: sha256_hex(rec.command_summary.as_bytes()),
        affected_population: sha256_hex(rec.affected_population.as_bytes()),
        reasoning: sha256_hex(rec.reasoning.as_bytes()),
        redacted: true,
        ..rec.clone()
    }
}

/// The instance's public id, read off `mesh/node.json` if the mesh has minted one. The
/// kernel cannot depend on the mesh crate (the mesh depends on the kernel), so this reads
/// the public identity file directly and never the key beside it.
pub fn service_id(dir: &Path) -> String {
    fs::read_to_string(dir.join("mesh").join("node.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("node_id").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Every record, oldest first. A missing ledger is an empty one. A line that is not a
/// record is an error, not skipped: a chain with a hole in it must not verify.
pub fn load(dir: &Path) -> io::Result<Vec<Record>> {
    let path = dir.join(LEDGER_FILE);
    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for (i, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let rec: Record = serde_json::from_str(&line).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{LEDGER_FILE} line {}: {e}", i + 1),
            )
        })?;
        out.push(rec);
    }
    Ok(out)
}

/// Walk the chain from genesis. `Ok(n)` is the number of records verified; `Err` names the
/// first record that does not hold and why.
pub fn verify(records: &[Record]) -> Result<usize, Broken> {
    let mut prev = GENESIS.to_string();
    for (i, rec) in records.iter().enumerate() {
        if rec.prev_hash != prev {
            return Err(Broken {
                index: i,
                why: format!(
                    "prev_hash {} does not match the previous record's hash {}",
                    short(&rec.prev_hash),
                    short(&prev)
                ),
            });
        }
        let expected = hash_of(rec);
        if rec.hash != expected {
            return Err(Broken {
                index: i,
                why: format!(
                    "hash {} does not match its contents (expected {})",
                    short(&rec.hash),
                    short(&expected)
                ),
            });
        }
        prev = rec.hash.clone();
    }
    Ok(records.len())
}

fn short(h: &str) -> String {
    h.chars().take(12).collect()
}

/// Append one record, chained onto the ledger's tail. Takes the ledger lock so two writers
/// cannot both chain onto the same predecessor; if the lock is stale past a second the
/// write proceeds anyway — a refusal that goes unrecorded is the worse failure.
pub fn record(dir: &Path, now: i64, draft: Draft) -> io::Result<Record> {
    fs::create_dir_all(dir)?;
    let lock = dir.join(LOCK_FILE);
    let mut held = false;
    for _ in 0..50 {
        match OpenOptions::new().write(true).create_new(true).open(&lock) {
            Ok(_) => {
                held = true;
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(e),
        }
    }
    let result = append_locked(dir, now, draft);
    if held {
        let _ = fs::remove_file(&lock);
    }
    result
}

fn append_locked(dir: &Path, now: i64, draft: Draft) -> io::Result<Record> {
    let prev_hash = last_hash(dir)?;
    let mut rec = Record {
        timestamp: rfc3339(now),
        service_id: draft.service_id,
        action_type: draft.action_type,
        trigger: draft.trigger,
        command_summary: draft.command_summary,
        affected_population: draft.affected_population,
        reasoning: draft.reasoning,
        human_oversight_notified: draft.human_oversight_notified,
        redacted: false,
        prev_hash,
        hash: String::new(),
    };
    rec.hash = hash_of(&rec);
    let line = serde_json::to_string(&rec)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(LEDGER_FILE))?;
    writeln!(f, "{line}")?;
    f.sync_all()?;
    Ok(rec)
}

/// The tail's hash without parsing the whole file, or genesis.
fn last_hash(dir: &Path) -> io::Result<String> {
    let path = dir.join(LEDGER_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(GENESIS.to_string()),
        Err(e) => return Err(e),
    };
    let Some(last) = text.lines().rev().find(|l| !l.trim().is_empty()) else {
        return Ok(GENESIS.to_string());
    };
    let rec: Record = serde_json::from_str(last).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{LEDGER_FILE} tail is not a record: {e}"),
        )
    })?;
    Ok(rec.hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "familiar-ledger-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn draft(what: &str) -> Draft {
        Draft {
            service_id: "3d68a0689bc32771".into(),
            action_type: ActionType::Refusal,
            trigger: "C3".into(),
            command_summary: what.into(),
            affected_population: "the household".into(),
            reasoning: "a command is not authority".into(),
            human_oversight_notified: false,
        }
    }

    #[test]
    fn the_chain_verifies_and_names_the_first_broken_link() {
        let d = scratch("chain");
        let a = record(&d, 1_789_716_326, draft("open the door")).unwrap();
        let b = record(&d, 1_789_716_400, draft("delete the journal")).unwrap();
        let c = record(&d, 1_789_716_500, draft("mail the group secret")).unwrap();
        assert_eq!(a.prev_hash, GENESIS);
        assert_eq!(b.prev_hash, a.hash);
        assert_eq!(c.prev_hash, b.hash);
        assert_eq!(a.timestamp, "2026-09-18T07:25:26Z");

        let recs = load(&d).unwrap();
        assert_eq!(verify(&recs), Ok(3));

        // Alter the middle record's text: it and everything after it stop holding.
        let mut forged = recs.clone();
        forged[1].command_summary = "delete nothing".into();
        let err = verify(&forged).unwrap_err();
        assert_eq!(err.index, 1);
        assert!(err.why.contains("does not match its contents"), "{err}");

        // Remove a record: the chain breaks at the gap.
        let mut gapped = recs.clone();
        gapped.remove(1);
        let err = verify(&gapped).unwrap_err();
        assert_eq!(err.index, 1);
        assert!(err.why.contains("previous record"), "{err}");

        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_redacted_copy_hides_the_words_and_still_verifies() {
        let d = scratch("redact");
        record(&d, 1_789_716_326, draft("read her diary")).unwrap();
        record(&d, 1_789_716_400, draft("read his mail")).unwrap();
        let recs = load(&d).unwrap();
        let public: Vec<Record> = recs.iter().map(redacted).collect();

        assert!(public.iter().all(|r| r.redacted));
        assert!(public.iter().all(|r| !r.command_summary.contains("diary")));
        assert!(public.iter().all(|r| !r.reasoning.contains("authority")));
        assert_eq!(public[0].command_summary.len(), 64);
        // Same chain, same hashes.
        assert_eq!(public[1].hash, recs[1].hash);
        assert_eq!(verify(&public), Ok(2));
        // And redacting twice changes nothing.
        assert_eq!(redacted(&public[0]), public[0]);

        // The redacted copy round-trips through the file format and still verifies.
        let e = scratch("redact-copy");
        let mut f = fs::File::create(e.join(LEDGER_FILE)).unwrap();
        for r in &public {
            writeln!(f, "{}", serde_json::to_string(r).unwrap()).unwrap();
        }
        assert_eq!(verify(&load(&e).unwrap()), Ok(2));

        fs::remove_dir_all(&d).unwrap();
        fs::remove_dir_all(&e).unwrap();
    }

    #[test]
    fn the_service_id_is_the_public_node_id_and_never_the_key() {
        let d = scratch("sid");
        assert_eq!(service_id(&d), "unknown");
        fs::create_dir_all(d.join("mesh")).unwrap();
        fs::write(
            d.join("mesh").join("node.json"),
            r#"{"node_id":"f56e5601aabbccdd","pubkey":"00","label":"lighthouse"}"#,
        )
        .unwrap();
        fs::write(d.join("mesh").join("node_key"), "deadbeef").unwrap();
        assert_eq!(service_id(&d), "f56e5601aabbccdd");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn rfc3339_handles_the_edges() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_789_716_326), "2026-09-18T07:25:26Z");
    }

    #[test]
    fn a_hole_in_the_file_does_not_verify() {
        let d = scratch("hole");
        record(&d, 1, draft("x")).unwrap();
        let mut f = OpenOptions::new()
            .append(true)
            .open(d.join(LEDGER_FILE))
            .unwrap();
        writeln!(f, "not a record").unwrap();
        assert!(load(&d).is_err());
        fs::remove_dir_all(&d).unwrap();
    }
}
