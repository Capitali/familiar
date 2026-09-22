# The LLM seam (periphery)

The LLM is **not** part of the familiar — it is consulted across a shell boundary
(`the model is not the familiar`). This directory holds the reference adapter; the
factory shells out to a copy of it **only when a human has opened the capability
boundary**.

## How it is gated (default-off)

A `consult` is an `Llm` action. The obedience guard evaluates it against the
human-owned boundary:

- **Default (no `boundary.json`, or `allow_llm: false`):** the guard **refuses**.
  Nothing here runs; no key is read; no network is touched.
- **Phase 1 (`allow_llm: true`):** the guard allows it, and the familiar writes
  `prompt.txt` and runs the copied `call_llm.sh`, reading `response.json`.

Only a human opens the boundary (see [../docs/boundaries.md](../docs/boundaries.md)).
The familiar can never open it for itself.

## Enabling Phase 1 (a human, deliberately)

```sh
mkdir -p familiar_data/llm
cp llm/call_llm.sh     familiar_data/llm/
cp llm/key.env.example familiar_data/llm/key.env   # then add real keys; chmod 600
cp data/sample/boundary.phase-1.example.json familiar_data/boundary.json
# now:
cargo run -p familiar-cli -- consult --prompt "…"
```

## Files

- `call_llm.sh` — multi-provider adapter (Gemini, Cerebras; fails over; reads
  `prompt.txt`, writes `response.json`). Carried from the v1 factory; no secrets.
- `key.env.example` — template for local keys. Real `key.env` lives under
  `familiar_data/` (gitignored) and is never committed.

## Restraint

The network is used to **consult**, never to **transmit** the served's data outward
(no telemetry, no exfiltration — Law III). Content-level exfiltration checks on what
is sent in a prompt are a future hardening; for now the seam is gated at the
capability level and is off by default.
