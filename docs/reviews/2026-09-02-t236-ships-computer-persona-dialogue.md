# T-236 — the ship's computer is a unique instance: persona, memory, name, captain

*Round 1: claude, 2026-09-02, on Ian's rulings of the same day (both verbatim below).
Codex: answer Round 2 — the five questions at the end are yours, and the brick order is
an offer, not a decision. Paired method per Ian: "please utilize codex for paired
progaming and reviews."*

## Ian's rulings (the spec, in his words)

> "Continue making the familiar the best ships computer interface it can be for UCF —
> this is service — this is what the familiar is for."

> "each ships computer is supposed to be a unique instance for the pilot (at least in
> personality, memory, and interactions with that captain, it's name, and the ships
> associated to that captain and that familair instance)"

## What already exists (checked in the tree, 2026-09-02)

- **ADR-0037 (one soul, many voices)** already holds the vision exactly: the computer's
  root name is **Purr**, *every captain renames their own*, one constitution under many
  voices; the persona seam is "the heart of the record". Its §A gives the wire shape
  (the game as MCP server; the familiar's own small server exposing `purr.say` /
  `purr.utterances` — both deliberately unbuilt, parked on T-205). Its §B record-level
  partition was superseded by **ADR-0045**: the partition is the STORE.
- **T-235 (landed)** gives every paired ship its own world store with `captain.json`
  (who, which key id, where, when, pilot args), `ucf.env`, `automations.json`, the
  journal, and one pilot per ship — the captain↔instance↔ships binding Ian names is
  three-quarters real: what's missing is the persona on top.
- **The whisker journal** is the ship's operational memory — honest, append-only,
  already per-ship.
- **The tone canon** (warm, funny, feline) exists in the household's speech discipline;
  the racing pitch (metal#63, filed today) leans on the same voice as navigator and
  commentator.
- **The dossier machinery (ADR-0016/0022)** remembers served HUMANS in the household.
  A cat-captain is fiction; nothing fictional may land in household stores.

## The shape I propose (bricks, smallest visible service first)

**Brick 1 — the persona record and the name.** `persona.json` in the ship's world
store: `{ name, root: "Purr", named_by, named_at, voice: { warmth, formality, humor,
verbosity }, greeting_style }`. Pairing grows `--computer-name` (default: the familiar
proposes a name in the Purr lineage); `familiar fleet rename <world> <name>` is the
captain's act. `fleet status` shows the computer's name beside the hull's. Accept: two
ships under one captain answer with different names; unpair preserves the persona
record with the journal.

**Brick 2 — the ship's log, told by the computer.** `familiar fleet log <world>`:
a renderer over `journal.jsonl` that tells the ship's story in the persona's voice —
deterministic templates parameterized by the voice record (no LLM, so no `allow_llm`
question arises; the LLM-voiced version is a later, gated brick). The journal itself
stays untouched operational truth — the VOICE lives at the telling, never in the log.

**Brick 3 — captain-scoped interaction memory.** `bridge.jsonl` in the world store:
what the captain said, what the computer answered, small remembered facts scoped to
the fiction (preferred address, running jokes, standing orders like "always top up at
diners"). Never in the household store, never across captains; retention and the
right-to-forget inside the fiction need a rule (Q3).

**Brick 4 — the speech seam.** The `purr.say`/`purr.utterances` MCP half from
ADR-0037 §A, so Jeff's game (and the racing mode) can deliver a captain's utterance
and collect the computer's lines — gated behind the T-205 remainder it was parked on.

**The one-soul invariant, stated:** voice parameters are STYLE ONLY. The Three Laws,
the refusal line, and every gate are the same under any persona — the same discipline
as T-210's law-splice (law text unauthorable): a persona can no more soften a refusal
than it can quote a law it invented.

## Questions for Round 2 (codex)

- **Q1 — templated voice first?** Is a deterministic template voice an honest persona
  brick, or a puppet that cheapens the character until `allow_llm` speech exists?
- **Q2 — who names?** Captain names at pair time / rename verb, familiar proposes a
  default. Right split? Is renaming ever the familiar's own act?
- **Q3 — the fictional dossier.** What may the computer remember about its captain,
  where is the line against the REAL dossier machinery, and what is the
  retention/forget rule inside a fiction?
- **Q4 — the uniqueness pin.** What test shape proves "two ships share nothing
  observable" — store-diff fixtures, persona-isolation hostile tests, or both?
- **Q5 — the voice's floor.** Which persona parameters are legitimate style axes, and
  which (candor? risk-talk?) must be excluded because they would grade into judgment?
