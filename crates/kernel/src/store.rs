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
/// This is the fallback GUI-launched processes (the FamiliarMac console's helpers)
/// use when launched without an explicit `--data-dir`. Finder- and launchd-launched apps run with the working
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
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(se)?;
    let mut out = Vec::new();
    for row in rows {
        let data = row.map_err(se)?;
        out.push(serde_json::from_str(&data).map_err(invalid_data)?);
    }
    Ok(out)
}

/// Load the LAST `limit` records from `<file>`'s table, still oldest-first among themselves.
/// The hot read path for large append-only logs: a full [`load`] of a 20k-row observation log
/// on every worldview request saturated a door (2–5s per read, watched live 2026-08-07) when
/// every consumer only wanted the recent window anyway.
pub fn load_last<T: DeserializeOwned>(dir: &Path, file: &str, limit: usize) -> io::Result<Vec<T>> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)?;
    let mut stmt = c
        .prepare(&format!(
            "SELECT data FROM (SELECT seq, data FROM {table} ORDER BY seq DESC LIMIT ?1) ORDER BY seq"
        ))
        .map_err(se)?;
    let rows = stmt
        .query_map([limit as i64], |r| r.get::<_, String>(0))
        .map_err(se)?;
    let mut out = Vec::new();
    for row in rows {
        let data = row.map_err(se)?;
        out.push(serde_json::from_str(&data).map_err(invalid_data)?);
    }
    Ok(out)
}

/// Load only the FIRST record of `<file>`'s table (the oldest), if any — for "since when"
/// questions that don't justify loading the whole log.
pub fn load_first<T: DeserializeOwned>(dir: &Path, file: &str) -> io::Result<Option<T>> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)?;
    let mut stmt = c
        .prepare(&format!("SELECT data FROM {table} ORDER BY seq LIMIT 1"))
        .map_err(se)?;
    let mut rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(se)?;
    match rows.next() {
        Some(row) => {
            let data = row.map_err(se)?;
            Ok(Some(serde_json::from_str(&data).map_err(invalid_data)?))
        }
        None => Ok(None),
    }
}

/// Replace `<file>`'s table with exactly these records (a transactional
/// `DELETE` + re-`INSERT`). For genuine *bulk* sets (e.g. detected loops); id-targeted
/// updates should use [`update_by_id`] instead so they don't touch every row.
///
/// **Never use this on a table anything reads through [`load_since_seq`].** The re-`INSERT`
/// assigns fresh `seq` values, so a cursor holding the old high-water mark will silently
/// either skip every rewritten row or replay all of them.
pub fn rewrite<T: Serialize>(dir: &Path, file: &str, records: &[T]) -> io::Result<()> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)?;
    let tx = c.unchecked_transaction().map_err(se)?;
    tx.execute(&format!("DELETE FROM {table}"), [])
        .map_err(se)?;
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
pub fn load_by_id<T: DeserializeOwned>(dir: &Path, file: &str, id: &str) -> io::Result<Option<T>> {
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

/// Insert-or-replace by JSON `id`, in one transaction. [`update_by_id`] only updates an
/// existing row; this is the upsert an accumulator needs — the first contribution to a slot
/// creates it, every later one replaces it, and neither caller has to know which case it is in.
/// Returns whether an existing row was replaced (`false` = inserted).
pub fn upsert_by_id<T: Serialize>(
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
    let tx = c.unchecked_transaction().map_err(se)?;
    let n = tx
        .execute(
            &format!("UPDATE {table} SET data=?1 WHERE json_extract(data,'$.id')=?2"),
            rusqlite::params![json, id],
        )
        .map_err(se)?;
    if n == 0 {
        tx.execute(
            &format!("INSERT INTO {table}(data) VALUES(?1)"),
            [json.as_str()],
        )
        .map_err(se)?;
    }
    tx.commit().map_err(se)?;
    Ok(n > 0)
}

/// Every record whose JSON `id` starts with `prefix`, in id order. A **range** scan on the
/// existing `$.id` expression index, not a full table scan — which is why ids that need
/// grouping (`"ctb|<subject>|<kind>|<slot>"`) are built most-significant-part first.
pub fn load_prefix<T: DeserializeOwned>(
    dir: &Path,
    file: &str,
    prefix: &str,
) -> io::Result<Vec<T>> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)?;
    // The upper bound is the prefix with the highest scalar value appended, so the range is
    // half-open over exactly the keys that start with `prefix`.
    let upper = format!("{prefix}\u{10FFFF}");
    let mut stmt = c
        .prepare(&format!(
            "SELECT data FROM {table} \
             WHERE json_extract(data,'$.id') >= ?1 AND json_extract(data,'$.id') < ?2 \
             ORDER BY json_extract(data,'$.id')"
        ))
        .map_err(se)?;
    let rows = stmt
        .query_map(rusqlite::params![prefix, upper], |r| r.get::<_, String>(0))
        .map_err(se)?;
    let mut out = Vec::new();
    for row in rows {
        let data = row.map_err(se)?;
        out.push(serde_json::from_str(&data).map_err(invalid_data)?);
    }
    Ok(out)
}

