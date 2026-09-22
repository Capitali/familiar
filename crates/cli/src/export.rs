//! `familiar export` — the household's whole record in open files, with a manifest.
//!
//! The charter's C4 says exit is a right. At household scale the files are already open
//! and local (JSON, JSONL, one SQLite database); this command makes leaving a single act:
//! copy them as they are into one directory, beside a `MANIFEST.json` that names every
//! file with its size and SHA-256, and a `README.md` that says how to read them and how
//! to move them into another familiar.
//!
//! Secrets never travel. Anything that would let the holder of the export act *as* this
//! familiar — the mesh node key, the group secret, TLS material, push tokens — is withheld
//! and named in the manifest with the reason, so the reader can see what is missing rather
//! than wonder.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use familiar_kernel::store;
use serde_json::json;

/// One copied file as the manifest lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileEntry {
    /// Path relative to the export directory (`data/…`).
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

/// One file deliberately not copied, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Withheld {
    /// Path relative to the export directory — where it *would* have landed.
    pub path: String,
    pub reason: String,
}

/// What an export produced.
#[derive(Debug)]
pub(crate) struct Report {
    pub dir: PathBuf,
    pub files: Vec<FileEntry>,
    pub withheld: Vec<Withheld>,
    pub bytes: u64,
    pub service_id: Option<String>,
}

