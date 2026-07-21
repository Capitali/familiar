# Storage — the embedded SQLite store

**Status: implemented (`kernel::store`).** The familiar's records live in a single embedded
**SQLite** database, `<data-dir>/familiar.db`. This replaced the original one-JSONL-file-per-type
layout, whose every update rewrote the entire file (`store::rewrite`) — O(n) per change and O(n²)
over a run. The candidate store once reached 7,380 rows at mutation depth 320 and ticks crawled;
the store change makes updates indexed and lets the daemon and any local reader share the data safely.

## Shape

- **One table per record type**, named from the old file stem (`candidates.jsonl` → table
  `candidates`): `(seq INTEGER PRIMARY KEY AUTOINCREMENT, data TEXT NOT NULL)`, with an index on
  `json_extract(data,'$.id')`. `data` is the same serde JSON the record always serialized to —
  no schema migration, no hand-mapped columns.
- **The public API is unchanged.** `store::append` / `load` / `rewrite` keep their signatures, so
  every consumer (cycle, cli, sense — and, before their archiving, glass/marble) is untouched. Insertion order is preserved via
  `seq`.
- **Indexed updates.** `store::load_by_id` / `update_by_id` do a single `… WHERE
  json_extract(data,'$.id')=?` — O(log n), not load-all + rewrite-all. The update-heavy paths
  (`candidate::update_status`, `thread::update_status`, `tool::record_use`,
  `request::update_status`/`set_feedback`, the `question` update helper) use these. Bounded sets
  (`identity`, `loops::save_all`) stay on `rewrite`.
- **Concurrency.** Opened with `PRAGMA journal_mode=WAL`, `busy_timeout=5000`,
  `synchronous=NORMAL`: the daemon writes while another local process reads, without the
  partial-read races the flat files had. One cached connection per data dir per process.

## Config is NOT in the database

Human-owned policy files stay plain text, read via `store::load_one` and written directly by
their owners — never in the DB:
- `boundary.json` — the capability boundary (Law III). Only the human's instrument writes it —
  a text editor, or the console via the loopback-only `POST /local/gate` endpoint (a local
  human boundary-write, never the autonomous cycle); the kernel has no write path. This must
  remain a file.
- `parameters.json` — the co-owned tuning parameters.
- `devices.json`, `llm/`, `mesh/`, `observer.txt` — config and secrets.

## Transparency preserved

The "cat-able, rebuildable truth" property survives:
- **One-time import.** On first touch of a table, an existing legacy `<file>.jsonl` is imported
  (validated per line, transactionally) and renamed `<file>.imported` — nothing is lost on
  upgrade, and a malformed legacy line surfaces as an error rather than being half-migrated.
- **Export / import.** `familiar db export [--out DIR]` dumps every table to readable JSONL
  (default `<data-dir>/export/`); `familiar db import` folds any legacy `.jsonl` still present
  into its table. Observations remain the only truth; derived tables can be dropped and rebuilt.

## The dependency concession

`kernel` is otherwise serde-only — a small, legible trust surface is part of the Law III
commitment. The store is the one place that takes on a C-backed dependency, `rusqlite` with the
`bundled` feature (SQLite compiled in — no system library). The kernel's own
`#![forbid(unsafe_code)]` still holds: it governs kernel code, not dependencies. Named here and
in `crates/kernel/Cargo.toml` so the concession stays visible.
