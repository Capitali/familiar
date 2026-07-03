//! JSONL persistence: append-only logs, one file per record type.
//!
//! Local-first, auditable, rebuildable — the substrate inherited from v1, now
//! without a hand-rolled JSON parser. Observations remain the only truth (the
//! Soul's method discipline); derived views can always be thrown away and rebuilt.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Default data directory when no override is given.
pub const DEFAULT_DATA_DIR: &str = "familiar_data";

/// Resolve the data directory from an optional override.
pub fn data_dir(override_dir: Option<&str>) -> PathBuf {
    PathBuf::from(override_dir.unwrap_or(DEFAULT_DATA_DIR))
}

/// The per-user data directory of the installed app:
/// `~/Library/Application Support/Familiar/data`.
///
/// This is the fallback the **GUI apps** (the Glass, the marble) use when launched without
/// an explicit `--data-dir`. Finder- and launchd-launched apps run with the working
/// directory set to `/`, where the relative [`DEFAULT_DATA_DIR`] would resolve under the
/// read-only system volume and every write would fail with `EROFS`. This absolute path is
/// the same one the launchd agents pass explicitly, so all launch paths agree. Falls back to
/// the relative default only if `HOME` is unset (never, for a real GUI session).
pub fn user_data_dir() -> PathBuf {
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join("Library/Application Support/Familiar/data"))
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_DATA_DIR))
}

/// Append one record as a single JSONL line to `<dir>/<file>`, creating the
/// directory and file as needed.
pub fn append<T: Serialize>(dir: &Path, file: &str, record: &T) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let line = serde_json::to_string(record).map_err(invalid_data)?;
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(file))?;
    writeln!(f, "{line}")
}

/// Load all records from `<dir>/<file>`.
///
/// A missing file is an empty log (`Ok(vec![])`). Blank lines are skipped. A
/// malformed line is a hard error rather than a silent skip — corruption should
/// surface early, never quietly change the derived state.
pub fn load<T: DeserializeOwned>(dir: &Path, file: &str) -> io::Result<Vec<T>> {
    let path = dir.join(file);
    let f = match File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    for line in BufReader::new(f).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        out.push(serde_json::from_str(&line).map_err(invalid_data)?);
    }
    Ok(out)
}

/// Overwrite `<dir>/<file>` with exactly these records, one JSONL line each.
/// For *derived* sets (e.g. detected loops) that are a pure function of the
/// observations — persisting them is a rewrite, not an append, so the file never
/// grows without bound.
pub fn rewrite<T: Serialize>(dir: &Path, file: &str, records: &[T]) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let mut s = String::new();
    for r in records {
        s.push_str(&serde_json::to_string(r).map_err(invalid_data)?);
        s.push('\n');
    }
    fs::write(dir.join(file), s)
}

/// Load a single JSON object from `<dir>/<file>` (not JSONL — one object spanning the
/// whole file). Returns `None` if the file is missing, an error if it is malformed.
/// Used for human-owned policy files like the capability boundary.
pub fn load_one<T: DeserializeOwned>(dir: &Path, file: &str) -> io::Result<Option<T>> {
    match fs::read_to_string(dir.join(file)) {
        Ok(s) => Ok(Some(serde_json::from_str(&s).map_err(invalid_data)?)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn invalid_data(e: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    /// A throwaway temp dir, unique per call site, removed on drop.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!("substrate_store_test_{tag}"));
            let _ = fs::remove_dir_all(&p);
            TempDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Rec {
        id: u32,
        name: String,
    }

    #[test]
    fn missing_file_is_empty_log() {
        let d = TempDir::new("missing");
        let got: Vec<Rec> = load(d.path(), "none.jsonl").unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn append_then_load_roundtrips_in_order() {
        let d = TempDir::new("roundtrip");
        let a = Rec {
            id: 1,
            name: "alpha".into(),
        };
        let b = Rec {
            id: 2,
            name: "beta".into(),
        };
        append(d.path(), "recs.jsonl", &a).unwrap();
        append(d.path(), "recs.jsonl", &b).unwrap();
        let got: Vec<Rec> = load(d.path(), "recs.jsonl").unwrap();
        assert_eq!(got, vec![a, b]);
    }

    #[test]
    fn blank_lines_skipped_malformed_errors() {
        let d = TempDir::new("malformed");
        fs::create_dir_all(d.path()).unwrap();
        fs::write(
            d.path().join("x.jsonl"),
            "{\"id\":1,\"name\":\"a\"}\n\nnot json\n",
        )
        .unwrap();
        let got: io::Result<Vec<Rec>> = load(d.path(), "x.jsonl");
        assert!(got.is_err());
    }
}