pub(crate) fn cmd_export(args: &[String]) -> ExitCode {
    let f = crate::flags(args);
    let dir = store::data_dir(f.get("data-dir").map(String::as_str));
    let out = f
        .get("out")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let now = unix_now();
    match run_export(&dir, &out, now) {
        Ok(r) => {
            println!(
                "exported {} files ({} bytes), {} withheld, service {} → {}",
                r.files.len(),
                r.bytes,
                r.withheld.len(),
                r.service_id.as_deref().unwrap_or("(no mesh node)"),
                r.dir.display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("export: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Build `<out>/familiar-export-<YYYY-MM-DD>/` from the data dir. `now` is unix seconds
/// and names the directory and the manifest's `exported_at`.
pub(crate) fn run_export(data_dir: &Path, out: &Path, now: u64) -> Result<Report, String> {
    if !data_dir.is_dir() {
        return Err(format!("data dir {} does not exist", data_dir.display()));
    }
    let export = out.join(format!("familiar-export-{}", ymd(now)));
    std::fs::create_dir_all(&export)
        .map_err(|e| format!("could not create {}: {e}", export.display()))?;
    // The export must never fold itself in (an `--out` inside the data dir).
    let export_abs = std::fs::canonicalize(&export).unwrap_or_else(|_| export.clone());

    let mut w = Walk {
        export: export.clone(),
        export_abs,
        files: Vec::new(),
        withheld: Vec::new(),
        bytes: 0,
    };

    w.data_dir(data_dir)?;

    w.files.sort_by(|a, b| a.path.cmp(&b.path));
    w.withheld.sort_by(|a, b| a.path.cmp(&b.path));

    let service_id = service_id(data_dir);
    let manifest = json!({
        "exported_at": now,
        "service_id": service_id,
        "files": w.files.iter().map(|f| json!({
            "path": f.path, "bytes": f.bytes, "sha256": f.sha256,
        })).collect::<Vec<_>>(),
        "withheld": w.withheld.iter().map(|x| json!({
            "path": x.path, "reason": x.reason,
        })).collect::<Vec<_>>(),
    });
    let manifest_text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(export.join("MANIFEST.json"), manifest_text)
        .map_err(|e| format!("MANIFEST.json: {e}"))?;
    std::fs::write(export.join("README.md"), readme(&w.withheld))
        .map_err(|e| format!("README.md: {e}"))?;

    Ok(Report {
        dir: export,
        files: w.files,
        withheld: w.withheld,
        bytes: w.bytes,
        service_id,
    })
}

struct Walk {
    export: PathBuf,
    export_abs: PathBuf,
    files: Vec<FileEntry>,
    withheld: Vec<Withheld>,
    bytes: u64,
}

impl Walk {
    /// The data dir is NOT copied whole: only the record — boundary, the database, every
    /// top-level JSON/JSONL, the mesh's public identity and records. Logs, pidfiles,
    /// caches and helper directories stay home.
    fn data_dir(&mut self, dir: &Path) -> Result<(), String> {
        let rel_root = Path::new("data");
        let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = rel_root.join(&name);
            if path.is_dir() {
                if name == "mesh" {
                    self.tree(&path, &rel)?;
                }
                continue;
            }
            let wanted = name == "boundary.json"
                || name == "familiar.db"
                || name == "familiar.db-wal"
                || name == "familiar.db-shm"
                || name.ends_with(".json")
                || name.ends_with(".jsonl");
            if wanted || withhold_reason(&rel).is_some() {
                self.consider(&path, &rel)?;
            }
        }
        Ok(())
    }

    /// Copy a directory tree, applying the withholding rules to every file.
    fn tree(&mut self, src: &Path, rel: &Path) -> Result<(), String> {
        if self.is_export_itself(src) {
            return Ok(());
        }
        let entries = std::fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("{}: {e}", src.display()))?;
            let path = entry.path();
            let child = rel.join(entry.file_name());
            if path.is_dir() {
                self.tree(&path, &child)?;
            } else if path.is_file() {
                self.consider(&path, &child)?;
            }
        }
        Ok(())
    }

    fn is_export_itself(&self, p: &Path) -> bool {
        std::fs::canonicalize(p)
            .map(|c| c == self.export_abs)
            .unwrap_or(false)
    }

    /// Withhold or copy one file, recording it either way.
    fn consider(&mut self, src: &Path, rel: &Path) -> Result<(), String> {
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if let Some(reason) = withhold_reason(rel) {
            self.withheld.push(Withheld {
                path: rel_str,
                reason: reason.to_string(),
            });
            return Ok(());
        }
        let bytes = std::fs::read(src).map_err(|e| format!("{}: {e}", src.display()))?;
        let dest = self.export.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
        }
        std::fs::write(&dest, &bytes).map_err(|e| format!("{}: {e}", dest.display()))?;
        self.bytes += bytes.len() as u64;
        self.files.push(FileEntry {
            path: rel_str,
            bytes: bytes.len() as u64,
            sha256: familiar_mesh::sha256_hex(&bytes),
        });
        Ok(())
    }
}

/// Why a file must not leave, or `None` if it may. `rel` is the path the file would have
/// in the export (`data/mesh/node_key`, `data/provider.token`, …). The rules are by name
/// and by place; they are deliberately blunt — a false withhold costs one file, a false
/// copy costs a key.
pub(crate) fn withhold_reason(rel: &Path) -> Option<&'static str> {
    let name = rel
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    if name == "daemon.log" {
        return Some("runtime log, not part of the record");
    }
    let comps: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if let Some(i) = comps.iter().position(|c| c == "mesh") {
        let after = &comps[i + 1..];
        if name == "node_key" && after.len() == 1 {
            return Some(
                "mesh node secret key — the familiar's identity; only node.json (public) travels",
            );
        }
        let public = after == ["node.json"] || after.first().map(String::as_str) == Some("records");
        if !public {
            return Some("mesh state other than the public identity and records (group secret, TLS key, peers, push tokens)");
        }
    }
    if lower.contains("secret") || lower.contains("token") || lower.contains(".key") {
        return Some("file name marks it as a secret");
    }
    None
}

