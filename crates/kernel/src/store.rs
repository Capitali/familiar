//! Persistence — an embedded **SQLite** store behind the historical JSONL API.
//!
//! Local-first, auditable, rebuildable — the substrate inherited from v1. It *was* one
//! append-only JSONL file per record type; that made every update a full-file rewrite
//! (`rewrite`), which is O(n) per change and O(n²) over a run — the candidate store once
//! ballooned to thousands of rows and ticks crawled. The records now live in a single SQLite
//! database (`<dir>/familiar.db`), one table per type, so an update is an indexed statement
//! and two processes (the daemon writing, the Glass reading) share the store safely under WAL.
//!
//! The public API is unchanged — `append` / `load` / `rewrite` keep their signatures, so every
//! caller is untouched — plus id-targeted [`load_by_id`] / [`update_by_id`] that make the
//! update paths O(log n) instead of load-all + rewrite-all. Human-owned config files
//! (`boundary.json`, `parameters.json`) are NOT in the database: they stay plain text the human
//! edits, read via [`load_one`] and written directly by their owners (Law III).
//!
//! **Transparency is preserved.** On first touch, an existing `<file>` is imported into its
//! table (then renamed `<file>.imported`), so nothing is lost on upgrade; and `familiar db
//! export` dumps any table back to JSONL, keeping the "cat-able, rebuildable truth" property.
//!
//! Observations remain the only truth; derived views can always be thrown away and rebuilt.

use rusqlite::Connection;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Default data directory when no override is given.
pub const DEFAULT_DATA_DIR: &str = "familiar_data";

/// The database file that holds every record table.
pub const DB_FILE: &str = "familiar.db";

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

// ---- connection cache -------------------------------------------------------------------

/// One cached connection per data dir (process-wide). rusqlite's `Connection` is `Send` but
/// not `Sync`, so it's wrapped in a `Mutex`; cross-process concurrency is handled by SQLite's
/// WAL + `busy_timeout`, not this lock.
fn conn(dir: &Path) -> io::Result<Arc<Mutex<Connection>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<Connection>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = dir.to_path_buf();
    {
        let map = cache.lock().unwrap();
        if let Some(c) = map.get(&key) {
            return Ok(c.clone());
        }
    }
    fs::create_dir_all(dir)?;
    let c = Connection::open(dir.join(DB_FILE)).map_err(se)?;
    // WAL: readers (Glass) don't block the writer (daemon) and vice-versa. NORMAL sync is
    // durable enough under WAL and much faster than FULL.
    c.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000; PRAGMA synchronous=NORMAL;",
    )
    .map_err(se)?;
    let arc = Arc::new(Mutex::new(c));
    cache.lock().unwrap().insert(key, arc.clone());
    Ok(arc)
}

/// The table name for a record file: the stem, sanitised to `[A-Za-z0-9_]` so it is safe to
/// interpolate into SQL (table names can't be bound parameters). `"candidates.jsonl"` →
/// `candidates`.
fn table_of(file: &str) -> String {
    let stem = file.strip_suffix(".jsonl").unwrap_or(file);
    stem.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Ensure the table exists (with a `$.id` index) and, on first touch only, import a legacy
/// `<dir>/<file>` JSONL log into it — then rename the file `<file>.imported`. Import is
/// transactional and validates every line, so a malformed legacy file surfaces as an error
/// and leaves the file in place (nothing half-migrated).
fn ensure(c: &Connection, table: &str, dir: &Path, file: &str) -> io::Result<()> {
    c.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {table} (seq INTEGER PRIMARY KEY AUTOINCREMENT, data TEXT NOT NULL);
         CREATE INDEX IF NOT EXISTS {table}_id ON {table}(json_extract(data,'$.id'));"
    ))
    .map_err(se)?;

    let legacy = dir.join(file);
    if !legacy.exists() {
        return Ok(());
    }
    // Only import into an empty table (so a failed import is retried, never doubled).
    let count: i64 = c
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .map_err(se)?;
    if count != 0 {
        return Ok(());
    }
    let content = fs::read_to_string(&legacy)?;
    let tx = c.unchecked_transaction().map_err(se)?;
    {
        let mut ins = tx
            .prepare(&format!("INSERT INTO {table}(data) VALUES(?1)"))
            .map_err(se)?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // Validate — a malformed legacy line is corruption we surface, not silently import.
            serde_json::from_str::<serde_json::Value>(line).map_err(invalid_data)?;
            ins.execute([line]).map_err(se)?;
        }
    }
    tx.commit().map_err(se)?;
    let _ = fs::rename(&legacy, dir.join(format!("{file}.imported")));
    Ok(())
}

