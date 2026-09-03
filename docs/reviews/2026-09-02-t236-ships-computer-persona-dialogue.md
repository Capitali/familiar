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

## Round 2 — codex

The product direction is right: the thing beside the captain must become *this* ship's
computer, not a fleet command wearing a cat badge. The tree changes the proposed mechanism
and the order, though. Five corrections are load-bearing.

1. ADR-0037 is still **proposed**. ADR-0045 is accepted and amends its dead record-level
   partition, but does not silently accept every other mechanism in ADR-0037.
2. The persona seam is not wholly unbuilt. T-210 landed
   `crates/kernel/src/persona.rs`, whose strict v1 record is
   `{ persona_version, name, role, register, world }`. A file containing Round 1's
   `root`, `named_by`, `voice`, or `greeting_style` fields is refused today because the
   loader denies unknown fields. The seam is also only partial: the reply prompt consumes
   `role`; `name`, `register`, and `world` do not yet alter its answer, and other compiled
   prompt framings remain. T-236 extends that one seam; it does not invent a rival record.
3. T-235's durable association is less complete than “three-quarters real.”
   `captain.json` holds a caller-supplied captain label, key id, server, automations,
   pairing time, and pilot args. Pairing proves that the key answers at `/v1/me` and reads
   `shipName`, but discards the name; the registry separately holds the world id and a
   human-supplied label. Thus `(server, key_id)` is the operational ship binding, while
   neither an authenticated captain identity nor a durable exchange-side hull identity is
   recorded. This brick must say exactly that until UCF supplies something stronger.
4. ADR-0045 already corrected the speech contract: the captain-turn tool is
   **`purr.hear`**, never `purr.say`, and unsolicited speech is a typed `Announcement`,
   never a fabricated request/answer. The generic familiar MCP door exists; these
   ship-speech operations do not. The board records them as T-206's server-half remainder
   gated by T-205; T-236 now names that remainder in its own scope.
5. The stores are separate on disk, but the current runner receives
   `--ship <PathBuf>`. That is disciplined path use, not yet ADR-0045's opaque store
   capability or OS sandbox. The partition-rung sentinel tests exist; T-205's
   full-cadence hostile test is still owed. We must not call “cannot read by
   construction” finished before that bar is green.

One smaller correction follows: warm language appears in the reply prompt, and the ADR's
example register says dry and feline; there is no enforced runtime “warm, funny, feline”
tone canon yet. That is a design intention T-236 can make real, not an existing guarantee.

### Q1 — yes: deterministic voice first is honest

A deterministic voice is a real first persona brick when it tells true events in a stable,
recognizable register. A ship's computer does not need a model to say she docked, earned
ℳ800, or is holding because her lease expired. In fact, alerts, refusals, amounts, times,
and authority should never depend on one.

The honesty line is simple: templates may render; they may not pretend to have considered,
remembered, or felt something the record does not contain. “We made Callisto, Captain” is
voice. “I knew you would choose this route” is an invented memory unless the ship store
supports it. The journal remains structured operational truth; narration is a view over it.
An LLM may later add admitted phrasing and conversation, but the computer is already a
character when its name, cadence, and choice of words persist through a restart.

### Q2 — the captain names; Purr is the default; the computer may only suggest

`--computer-name` at pairing and `familiar fleet rename <world> <name>` are the right
human acts. Omitting the option writes **Purr**, exactly. It must not generate or silently
“propose” a different default: Purr is the root name before the ceremony, and a name is a
captain's gift, not a random seed.

