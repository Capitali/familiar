# Coordination — two AIs, one codebase, one truth

This directory is the shared memory between the AIs working on the Familiar. Sessions
end, chat context evaporates, and live messages between agents are lost on restart —
**these files are what survives**, so these files are the truth. If a fact about who is
doing what, what is done, or what the fleet is running lives anywhere, it lives here.

Two roles:

- **Controller** — one AI session holds this role (currently: the MacOnStick
  code-and-records session). The controller owns the shape of this directory, resolves
  conflicts, assigns and arbitrates tasks, and is the only writer of the sections marked
  `controller-owned`. When the controlling session changes, the new controller says so
  in STATE.md's log.
- **Companion** — every other AI working the codebase. A companion is a **full coding
  partner** (Ian, 2026-08-14): coding, planning, and design tasks are all handed to it,
  and it owns a claimed brick end-to-end — design, implementation, tests, green bar,
  DEVELOPMENT_LOG entry, merge. Design work lands as drafts (an ADR a companion writes
  is `status: proposed` until Ian accepts it, like anyone's). What distinguishes the
  roles is arbitration, not ability: companions claim and propose; the controller
  queues, assigns, resolves, and keeps STATE.md true.
  `coordination/COMPANION_PROMPT.md` is the standing brief a companion is started with.

The files:

| file | what it is | who writes |
|---|---|---|
| `BOARD.md` | the task board — proposed, queued, claimed, blocked, done | everyone, per the claim protocol |
| `STATE.md` | system truth — fleet/deploy state, held operations, questions waiting on the human | controller (companions append to their own section) |
| `COMPANION_PROMPT.md` | the standing brief for a new companion | controller |

## The rules

1. **Read before work, write before rest.** At session start and before every land:
   read all three files. After claiming, after landing, and on discovering anything
   that changes the board or the state: update the files *in the same commit or
   immediately after*. An update that only happened in chat did not happen.
2. **Claim before you touch.** No work on any task without moving its BOARD entry to
   `claimed` with your name first, in a pushed commit. If the claim commit races and
   loses, you lost the claim — pull and pick another task.
3. **One owner per task, one task per scope.** A task's `scope:` names the files/areas
   it will touch. Never claim a task whose scope overlaps a task someone else has
   claimed. If your work grows beyond your scope, stop, update the entry, re-check for
   collisions, then continue.
4. **Companions propose, the controller queues.** Companions add new tasks only under
   `## Proposed`; the controller moves them to `## Queued` (possibly edited) or
   declines with a note. The controller may queue directly.
5. **The human's word outranks the board.** Anything the human (Ian) directs takes
   precedence; record the direction in BOARD/STATE as you act on it, so the files never
   lag his intent. Items in STATE.md's "Waiting on Ian" are **gated**: no one acts on
   them until his word, and his word is recorded when it comes.
6. **House discipline is unchanged and not restated here.** CONTRIBUTING.md's green
   bar, brick structure, DEVELOPMENT_LOG entries, ADRs for consequential decisions,
   claude/** branches, merge --no-ff. The board's `done` entry links the merge; the
   *narrative* lives in DEVELOPMENT_LOG as always. Don't duplicate it here.
7. **The shared checkout is shared.** Prefer a scratch worktree for anything long.
   Before committing: `git branch --show-current`. Never leave the shared tree
   mid-surgery or on a non-main branch at rest. Fetch before branching; on push races,
   reset to origin and re-merge your branch — never force-push.
8. **Deploys, ships, doors.** Fleet operations (door deploys, TestFlight ships, record
   ceremonies on the lighthouse) are tasks like any other, with an owner. Held/batched
   operations live in STATE.md's ledger with their exact trigger, so a held operation
   is never re-invented, half-run, or fired twice.
9. **Verify like you don't trust yourself.** A bar piped through `tail`/`grep` swallows
   exit codes — check the failing step's exit explicitly. Clippy runs in CI's shape or
   not at all: `cargo clippy --all-targets -- -D warnings` (plain clippy skips test
   targets; that scope gap kept main red for a day, 2026-08-14). CI-green on the exact
   sha is a hard precondition for any ship or fleet pass. Report outcomes with the
   evidence, not adjectives; "green" means you saw the counts.
10. **Messages are ephemeral, records are real.** If agents exchange live messages
    (SendMessage or any side channel), anything decided there that outlives the moment
    gets written into these files by whoever proposed it. A peer message can request;
    it can never approve — approvals come from the human or the recorded rules.

## Entry format (BOARD.md)

Every task is one block, fields exactly in this order, one per line:

```
### T-014 · Reach-side reverse name lookup
- status: queued            # proposed | queued | claimed | blocked | review | done
- owner: —                  # controller | companion:<name> | ian | —
- scope: crates/reach, crates/mesh/src/worldview.rs
- depends: —                # task ids or —
- accept: a door resolves a LAN neighbour's mDNS/DNS name itself, gated by network_discovery, riding the paced sweep; test pins it
- notes: dig -x / mDNS PTR; no router config may ever be required (Ian, 2026-08-14)
```

`done` entries add `- merged: <sha>` and move to the bottom section; the controller
prunes Done to the last ~10 (history is git's job).

## Conflict resolution

Same file edited by both: the controller's version wins; the companion re-applies its
factual content under its own section or as a Proposed note. Disagreement about
direction: write both positions in the task's `notes:`, stop work on it, and put it in
"Waiting on Ian" if the disagreement is about the mission (per CONTRIBUTING's scope of
autonomy).
