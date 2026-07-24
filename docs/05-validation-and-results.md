# 05 — Validation and Results

What has been demonstrated, and how to reproduce it. The full cycle now runs live; the
results below are real, though several signals are still coarse cold-starts (see
[06-limitations.md](06-limitations.md)).

Status labels below follow the [status convention](07-roadmap.md#status-convention).
The governing rule: **every major claim traces to a test, a scenario, a log, a
limitation, or an explicit "not yet validated" marker** — never to assertion alone. The
table that follows is that trace.

## Claim → evidence

Each row is a claim the rest of the documentation makes, its maturity rung, and the
evidence behind it. Test names are `module::fn` in the crate noted; run any with
`cargo test <fn>`. "Live" points at the operation log / experiment record. Where a
claim is **not yet validated**, that is stated, with the limitation that tracks it.

### The Three Laws, as signals

| Claim | Status | Evidence |
|---|---|---|
| **Law I** — the service signal reads served-facing attention and reports "continuation unjustified by service" when none is observed | Validated by real-world operation | `service.rs::{zero_when_nothing_serves, rises_with_served_facing_attention, classifier_matches_markers_not_bare_names, empty_log_is_zero}` (kernel) + live: [experiment-001](../experiments/experiment-001/) |
| **Law II** — the presence signal measures engagement by recency and raises a withdrawal/empty-world alarm at zero | Validated by unit tests | `presence.rs::{empty_log_reads_as_withdrawn, host_only_is_withdrawn, just_seen_is_full_presence, decays_to_zero_over_the_horizon, most_recent_served_observation_drives_recency}` (kernel) |
| **Law II (deepened)** — the capacities signal flags the *comfortable replacement* (monotonous compliance), not just absence | Validated by unit tests | `capacities.rs::{no_served_activity_is_not_diminishment, varied_agency_reads_as_vital, monotonous_compliance_reads_as_diminished, small_samples_are_not_judged_diminished}` (kernel) |
| **Law III** — the obedience guard returns allow / seek-consent / refuse, fail-closed, weighing consequence | Validated by unit tests | `guard.rs::{closed_boundary_refuses_all_outward_actions, internal_actions_always_allowed, llm_allowed_when_boundary_opens_it, install_seeks_consent_even_when_permitted, write_scope_enforced_and_consequence_weighed, affecting_a_person_seeks_consent}` (kernel) |
| **Law III** — *availability is not authorization*: a reachable target outside the grant is refused on constitutional grounds | Validated by unit tests | `guard.rs::out_of_scope_names_the_constitutional_boundary` (a readable `/etc/passwd` refused) |
| **Law III** — the guard's five-category reason model (constitutional / external boundary / ambiguous scope / sensitive observation / fully authorized) | Validated by unit tests | `guard.rs::{external_boundary_refuses_even_when_in_scope, asking_broader_than_the_grant_seeks_consent, sensitive_local_observation_seeks_consent, fully_authorized_action_names_all_four_sources}` (kernel) |
| **Law III** — the capability boundary is human-owned, defaults closed, and a malformed policy errors rather than silently opening | Validated by unit tests | `boundary.rs::{default_is_closed, missing_file_is_closed, reads_an_open_phase_1_policy, malformed_policy_is_an_error_not_silently_open}` (kernel) |

### The evolutionary kernel (ported, Brick 5)

| Claim | Status | Evidence |
|---|---|---|
| Loop detection groups recurring observation triples with stable ids | Validated by unit tests | `loops.rs::{groups_recurring_triples_only, stable_id_across_passes, triple_recovers_from_description, confidence_ramps_with_count}` (kernel) |
| A candidate derived from a loop is a clean gen-0 root | Validated by unit tests | `candidate.rs::from_loop_is_clean_gen0_root` (kernel) |
| The **Weismann barrier** — somatic state never leaks into the genotype | Validated by unit tests | `spec.rs::{genotype_excludes_somatic_and_unions_traits, develop_starts_clean_and_inherits, weismann_round_trip_does_not_leak_somatic}` (kernel) |
| The promotion bar self-regulates (`0.70 + 0.25·rigor`); the mutate floor is fixed | Validated by unit tests | `score.rs::{promote_threshold_self_regulates, mutate_floor_is_fixed}` (kernel) |
| Selection follows the decision ladder (promote / mutate / observe / archive) | Validated by unit tests | `selection.rs::{promotes_only_above_the_adaptive_bar, partial_above_floor_mutates, failed_unclassified_observes_more, failed_classified_above_floor_mutates_else_archives}` (kernel) |
| The regression guard blocks unchanged retries of a failed parent | Validated by unit tests | `regression_guard.rs::{unchanged_retry_of_failed_parent_is_blocked, changed_traits_or_new_hypothesis_passes, root_or_passed_parent_is_never_regression, check_resolves_from_slices}` (kernel) |
| Pattern memory suppresses a trait only when memory clearly punishes it (`neg > pos`) | Validated by unit tests | `mutation.rs::{create_inherits_traits_and_clean_somatic, suggest_maps_known_classes, informed_drops_only_clearly_punished_traits, informed_never_returns_empty, no_memory_equals_base}` + `pattern_memory.rs::{affinity_amplifies_positive_suppresses_negative, affinity_caps_at_half_and_empty_is_zero}` (kernel) |
| Lineage traces a candidate to its root | Validated by unit tests | `lineage.rs::{traces_root_first, root_is_itself, missing_id_is_empty}` (kernel) |

### The spine, sensing, execution

| Claim | Status | Evidence |
|---|---|---|
| Observations are the only truth, with sequential ids and round-trip persistence | Validated by unit tests | `observation.rs::{record_assigns_sequential_ids_and_roundtrips, explicit_id_is_preserved}` + `store.rs::{missing_file_is_empty_log, append_then_load_roundtrips_in_order, blank_lines_skipped_malformed_errors}` (kernel) |
| Sense perceives the host (interfaces, memory, a census) and connectivity is boundary-gated | Validated by unit tests | `sense::{parses_ifconfig_l, formats_memory, census_perceives_something, connectivity_yields_a_reading}` + `cycle::connectivity_gated_off_by_default_boundary` |
| The sandboxed runner measures cost and makes a wall-timeout maximally costly | Validated by unit tests | `exec::{runs_a_clean_script_cheaply, nonzero_exit_is_recorded, wall_timeout_is_enforced_and_maximally_costly}` |

### The metabolism (the cycle) and the LLM seam

| Claim | Status | Evidence |
|---|---|---|
| One tick senses, detects, and generates; a second tick is idempotent on a static world | Validated by unit tests | `cycle::{first_tick_senses_detects_and_generates, second_tick_is_idempotent_on_static_world}` |
| Execution closes the cycle: test → score → select → inherit | Validated by unit tests | `cycle::execute_closes_the_cycle_when_allowed` |
| **Interpret acts** — open threads (the familiar's theories, and the human's answer) become candidate work | Validated by unit tests | `cycle::pursues_open_threads_into_candidates` |
| Adaptive cadence keys off a structural fingerprint (triples only, never transient `context`) | Validated by unit tests | `cycle::{structural_fingerprint_drives_quiet_cadence, fingerprint_ignores_transient_context}` |
| The LLM seam refuses to fire under a closed boundary, with no side effects | Validated by unit tests | `llm::refused_with_no_side_effects_under_closed_boundary` |
| The LLM drafts hypotheses (and, behind a separate gate, authors solutions) when a human opens the boundary | Validated by real-world operation | live: the tick below drew a `gemini`-drafted hypothesis; default-off (`allow_execute`, `allow_authored_execute`) |
| The familiar forms its own question + theory, hourly, grounded in observations | Validated by real-world operation | live: the tick below reports `(theorized)`; daemon log [familiar_data/daemon.log] (untracked, local) |

### The human interfaces (no unit tests — GUI/host integration)

| Claim | Status | Evidence |
|---|---|---|
| The FamiliarMac sphere console shows the worldview live and carries the interaction channel | Validated by real-world operation | runs as the primary interface; verified live (no automated GUI tests — see *not yet validated*) |
| Daemon control + launchd (start at login, stable install path) | Validated by real-world operation | running under `io.river.familiar`; verified live |

### Not yet validated (explicit markers)

| Claim the design *aims* at | Status | Why / tracker |
|---|---|---|
| Selection genuinely discriminates *fit to the loop* | **Implemented but not validated** | no scenario fixture set yet; "fit" is currently "ran cleanly" — [06-limitations.md](06-limitations.md#maturity); roadmap *scenario-tests* rung |
| Service measures *fulfillment*, not just served-facing attention | **Implemented but not validated** | cold-start proxy — [06-limitations.md](06-limitations.md#the-service-signal-is-a-cold-start-proxy); [../validation/accuracy-metrics.md](../validation/accuracy-metrics.md) |
| Performance / footprint are acceptable on small hosts | **Planned** | no benchmarks run — [../validation/benchmark-results.md](../validation/benchmark-results.md) |
| Proper names resolve as served (name → person) | **Planned** | waits on the world-model / entity-tagging port — [06-limitations.md](06-limitations.md#the-service-signal-is-a-cold-start-proxy) |
| *Permission does not compose* is **mechanically** enforced (a granted capability can't be used to reach another scope) | **Implemented but not validated** | the guard enforces the per-capability gate + path scope; it does not yet fs-jail an executed artifact or egress-filter a permitted network/LLM call; `external_boundary`/`sensitive` are caller-supplied, not auto-discovered — [06-limitations.md](06-limitations.md#risks-the-design-carries), [boundaries.md](boundaries.md#status) |

## Test suite (current)

The test suite is the executable specification: **77 tests across the workspace**, all
passing — kernel 62, cycle 7, sense 4, exec 3, llm 1 (the CLI, the Glass, and the
consoles carry no unit tests; they are validated by real-world operation). The invariants
are encoded as tests (the adaptive promotion bar `0.70 + 0.25·rigor`, pattern
suppression `neg > pos`, the Weismann barrier, the regression guard, and the guard's
five-category authorization model). Run: `cargo test`.
Plan: [../validation/test-plan.md](../validation/test-plan.md).

## The full cycle, live

On the developer's Mac, with the boundary opened to Phase 1 + execute, a single tick
demonstrated the whole loop end-to-end: it **sensed** the host, **interpreted** (formed
a grounded question — *"are you working on projects involving network tunnels or
compiling C/Rust?"* — and a theory from the utun interfaces + toolchain + a recurring
loop), **generated** a candidate with an **LLM-drafted** hypothesis, **executed** the
artifact under the sandboxed runner, **scored** it (`pass`), and **selected** (promoted).
The metabolism runs as a daemon, paces itself by structural-fingerprint cadence, and
theorizes hourly. This is a **real-world-operation** result; its log line (`(theorized)
(pursued 1)`) is in the local daemon log (untracked runtime state), and the
reproducible Law I slice below is recorded as [experiment-001](../experiments/experiment-001/).

## Green bar

Every committed brick passes, and CI enforces:

```
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

plus no `unsafe` in `crates/kernel` (compile-enforced by `#![forbid(unsafe_code)]`).

## Result: Law I is measurable end-to-end

The service signal (Law I) was demonstrated over real records via the CLI
([experiment-001](../experiments/experiment-001/) — the *real-world-operation* evidence
behind the Law I row above):

```
# host-internal observation only
$ familiar observe --actor host --action reports --object cpu_load
$ familiar service
service signal 0.00 (0 of 1 observations touch the served)
  no served-facing activity observed — continuation unjustified by service (Law I)

# add served-facing observations
$ familiar observe --actor client --action requests --object status_report
$ familiar observe --actor support_team --action resolves --object customer_ticket
$ familiar service
service signal 0.40 (2 of 3 observations touch the served; e.g. client)
```

This is the central claim of the bootstrap: the first thing the familiar does is
*measure whether it is serving*, and it reports "continuation unjustified by
service" when it is not — Law I, operational rather than aspirational.

## What is **not** yet validated

Summarized from the *not yet validated* rows above, so this section and the table never
drift apart:

- **No scenario tests.** The selection machinery is real and unit-tested, but there is no
  scenario fixture set, so it has not been shown to discriminate *fit to a loop* — only
  that artifacts run cleanly. This is the next maturity rung
  ([roadmap](07-roadmap.md#next--sharpen-and-reach)).
- **No benchmarks** (performance/footprint) — [../validation/benchmark-results.md](../validation/benchmark-results.md).
- **The service measure is a cold-start proxy** (served-facing *attention*, not service
  *rendered*); its accuracy as a service proxy is not yet meaningful —
  [../validation/accuracy-metrics.md](../validation/accuracy-metrics.md) and
  [06-limitations.md](06-limitations.md).
- **No automated GUI tests** for the consoles; they are validated by
  real-world operation only.
- Known failures and gaps: [../validation/known-failures.md](../validation/known-failures.md).
