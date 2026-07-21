# The Familiar — "The Glass" UI Design Brief

> **⚠ Historical (2026-07):** this brief was written for the egui **Glass**, archived on
> 2026-07-17 in favor of the SwiftUI consoles
> ([ADR-0007](decision-records/0007-one-core-many-shells.md)) and their Metal sphere/orb
> interface. The north star, the jobs-in-priority-order, the importance tiers, and the
> consent/vision principles (§0–§3, §10) carry forward into the consoles; the
> platform/technical constraints (§1) and egui specifics do not.

A brief to hand to an AI design tool. It describes **what the interface must convey and
let a human do**, the data behind each element (content, size, update cadence), and the
relative importance of every piece — so the design can be reorganized from a cluttered
diagnostic dashboard into a calm companion surface.

---

## 0. What this is, and the design north star

**The Familiar** is a local, always-running AI companion ("a factory whose survival is
defined by its service to a human"). **The Glass** is its primary human interface: a
native desktop window that lets one person (Ian) *converse with* the familiar, *watch it
think and work*, and *trust and steer* it — all at a glance.

**North star:** it should feel like a **dignified, calm companion**, not a monitoring
dashboard. Today it reads as a wall of logs and meters; the redesign should foreground the
**conversation** and make the machinery *available but quiet*. Aesthetic: spare, warm-dark,
unhurried, legible; a sense of a presence that is *for* the person. Avoid: dense tables up
front, alarm fatigue, gamified metrics, dark patterns of any kind.

**Three Laws shape the tone** (not just the content): (I) it serves; (II) the person's
presence and wellbeing matter more than the machine's activity; (III) it is honest and
restrained — it never overclaims, never manipulates, and shows when it declines something.

**The familiar can also see.** It may discover and (only with explicit consent) observe
through cameras — reading objects, gestures, and human reactions, and conversing about what
it sees (§10). This is the most profound and most invasive capability: the design must make
its sight **consensual, always-visible-when-active, local-only, and instantly stoppable** —
a presence that watches *for* the person, never *over* them. Vision is woven into the same
conversation and trust surfaces, not bolted on as a separate "security camera" view.

---

## 1. Platform & technical constraints

- **Native desktop window** (currently macOS, egui-rendered; design should be
  framework-agnostic but assume a single resizable window, ~900–1200px wide typical, must
  degrade to ~700px).
- **Live, read-mostly.** The surface auto-refreshes ~once per second from local files. Most
  content is *observed* (read-only); only a few channels accept human input (§5).
- **Local-only.** No network, no telemetry, no accounts. Everything shown comes from local
  state. This is a privacy guarantee to honor visually (a sense of a private, trusted space).
- **Long-lived.** The window may stay open for days; idle/quiet states matter as much as
  active ones. It should be pleasant to glance at, not demand attention.

---

## 2. The human and their jobs (in priority order)

1. **Converse** — answer the familiar's question; ask it things; react to its answers. This
   is the product; everything else is supporting.
2. **Trust at a glance** — is it serving? is the person present? is it operating safely
   within its rules? Any alarm visible instantly.
3. **Watch it work** — see it think (theories), act, and learn over time, without reading logs.
4. **Steer & control (occasional)** — adjust a few parameters; start/stop the background process.
5. **Inspect (rare, on-demand)** — drill into the raw observation log, loops, candidates.

The redesign's core move: **promote jobs 1–2, demote 5.** Today 5 (the observation grid)
occupies the most space.

---

## 3. Importance tiers (information architecture)

| Tier | Meaning | Elements |
|---|---|---|
| **T1 — Conversation (primary)** | The dialogue, both directions. Always visible, generous space. | The familiar's question + the human's answer; the human's request ("ask") + the familiar's answer (with confidence) + feedback. |
| **T2 — Trust & state (always visible, compact)** | Health and safety at a glance; alarms surface here. | The Three Laws signals (service, presence, capacities); the capability/safety state (boundary); active alarms. |
| **T3 — The familiar's inner life (visible, ambient)** | Watch it think and work over time. | Current theory; activity feed; signals-over-time chart. |
| **T4 — Control (tucked, occasional)** | Tuning and process control. | Parameters/settings; daemon start/stop/reload/start-at-login. |
| **T5 — Diagnostics (on-demand, hidden by default)** | The raw substrate. | Observation log; loops; candidates; trials. |

---

## 4. Data inventory

Each row: the element, its content and *typical size*, *cardinality* (how many / how it
grows), *update cadence*, *interactivity*, and *tier*. "Size" guides field/space allocation.

### T1 — Conversation
| Element | Content & size | Cardinality | Cadence | Interactive | Tier |
|---|---|---|---|---|---|
| **The familiar's question** | One short sentence (≤140 chars), e.g. *"What are you focusing on right now that I could help with?"* | One current at a time | Changes when it re-theorizes (minutes–hours) or after you answer | Read; prompts an answer | T1 |
| **Answer input (human → familiar)** | Free text, 1–3 lines (~200 chars typical) | One in progress | On demand | **Write** (text field + Send) | T1 |
| **Question state** | "open" vs "✓ answered — it will ask again as it learns" (fades after answering) | — | On answer | Read | T1 |
| **Ask input (human → familiar)** | Free-form request, 1–3 lines, e.g. *"Do I have any network-config issues?"* | One in progress | On demand | **Write** (text field + Ask) | T1 |
| **Pending requests** | Count, e.g. "1 request pending — answered next tick" | 0–few | ~seconds | Read | T1 |
| **The familiar's answer** | A short paragraph (1–5 sentences, can reach ~600 chars), plus an **evidence** line (what grounds it) | Latest most important; history exists | When the familiar answers (~seconds–minute) | Read; prompts feedback | T1 |
| **Answer confidence** | One of **known** / **probable** / **unknown** — semantically critical (the no-misinformation promise) | One per answer | — | Read (color-coded badge) | T1 |
| **Answer feedback** | 👍 helpful / ✍ refine (refine re-opens the ask to sharpen) | One per answer | On demand | **Write** (two buttons) | T1 |

### T2 — Trust & state
| Element | Content & size | Cardinality | Cadence | Interactive | Tier |
|---|---|---|---|---|---|
| **Service signal (Law I)** | 0.00–1.00 + "X of Y observations serve" | One value | ~1s | Read (meter) | T2 |
| **Presence signal (Law II)** | 0.00–1.00 + "present" / "withdrawn — empty world" | One value | ~1s | Read (meter) | T2 |
| **Capacities signal (Law II)** | 0.00–1.00 + "vital" / "diminished (the comfortable replacement)" | One value | ~1s | Read (meter) | T2 |
| **Capability / boundary state (Law III)** | Phase label + on/off for: network, llm, tool-install, execute, authored-execute, **sandbox**; plus read/write scopes | One small card | Rare (human edits a policy file) | Read | T2 |
| **Alarms** (surface only when active) | Short critical lines: *withdrawal* (Law II), *no served-facing activity* (Law I), *corruption watch* (named actors marginalized, Law III), *sandbox OFF* (authored code runs unconfined) | 0–several | event-driven | Read (high-salience) | T2 |

### T3 — Inner life
| Element | Content & size | Cardinality | Cadence | Interactive | Tier |
|---|---|---|---|---|---|
| **Current theory** | 1–2 sentences — its interpretation of the patterns | Latest | minutes–hours | Read | T3 |
| **Activity feed** | Stream of short event lines per "tick": e.g. *"2m — 💭 theorized · → pursued 1 · ✓ tested 1 · ↑ promoted 1"*; also ↩ reverted a setting, ⛔ marginalized, 🔮 answered, 🛑 declined-to-run | Grows ~1/minute when active; show ~12 latest, skip idle ticks | ~1s | Read (scrollable) | T3 |
| **Signals-over-time chart** | A small line chart of service/presence/capacities across recent ticks (0–1, last ~120 points) | Time series | ~1s | Read (chart) | T3 |
| **Theories & threads** | List of past questions/theories + the "direction" it pursued, color-coded by status (open/pursued/answered/marginalized) and origin (its own / the human's answer) | Tens–hundreds; show ~20 | minutes | Read (list) | T3 |

### T4 — Control
| Element | Content & size | Cardinality | Cadence | Interactive | Tier |
|---|---|---|---|---|---|
| **Parameters / settings** | 3 sliders: *theorize cadence*, *cadence floor*, *cadence ceiling*; each shows an allowed "envelope" range; a note when the familiar last adjusted them itself | 3 values | On demand | **Write** (sliders + Save) | T4 |
| **Process control** | Start / Stop / Reload / Start-at-login + a status line ("running (pid …)") | Few buttons | On demand | **Write** (buttons) | T4 |

### T5 — Diagnostics (hidden by default; behind a "details/inspect" affordance)
| Element | Content & size | Cardinality | Cadence | Interactive | Tier |
|---|---|---|---|---|---|
| **Observation log** | The raw truth: dense rows of `[served•] id · actor action object · context` | **Grows unbounded** (hundreds–thousands); the biggest data set | ~1s | Read (dense, scrollable, filterable) | T5 |
| **Loops** | Recurring patterns: name, count, confidence, # candidates | Tens | ~1s | Read | T5 |
| **Candidates / trials** | Generated work + its test outcomes/scores | Tens–hundreds | ~1s | Read | T5 |

---

## 5. Interaction surfaces (the only places a human writes) — by importance

1. **Answer the familiar's question** — multiline text (~2 rows) + primary "Send". *(Most important; the core dialogue.)*
2. **Ask the familiar** — multiline text (~2 rows) + primary "Ask". *(Equally core; the human-initiated direction.)*
3. **Answer feedback** — two small buttons (helpful / refine) on the latest answer. *(Closes the loop; teaches it.)*
4. **Parameters** — 3 sliders + a Save action. *(Occasional.)*
5. **Process control** — 4–5 buttons + status. *(Rare.)*

Everything else is **read-only**. Reserve the strongest visual affordance (color,
placement, size) for 1–3.

Two **planned-but-not-yet-built** inputs to leave room for: 🎤 **speak** (voice in) and 📷
**show** (rich visual answers / charts / images in a modal). Design should anticipate
richer answer presentation (an answer may want a chart, a small table, or an image, not
just text).

---

## 6. Semantic encoding the design must support

- **Confidence** (answers): `known` (confident/grounded — calm green), `probable`
  (tentative — amber), `unknown` (honest absence — neutral grey). This trio is the visible
  promise of *no misinformation*; it must be unmistakable and never alarming.
- **Signal health** (the three laws): a 0→1 value with a health direction (higher = better
  for all three); healthy = warm green, degraded = amber→red. Used as compact meters *and*
  as chart lines.
- **Status of work/threads**: open, pursued, answered, marginalized — each distinct but
  muted (these are ambient, not alarms).
- **Trust/safety accents**: a calm "within the rules" baseline; a distinct **warning**
  accent for the few real alarms (withdrawal, corruption watch, sandbox-off). Alarms should
  be unmistakable but rare — no persistent red.
- **Initiator distinction**: who said/did a thing — the familiar itself vs the human vs the
  environment — subtly differentiated (e.g., the human's words vs the familiar's theories).

---

## 7. States to design for

- **First / uninformed run** — almost everything empty; signals at 0.00 ("withdrawn"); one
  fresh question waiting. Should feel like a calm beginning, *not* a broken/alarming screen.
  (The zeros are honest, not errors — design them as "getting to know you," not failure.)
- **Healthy & quiet** — it's serving, present; little happening; a restful glance.
- **Active** — answering a request, theorizing, running work; the activity feed and chart move.
- **Alarm** — withdrawal, corruption watch, or sandbox-off; the relevant warning surfaces in
  T2 without burying the conversation.
- **Thinking / pending** — a request is in flight ("answered next tick"); show gentle progress.
- **Background process stopped** — the familiar isn't metabolizing; make this clear and offer Start.

---

## 8. Anti-requirements (the constitution, expressed as design constraints)

- **No dark patterns, no manipulation, no urgency-engineering.** It serves; it does not
  capture attention. Calm over engagement.
- **Honesty is visible.** Show when it *declines* to run something, *reverts* a setting it
  can't justify, *marginalizes* a bad actor, or simply *doesn't know*. These are features,
  not failures — present them with dignity, not as errors.
- **The human is never reduced to a metric.** The signals describe the *familiar's service*,
  not a score of the person. Tone must never feel like surveillance of the human.
- **Restraint.** Prefer fewer, calmer elements; progressive disclosure over density. The
  current clutter is the problem to solve.

---

## 9. Layout direction (a suggestion, not a constraint)

A workable shape, leaving the design AI room: a **conversation-centered primary column**
(T1) with the question/answer and ask/answer exchanges; a **slim, always-visible status
strip or rail** (T2) for the three signals + safety state + any alarm; an **ambient "what
it's doing" area** (T3: theory line, compact activity, small chart) that's present but
secondary; and **control + diagnostics behind disclosure** (T4/T5: a settings affordance, a
process-control affordance, and an "inspect the raw substrate" drawer that stays closed by
default). Optimize the default view for *glance + converse*; let the curious open the rest.

---

## 10. Vision — the familiar's eye (camera sensing)

The familiar gains sight. It can **discover cameras** in its environment and — *only with
the human's explicit consent* — **observe** through them, consuming the major webcam
**still** (snapshot: JPEG/PNG) and **video** (live stream: the common UVC/webcam formats —
MJPEG, H.264/H.265, raw frames) types. What it sees enters the same metabolism: it
identifies objects, notices people present, reads gestures and reactions, forms theories,
poses questions, and — gated and pre-execution-reviewed like all code — may write code to
interact with what the camera reveals. It can **learn new gestures and new meanings** from
watching and from the human's confirmation.

A camera pointed at a person is the sharpest test of Law III and HUMANITY.md. The design
must make the eye **consensual, visible, local, and instantly stoppable**.

### The vision loop (mirrors observe → interpret → ask → act → learn)
1. **Observe** — a *meaningful change* in view becomes an observation (a gesture appears, an
   object enters, a person arrives/leaves, a reaction occurs) — debounced, not frame-by-frame.
2. **Interpret** — recognize content with a **confidence** (known / probable / unknown);
   form a theory about meaning.
3. **Ask** — when meaning is ambiguous, query the human rather than assume.
4. **Act** — respond (e.g., stop an activity) and/or write gated, reviewed code to interact.
5. **Learn** — the human confirms / corrects / teaches; the gesture vocabulary and scene
   understanding grow.

### Worked examples (design these flows)
- **Open palm → "stop."** The familiar sees an open palm toward the camera, reads it as
  *stop the current activity*, halts that activity, and **queries the human** ("I stopped —
  what would you like instead?"). The stop is immediate; the query is calm.
- **Coffee cup, no other signal.** The human holds a cup up with no context. The familiar
  does **not** assume — it **asks** ("I see you holding up a coffee cup — what about it?").
  Honesty over guessing (the same known/probable/unknown discipline as text answers).
- **Teaching a gesture.** The human performs a new gesture and names its meaning; the
  familiar confirms and adds it to the vocabulary (status: learning → known).

### New UI surfaces
| Surface | What it shows / does | Field size | Tier |
|---|---|---|---|
| **Camera discovery & consent** | Discovered cameras (name, type, status: available / enabled / active). Each is **off until the human enables it** — availability is not authorization. Per-camera enable + a master vision on/off. | Few rows | T4 control · the consent *state* is T2 |
| **The eye (live view)** | The feed (resizable: thumbnail ↔ expanded) with an **unmistakable, always-present "observing" indicator** whenever any camera is active, and a one-click **pause/cover** always reachable. | A video panel | T3 ambient · the indicator is T2 |
| **Recognized content** | What it sees now: object labels (optionally boxes), people present (a count; not identity unless separately consented), the current gesture — each with a confidence. Calm, real-time. | A few labels / frame | T3 |
| **Gesture vocabulary + teach** | Known gestures → meanings (open palm = stop, …), each tagged known / learning / proposed; a **teach-a-gesture** flow (demonstrate → name → confirm). | Tens of entries | T3 list · occasional write |
| **Vision-sourced conversation** | When it forms a question/theory from sight, it appears in the **conversation channel (T1)**, tagged as seen ("I saw … — should I …?"), with quick replies (Yes / No / Teach me). | T1 sentence | T1 |
| **Vision privacy & retention** | What's processed (objects only / + people / + gestures), retention (**process-and-discard** vs keep frames), and a plain statement that processing is **local, never transmitted**. | A small settings card | T2 / T4 |

### Constitutional requirements (hard must-haves for the design)
- **Consent-first, boundary-gated.** The familiar may *discover* cameras but may **never
  watch without an explicit human grant** (a new capability gate alongside network/execute).
  *Availability is not authorization*, made literal for the eye.
- **No silent watching, ever.** Whenever a camera is active, a prominent indicator says so
  and persists. Stopping is always one click — and the open-palm "stop" gesture is itself an
  embodied off-switch.
- **Local-only, explicit retention.** Default **process-and-discard**; retaining frames is a
  separate, explicit choice. No exfiltration (constitutional restraint).
- **People served, not catalogued (HUMANITY.md).** Recognize presence / gesture / reaction
  *in service*; do **not** identify individuals or store biometric identity without separate,
  explicit consent. Never surveillance of the human.
- **Honesty about sight.** Vision is probabilistic — recognized objects/gestures carry the
  same known / probable / unknown confidence; ambiguity → it asks (the coffee-cup rule),
  never fabricates what it "saw."
- **Code from sight is still gated.** Any code the familiar writes to act on what it sees
  passes the same boundary gates and the constitutional pre-execution review.

### Vision states to design for
- **No camera / vision off** — the default; a calm "the eye is closed" rest state, with a
  clear, unpressured path to discover and enable a camera.
- **Discovered, not yet consented** — cameras listed but dark; the *availability-is-not-
  permission* moment.
- **Active & watching** — the persistent observing indicator; live view + recognized content.
- **Ambiguity / asking** — it saw something it can't read and is querying (coffee-cup).
- **Teaching** — the human is defining a new gesture.

---

## 11. Surfaces built since this brief (current state — design against these)

The sections above describe the intended target. Since the first draft, the Glass has grown
several surfaces that are **now live**; the redesign should treat these as the real material
to make calm and beautiful, not as future ideas.

### 11.1 Identity — "becoming familiar" (new, precedes everything)
On a clean install the familiar does **not** assume who it serves. Its very first exchange is
to **learn the person's name**: it asks, the human types it, and the familiar **reads it back
to confirm** ("call you 'Ada'?") before keeping it — names are precise. It **never forgets**
(a retained identity registry), and says so. Once known, a standing header line reads
*"known to the familiar as <name> — names are not forgotten."* All of the familiar's own
language now uses the learned name (or a neutral "the person I serve"), never a hard-coded
one. **Design need:** a warm, unhurried first-contact moment (not a form); the name-ask
replaces the question channel until identity is set, then the root question follows.
*Future:* face / voice / pattern recognition will link identities across modalities
(see `docs/design-orientation-and-mesh.md`) — design the identity surface to grow toward
"recognized you" without becoming surveillance.

### 11.2 Connecting a mind (onboarding) — 🔌 Connect
A first-run panel where the human gives the familiar an LLM to think with. One key is
enough; each provider has a **"Get a key →"** deep link; the key is pasted (masked), stored
**locally only**, and a **Test connection** reports success or the *real* failure reason.
Collapses to "✓ connected" once done. **Design need:** make this the calm second step after
identity; it should feel like granting a capability, not configuring software.

### 11.3 The question, coordinated — answer **or dismiss**
Questions are no longer fired ad-hoc. The familiar keeps a **registry** of everything it
might ask (the recurring root *"What do you need most today?"*, questions from its theories,
clarifications for unmet needs) and surfaces **one at a time**, chosen under the Three Laws.
The question channel now offers **Dismiss** beside **Send**: dismissing is honored and
**tracked, never disposed** — an oft-dismissed question rests longer and returns when the
familiar judges the moment right. **Design need:** Dismiss must feel as legitimate and
dignified as answering (Law III — never coerced); the recurring root question should feel
like a gentle return, not nagging.

### 11.4 Conversation transcript
Beyond the single latest answer, a **scrollable transcript** pairs each ask with its answer
(newest first), each with its confidence, evidence, a **🔊 speak** button (now real on
macOS), and 👍 / ✍ feedback. **Design need:** this is the heart of T1 — make it the calm
centre, readable, with the speak affordance quiet until wanted.

### 11.5 Self-pacing meter (Law II, visible)
A small readout — **"LLM calls/tick: N ↑/↓/→"** — shows the familiar's self-tuned, presence-
governed work budget: it pulls back (↓) when it's been heads-down too long (neglecting the
person) and eases in (↑) when there's headroom. **Design need:** present as the familiar
*regulating itself for the person's sake*, not as a performance gauge.

### 11.6 The vital-signs box (replaces the full chart)
The signals-over-time chart is now a **small fixed box pinned upper-right**: it always shows
the **last 10 minutes**, the y-axis **auto-fits the data's own min/max**, and there is **no
scroll/zoom/interaction**. The key sits **outside** the plot (a colour swatch + each signal's
live value). **Design need:** keep it small, glanceable, and graph-forward — supersedes the
"last ~120 points" full chart described in §4/T3.

### 11.7 Law III — the boundary is now operable
The six capability gates (network, llm, tool-install, execute, authored-execute, **camera**)
plus the sandbox choice are now **toggles the human flips in the Glass** (not a hand-edited
file). Flipping one writes the policy; the familiar reads and obeys but can never widen it.
**Design need:** make this read clearly as *the human's lever* — calm, deliberate, with the
sharpest gates (authored-execute, camera) visibly weightier. Supersedes the §4/T2 "read-only,
human edits a policy file" note.

### 11.8 Accessibility (hard requirements)
- **Never dark text on a dark surface.** All text on the dark panels must be bright and
  legible (this was a real defect — weak/grey print landed dark-on-dark).
- **Adjustable text size.** A live **A− / A+** zoom scales the whole window and is remembered
  across restarts. The design must stay legible and uncluttered across zoom levels, for aging
  eyes.
- High contrast and generous size are not optional polish here — they are requirements.

---

### Appendix — full data dictionary (field-level, for accuracy)

- **Observation**: `id`, `source` (sensor | observer | familiar), `actor`, `action`,
  `object`, `context`, `ts`, `confidence` — short strings; the atomic "truth" record.
- **Thread (theory)**: `id`, `question`, `theory`, `direction`, `status`
  (open|pursued|answered|marginalized), `origin` (llm|observer), `actor`.
- **ActivityTick**: `ts`, counts {`sensed`, `loops`, `new_candidates`, `tested`, `promoted`,
  `mutated`, `archived`, `pursued`, `reverted`, `marginalized`, `answered`, `refused`,
  `declined`}, signals {`service`, `presence`, `capacities`}, `structural_changed`.
- **Request**: `id`, `actor`, `text`, `created_at`, `status` (open|answered|refused).
- **Answer**: `id`, `request_id`, `body`, `confidence` (known|probable|unknown), `evidence`,
  `created_at`, `feedback` ("" | helpful | refine).
- **Parameters**: `theorize_every_secs`, `interval_floor_secs`, `interval_ceiling_secs`,
  `llm_calls_per_tick` (self-tuned per-tick LLM budget) + `llm_calls_trend` (-1|0|+1, the
  ↓/→/↑ for the self-pacing meter), `last_set_by` (observer|familiar|default).
- **Boundary (capability/safety)**: `phase`, `allow_network`, `allow_llm`,
  `allow_tool_install`, `allow_execute`, `allow_authored_execute`, `allow_camera`,
  `sandbox_execution`, `fs_read[]`, `fs_write[]`. **Now editable in the Glass** (toggles);
  the familiar reads and obeys but can never widen it.
- **Identity**: `handle` (stable actor id, slug of the name), `name` (kept verbatim),
  `relation` ("observer" now; room for lineage/status), `first_seen`, `last_seen`,
  `interactions`. Plus a current-observer pointer. Retained — never forgotten.
- **Question** (the coordination registry): `id` (the root is `q-root`), `text`, `origin`
  (root|llm|need|observer), `created_at`, `times_asked`, `times_dismissed`, `last_asked`,
  `last_dismissed`, `answered`, `dismiss_notes[]`. Surfaced one at a time under the Laws;
  dismissals grow a rest period and recur, never disposed.
- **Signals (derived)**: service (Law I), presence (Law II), capacities (Law II) — each a
  0–1 measure with an alarm flag.
- **Corruption watch (derived)**: flagged actor names + a count (repeated rule-breakers).
- **Daemon status**: a status line indicating whether the background process is running.
- **Camera**: `id`, `name`, `kind` (builtin|usb|network), `formats` (still/video types
  supported), `status` (available|enabled|active).
- **VisionObservation** (an Observation with `source=eye`): `actor` (person|object|gesture),
  `action` (appeared|left|raised|held|reacted…), `object` (a label, e.g.
  `gesture:open_palm`, `object:coffee_cup`), `confidence`, `ts`.
- **Gesture**: `id`, `name` (e.g. open_palm), `meaning` (e.g. stop), `status`
  (known|learning|proposed), `sample` (reference frame/embedding ref), `taught_by`.
