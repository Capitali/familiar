# The Familiar — Holographic Glass UI Design Brief

A brief to hand to an AI design tool. It describes **what the interface must convey and
let a human do**, the data behind each element (content, size, update cadence), and the
relative importance of every piece. See ADR-0007 for why the presentation layer moved to
a wgpu/egui holographic engine (`crates/hologram`), superseding both the earlier egui
Glass (ADR-0006) and the SwiftUI shells (archived to `archive/ios`).

---

## 0. What this is, and the design north star

**The Familiar** is a local, always-running AI companion ("a factory whose survival is
defined by its service to a human"). **The Glass** is its primary human interface: a
single wgpu/winit window, present on every platform the familiar runs on, that lets one
person (Ian) *converse with* the familiar, *watch it think and work*, and *trust and
steer* it — all at a glance.

**North star:** it should feel like a **visibly alive holographic projection** — a
presence, not a settings panel. Aesthetic: sci-fi holographic (Iron Man / Westworld
register) — semi-transparent panels, chromatic aberration at the edges, procedural
glitch/noise, moving scanlines, a Fresnel glow rim on every panel silhouette, and an
ambient flicker that reads as barely-contained plasma rather than a static frame. The
UI should never sit *still*; the animation itself is part of what tells the person
something is alive and thinking, not just displayed.

**The Three Laws still shape the *content and behavior*** (not the visual register): (I)
it serves; (II) the person's presence and wellbeing matter more than the machine's
activity; (III) it is honest and restrained — it never overclaims, never manipulates, and
shows when it declines something. Those commitments are enforced in what the UI *says and
does* (no fabricated confidence, no coercive prompts, an honest "unknown" when it doesn't
know) — the holographic ornamentation is a presentation choice layered on top, not a
loosening of those commitments.

**The familiar can also see.** It may discover and (only with explicit consent) observe
through cameras — reading objects, gestures, and human reactions, and conversing about what
it sees (§10). This is the most profound and most invasive capability: the design must make
its sight **consensual, always-visible-when-active, local-only, and instantly stoppable** —
a presence that watches *for* the person, never *over* them. Vision is woven into the same
conversation and trust surfaces, not bolted on as a separate "security camera" view.

---

## 1. Platform & technical constraints

- **wgpu/winit window, every platform.** One rendering engine (`crates/hologram`) runs
  identically on macOS, iOS, Linux, and Windows — `wgpu::Backends::METAL` on Apple
  Silicon, Vulkan/DX12/GL elsewhere. No per-platform native shell.
- **Live, animated, read-mostly.** The surface redraws continuously (the holographic
  effects require a per-frame `time` uniform, not a once-a-second refresh); most content
  is *observed* (read-only); only a few channels accept human input (§5).
- **Local-only.** No network, no telemetry, no accounts. Everything shown comes from local
  state. This is a privacy guarantee to honor visually (a sense of a private, trusted space).
- **Long-lived.** The window may stay open for days; idle/quiet states matter as much as
  active ones — even "idle" should keep a low-amplitude flicker/scanline motion so the
  projection never looks frozen or crashed.

---

## 2. The human and their jobs (in priority order)

1. **Converse** — answer the familiar's question; ask it things; react to its answers. This
   is the product; everything else is supporting.
2. **Trust at a glance** — is it serving? is the person present? is it operating safely
   within its rules? Any alarm visible instantly.
3. **Watch it work** — see it think (theories), act, and learn over time, without reading logs.
4. **Steer & control (occasional)** — adjust a few parameters; start/stop the background process.
5. **Inspect (rare, on-demand)** — drill into the raw observation log, loops, candidates.

The redesign's core move: **promote jobs 1–2, demote 5.**

---

## 3. Importance tiers (information architecture)

| Tier | Meaning | Elements |
|---|---|---|
| **T1 — Conversation (primary)** | The dialogue, both directions. Always visible, generous space. | The familiar's question + the human's answer; the human's request ("ask") + the familiar's answer (with confidence) + feedback. |
| **T2 — Trust & state (always visible, compact)** | Health and safety at a glance; alarms surface here. | The Three Laws signals (service, presence, capacities); the capability/safety state (boundary); active alarms. |
| **T3 — The familiar's inner life (visible, ambient)** | Watch it think and work over time. | Current theory; activity feed; signals-over-time chart. |
| **T4 — Control (tucked, occasional)** | Tuning and process control. | Parameters/settings; daemon start/stop/reload/start-at-login. |
| **T5 — Diagnostics (on-demand, hidden by default)** | The raw substrate. | Observation log; loops; candidates; trials. |

(Data inventory, interaction surfaces, semantic encoding, and states-to-design-for from
the prior brief — §4–§7 of the original — carry over unchanged; only the visual register
below replaces the old "spare, warm-dark, calm" direction.)

---

## 8. Visual language (replaces the prior calm/anti-engagement direction)

- **Panels as holographic glass:** semi-transparent panels (`BlendState::ALPHA_BLENDING`
  — light-stacking, not opaque cards) with a **Fresnel glow rim**: brighter at grazing
  silhouette angles, dimmer face-on, so every panel reads as a projected surface with
  depth rather than a flat 2D card.
- **Chromatic aberration:** RGB channels sampled with a small sine-driven UV offset,
  strongest at panel edges and during state transitions (an answer arriving, an alarm
  surfacing) — a signal that something *just happened*, not constant distraction in the
  quiet state.
- **Procedural glitch/noise:** high-frequency vertical-offset noise, low amplitude at
  rest, spiking briefly on transitions (new question, alarm, answer arriving) — glitch as
  *punctuation*, not wallpaper.
- **Moving scanlines:** a continuous horizontal brightness wave, slow and low-contrast at
  rest so it doesn't fight legibility of the T1 conversation text.
- **Ambient flicker:** a combined macro-oscillation on final output brightness, subtle
  enough that body text stays comfortably readable — flicker sells "alive," not "broken."
- **Restraint still applies to *intensity*, not *presence*:** alarms (withdrawal,
  corruption watch, sandbox-off) get a distinct, stronger visual state (sharper
  aberration/glitch spike, a warning-tinted rim) precisely because the ambient baseline is
  animated — the alarm must still be unmistakably differentiable from normal "alive"
  motion, per Law III (never manufacture false urgency, but real alarms must read as
  real).
- **Accessibility floor unchanged from the prior brief:** never dark text on a dark
  surface; the A−/A+ text-size control persists; body/conversation text must stay legible
  through the chromatic aberration and scanline passes at every zoom level (render UI text
  via egui to a texture *before* the holographic composite pass, so text sampling stays
  crisp — only the composite adds the effect, not the glyph rasterization itself).

---

## 9. Layout direction (unchanged from the prior brief)

A **conversation-centered primary column** (T1); a **slim, always-visible status
strip or rail** (T2) for the three signals + safety state + any alarm; an **ambient "what
it's doing" area** (T3); and **control + diagnostics behind disclosure** (T4/T5).
Optimize the default view for *glance + converse*; let the curious open the rest. The
holographic composite pass wraps this whole layout — it is a post-process over the egui
frame, not a per-widget effect.

---

## 10. Vision — the familiar's eye (camera sensing)

(Unchanged from the prior brief — the eye's consent/visibility/locality/honesty
requirements are Law III/HUMANITY.md commitments, independent of the visual register.)

The familiar gains sight. It can **discover cameras** in its environment and — *only with
the human's explicit consent* — **observe** through them, consuming the major webcam
**still** (snapshot: JPEG/PNG) and **video** (live stream: the common UVC/webcam formats —
MJPEG, H.264/H.265, raw frames) types. What it sees enters the same metabolism: it
identifies objects, notices people present, reads gestures and reactions, forms theories,
poses questions, and — gated and pre-execution-reviewed like all code — may write code to
interact with what the camera reveals. It can **learn new gestures and new meanings** from
watching and from the human's confirmation. The live camera feed, when active, renders as
its own holographic panel (same Fresnel/scanline treatment) with the "observing" indicator
as a persistent, high-contrast rim — the one place intensity should never be subtle.

### Constitutional requirements (hard must-haves for the design)
- **Consent-first, boundary-gated.** Never watch without an explicit human grant.
- **No silent watching, ever.** A prominent, persistent indicator whenever a camera is
  active; stopping is always one click.
- **Local-only, explicit retention.** Default process-and-discard.
- **People served, not catalogued.** No identity/biometric storage without separate,
  explicit consent.
- **Honesty about sight.** known/probable/unknown confidence; ambiguity → it asks.
- **Code from sight is still gated** through the same boundary + review.

---

## Appendix — data dictionary

Unchanged from the prior brief (Observation, Thread, ActivityTick, Request, Answer,
Parameters, Boundary, Identity, Question, Signals, Corruption watch, Daemon status,
Camera, VisionObservation, Gesture) — the holographic engine is a renderer over the same
kernel state, not a new data model.