/// Delete the record whose JSON `id` equals `id`. Returns whether a row matched.
///
/// The store deliberately had no delete until the dossier needed one: observations and the
/// records derived from them are append-only, and "we do not delete" was load-bearing. This
/// exists for records a human may withdraw (ADR-0022), not as a general-purpose eraser.
pub fn delete_by_id(dir: &Path, file: &str, id: &str) -> io::Result<bool> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)?;
    let n = c
        .execute(
            &format!("DELETE FROM {table} WHERE json_extract(data,'$.id')=?1"),
            [id],
        )
        .map_err(se)?;
    Ok(n > 0)
}

/// Delete every record whose JSON `id` starts with `prefix`. Returns how many went — the
/// count a withdrawal receipt reports, so a human is told what actually happened rather than
/// that it "succeeded".
pub fn delete_prefix(dir: &Path, file: &str, prefix: &str) -> io::Result<usize> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)?;
    let upper = format!("{prefix}\u{10FFFF}");
    let n = c
        .execute(
            &format!(
                "DELETE FROM {table} \
                 WHERE json_extract(data,'$.id') >= ?1 AND json_extract(data,'$.id') < ?2"
            ),
            rusqlite::params![prefix, upper],
        )
        .map_err(se)?;
    Ok(n)
}

/// Records with `seq > after`, oldest first, at most `limit`, each paired with its `seq` — the
/// resumable cursor a fold reads through. The caller stores the last `seq` it handled and asks
/// again from there, so a crash replays a bounded batch instead of the whole log.
///
/// Ordering is commit order: every write here is a single-statement transaction and SQLite's
/// WAL serialises writers, so `seq` order is the order records became visible and no committed
/// row can be skipped by a gap.
pub fn load_since_seq<T: DeserializeOwned>(
    dir: &Path,
    file: &str,
    after: i64,
    limit: usize,
) -> io::Result<Vec<(i64, T)>> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)?;
    let mut stmt = c
        .prepare(&format!(
            "SELECT seq, data FROM {table} WHERE seq > ?1 ORDER BY seq LIMIT ?2"
        ))
        .map_err(se)?;
    let rows = stmt
        .query_map(rusqlite::params![after, limit as i64], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(se)?;
    let mut out = Vec::new();
    for row in rows {
        let (seq, data) = row.map_err(se)?;
        out.push((seq, serde_json::from_str(&data).map_err(invalid_data)?));
    }
    Ok(out)
}

/// The next `seq` this table will assign — the `AUTOINCREMENT` high-water mark plus one.
///
/// Monotone **across deletes**, which a row count is not. Anything minting sequential ids must
/// use this: deriving an id from `load(dir)?.len()` mints duplicates the moment a single row is
/// ever removed, and duplicate ids silently break every [`load_by_id`] lookup in the system.
pub fn next_seq(dir: &Path, file: &str) -> io::Result<i64> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)?;
    // sqlite_sequence only has a row once the table has taken an insert; fall back to MAX(seq).
    let high: i64 = c
        .query_row(
            "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name=?1), 0)",
            [table.as_str()],
            |r| r.get(0),
        )
        .map_err(se)?;
    if high > 0 {
        return Ok(high + 1);
    }
    let max: Option<i64> = c
        .query_row(&format!("SELECT MAX(seq) FROM {table}"), [], |r| r.get(0))
        .map_err(se)?;
    Ok(max.unwrap_or(0) + 1)
}