// ---- the log API (unchanged signatures) -------------------------------------------------

/// Append one record to `<file>`'s table (an `INSERT`, ordered by insertion).
pub fn append<T: Serialize>(dir: &Path, file: &str, record: &T) -> io::Result<()> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)?;
    let json = serde_json::to_string(record).map_err(invalid_data)?;
    c.execute(&format!("INSERT INTO {table}(data) VALUES(?1)"), [json])
        .map_err(se)?;
    Ok(())
}

/// Load all records from `<file>`'s table, oldest first. A missing table is an empty log.
/// A row that fails to deserialize is a hard error — corruption surfaces, never silently.
pub fn load<T: DeserializeOwned>(dir: &Path, file: &str) -> io::Result<Vec<T>> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)?;
    let mut stmt = c
        .prepare(&format!("SELECT data FROM {table} ORDER BY seq"))
        .map_err(se)?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(se)?;
    let mut out = Vec::new();
    for row in rows {
        let data = row.map_err(se)?;
        out.push(serde_json::from_str(&data).map_err(invalid_data)?);
    }
    Ok(out)
}

/// Replace `<file>`'s table with exactly these records (a transactional
/// `DELETE` + re-`INSERT`). For genuine *bulk* sets (e.g. detected loops); id-targeted
/// updates should use [`update_by_id`] instead so they don't touch every row.
pub fn rewrite<T: Serialize>(dir: &Path, file: &str, records: &[T]) -> io::Result<()> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)?;
    let tx = c.unchecked_transaction().map_err(se)?;
    tx.execute(&format!("DELETE FROM {table}"), []).map_err(se)?;
    {
        let mut ins = tx
            .prepare(&format!("INSERT INTO {table}(data) VALUES(?1)"))
            .map_err(se)?;
        for r in records {
            let json = serde_json::to_string(r).map_err(invalid_data)?;
            ins.execute([json]).map_err(se)?;
        }
    }
    tx.commit().map_err(se)?;
    Ok(())
}

/// Load the single record whose JSON `id` field equals `id`, if any — an indexed lookup, not
/// a full scan. The record type must serialize an `"id"` field.
pub fn load_by_id<T: DeserializeOwned>(
    dir: &Path,
    file: &str,
    id: &str,
) -> io::Result<Option<T>> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)?;
    let data: Option<String> = c
        .query_row(
            &format!("SELECT data FROM {table} WHERE json_extract(data,'$.id')=?1 LIMIT 1"),
            [id],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(se(other)),
        })?;
    match data {
        Some(s) => Ok(Some(serde_json::from_str(&s).map_err(invalid_data)?)),
        None => Ok(None),
    }
}

/// Replace the record whose JSON `id` field equals `id` with `record` — a single indexed
/// `UPDATE` (O(log n)), the O(1)-ish path that replaces load-all + rewrite-all. Returns
/// whether a row matched.
pub fn update_by_id<T: Serialize>(
    dir: &Path,
    file: &str,
    id: &str,
    record: &T,
) -> io::Result<bool> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)?;
    let json = serde_json::to_string(record).map_err(invalid_data)?;
    let n = c
        .execute(
            &format!("UPDATE {table} SET data=?1 WHERE json_extract(data,'$.id')=?2"),
            rusqlite::params![json, id],
        )
        .map_err(se)?;
    Ok(n > 0)
}

/// Export a table back to JSONL text (oldest first) — the auditability seam behind
/// `familiar db export`. A missing/empty table yields an empty string.
pub fn export_jsonl(dir: &Path, file: &str) -> io::Result<String> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)?;
    let mut stmt = c
        .prepare(&format!("SELECT data FROM {table} ORDER BY seq"))
        .map_err(se)?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(se)?;
    let mut s = String::new();
    for row in rows {
        s.push_str(&row.map_err(se)?);
        s.push('\n');
    }
    Ok(s)
}

// ---- config files (NOT in the database — human-owned) -----------------------------------