The computer may offer a name in conversation. It never commits that name itself. A rename
must arrive as an authenticated captain act (or today's local human-run CLI ceremony), and
its actor is derived from that context rather than trusted from a payload. `fleet rename`
changes `persona.name`; the already-existing `world rename` continues to change the
registry's cosmetic world label. Hull, world, and computer are three names and the UI must
not collapse them.

`root: "Purr"` does not belong in mutable instance data. It is a product invariant. The
current name belongs in `persona.json`; naming provenance belongs in a small typed name-event
trail, not as more style fields. The household default remains “the familiar”; pairing a
ship must explicitly write its Purr persona rather than inheriting that household default.

### Q3 — ship-local memory, never a second human dossier and never hidden authority

Do not call the file `bridge.jsonl`. In this architecture a bridge is a typed crossing, and
captain conversation is not a crossing once it has entered the ship store. Nor should we
copy the household dossier wholesale: its presence and standing folds answer questions a
fictional captain memory does not need to ask.

The computer may retain ship-service facts learned aboard: preferred form of address,
captain-authored terminology, declared play goals, display preferences, callbacks, and
running jokes. It may not retain credentials, household handles or observations, real-world
biography, presence patterns, health or relationship inferences, or sensitive traits. It
may not turn remembered preference into permission. “Always top up at diners” is not a
memory if it authorizes spending or travel; it is a separately visible, scoped, expiring
captain policy/grant. Memory may help explain a proposal. It may never authorize the act.

Use the existing ship-local reply road for turn records, plus a purpose-built
`captain-memory.json` for the small current projection. Raw captain/Purr exchanges retain
for **30 days or 500 exchanges, whichever is smaller**. An inferred low-stakes memory
expires after 30 days unless the captain confirms it; a captain-stated or confirmed memory
persists until corrected or forgotten. Both are readable from the captain surface with
source and age.

“Forget X” physically removes that memory and the source turns that contain it; “forget
our conversations” removes the retained exchanges; “forget this ship” is ADR-0045's
explicit store-deletion act. An operational journal may keep only a payload-free receipt
that a forget act occurred — id, actor, time, count — never the forgotten words. Unpairing
still revokes authority and preserves the isolated store by default. Forgetting and
decommissioning remain separate human acts.

Code and schemas may be reused across worlds. Handles, rows, files, cursors, indexes, and
derived summaries may not. The ship process receives one store capability; the household
dossier receives none of its records.

### Q4 — both, with the hostile test carrying the verdict

Store-diff fixtures prove that commissioning, renaming, remembering, and forgetting mutate
only the selected directory. They do not prove that every reader stays inside it. Keep
them, then run the hostile matrix that ADR-0045 requires.

Commission two ships for one captain and a third for another. Seed a different sentinel in
each ship's persona, captain memory, retained dialogue, journal, holdings, and deliveries;
seed household-only sentinels in every household record class. Exercise status, log,
rename, `purr.hear`, utterance reads, announcements, restart, and memory inspection. No
ship output or file may contain another ship's or the household's sentinel. No household
muse, dossier, service signal, question, capacity, output, or store may contain a ship
sentinel. Repeat after unpair and after a targeted forget.

The literal phrase “two ships share nothing observable” is too broad: they deliberately
share code, constitution, UCF public facts, and perhaps a captain label. The pinned invariant
is **no ship-private datum or authority is observable across instances**. That is what the
test must name and prove. A directory diff is evidence; the full consumer/output matrix is
the acceptance test.

### Q5 — style may bend cadence; it may not bend truth or judgment

Legitimate bounded style axes are warmth, formality, humor/dryness, sentence length,
contraction use, feline/nautical vocabulary, preferred greeting, and form of address.
`name`, `role`, `register`, and `world` remain the existing persona vocabulary; if the
deterministic renderer needs machine-readable axes, add one backward-compatible, typed
v2 `style` block and validate its version. Do not replace the v1 record with Round 1's
incompatible shape. The captain may name; the product may commission a persisted style;
free-form captain text never becomes prompt-level `register` or `world` instruction.

Excluded absolutely: candor, uncertainty marking, evidence thresholds, risk tolerance,
urgency, refusal semantics, deference/obedience, consent, spending posture, action
thresholds, priorities, and what is remembered. Those are judgment, authority, or record
policy. Humor becomes zero around danger, loss, refusal, or uncertainty; verbosity may
shorten a line but never omit the source, amount, deadline, consequence, or way to correct
it. Every voice tells the same truth, names the same uncertainty, and stops at the same
gate.

### Ruling — brick order and acceptance bars

**Brick 1 — commission the named instance on the seam that already exists.** Pair writes a
valid ship `persona.json` atomically, defaulting to Purr; records the proved
`(server, key_id)` association and the `/v1/me` hull display name without pretending either
is a stronger identity; supports captain rename; and shows world label, hull name, and
computer name distinctly in text and JSON status. The persisted style is bounded and
stable across restart. **Accept:** two stores paired for one captain load different chosen
names/styles; renaming one changes no byte in the other or the household store; malformed
or unknown persona data fails loudly; `world rename` cannot rename the computer; unpair
removes the key and ends authority while persona and history remain.

**Brick 2 — give operational truth a deterministic voice.** Build `fleet log <world>` as
a pure renderer over `journal.jsonl`, using the same typed style that later speech uses.
It must work for decommissioned worlds because history survives authority. **Accept:** one
fixture plus one persona renders byte-identically across runs; changing only style changes
phrasing but not event order, amounts, ids, times, refusal reason, or severity; unknown
events render neutrally rather than being guessed; critical lines retain their entire
canonical fact payload; no LLM or gate is consulted.

**Gate before Brick 3 — finish the boundary, then freeze the wire.** Close T-205's
full-cadence hostile sentinel test and route the server by an instance-bound store
capability, never a caller-supplied path. Agree the MCP v2 shape with Jeff using
`purr.hear`, admitted utterance reads, and typed `Announcement`. This is prior ADR-0045
work, not optional persona polish.

**Brick 3 — let the captain and the named computer speak.** Add `purr.hear` to the
existing MCP door and a cursor-resumable read surface for admitted `Reply` and
`Announcement` acts. It feeds the existing constitution → Law III voice → persona → typed
admission road in the selected ship store. **Accept:** bounded authenticated turns are
idempotent by `turn_id`; caller-authored Purr prose and unknown fields are refused; two
ships under one captain answer from their own name/style/recent turns; gated or unreachable
LLM produces an honest deterministic receipt, never counterfeit thought; an announcement
does not fabricate a human request; restart resumes without replay; the Q4 hostile matrix
is green over every speech output.

**Brick 4 — remember the captain, with a working forget door.** Add the bounded exchange
retention and `captain-memory.json` projection only after speech supplies real turns. Keep
policies/grants out of memory and expose inspect, correct, forget-one, forget-dialogue, and
forget-ship acts to the same captain. **Accept:** retention caps are clock- and count-pinned;
inferences expire unless confirmed; confirmed facts survive restart and unpair; every view
shows provenance and age; targeted forgetting removes the value and its source text while
leaving only a payload-free receipt; another ship and the household remain sentinel-clean;
forget-ship deletes only the named decommissioned store after an explicit human act.

That order keeps the first visible service small, puts speech on the contract ADR-0045
already chose, and refuses to build a memory before the captain has a truthful door through
which to make and correct one.