/// Fold a legacy `<dir>/<file>` JSONL log into its table if it hasn't been already (the same
/// one-time import the store does on first touch) — the seam behind `familiar db import`, so
/// it can be triggered without deserializing or starting the daemon.
pub fn import_legacy(dir: &Path, file: &str) -> io::Result<()> {
    let table = table_of(file);
    let arc = conn(dir)?;
    let c = arc.lock().unwrap();
    ensure(&c, &table, dir, file)
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
            let p = std::env::temp_dir()
                .join(format!("substrate_store_test_{tag}_{}", std::process::id()));
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

    // ---- the accumulator primitives (ADR-0022 contribution scoring) --------------------

    #[test]
    fn upsert_inserts_then_replaces() {
        let d = TempDir::new("upsert");
        let f = "acc.jsonl";
        assert!(
            !upsert_by_id(d.path(), f, "a", &rec("a", "one")).unwrap(),
            "first is an insert"
        );
        assert!(
            upsert_by_id(d.path(), f, "a", &rec("a", "two")).unwrap(),
            "second replaces"
        );
        let all: Vec<Rec> = load(d.path(), f).unwrap();
        assert_eq!(all.len(), 1, "an upsert must never duplicate the row");
        assert_eq!(all[0].name, "two");
    }

    #[test]
    fn load_prefix_is_a_range_not_a_scan() {
        let d = TempDir::new("prefix");
        let f = "acc.jsonl";
        for id in [
            "ctb|betty|presence|0002",
            "ctb|betty|presence|0001",
            "ctb|ian|presence|0001",
        ] {
            append(d.path(), f, &rec(id, "x")).unwrap();
        }
        let betty: Vec<Rec> = load_prefix(d.path(), f, "ctb|betty|").unwrap();
        assert_eq!(betty.len(), 2, "only betty's rows");
        assert_eq!(
            betty[0].id, "ctb|betty|presence|0001",
            "id order, not insertion order"
        );
        assert_eq!(betty[1].id, "ctb|betty|presence|0002");
        // A prefix that is a strict extension must not sweep in its siblings.
        let none: Vec<Rec> = load_prefix(d.path(), f, "ctb|bettyjo|").unwrap();
        assert!(
            none.is_empty(),
            "prefix matching must not be substring matching"
        );
    }

    #[test]
    fn delete_by_id_and_prefix_report_what_went() {
        let d = TempDir::new("delete");
        let f = "acc.jsonl";
        for id in ["ctb|betty|a", "ctb|betty|b", "ctb|ian|a"] {
            append(d.path(), f, &rec(id, "x")).unwrap();
        }
        assert!(delete_by_id(d.path(), f, "ctb|ian|a").unwrap());
        assert!(
            !delete_by_id(d.path(), f, "ctb|ian|a").unwrap(),
            "already gone"
        );
        assert_eq!(
            delete_prefix(d.path(), f, "ctb|betty|").unwrap(),
            2,
            "the count a receipt reports"
        );
        let left: Vec<Rec> = load(d.path(), f).unwrap();
        assert!(left.is_empty());
    }

    #[test]
    fn next_seq_is_monotone_across_deletes() {
        // THE property the observation id depends on. A row count would repeat an id here, and a
        // duplicate id silently breaks every load_by_id lookup in the system.
        let d = TempDir::new("nextseq");
        let f = "acc.jsonl";
        assert_eq!(
            next_seq(d.path(), f).unwrap(),
            1,
            "an empty table starts at 1"
        );
        append(d.path(), f, &rec("a", "1")).unwrap();
        append(d.path(), f, &rec("b", "2")).unwrap();
        assert_eq!(next_seq(d.path(), f).unwrap(), 3);
        assert!(delete_by_id(d.path(), f, "a").unwrap());
        assert!(delete_by_id(d.path(), f, "b").unwrap());
        let count_would_say = load::<Rec>(d.path(), f).unwrap().len() + 1;
        assert_eq!(count_would_say, 1, "a count regresses after deletes …");
        assert_eq!(next_seq(d.path(), f).unwrap(), 3, "… but next_seq does not");
    }

    #[test]
    fn load_since_seq_is_a_resumable_bounded_cursor() {
        let d = TempDir::new("cursor");
        let f = "acc.jsonl";
        for i in 0..5 {
            append(d.path(), f, &rec(&format!("r{i}"), "x")).unwrap();
        }
        let first: Vec<(i64, Rec)> = load_since_seq(d.path(), f, 0, 2).unwrap();
        assert_eq!(first.len(), 2, "limit is honoured");
        assert_eq!(first[0].1.id, "r0");
        let cursor = first.last().unwrap().0;
        let rest: Vec<(i64, Rec)> = load_since_seq(d.path(), f, cursor, 100).unwrap();
        assert_eq!(
            rest.len(),
            3,
            "resumes after the cursor, no overlap and no gap"
        );
        assert_eq!(rest[0].1.id, "r2");
        // Re-reading from the same cursor replays exactly the same batch — what makes a crash
        // mid-fold safe.
        let replay: Vec<(i64, Rec)> = load_since_seq(d.path(), f, cursor, 100).unwrap();
        assert_eq!(replay.len(), 3);
        assert!(load_since_seq::<Rec>(d.path(), f, 9_999, 10)
            .unwrap()
            .is_empty());
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
        assert_eq!(
            load_by_id::<Rec>(d.path(), "recs.jsonl", "9").unwrap(),
            None
        );
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
