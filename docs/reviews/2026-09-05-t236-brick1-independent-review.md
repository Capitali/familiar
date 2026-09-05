# T-236 brick 1 — independent re-verification (persona seam + captain store)

**Reviewer:** MacOnStick lane, 2026-09-05. Independent of the chair (wildhorse) that wrote it.
**Scope:** `crates/kernel/src/persona.rs` (the seam), its touch points in `crates/cli/src/fleet.rs`
(pair / rename / status), and the captain-store move `82c922a` (persona out of the ship store into
`captains/<slug>/`, feed rows and `PUT /ships/{w}/captain` in `fleet_serve.rs`).
**Method:** read the code at `1c439ec`; ran the kernel persona tests locally; did not re-run the fleet
against a live store (that proof is the chair's, and Ian's iPad shows Felix across both hulls).

## Verdict: ACCEPT. Two findings and the missing test were fixed in the same landing (see the end).

The seam does what ADR-0037 §1 and the T-236 dialogue rulings say it must:

- **Mask, never authority.** The module only produces a role phrase, a name and bounded STYLE axes.
  The Three Laws, `guard::evaluate`, boundary gates and the Law III voice are not reachable from
  `persona.json`; `Style` is `deny_unknown_fields`, so a hostile file cannot smuggle a new axis in.
  Candor, uncertainty, risk-talk, refusal and spending posture are deliberately unrepresentable (Q5).
- **No-op for every existing deployment.** Absent file → `Persona::default()`, and the default role is
  pinned byte-for-byte to the literal it replaced (`the_default_persona_is_todays_words_byte_for_byte`).
- **Present-but-broken is an error, not a fallback.** `load` refuses an unparseable or invalid file
  with a message naming the file. Right call: a human who wrote the file and got the default anyway
  would never be told.
- **Atomic write.** `write` validates first, then tmp + rename, so a crash mid-write cannot leave a
  half-voice for the loader to refuse.
- **Naming trail** is append-only jsonl (`record_naming` / `namings`), oldest first, lenient on a
  torn line. Fine for an audit trail.
- **Captain store.** `captain_store` slugs the captain name and puts the record beside `worlds/`;
  pairing a second ship joins the existing computer; rename names the captain's computer and lists
  the hulls; status and feed rows resolve captain → ship-local → unnamed. Matches Ian's rulings
  verbatim ("one ship's computer per captain … under a name he chooses"; hulls keep their names).

## Findings

**F1 — pairing with `--computer-name` onto a captain who already has a computer wipes their style.**
`fleet.rs` pair: `if already && computer_name.is_none() { join } else { write(persona_dir, &persona) }`
where `persona` is a fresh record with `Style::default()`. So `fleet pair <second ship> --computer-name Felix`
for a captain whose Felix already carries a tuned greeting / vocabulary / warmth silently resets all of
it to defaults, keeping only the new name. The commit message promises "a name given at pairing still
names it", which reads as rename, not replace. Fix: when `already`, `load` the existing persona, set
`name`, `write` it back (and record the naming as now). Small, but it is exactly the class of silent
loss the seam's own loader was built to refuse.

**F2 — the feed reads persona leniently where the kernel reads it strictly.** `persona_for` parses
`persona.json` as a raw `serde_json::Value` and returns whatever parses, so a file the kernel's
`load` would refuse as invalid (bad `persona_version`, out-of-range style axis, unknown field) still
flows to `/ships` rows, `fleet status`, and `/captains/{slug}/brief` as if it were good. The CLI
path and the wire path can therefore disagree about the same file. Fix: route `persona_for` through
`familiar_kernel::persona::load` and surface the error (`"persona": {"error": …}` or Null plus a
host note) rather than the raw value. Not urgent today: every store in the house was written by
`write`, which validates. It matters the first time a human edits the file by hand.

**N1 — no CLI-level test for the captain store.** The kernel seam has four tests (default byte
pin, round-trip, refuse-invalid, absent = default). `captain_store` / `persona_for` / the join-on-pair
branch / the rename-lists-hulls output have none; the only proof is the chair's live run and Ian's
iPad. A tempdir test that pairs two ships for one captain and asserts a single `captains/<slug>/persona.json`
and one shared name would pin the ruling against the next refactor. Suggest it rides with F1.

Minor, no action: the slug collapses case and punctuation ("Luke SkyWhisker" and "luke-skywhisker"
share a store), which is the behaviour we want after the pairing-sheet mishap on 2026-09-04; an
empty captain falls to `captains/captain/`, which is honest enough.

## What I did not verify
Live `fleet rename` output and the `PUT /ships/{w}/captain` old-store sweep (`retired_old_captain_store`)
were read, not run. The chair ran both on wildhorse on 2026-09-04 with Ian watching the iPad.

## Fixed with this review (same commit, MacOnStick lane)
- **F1:** pairing onto a captain who already has a computer now loads the existing persona and, when
  `--computer-name` is given, only sets `name` before writing it back; style, greeting and vocabulary
  survive. A captain's persona that will not load fails the pair loudly instead of being overwritten.
- **F2:** `persona_for` reads through `familiar_kernel::persona::load`; a present-but-invalid record
  reaches the feed as `{"error": "<why>"}` (the app shows the hull as unnamed and the settings sheet
  surfaces the decode error) instead of the raw file. **Behaviour change, deliberate:** the old
  loop continued past a captain record that failed to parse and fell through to the ship's own
  `persona.json`; the new one stops at the first file that *exists*. A broken captain record is
  therefore an error on the feed, not a silent fall-back to a different name — under the
  one-computer-per-captain ruling a silent fall-back would look like the feed working while a
  different name spoke for the captain. Confirmed with the chair 2026-09-05; if you meet
  `{"error": …}` on a row, that is the design, not a regression.
- **N1:** three tests in `fleet.rs` (`captain_store_tests`): the slug and location, captain-over-ship
  precedence with the pre-ruling fallback and captain isolation, and the broken-file error shape.
The join-on-pair branch itself is still only proven live; it sits inside the command match and wants
the pair command factored out before it can be exercised in a tempdir.