/// Load a single JSON object from `<dir>/<file>` (one object spanning the whole file). Returns
/// `None` if missing, an error if malformed. For human-owned policy files (the capability
/// boundary, the co-owned parameters) that a person edits in a text editor — these are
/// deliberately **not** in the database.
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

fn se(e: rusqlite::Error) -> io::Error {
    io::Error::other(format!("sqlite: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    /// A throwaway temp dir, unique per call site, removed on drop.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!("substrate_store_test_{tag}_{}", std::process::id()));
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

    #[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
    struct Rec {
        id: String,
        name: String,
    }

    fn rec(id: &str, name: &str) -> Rec {
        Rec {
            id: id.into(),
            name: name.into(),
        }
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
        let a = rec("1", "alpha");
        let b = rec("2", "beta");
        append(d.path(), "recs.jsonl", &a).unwrap();
        append(d.path(), "recs.jsonl", &b).unwrap();
        let got: Vec<Rec> = load(d.path(), "recs.jsonl").unwrap();
        assert_eq!(got, vec![a, b]);
    }

    #[test]
    fn blank_lines_skipped_malformed_errors() {
        // A malformed legacy file surfaces as an error on first load (via the import).
        let d = TempDir::new("malformed");
        fs::create_dir_all(d.path()).unwrap();
        fs::write(
            d.path().join("x.jsonl"),
            "{\"id\":\"1\",\"name\":\"a\"}\n\nnot json\n",
        )
        .unwrap();
        let got: io::Result<Vec<Rec>> = load(d.path(), "x.jsonl");
        assert!(got.is_err());
    }

    #[test]
    fn imports_legacy_jsonl_then_renames_it() {
        let d = TempDir::new("import");
        fs::create_dir_all(d.path()).unwrap();
        fs::write(
            d.path().join("recs.jsonl"),
            "{\"id\":\"1\",\"name\":\"a\"}\n{\"id\":\"2\",\"name\":\"b\"}\n",
        )
        .unwrap();
        let got: Vec<Rec> = load(d.path(), "recs.jsonl").unwrap();
        assert_eq!(got, vec![rec("1", "a"), rec("2", "b")]);
        // the legacy file is archived, not left to be re-imported
        assert!(!d.path().join("recs.jsonl").exists());
        assert!(d.path().join("recs.jsonl.imported").exists());
        // a fresh append doesn't re-import
        append(d.path(), "recs.jsonl", &rec("3", "c")).unwrap();
        let got: Vec<Rec> = load(d.path(), "recs.jsonl").unwrap();
        assert_eq!(got.len(), 3);
    }

    #[test]
    fn update_by_id_touches_one_row_only() {
        let d = TempDir::new("update");
        for (i, n) in [("1", "a"), ("2", "b"), ("3", "c")] {
            append(d.path(), "recs.jsonl", &rec(i, n)).unwrap();
        }
        // update just #2
        assert!(update_by_id(d.path(), "recs.jsonl", "2", &rec("2", "BETA")).unwrap());
        // a miss returns false and changes nothing
        assert!(!update_by_id(d.path(), "recs.jsonl", "9", &rec("9", "z")).unwrap());
        let got: Vec<Rec> = load(d.path(), "recs.jsonl").unwrap();
        assert_eq!(got, vec![rec("1", "a"), rec("2", "BETA"), rec("3", "c")]);
        // and the targeted lookup finds it
        let one: Option<Rec> = load_by_id(d.path(), "recs.jsonl", "2").unwrap();
        assert_eq!(one, Some(rec("2", "BETA")));
        assert_eq!(load_by_id::<Rec>(d.path(), "recs.jsonl", "9").unwrap(), None);
    }

    #[test]
    fn rewrite_replaces_all_and_export_round_trips() {
        let d = TempDir::new("rewrite");
        append(d.path(), "recs.jsonl", &rec("1", "a")).unwrap();
        rewrite(d.path(), "recs.jsonl", &[rec("7", "x"), rec("8", "y")]).unwrap();
        let got: Vec<Rec> = load(d.path(), "recs.jsonl").unwrap();
        assert_eq!(got, vec![rec("7", "x"), rec("8", "y")]);
        // export reproduces readable JSONL that re-parses to the same records
        let jsonl = export_jsonl(d.path(), "recs.jsonl").unwrap();
        let back: Vec<Rec> = jsonl
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(back, got);
    }
}
