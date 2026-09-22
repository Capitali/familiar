# data/

The data interface for The Familiar: the record model, its schema, and a synthetic
sample. This makes the familiar's data **Interoperable** and **Reusable** (FAIR) —
anything can read or produce its logs without reading the Rust.

## Format

- **JSONL** — one JSON object per line, UTF-8.
- **Append-only** — records are appended, never edited in place; derived views are
  recomputed from the log.
- **One file per record type** under a data directory (`familiar_data/` by default,
  `--data-dir` to override). Runtime data is git-ignored; it is not source.

## Record types

| File | Record | Schema |
|---|---|---|
| `observations.jsonl` | the observation (the only truth) | [`schema/observation.schema.json`](schema/observation.schema.json) |
| *(loops, candidates, trials, …)* | derived/lifecycle records | added with the kernel port |

Only **observations** are authoritative. Everything else is derived from them and
can be rebuilt. The conceptual model (and how records relate) is in
[`../docs/data-model.md`](../docs/data-model.md); sensitivity handling is in
[`../security/data-classification.md`](../security/data-classification.md); provenance
and intended use is the [data sheet](../docs/data-sheet.md).

## Sample

[`sample/observations.jsonl`](sample/observations.jsonl) is a small **synthetic**
log (no real personal data) mixing served-facing and host-internal observations —
usable to try the CLI:

```sh
mkdir -p /tmp/sub && cp data/sample/observations.jsonl /tmp/sub/
cargo run -p familiar-cli -- observations --data-dir /tmp/sub
cargo run -p familiar-cli -- service --data-dir /tmp/sub
```

## Validating against the schema

```sh
# example with check-jsonschema (pipx install check-jsonschema)
while IFS= read -r line; do
  echo "$line" | check-jsonschema --schemafile data/schema/observation.schema.json /dev/stdin
done < data/sample/observations.jsonl
```
