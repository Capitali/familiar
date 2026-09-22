# Known Failures & Gaps

Preserved deliberately — failures are fossils, and a gap named is a gap that can't
masquerade as coverage. (This is the lightweight, human-facing companion to the
factory's own pattern memory, which will hold negative evidence once the kernel
lands.)

## Functional gaps (by design, sequenced to close)

| Gap | Impact | Closes at |
|---|---|---|
| No autonomous cycle (metabolism) | Factory records & measures but does not yet act | Brick 6 |
| Presence signal (Law II) unbuilt | Empty-world failure state not yet detectable | Brick 3 |
| Obedience guard (Law III) unbuilt | No active pre-action refusal path yet | Brick 4 |
| Evolutionary kernel not ported | Loops/candidates/trials/selection absent | Brick 5 |

## Known correctness limits (current code)

- **Service classifier misses proper names** (a bare given name is not served-facing).
  Lexical-only cold-start; awaits entity resolution. Documented in
  [../docs/06-limitations.md](../docs/06-limitations.md).
- **Service measures attention, not fulfillment.** A surfaced need and a met need
  score the same.
- **Sequential observation ids assume append-only.** `obs-NNNN` is derived from
  current count; manual deletion from the log would collide ids. Observations are
  append-only by contract, so this is acceptable but worth stating.
- **Strict load on corruption.** A malformed JSONL line aborts the load (by design —
  corruption should surface). A crash mid-append could leave a partial last line that
  blocks loading until trimmed; a tolerant-recovery mode is a possible future option.

## Inherited-but-unverified

The v1 invariants are documented but not yet re-validated in Rust; until the kernel
port encodes them as tests, they are claims about the ancestor, not guarantees here.

## How to add to this file

When a brick reveals a failure or a limit, record it here with its impact and the
brick that will (or did) close it. Don't delete entries; mark them resolved with the
commit that fixed them.