/// The mesh node id from `<data dir>/mesh/node.json`, if the familiar has one.
fn service_id(data_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(data_dir.join("mesh").join("node.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("node_id")?.as_str().map(str::to_string)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `YYYY-MM-DD` (UTC) from unix seconds — the civil-from-days algorithm, so no date crate.
pub(crate) fn ymd(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn readme(withheld: &[Withheld]) -> String {
    let mut reasons: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for w in withheld {
        reasons.entry(w.reason.as_str()).or_default().push(&w.path);
    }
    let mut s = String::new();
    s.push_str("# A familiar's export\n\n");
    s.push_str(
        "This directory is one household's whole record, copied out of a running familiar as the\n",
    );
    s.push_str("plain files it already keeps. Nothing here is encoded in a private format: every file is\n");
    s.push_str("JSON, JSONL (one JSON object per line) or, for `familiar.db`, a SQLite database that any\n");
    s.push_str("`sqlite3` can open. `familiar db export` on the original will also dump every table to JSONL.\n\n");
    s.push_str("## What is here\n\n");
    s.push_str("- `data/` — the familiar's data directory: `boundary.json` (the Pact, the human's lever),\n");
    s.push_str("  `familiar.db` (observations, threads, beliefs, predictions, answers — the mind's record),\n");
    s.push_str(
        "  every top-level `.json`/`.jsonl` (persona, parameters, devices, `ledger.jsonl` if the\n",
    );
    s.push_str(
        "  refusal ledger exists), and `mesh/node.json` (the node's PUBLIC identity only) with\n",
    );
    s.push_str("  `mesh/records/` (signed mesh records).\n");
    s.push_str(
        "- `MANIFEST.json` — `exported_at` (unix seconds), `service_id` (mesh node id or null),\n",
    );
    s.push_str("  `files` (path, bytes, sha256 of every copied file) and `withheld` (path and reason for\n");
    s.push_str("  every file deliberately left behind).\n\n");
    s.push_str("## Verifying\n\n");
    s.push_str(
        "Every hash in the manifest is a plain SHA-256 of the file's bytes. To check one file:\n\n",
    );
    s.push_str("    shasum -a 256 data/boundary.json\n\n");
    s.push_str("To check all of them at once (needs `jq`):\n\n");
    s.push_str(
        "    jq -r '.files[] | \"\\(.sha256)  \\(.path)\"' MANIFEST.json | shasum -a 256 -c\n\n",
    );
    s.push_str("## What was withheld, and why\n\n");
    s.push_str("Secrets never travel in an export: anything that would let the holder act AS this familiar\n");
    s.push_str("(speak as its mesh node, hold its group secret, spend against a provider key) is left behind\n");
    s.push_str(
        "and named in the manifest. A new home mints its own keys. Withheld in this export:\n\n",
    );
    if reasons.is_empty() {
        s.push_str("- nothing — no secret-bearing files were present.\n");
    } else {
        for (reason, paths) in &reasons {
            s.push_str(&format!("- {reason}: {}\n", paths.join(", ")));
        }
    }
    s.push_str("\n## Moving into another familiar\n\n");
    s.push_str("Start the new familiar with `--data-dir <this dir>/data`; it reads the record exactly as\n");
    s.push_str("the original did. Then `familiar mesh create-group` (or `mesh join`) for a fresh node key,\n");
    s.push_str(
        "and your own provider keys. The record — every observation, thread, belief and name —\n",
    );
    s.push_str("carries over as it was.\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path, s: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    }

    #[test]
    fn export_copies_the_record_and_withholds_secrets() {
        let base =
            std::env::temp_dir().join(format!("familiar-export-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let data = base.join("data");

        let boundary = r#"{"allow_llm":false}"#;
        let obs =
            "{\"actor\":\"ian\",\"action\":\"said\"}\n{\"actor\":\"ian\",\"action\":\"left\"}\n";
        let node = r#"{"node_id":"node-abc123","label":"kitchen-mac"}"#;
        let ledger = "{\"t\":1,\"refused\":\"network\"}\n";

        write(&data.join("boundary.json"), boundary);
        write(&data.join("observations.jsonl"), obs);
        write(&data.join("ledger.jsonl"), ledger);
        write(&data.join("mesh").join("node.json"), node);
        write(&data.join("mesh").join("node_key"), "SECRET-NODE-KEY");
        write(&data.join("provider.token"), "SECRET-TOKEN");
        write(&data.join("daemon.log"), "noise\n");

        let out = base.join("out");
        let now = 1_789_000_000; // 2026-09-10 UTC
        let report = run_export(&data, &out, now).expect("export runs");

        assert_eq!(report.dir, out.join("familiar-export-2026-09-10"));
        assert!(report.dir.is_dir());
        assert_eq!(report.service_id.as_deref(), Some("node-abc123"));

        // The manifest lists exactly the copied files, with the right hashes.
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(report.dir.join("MANIFEST.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["exported_at"], json!(now));
        assert_eq!(manifest["service_id"], json!("node-abc123"));
        let mut listed: Vec<(String, String)> = manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                (
                    f["path"].as_str().unwrap().to_string(),
                    f["sha256"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        listed.sort();
        let mut expected: Vec<(String, String)> = vec![
            ("data/boundary.json", boundary),
            ("data/ledger.jsonl", ledger),
            ("data/mesh/node.json", node),
            ("data/observations.jsonl", obs),
        ]
        .into_iter()
        .map(|(p, body)| (p.to_string(), familiar_mesh::sha256_hex(body.as_bytes())))
        .collect();
        expected.sort();
        assert_eq!(listed, expected);
        for (p, _) in &expected {
            assert!(report.dir.join(p).is_file(), "{p} copied");
        }
        let total: u64 = expected
            .iter()
            .map(|(p, _)| std::fs::metadata(report.dir.join(p)).unwrap().len())
            .sum();
        assert_eq!(report.bytes, total);
        assert_eq!(report.files.len(), expected.len());

        // Secrets are named as withheld, with a reason, and are not on disk.
        let withheld: Vec<(String, String)> = manifest["withheld"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| {
                (
                    w["path"].as_str().unwrap().to_string(),
                    w["reason"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        for secret in [
            "data/mesh/node_key",
            "data/provider.token",
            "data/daemon.log",
        ] {
            let entry = withheld
                .iter()
                .find(|(p, _)| p == secret)
                .unwrap_or_else(|| panic!("{secret} listed as withheld"));
            assert!(!entry.1.is_empty(), "{secret} has a reason");
            assert!(!report.dir.join(secret).exists(), "{secret} not on disk");
        }
        assert_eq!(withheld.len(), 3);

        let readme = std::fs::read_to_string(report.dir.join("README.md")).unwrap();
        assert!(readme.contains("shasum -a 256"));
        assert!(readme.contains("provider.token"));
        assert!(readme.contains("--data-dir"));

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn withholding_rules_are_by_name_and_by_place() {
        let held = |p: &str| withhold_reason(Path::new(p)).is_some();
        assert!(held("data/mesh/node_key"));
        assert!(held("data/mesh/group.json"));
        assert!(held("data/mesh/tls_key.der"));
        assert!(held("data/mesh/push_tokens.json"));
        assert!(held("data/mesh/scope_salt"));
        assert!(held("data/daemon.log"));
        assert!(held("data/api.key"));
        assert!(held("data/client_secret.json"));
        assert!(held("data/provider.token"));
        assert!(!held("data/mesh/node.json"));
        assert!(!held("data/mesh/records/2026/r1.json"));
        assert!(!held("data/boundary.json"));
        assert!(!held("data/ledger.jsonl"));
        assert!(!held("data/persona.json"));
    }

    #[test]
    fn ymd_formats_utc_dates() {
        assert_eq!(ymd(0), "1970-01-01");
        assert_eq!(ymd(951_782_400), "2000-02-29");
        assert_eq!(ymd(1_789_000_000), "2026-09-10");
        assert_eq!(ymd(1_704_067_199), "2023-12-31");
    }
}
