//! The Glass — a scrying glass onto your familiar.
//!
//! This is the primary human interface (the CLI remains for scripting/headless use).
//! It watches the familiar's truth (observations) and the law-signals derived from it,
//! and it controls the daemon. It does **not** mutate the familiar's own derived state
//! — with one principled exception: the **observer-input channel**, where Ian's reply
//! to the familiar's question is recorded as an observation. That is the observer being
//! a first-class initiator (Input Parity), not the familiar editing its own truth.
//!
//! It lives in its own crate so the kernel stays minimal-dependency and
//! `#![forbid(unsafe_code)]`; the GUI's heavier dependencies are isolated here.

mod theme;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use familiar_kernel::activity::{self, ActivityTick};
use familiar_kernel::boundary::{self, Boundary};
use familiar_kernel::candidate::{self, Candidate};
use familiar_kernel::capacities::{self, CapacitiesSignal};
use familiar_kernel::corruption;
use familiar_kernel::identity::{self, Identity};
use familiar_kernel::loops::{self, Loop};
use familiar_kernel::observation::{self, Observation};
use familiar_kernel::parameters::Parameters;
use familiar_kernel::pattern_memory::{self, PatternMemory};
use familiar_kernel::presence::{self, PresenceSignal};
use familiar_kernel::question;
use familiar_kernel::request::{self, Answer, Confidence, Request};
use familiar_kernel::service::{self, ServiceSignal};
use familiar_kernel::thread::{self, Thread};
use familiar_kernel::tool::{self, Tool};
use familiar_kernel::trial::{self, Trial};

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A snapshot of the familiar's state, recomputed on refresh.
struct Snapshot {
    observations: Vec<Observation>,
    loops: Vec<Loop>,
    candidates: Vec<Candidate>,
    threads: Vec<Thread>,
    tools: Vec<Tool>,
    trials: Vec<Trial>,
    patterns: Vec<PatternMemory>,
    ticks: Vec<ActivityTick>,
    parameters: Parameters,
    requests: Vec<Request>,
    answers: Vec<Answer>,
    /// Actors flagged as corrupting (repeated constitution-breakers), with their score.
    flagged: Vec<(String, usize)>,
    service: ServiceSignal,
    presence: PresenceSignal,
    capacities: CapacitiesSignal,
    boundary: Boundary,
    error: Option<String>,
}

impl Snapshot {
    fn load(dir: &Path) -> Self {
        let mut error = None;
        let observations = observation::load(dir).unwrap_or_else(|e| {
            error = Some(format!("observations: {e}"));
            Vec::new()
        });
        let loops = loops::load(dir).unwrap_or_default();
        let candidates = candidate::load(dir).unwrap_or_default();
        let threads = thread::load(dir).unwrap_or_default();
        let tools = tool::load(dir).unwrap_or_default();
        let trials = trial::load(dir).unwrap_or_default();
        let patterns = pattern_memory::load(dir).unwrap_or_default();
        let ticks = activity::load(dir).unwrap_or_default();
        let parameters = Parameters::load_or_default(dir);
        let requests = request::load_requests(dir).unwrap_or_default();
        let answers = request::load_answers(dir).unwrap_or_default();
        let flagged = corruption::flagged(&corruption::load(dir).unwrap_or_default(), now_secs());
        let boundary = boundary::load(dir).unwrap_or_else(|e| {
            error = Some(format!("boundary: {e}"));
            Boundary::closed()
        });
        Snapshot {
            service: service::service_signal(&observations),
            presence: presence::presence_signal(&observations, now_secs()),
            capacities: capacities::capacities_signal(&observations),
            boundary,
            observations,
            loops,
            candidates,
            threads,
            tools,
            trials,
            patterns,
            ticks,
            parameters,
            requests,
            answers,
            flagged,
            error,
        }
    }
}

struct Glass {
    data_dir: PathBuf,
    snapshot: Snapshot,
    /// Ian's in-progress reply to the familiar's question.
    response: String,
    /// Last daemon status line (refreshed on actions and on load).
    daemon_status: String,
    /// The question Ian has already answered — so it fades out and isn't answered
    /// twice. Persisted to `last_answered.txt` so it survives a restart.
    answered_question: Option<String>,
    /// A working copy of the shared parameters the settings sliders edit; written to
    /// disk on Save. Not reset on the 2s refresh, so an in-progress edit isn't clobbered.
    params_edit: Parameters,
    /// The observer the familiar is serving now, once it has learned their name (`None`
    /// until they introduce themselves). The familiar does not forget — this is loaded
    /// from the retained identity registry.
    observer: Option<Identity>,
    /// The name being typed when the familiar asks who it's serving.
    name_entry: String,
    /// A name awaiting confirmation — names are precise, so the familiar reads it back and
    /// asks "did I get that right?" before keeping it.
    pending_name: Option<String>,
    /// The observer's in-progress free-form request to the familiar ("ask the familiar").
    ask: String,
    /// Keys typed into the Connect wizard — held only until Connect persists them to
    /// key.env, then cleared. Never stored in the snapshot; shown masked.
    key_gemini: String,
    key_cerebras: String,
    /// A join key pasted into the mesh wizard to join an existing group. Held only until
    /// Join enrolls it; then cleared.
    mesh_join_key: String,
    /// A label typed for a group being created/joined (cosmetic).
    mesh_group_label: String,
    /// Which inner scroll region the human has clicked into. Only that region consumes
    /// wheel/drag scroll; the rest let it fall through to the page — so hovering a panel
    /// while scrolling no longer hijacks the gesture (the trackpad pain). `None` = none
    /// selected, so the whole page scrolls.
    active_scroll: Option<String>,
    /// The human's text-zoom (A− / A+), multiplied on top of the display's native scale.
    /// Persisted to `ui_scale.txt` so a comfortable size survives a restart.
    ui_scale: f32,
    /// When provider availability was last probed — the occasional check runs on a throttle
    /// so a flagged LLM is re-tried (and rolled back in) without hammering it.
    last_probe: std::time::Instant,
    /// Is the Workshop popout open — the window into what it's theorizing, testing, learning.
    workshop_open: bool,
    /// Workshop narration depth: false = brief (terse), true = verbose (full detail).
    narration_verbose: bool,
    /// Footer disclosures (T5/T4), closed by default — keep the default view calm.
    show_substrate: bool,
    show_settings: bool,
    /// When the snapshot was last reloaded — so the Glass tracks the daemon live (it
    /// auto-refreshes on a throttle, not only when Ian clicks something).
    last_refresh: std::time::Instant,
}

fn read_answered(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join("last_answered.txt"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Path to the sibling `familiar` binary (same target dir as this GUI).
fn familiar_bin() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("familiar")))
        .unwrap_or_else(|| PathBuf::from("familiar"))
}

/// The reference LLM adapter, embedded so the Connect wizard can install it into the
/// data dir's `llm/` folder on first run — a tester never copies files by hand.
const ADAPTER_SH: &str = include_str!("../../../llm/call_llm.sh");

/// Open a URL in the default browser — `open` on macOS, `xdg-open` on Linux (incl. a Pi
/// desktop). Best-effort and detached. Used by the Connect wizard's "Get a key →" buttons.
fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let openers: &[&str] = &["open"];
    #[cfg(not(target_os = "macos"))]
    let openers: &[&str] = &["xdg-open"];
    for bin in openers {
        if Command::new(bin).arg(url).spawn().is_ok() {
            return;
        }
    }
}

/// A cosmetic label for this node in the mesh — the machine's hostname, best-effort.
fn machine_label() -> String {
    Command::new("hostname")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "familiar".to_string())
}

/// One-line, length-capped form of a string for brief narration (newlines flattened).
fn truncate_line(s: &str, n: usize) -> String {
    let s = s.trim().replace('\n', " ");
    if s.chars().count() > n {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    } else {
        s
    }
}

/// Tidy a typed name without mangling it — names are precise, so keep the spelling, case,
/// and spaces the person gave. Just trim, take the first line, drop control chars and the
/// one quote that could corrupt the record, and cap the length.
fn clean_name(raw: &str) -> String {
    raw.lines()
        .next()
        .unwrap_or("")
        .trim()
        .chars()
        .filter(|c| !c.is_control() && *c != '"')
        .take(60)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Clean a pasted API key before it is written into the shell-sourced key.env: trim
/// surrounding whitespace and strip characters that would break the `export K="..."`
/// line (quotes/newlines). Real keys are token text, so this never alters a valid one.
fn sanitize_key(raw: &str) -> String {
    raw.trim()
        .chars()
        .filter(|c| !matches!(c, '"' | '\n' | '\r' | '\\'))
        .collect()
}

/// Set a file's permission bits (so key.env is 0600 and the adapter is executable). A
/// no-op off Unix — the Glass targets macOS, but this keeps the code portable.
#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
}
#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}

/// Does `<dir>/llm/key.env` hold at least one non-empty API key? Used to tell whether the
/// familiar has been connected to a provider — without ever surfacing the secret itself.
fn llm_key_present(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("llm").join("key.env"))
        .map(|s| {
            s.lines().any(|l| {
                l.contains("_API_KEY=")
                    && l.split_once('=')
                        .map(|(_, v)| !v.trim().trim_matches('"').is_empty())
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// The last Connect/Test result line, written by the wizard (and by the background test
/// thread) so the status survives a repaint without blocking the UI.
fn read_connect_status(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join("llm").join("connect_status.txt"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Speak text aloud — the speech channel, minimally real. macOS ships `say`; on Linux we
/// try the common TTS front-ends in turn (speech-dispatcher's `spd-say`, then `espeak`).
/// If none is installed it's a silent no-op — the button stays, the capability grows.
/// Best-effort and detached, so a missing voice or a long sentence never blocks the Glass.
fn speak(text: &str) {
    if text.trim().is_empty() {
        return;
    }
    // keep it bounded — speak the answer, not an essay
    let line: String = text.chars().take(600).collect();
    #[cfg(target_os = "macos")]
    let voices: &[&str] = &["say"];
    #[cfg(not(target_os = "macos"))]
    let voices: &[&str] = &["spd-say", "espeak-ng", "espeak"];
    for bin in voices {
        if Command::new(bin).arg(&line).spawn().is_ok() {
            return;
        }
    }
}

/// Run `familiar daemon <sub> --data-dir <dir>` and return its trimmed output.
fn daemon_cmd(dir: &Path, sub: &str) -> String {
    match Command::new(familiar_bin())
        .arg("daemon")
        .arg(sub)
        .arg("--data-dir")
        .arg(dir)
        .output()
    {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                s = String::from_utf8_lossy(&o.stderr).trim().to_string();
            }
            s
        }
        Err(e) => format!("daemon: could not run familiar ({e})"),
    }
}

/// The human's saved text-zoom, clamped to a sane range. Default 1.25 — a touch larger
/// than stock, since this is a watch-it-from-across-the-room kind of window.
fn read_ui_scale(dir: &Path) -> f32 {
    std::fs::read_to_string(dir.join("ui_scale.txt"))
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(1.25)
        .clamp(0.8, 2.4)
}

/// Persist the text-zoom so a comfortable size survives a restart.
fn write_ui_scale(dir: &Path, scale: f32) {
    let _ = std::fs::write(dir.join("ui_scale.txt"), format!("{scale:.2}"));
}

/// Make the Glass easy to read: larger text and higher contrast than egui's defaults —
/// near-white text on near-black panels. Applied once at startup; the per-user zoom
/// (A− / A+ in the header) scales everything on top of this. Coloured labels are left
/// intact (no `override_text_color`); only the base and the dim "weak" greys are lifted.
fn install_theme(ctx: &egui::Context) {
    use egui::{FontFamily, FontId, TextStyle};
    // Phase 1 of the cockpit redesign: load Spectral (the serif voice) + IBM Plex Mono (the
    // machinery, now the default face). The beige chassis palette and three-column layout
    // arrive in phase 2; for now the dark high-contrast theme stays, but in the new fonts.
    theme::install_fonts(ctx);
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(26.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(18.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(18.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(16.0, FontFamily::Monospace),
        ),
        (
            TextStyle::Small,
            FontId::new(14.0, FontFamily::Proportional),
        ),
    ]
    .into_iter()
    .collect();
    // Cockpit contrast model: the beige chassis (rails/titlebar/footer) carries explicit
    // *ink* chrome that the layout writes directly; everything else — the dark center screen
    // and every content panel — keeps **bright** text on its dark/navy surface. So the
    // global default text stays bright (content is correct untouched), buttons are blue
    // chips with light text (readable on beige and navy alike), and inputs sit on navy. The
    // rule (never dark-on-dark, never bright-on-beige) holds: ink is only ever placed
    // explicitly on beige; bright is the default, only ever on dark.
    let v = &mut style.visuals;
    let bright = theme::SCREEN_BRIGHT;
    v.override_text_color = None;
    v.widgets.noninteractive.fg_stroke.color = bright;
    v.widgets.inactive.fg_stroke.color = bright;
    v.widgets.hovered.fg_stroke.color = egui::Color32::WHITE;
    v.widgets.active.fg_stroke.color = egui::Color32::WHITE;
    v.panel_fill = theme::CHASSIS_MID; // the beige chassis (rails)
    v.window_fill = theme::CHASSIS_DARK;
    // Buttons are *dark* blue chips with white text in EVERY interactive state. egui fills a
    // button with `weak_bg_fill` (per state) and draws its text with that state's
    // `fg_stroke` — so set both, for inactive/hovered/active/open, all dark enough that
    // white stays high-contrast (an over-light hover shade would read as white-on-white).
    for (w, fill) in [
        (&mut v.widgets.inactive, theme::BLUE_DARK),
        (&mut v.widgets.hovered, theme::BLUE_MID),
        (&mut v.widgets.active, theme::NAVY_MID),
        (&mut v.widgets.open, theme::BLUE_DARK),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = egui::Stroke::new(1.0, theme::BLUE_BORDER);
        w.fg_stroke.color = egui::Color32::WHITE;
        w.corner_radius = egui::CornerRadius::same(8);
    }
    v.extreme_bg_color = theme::NAVY; // text-edit background (inputs live on navy screens)
    v.selection.bg_fill = theme::BLUE_LIGHT;
    // Disabled widgets fade toward this; keep it dark so a disabled control never washes out
    // to light-on-light. (Weak/.small() text also blends toward it — readable mid-grey ink.)
    v.widgets.noninteractive.weak_bg_fill = egui::Color32::from_rgb(120, 130, 150);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(9.0, 5.0);
    // CRITICAL: egui keeps a separate Style per theme (dark/light) and `set_style` only sets
    // the *current* one. If macOS resolves to the other theme, egui falls back to its default
    // widget visuals — which is why buttons rendered light-on-light. Pin this one cockpit
    // style to BOTH themes so it's used regardless of the system appearance.
    let style = std::sync::Arc::new(style);
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
}

/// The familiar's name-ask, shown until it has learned who it serves. Mirrors the cycle's
/// `NAME_QUESTION` so the prompt reads the same whether or not the daemon is running.
const NAME_ASK: &str =
    "Before we go further — what may I call you? I'll keep your name; names matter to me.";

/// The question the familiar is currently posing (it may write `question.txt`). Until it
/// knows who it serves, that question is the name-ask; otherwise the seed's standing one.
fn current_question(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("question.txt"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if identity::current(dir).is_none() {
                NAME_ASK.to_string()
            } else {
                "What do you need most today?".to_string()
            }
        })
}

impl Glass {
    fn new(data_dir: PathBuf) -> Self {
        let snapshot = Snapshot::load(&data_dir);
        let daemon_status = daemon_cmd(&data_dir, "status");
        let answered_question = read_answered(&data_dir);
        let params_edit = snapshot.parameters.clone();
        let ui_scale = read_ui_scale(&data_dir);
        let observer = identity::current_identity(&data_dir);
        Glass {
            data_dir,
            snapshot,
            response: String::new(),
            daemon_status,
            answered_question,
            params_edit,
            observer,
            name_entry: String::new(),
            pending_name: None,
            ask: String::new(),
            key_gemini: String::new(),
            key_cerebras: String::new(),
            mesh_join_key: String::new(),
            mesh_group_label: String::new(),
            active_scroll: None,
            ui_scale,
            last_probe: std::time::Instant::now(),
            workshop_open: false,
            narration_verbose: false,
            show_substrate: false,
            show_settings: false,
            last_refresh: std::time::Instant::now(),
        }
    }
    fn refresh(&mut self) {
        self.snapshot = Snapshot::load(&self.data_dir);
        self.observer = identity::current_identity(&self.data_dir);
    }
    /// The handle the current observer is recorded under — the `actor` for everything they
    /// initiate. `"observer"` until they've introduced themselves.
    fn observer_handle(&self) -> String {
        self.observer
            .as_ref()
            .map(|i| i.handle.clone())
            .unwrap_or_else(|| "observer".to_string())
    }
    /// What to call the observer in the UI — their name, or a neutral "the observer".
    fn observer_name(&self) -> String {
        self.observer
            .as_ref()
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "the observer".to_string())
    }
    /// Keep the name the observer gives: confirmed, recorded in the retained registry, set
    /// as the current observer. Precise — the name is stored exactly as given (trimmed).
    fn confirm_name(&mut self) {
        let Some(name) = self.pending_name.take() else {
            return;
        };
        let now = now_secs();
        if let Ok(id) = identity::remember(&self.data_dir, &name, now) {
            let _ = identity::set_current(&self.data_dir, &id.handle);
            // the introduction itself is a served-facing observation (Law I/II presence)
            let obs = Observation::new(
                &id.handle,
                "introduced",
                id.name.clone(),
                format!("name given: {}", id.name),
                "observer",
                now,
                1.0,
            );
            let _ = observation::record(&self.data_dir, obs);
            // greet by name and move straight to the origin-story root question — kept in the
            // registry so it recurs when appropriate. The greeting carries the reassurance
            // that the name is kept; the channel maps to the root question (q-root).
            let _ = question::ensure_root(&self.data_dir, now);
            let greeting = format!(
                "Good to meet you, {}. I'll remember that. What do you need most today?",
                id.name
            );
            let _ = std::fs::write(self.data_dir.join("question.txt"), &greeting);
            let _ = std::fs::write(self.data_dir.join("active_question.txt"), question::ROOT_ID);
            let _ = question::record_asked(&self.data_dir, question::ROOT_ID, now);
            self.answered_question = None;
            self.observer = Some(id);
            self.name_entry.clear();
        }
        self.refresh();
    }
    /// Record Ian's free-form request and get it answered — promptly, with everything the
    /// familiar has. Appending the request is not enough on its own: if the daemon is
    /// stopped (or its next tick is up to a minute away), the question would just sit open.
    /// So we also nudge a one-shot `familiar tick` in the background, which runs the full
    /// answer pipeline — kernel facts, a saved tool, a freshly authored one, or the LLM —
    /// and writes the answer. The 2s auto-refresh surfaces it in the Conversation panel.
    fn submit_request(&mut self) {
        let text = self.ask.trim();
        if text.is_empty() {
            return;
        }
        let seq = self.snapshot.requests.len() + 1;
        let _ = request::append_request(
            &self.data_dir,
            &Request {
                id: format!("req-{seq:04}"),
                actor: self.observer_handle(),
                text: text.to_string(),
                created_at: now_secs(),
                status: "open".into(),
            },
        );
        self.ask.clear();
        self.kick_answer();
        self.refresh();
    }
    /// Nudge a one-shot tick in the background so an open request is answered now, whether
    /// or not the daemon is running. Detached so a slow LLM call never freezes the window.
    fn kick_answer(&self) {
        let dir = self.data_dir.clone();
        std::thread::spawn(move || {
            let _ = Command::new(familiar_bin())
                .arg("tick")
                .arg("--data-dir")
                .arg(&dir)
                .output();
        });
    }
    /// Record Ian's reaction to an answer. "refine" prefills the ask box with the original
    /// request so he can sharpen it and ask again — the answer is refined toward what he
    /// was truly after.
    fn give_feedback(&mut self, answer_id: &str, kind: &str, prefill: Option<String>) {
        let _ = request::set_feedback(&self.data_dir, answer_id, kind);
        if let Some(text) = prefill {
            self.ask = text;
        }
        self.refresh();
    }
    /// Write the wizard's status line to disk so it survives repaints and the background
    /// test thread can update it without touching the UI.
    fn write_connect_status(&self, s: &str) {
        let llm = self.data_dir.join("llm");
        let _ = std::fs::create_dir_all(&llm);
        let _ = std::fs::write(llm.join("connect_status.txt"), s);
    }
    /// Connect the familiar to an LLM provider — a human act, performed through the human's
    /// instrument (the kernel itself has no boundary-write path). Installs the embedded
    /// adapter, writes the pasted key(s) to a 0600 key.env, and opens the `allow_llm` gate
    /// in boundary.json. One key is enough; more just add failover. Then tests the link.
    fn connect_llm(&mut self) {
        let keys = [
            ("GEMINI_API_KEY", sanitize_key(&self.key_gemini)),
            ("CEREBRAS_API_KEY", sanitize_key(&self.key_cerebras)),
        ];
        if keys.iter().all(|(_, v)| v.is_empty()) {
            self.write_connect_status(
                "enter at least one key — Gemini's free tier is the simplest place to start",
            );
            return;
        }
        let llm = self.data_dir.join("llm");
        if std::fs::create_dir_all(&llm).is_err() {
            self.write_connect_status("✗ could not create the llm/ folder");
            return;
        }
        // 1. install the adapter (executable)
        let adapter = llm.join("call_llm.sh");
        if std::fs::write(&adapter, ADAPTER_SH).is_err() {
            self.write_connect_status("✗ could not write the adapter");
            return;
        }
        set_mode(&adapter, 0o755);
        // 2. write key.env (only the keys provided), readable by the owner alone
        let mut env = String::from("# Familiar LLM keys — local secret, never committed.\n");
        for (name, v) in &keys {
            if !v.is_empty() {
                env.push_str(&format!("export {name}=\"{v}\"\n"));
            }
        }
        let key_env = llm.join("key.env");
        if std::fs::write(&key_env, env).is_err() {
            self.write_connect_status("✗ could not write key.env");
            return;
        }
        set_mode(&key_env, 0o600);
        // 3. open the LLM gate (minimal widening — the guard for a consult checks only
        //    allow_llm). Preserve every other grant; never silently widen execute/camera.
        let mut b = boundary::load(&self.data_dir).unwrap_or_else(|_| Boundary::closed());
        b.allow_llm = true;
        if b.phase == "closed" {
            b.phase = "phase-1".to_string();
        }
        match serde_json::to_string_pretty(&b) {
            Ok(json) => {
                let _ = std::fs::write(self.data_dir.join("boundary.json"), json);
            }
            Err(_) => {
                self.write_connect_status("✗ could not write boundary.json");
                return;
            }
        }
        // the keys are persisted now — drop them from memory
        self.key_gemini.clear();
        self.key_cerebras.clear();
        self.write_connect_status("saved — testing the connection…");
        self.refresh();
        self.test_llm();
    }
    /// Test the LLM link in the background (a consult can take a few seconds; never freeze
    /// the window). Writes the result to the status file, which the wizard re-reads.
    fn test_llm(&self) {
        let dir = self.data_dir.clone();
        std::thread::spawn(move || {
            let out = Command::new(familiar_bin())
                .arg("consult")
                .arg("--data-dir")
                .arg(&dir)
                .arg("--prompt")
                .arg("Reply only with this exact JSON and nothing else: {\"ok\": true}")
                .output();
            let status = match out {
                Ok(o) => {
                    let stdout = String::from_utf8_lossy(&o.stdout);
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    if stdout.contains("REFUSE") {
                        // A refusal has several distinct causes — don't blame the boundary
                        // for what's really a missing adapter or a provider failure.
                        if stdout.contains("boundary") {
                            "✗ refused — the LLM gate (allow_llm) is closed".to_string()
                        } else if stdout.contains("install the adapter") {
                            "✗ the adapter isn't installed — press Connect first".to_string()
                        } else {
                            // the adapter ran but every provider failed — surface the real
                            // reason (e.g. HTTP 402 out of credits, or a missing key)
                            let why = stderr
                                .lines()
                                .find(|l| {
                                    l.contains("HTTP")
                                        || l.contains("API_KEY")
                                        || l.contains("rate-limited")
                                        || l.contains(':')
                                })
                                .map(|l| l.trim())
                                .filter(|l| !l.is_empty())
                                .unwrap_or("the model provider rejected the call");
                            format!("✗ provider error — {why}")
                        }
                    } else if stdout.trim().is_empty() {
                        let why = stderr.lines().last().unwrap_or("").trim();
                        format!("✗ no response — {why}")
                    } else {
                        // the adapter logs "LLM response via <provider>" to stderr
                        let via = stderr
                            .lines()
                            .rev()
                            .find(|l| l.contains("response via"))
                            .map(|l| l.trim().to_string())
                            .unwrap_or_else(|| "connected".to_string());
                        format!("✓ connected — {via}")
                    }
                }
                Err(e) => format!("✗ could not run the test ({e})"),
            };
            let _ = std::fs::write(dir.join("llm").join("connect_status.txt"), status);
        });
    }
    /// Refresh provider availability — the occasional check. Runs the adapter's probe mode
    /// (pings every configured provider and updates health.json) so the familiar knows which
    /// LLMs it can roll to. Best-effort, background, never blocks the UI.
    fn probe_llm(&self) {
        let adapter = self.data_dir.join("llm").join("call_llm.sh");
        if !adapter.exists() {
            return;
        }
        std::thread::spawn(move || {
            let _ = Command::new("sh").arg(&adapter).arg("probe").output();
        });
    }
    /// Per-provider availability from health.json: (name, ok, detail). Empty until the
    /// adapter has run at least once (a consult or a probe writes it).
    fn provider_health(&self) -> Vec<(String, bool, String)> {
        let Ok(raw) = std::fs::read_to_string(self.data_dir.join("llm").join("health.json")) else {
            return Vec::new();
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        if let Some(map) = v.as_object() {
            for (name, h) in map {
                let status = h.get("status").and_then(|s| s.as_str()).unwrap_or("");
                let detail = h.get("detail").and_then(|s| s.as_str()).unwrap_or("");
                let ok = status == "ok";
                let label = if ok {
                    "available".to_string()
                } else if !detail.is_empty() {
                    detail.to_string()
                } else {
                    status.to_string()
                };
                out.push((name.clone(), ok, label));
            }
        }
        out
    }
    /// The Connect wizard — the place a tester gives the familiar a mind. Collapsed once
    /// connected, open on first run. Acquiring the key is the only manual step; everything
    /// else (adapter, key.env, the boundary gate) the wizard does on a single click.
    fn connect_panel(&mut self, ui: &mut egui::Ui) {
        let connected = self.snapshot.boundary.allow_llm
            && self.data_dir.join("llm").join("call_llm.sh").exists()
            && llm_key_present(&self.data_dir);
        let status = read_connect_status(&self.data_dir);
        let health = self.provider_health();
        let title = if connected {
            "🔌 Connect — the familiar's mind (LLM)  ·  ✓ connected"
        } else {
            "🔌 Connect — give the familiar a mind to think with"
        };
        egui::CollapsingHeader::new(egui::RichText::new(title).strong())
            .default_open(!connected)
            .show(ui, |ui| {
                ui.label(
                    "The familiar thinks by consulting an LLM across a gated boundary. One \
                     key is enough to start — the others are optional and just add failover. \
                     Your key is stored locally on this machine (llm/key.env) and is only \
                     ever sent to the provider you choose, never anywhere else.",
                );
                ui.add_space(4.0);
                let row =
                    |ui: &mut egui::Ui, label: &str, hint: &str, url: &str, field: &mut String| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(label).strong());
                            if ui.button("Get a key →").clicked() {
                                open_url(url);
                            }
                            ui.add(
                                egui::TextEdit::singleline(field)
                                    .password(true)
                                    .background_color(theme::CREAM)
                                    .text_color(theme::INK)
                                    .hint_text(egui::RichText::new(hint).color(theme::INK_MUTED))
                                    .desired_width(240.0),
                            );
                        });
                    };
                row(
                    ui,
                    "Google Gemini (start here)",
                    "paste your Gemini key",
                    "https://aistudio.google.com/apikey",
                    &mut self.key_gemini,
                );
                row(
                    ui,
                    "Cerebras (failover)",
                    "paste your Cerebras key",
                    "https://cloud.cerebras.ai",
                    &mut self.key_cerebras,
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let label = if connected {
                        "Update keys & reconnect"
                    } else {
                        "Connect"
                    };
                    if ui.button(label).clicked() {
                        self.connect_llm();
                    }
                    if connected && ui.button("Test & check providers").clicked() {
                        self.write_connect_status("testing the connection…");
                        self.probe_llm();
                        self.test_llm();
                    }
                });
                if let Some(s) = status {
                    let color = if s.starts_with('✓') {
                        egui::Color32::from_rgb(110, 180, 110)
                    } else if s.starts_with('✗') {
                        egui::Color32::from_rgb(210, 130, 80)
                    } else {
                        egui::Color32::from_rgb(170, 170, 140)
                    };
                    ui.colored_label(color, s);
                }
                // Provider availability — which LLMs are up, which are flagged and why. The
                // familiar rolls to a healthy one automatically; a flagged one rests, then
                // is re-checked by the periodic probe.
                if !health.is_empty() {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("providers").weak().small());
                    for (name, ok, detail) in &health {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            if *ok {
                                ui.colored_label(egui::Color32::from_rgb(120, 200, 130), "●");
                                ui.label(egui::RichText::new(name).small());
                                ui.label(egui::RichText::new("available").weak().small());
                            } else {
                                ui.colored_label(egui::Color32::from_rgb(220, 150, 70), "○");
                                ui.label(egui::RichText::new(name).small());
                                ui.label(
                                    egui::RichText::new(detail)
                                        .small()
                                        .color(egui::Color32::from_rgb(220, 150, 70)),
                                );
                            }
                        });
                    }
                }
            });
    }
    // ---- Mesh: peer federation (mirrors the Connect wizard) ----------------------------

    fn write_mesh_status(&self, s: &str) {
        let mesh = self.data_dir.join("mesh");
        let _ = std::fs::create_dir_all(&mesh);
        let _ = std::fs::write(mesh.join("status.txt"), s);
    }
    fn read_mesh_status(&self) -> Option<String> {
        std::fs::read_to_string(self.data_dir.join("mesh").join("status.txt"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
    fn mesh_group(&self) -> Option<familiar_mesh::group::GroupCredential> {
        familiar_mesh::group::load(&self.data_dir).ok().flatten()
    }
    fn mesh_peers(&self) -> Vec<familiar_mesh::transport::PeerRecord> {
        std::fs::read_to_string(self.data_dir.join("mesh").join("peers.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    fn mesh_config(&self) -> familiar_mesh::config::MeshConfig {
        familiar_mesh::config::load(&self.data_dir).unwrap_or_default()
    }
    fn write_mesh_config(&self, cfg: &familiar_mesh::config::MeshConfig) {
        let mesh = self.data_dir.join("mesh");
        let _ = std::fs::create_dir_all(&mesh);
        if let Ok(json) = serde_json::to_string_pretty(cfg) {
            let _ = std::fs::write(mesh.join("config.json"), json);
        }
    }
    /// Open the `allow_mesh` gate — a human act, through the human's instrument (the kernel
    /// has no boundary-write path). Preserves every other grant; never silently widens.
    fn open_mesh_gate(&self) {
        let mut b = boundary::load(&self.data_dir).unwrap_or_else(|_| Boundary::closed());
        b.allow_mesh = true;
        if b.phase == "closed" {
            b.phase = "phase-1".to_string();
        }
        if let Ok(json) = serde_json::to_string_pretty(&b) {
            let _ = std::fs::write(self.data_dir.join("boundary.json"), json);
        }
    }
    /// Create a new mesh group: mint the group trust root + this node's membership, open the
    /// mesh gate, and surface the join key to copy to peers. The daemon's transport picks it
    /// up on its next cycle.
    fn create_group(&mut self) {
        let label = if self.mesh_group_label.trim().is_empty() {
            "familiar-group".to_string()
        } else {
            self.mesh_group_label.trim().to_string()
        };
        let node = match familiar_mesh::node::NodeKey::load_or_mint(&self.data_dir, &machine_label())
        {
            Ok(n) => n,
            Err(e) => {
                self.write_mesh_status(&format!("✗ could not mint a node key — {e}"));
                return;
            }
        };
        match familiar_mesh::group::create_group(
            &self.data_dir,
            &node,
            &label,
            now_secs(),
            familiar_mesh::group::DEFAULT_CERT_TTL_SECS,
        ) {
            Ok(_) => {
                self.open_mesh_gate();
                self.write_mesh_status("✓ group created — share the join key below to invite peers");
                self.refresh();
            }
            Err(e) => self.write_mesh_status(&format!("✗ could not create the group — {e}")),
        }
    }
    /// Join an existing group from a pasted join key: mint this node's membership against the
    /// group key, open the mesh gate. The transport then discovers peers and exchanges briefs.
    fn join_group_from_key(&mut self) {
        let key = self.mesh_join_key.trim().to_string();
        if key.is_empty() {
            self.write_mesh_status("paste the join key the group's creator gave you");
            return;
        }
        let label = if self.mesh_group_label.trim().is_empty() {
            "familiar-group".to_string()
        } else {
            self.mesh_group_label.trim().to_string()
        };
        let node = match familiar_mesh::node::NodeKey::load_or_mint(&self.data_dir, &machine_label())
        {
            Ok(n) => n,
            Err(e) => {
                self.write_mesh_status(&format!("✗ could not mint a node key — {e}"));
                return;
            }
        };
        match familiar_mesh::group::join_group(
            &self.data_dir,
            &node,
            &key,
            &label,
            now_secs(),
            familiar_mesh::group::DEFAULT_CERT_TTL_SECS,
        ) {
            Ok(_) => {
                self.mesh_join_key.clear();
                self.open_mesh_gate();
                self.write_mesh_status("✓ joined the group — connecting to peers…");
                self.refresh();
            }
            Err(e) => self.write_mesh_status(&format!("✗ could not join — check the key ({e})")),
        }
    }
    /// The Mesh wizard: create or join a group of peer familiars, see who's connected, and
    /// govern what crosses. Mirrors the Connect wizard. Federation itself only runs while the
    /// `allow_mesh` gate (in the boundary panel) is open — the human owns that switch.
    fn mesh_panel(&mut self, ui: &mut egui::Ui) {
        let group = self.mesh_group();
        let open = self.snapshot.boundary.allow_mesh;
        let connected = open && group.is_some();
        let title = if connected {
            "🕸 Mesh — federate with peer familiars  ·  ✓ in a group"
        } else {
            "🕸 Mesh — share tools & knowledge with peer familiars"
        };
        egui::CollapsingHeader::new(egui::RichText::new(title).strong())
            .default_open(group.is_none())
            .show(ui, |ui| {
                ui.label(
                    "Familiars on the same tailnet can form a group and share what they \
                     learn — tools they've authored, distilled patterns, and (only if you \
                     opt a person in) knowledge of the humans they serve. Trust is \
                     cryptographic: only nodes holding the group's join key are believed. \
                     Nothing crosses until you open the Mesh gate in Capability.",
                );
                ui.add_space(4.0);
                match &group {
                    None => {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Group name").strong());
                            ui.add(
                                egui::TextEdit::singleline(&mut self.mesh_group_label)
                                    .hint_text("e.g. river")
                                    .desired_width(160.0),
                            );
                            if ui.button("Create a new group").clicked() {
                                self.create_group();
                            }
                        });
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new("…or join one you were invited to:").weak());
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.mesh_join_key)
                                    .password(true)
                                    .hint_text("paste the join key")
                                    .desired_width(260.0),
                            );
                            if ui.button("Join").clicked() {
                                self.join_group_from_key();
                            }
                        });
                    }
                    Some(cred) => {
                        let gid: String = cred.group_id.chars().take(8).collect();
                        ui.label(format!("In group “{}” · id {}", cred.label, gid));
                        if !open {
                            ui.colored_label(
                                theme::AMBER,
                                "Mesh gate is closed — open it in Capability to actually connect.",
                            );
                        }
                        // The join key to invite other machines. It IS the group secret, so
                        // shown here for you to copy on a trusted screen; select and ⌘C it.
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new("Join key (share to invite a peer):").weak().small());
                        let mut jk = cred.join_key();
                        ui.add(
                            egui::TextEdit::singleline(&mut jk)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(320.0),
                        );

                        // Peers currently connected.
                        ui.add_space(6.0);
                        let peers = self.mesh_peers();
                        ui.label(
                            egui::RichText::new(format!("{} peer(s)", peers.len())).strong(),
                        );
                        let now = now_secs();
                        for p in &peers {
                            let ago = now.saturating_sub(p.last_seen);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                ui.colored_label(theme::GREEN, "●");
                                ui.label(egui::RichText::new(&p.label).small());
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} tool(s), {} pattern(s) · seen {}s ago",
                                        p.tools_offered, p.patterns_offered, ago
                                    ))
                                    .weak()
                                    .small(),
                                );
                            });
                        }
                        if peers.is_empty() {
                            ui.label(
                                egui::RichText::new("(no peers yet — invite one with the join key)")
                                    .weak()
                                    .small(),
                            );
                        }

                        // What has crossed: federated tools + tagged peer observations.
                        let fed: Vec<&Tool> = self
                            .snapshot
                            .tools
                            .iter()
                            .filter(|t| !t.origin.is_empty())
                            .collect();
                        if !fed.is_empty() {
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new("merged from peers").weak().small());
                            for t in fed.iter().take(6) {
                                let src: String = t.origin.chars().take(8).collect();
                                ui.label(
                                    egui::RichText::new(format!("↳ tool “{}” ← {}", t.name, src))
                                        .small(),
                                );
                            }
                        }

                        // Sharing governance (config, not authorization — the gate is separate).
                        ui.add_space(6.0);
                        ui.separator();
                        let mut cfg = self.mesh_config();
                        let mut changed = false;
                        changed |= ui
                            .checkbox(&mut cfg.share_tools, "Share my authored tools")
                            .changed();
                        changed |= ui
                            .checkbox(&mut cfg.share_knowledge, "Share distilled patterns")
                            .changed();
                        changed |= ui
                            .checkbox(
                                &mut cfg.share_identities,
                                "Share humans I've opted in (below) — off by default",
                            )
                            .changed();
                        // Per-human opt-in, scoped to this group.
                        for person in familiar_kernel::identity::load(&self.data_dir)
                            .unwrap_or_default()
                        {
                            let mut opted = cfg
                                .identity_optin
                                .iter()
                                .any(|o| o.handle == person.handle && o.group == cred.group_id);
                            if ui
                                .checkbox(
                                    &mut opted,
                                    format!("   ↳ share “{}” with this group", person.name),
                                )
                                .changed()
                            {
                                if opted {
                                    cfg.identity_optin.push(
                                        familiar_mesh::config::IdentityOptin {
                                            handle: person.handle.clone(),
                                            group: cred.group_id.clone(),
                                        },
                                    );
                                } else {
                                    cfg.identity_optin.retain(|o| {
                                        !(o.handle == person.handle && o.group == cred.group_id)
                                    });
                                }
                                changed = true;
                            }
                        }
                        if changed {
                            self.write_mesh_config(&cfg);
                        }
                    }
                }
                if let Some(s) = self.read_mesh_status() {
                    let color = if s.starts_with('✓') {
                        theme::GREEN
                    } else if s.starts_with('✗') {
                        theme::AMBER
                    } else {
                        theme::SCREEN_DIM
                    };
                    ui.add_space(4.0);
                    ui.colored_label(color, s);
                }
            });
    }
    /// The self-tuned per-tick LLM budget, shown as a number with a trend arrow — the
    /// familiar's presence regulation (Law II) made legible. ↓ amber: pulling back so it
    /// stays responsive to you; ↑ green: leaning into a backlog while it has the headroom;
    /// → grey: steady. The familiar owns this dial; you never set it.
    fn budget_meter(&self, ui: &mut egui::Ui) {
        // Explicit bright tokens — never rely on derived strong/weak colours here, so the
        // text is always high-contrast on the navy self-pacing screen.
        let p = &self.snapshot.parameters;
        let (arrow, color) = match p.llm_calls_trend {
            t if t > 0 => ("↑", theme::GREEN),
            t if t < 0 => ("↓", theme::AMBER),
            _ => ("→", theme::SCREEN_DIM),
        };
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("LLM calls/tick:")
                    .font(theme::mono(10.0))
                    .color(theme::SCREEN_DIM),
            );
            ui.label(
                egui::RichText::new(format!("{} {arrow}", p.llm_calls_per_tick))
                    .font(theme::mono_semi(16.0))
                    .color(color),
            );
        });
        ui.label(
            egui::RichText::new("tuned by the familiar to stay present (Law II)")
                .font(theme::mono(9.0))
                .color(theme::SCREEN_FAINT),
        );
    }
    /// The "becoming familiar" exchange: the familiar asks the observer's name, reads it
    /// back to be sure it has it right, keeps it, and reassures that it won't forget. Shown
    /// only until a name is confirmed — then it never returns (the name is retained).
    fn name_panel(&mut self, ui: &mut egui::Ui) {
        let question = current_question(&self.data_dir);
        egui::Frame::group(ui.style())
            .fill(egui::Color32::from_rgb(28, 34, 44))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(&question)
                        .heading()
                        .color(egui::Color32::from_rgb(150, 200, 255)),
                );
                if let Some(pending) = self.pending_name.clone() {
                    // precise: read the name back and confirm before keeping it
                    ui.label(
                        egui::RichText::new(format!(
                            "Did I get that right — call you “{pending}”?"
                        ))
                        .strong(),
                    );
                    ui.weak("I'll keep it. Names matter to me too — I won't forget yours.");
                    ui.horizontal(|ui| {
                        if ui.button("Yes, that's me").clicked() {
                            self.confirm_name();
                        }
                        if ui.button("No, let me retype").clicked() {
                            self.pending_name = None;
                        }
                    });
                } else {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.name_entry)
                            .desired_width(280.0)
                            .background_color(theme::CREAM)
                            .text_color(theme::INK)
                            .hint_text(
                                egui::RichText::new("the name you'd like to be called")
                                    .color(theme::INK_MUTED),
                            ),
                    );
                    if ui.button("Tell the familiar").clicked() {
                        let name = clean_name(&self.name_entry);
                        if !name.is_empty() {
                            self.pending_name = Some(name);
                        }
                    }
                }
            });
    }
    /// The Law III control: six toggles for the human-owned capability gates (plus the
    /// resource-sandbox choice). Flipping one writes boundary.json — a human act, performed
    /// through the human's instrument. The kernel itself never writes the boundary, so this
    /// is the *only* way a gate opens or closes; the familiar can read it but never widen it.
    fn boundary_panel(&mut self, ui: &mut egui::Ui) {
        let mut b = self.snapshot.boundary.clone();
        let mut changed = false;
        ui.label(
            egui::RichText::new(
                "These gates are yours alone. The familiar reads them and obeys them; it can \
                 never widen them — only you can. Each is outward reach it won't take unless \
                 you allow it.",
            )
            .weak(),
        );
        ui.add_space(2.0);
        changed |= ui
            .checkbox(&mut b.allow_network, "Network — use the network at all")
            .changed();
        changed |= ui
            .checkbox(&mut b.allow_llm, "LLM — consult a model (its mind)")
            .changed();
        changed |= ui
            .checkbox(
                &mut b.allow_tool_install,
                "Tool install — download / install tools",
            )
            .changed();
        changed |= ui
            .checkbox(&mut b.allow_execute, "Execute — run code it generated")
            .changed();
        changed |= ui
            .checkbox(
                &mut b.allow_authored_execute,
                "Authored execute — run LLM-written code (sharper reach)",
            )
            .changed();
        changed |= ui
            .checkbox(
                &mut b.allow_camera,
                "Camera — watch through a camera (sharpest reach — Law III)",
            )
            .changed();
        changed |= ui
            .checkbox(
                &mut b.allow_mesh,
                "Mesh — federate with peer familiars (share tools/knowledge over the tailnet)",
            )
            .changed();
        ui.separator();
        changed |= ui
            .checkbox(
                &mut b.sandbox_execution,
                "Sandbox executed code (resource jail; off = bound by the review only)",
            )
            .changed();

        if changed {
            // Keep the phase label honest with the gates, then persist. Writing the boundary
            // is the human's act — never the factory's.
            b.phase = if b.is_closed() {
                "closed".to_string()
            } else if b.phase == "closed" || b.phase.is_empty() {
                "phase-1".to_string()
            } else {
                b.phase.clone()
            };
            if let Ok(json) = serde_json::to_string_pretty(&b) {
                let _ = std::fs::write(self.data_dir.join("boundary.json"), json);
                self.refresh();
            }
        }
    }
    /// The Workshop (rendered in the popout): the familiar narrating itself — what it's
    /// theorizing, what it's testing/mutating and how it scored, and the lessons it has
    /// drawn — at the chosen depth. Brief = terse one-liners; verbose = the full detail the
    /// metabolism already records (hypothesis, lineage, score breakdown, failure, mutation).
    fn workshop_ui(&mut self, ui: &mut egui::Ui) {
        theme::on_screen(ui); // dark screen — text is bright on navy, never dark-on-dark
        let blue = egui::Color32::from_rgb(150, 200, 255);
        let green = egui::Color32::from_rgb(150, 205, 150);
        let amber = egui::Color32::from_rgb(220, 150, 70);

        ui.heading("🧪 Workshop");
        ui.label(
            egui::RichText::new("what the familiar is theorizing, testing, and learning").weak(),
        );
        ui.add_space(4.0);
        // the brief ⇄ verbose switch
        ui.horizontal(|ui| {
            ui.label("narration:");
            ui.selectable_value(&mut self.narration_verbose, false, "Brief");
            ui.selectable_value(&mut self.narration_verbose, true, "Verbose");
        });
        ui.separator();
        let verbose = self.narration_verbose;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // --- Theorizing ---
                ui.label(egui::RichText::new("💭 Theorizing").strong().color(blue));
                let theories: Vec<&Thread> = self
                    .snapshot
                    .threads
                    .iter()
                    .rev()
                    .filter(|t| !t.theory.is_empty())
                    .take(8)
                    .collect();
                if theories.is_empty() {
                    ui.weak("(no theories yet — they form as it interprets what it observes)");
                }
                for t in theories {
                    if verbose {
                        ui.group(|ui| {
                            if !t.question.is_empty() {
                                ui.weak(format!("on: {}", t.question));
                            }
                            ui.label(format!("💭 {}", t.theory));
                            if !t.direction.is_empty() {
                                ui.colored_label(green, format!("→ pursuing: {}", t.direction));
                            }
                            ui.weak(format!("status: {} · origin: {}", t.status, t.origin));
                        });
                    } else {
                        ui.label(format!("💭 {}", truncate_line(&t.theory, 90)));
                    }
                }
                ui.add_space(6.0);

                // --- Testing & mutating ---
                ui.label(egui::RichText::new("🧪 Testing & mutating").strong().color(blue));
                let cands: Vec<&Candidate> = self.snapshot.candidates.iter().rev().take(15).collect();
                if cands.is_empty() {
                    ui.weak("(no candidates yet — work appears once a loop forms)");
                }
                for c in cands {
                    let trial = self
                        .snapshot
                        .trials
                        .iter()
                        .rev()
                        .find(|t| t.candidate_id == c.id);
                    if verbose {
                        ui.group(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{} · {}", c.id, c.status)).strong(),
                            );
                            if !c.hypothesis.is_empty() {
                                ui.label(format!("hypothesis: {}", c.hypothesis));
                            }
                            let lineage = if c.parent_id.is_empty() {
                                format!("gen {}", c.generation)
                            } else {
                                format!("gen {} · from {}", c.generation, c.parent_id)
                            };
                            ui.weak(lineage);
                            if let Some(t) = trial {
                                ui.label(format!("test: {} · overall {:.2}", t.result, t.overall));
                                ui.weak(format!(
                                    "fit {:.2} · clarity {:.2} · useful {:.2} · novelty {:.2} · safety {:.2} · cost {:.2}",
                                    t.fit, t.clarity, t.usefulness, t.novelty, t.safety, t.complexity
                                ));
                                if !t.failure_class.is_empty() && t.failure_class != "none" {
                                    ui.colored_label(amber, format!("failure: {}", t.failure_class));
                                }
                                if !t.notes.is_empty() {
                                    ui.weak(format!("notes: {}", t.notes));
                                }
                            }
                            if !c.mutation_reason.is_empty() {
                                ui.colored_label(
                                    amber,
                                    format!("⤳ mutated: {} — changed {}", c.mutation_reason, c.changed_traits),
                                );
                            }
                        });
                    } else {
                        let line = if !c.mutation_reason.is_empty() {
                            format!("⤳ {} — mutated ({})", truncate_line(&c.hypothesis, 56), c.mutation_reason)
                        } else {
                            let r = trial
                                .map(|t| format!("{} {:.2}", t.result, t.overall))
                                .unwrap_or_else(|| c.status.clone());
                            format!("🧪 {} → {r}", truncate_line(&c.hypothesis, 56))
                        };
                        ui.label(line);
                    }
                }
                ui.add_space(6.0);

                // --- Lessons ---
                ui.label(egui::RichText::new("📚 Lessons learned").strong().color(blue));
                let lessons: Vec<&PatternMemory> = self.snapshot.patterns.iter().rev().take(8).collect();
                if lessons.is_empty() {
                    ui.weak("(no lessons yet — outcomes become patterns, failures are fossils)");
                }
                for p in lessons {
                    if verbose {
                        ui.group(|ui| {
                            ui.label(format!("📚 {}", p.lesson));
                            if !p.applies_when.is_empty() {
                                ui.weak(format!("applies when: {}", p.applies_when));
                            }
                            ui.weak(format!("confidence {:.2}", p.confidence));
                        });
                    } else {
                        ui.label(format!("📚 {}", truncate_line(&p.lesson, 90)));
                    }
                }
            });
    }
    /// An ink section heading on the beige rail (mono, tracked-looking, muted).
    fn rail_label(ui: &mut egui::Ui, text: &str) {
        ui.label(
            egui::RichText::new(text)
                .font(theme::mono(10.0))
                .color(theme::INK_LABEL),
        );
    }
    /// The left rail (T2): the Three Laws as segmented meters, the self-pacing readout, and
    /// the daemon plate. Ink chrome on beige; the meter wells and reused panels are dark.
    fn laws_rail(&mut self, ui: &mut egui::Ui, running: bool) {
        let svc = self.snapshot.service.measure;
        let pres = self.snapshot.presence.measure;
        let cap = self.snapshot.capacities.measure;
        let withdrawn = self.snapshot.presence.withdrawn;
        let served = self.snapshot.service.served_facing;

        Self::rail_label(ui, "THE THREE LAWS");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let meter = |ui: &mut egui::Ui, val: f64, label: &str| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{val:.2}"))
                            .font(theme::mono_semi(13.0))
                            .color(theme::INK_HEAD),
                    );
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(40.0, 88.0), egui::Sense::hover());
                    let lit = if !running {
                        theme::FROZEN
                    } else if val >= 0.66 {
                        theme::GREEN
                    } else if val >= 0.33 {
                        theme::AMBER
                    } else {
                        theme::RED
                    };
                    theme::segmented_meter(ui, rect, val, lit);
                    ui.label(
                        egui::RichText::new(label)
                            .font(theme::mono(9.0))
                            .color(theme::INK_LABEL),
                    );
                });
            };
            meter(ui, svc, "SERVICE");
            meter(ui, pres, "PRESENCE");
            meter(ui, cap, "CAPAC.");
        });
        let (note, col) = if withdrawn {
            ("withdrawn — empty world", theme::RED_DEEP)
        } else if served == 0 {
            ("no served-facing activity yet", theme::AMBER_DEEP)
        } else {
            ("the served are present", theme::INK_MUTED)
        };
        ui.label(egui::RichText::new(note).font(theme::mono(9.0)).color(col));
        if !running {
            ui.label(
                egui::RichText::new("signals frozen — daemon stopped")
                    .font(theme::mono(9.0))
                    .color(theme::INK_MUTED2),
            );
        }
        if !self.snapshot.flagged.is_empty() {
            // Law III, outward: actors who repeatedly tried to break the constitution are
            // flagged; their directives are marginalized so legitimate work proceeds.
            let who = self
                .snapshot
                .flagged
                .iter()
                .map(|(a, n)| format!("{a} ({n})"))
                .collect::<Vec<_>>()
                .join(", ");
            ui.label(
                egui::RichText::new(format!("⛔ corruption watch: {who}"))
                    .font(theme::mono(9.0))
                    .color(theme::RED_DEEP),
            );
        }

        ui.add_space(12.0);
        Self::rail_label(ui, "SELF-PACING · LAW II");
        theme::instrument_screen().show(ui, |ui| {
            theme::on_screen(ui);
            self.budget_meter(ui);
        });

        ui.add_space(12.0);
        Self::rail_label(ui, "DAEMON");
        self.daemon_plate(ui, running);
    }
    /// The daemon plate (dark instrument): status + the process controls.
    fn daemon_plate(&mut self, ui: &mut egui::Ui, running: bool) {
        theme::instrument_screen().show(ui, |ui| {
            theme::on_screen(ui);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("●").color(if running { theme::GREEN } else { theme::RED }),
                );
                ui.label(
                    egui::RichText::new(if running { "RUNNING" } else { "STOPPED" })
                        .font(theme::mono_semi(11.0))
                        .color(theme::SCREEN_BRIGHT),
                );
            });
            let status = self.daemon_status.clone();
            if !status.is_empty() {
                ui.label(
                    egui::RichText::new(status)
                        .font(theme::mono(9.5))
                        .color(theme::SCREEN_TEXT),
                );
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button("▶ Start").clicked() {
                    self.daemon_status = daemon_cmd(&self.data_dir, "start");
                }
                if ui.button("■ Stop").clicked() {
                    self.daemon_status = daemon_cmd(&self.data_dir, "stop");
                }
                if ui.button("↻").clicked() {
                    self.daemon_status = daemon_cmd(&self.data_dir, "reload");
                }
                if ui.button("⏻ login").clicked() {
                    self.daemon_status = daemon_cmd(&self.data_dir, "install");
                }
            });
        });
    }
    /// The right column (T2/T3): vital signs, the Law III gates, the current theory, and the
    /// activity ticker. Ink section labels on beige; the content sits on dark screens.
    fn right_column(&mut self, ui: &mut egui::Ui) {
        // (The vital-signs over-time graph is gone — it was the same three signals the
        // left-rail LED meters already show, just as a trend; the meters carry it.)
        Self::rail_label(ui, "CAPABILITY · LAW III — YOUR GATES");
        theme::instrument_screen().show(ui, |ui| {
            theme::on_screen(ui);
            self.boundary_panel(ui);
        });

        ui.add_space(12.0);
        Self::rail_label(ui, "CURRENT THEORY · WHAT IT'S WORKING");
        let theory = self
            .snapshot
            .threads
            .iter()
            .rev()
            .find(|t| !t.theory.is_empty())
            .cloned();
        // The candidate currently in work — newest first, prefer one still being pursued.
        let cand = self
            .snapshot
            .candidates
            .iter()
            .rev()
            .find(|c| matches!(c.status.as_str(), "generated" | "mutated" | "observing"))
            .or_else(|| self.snapshot.candidates.last())
            .cloned();
        theme::instrument_screen().show(ui, |ui| {
            theme::on_screen(ui);
            match &theory {
                Some(t) => {
                    ui.label(
                        egui::RichText::new(format!("💭 {}", t.theory))
                            .font(theme::serif_italic(14.0))
                            .color(theme::SCREEN_BRIGHT),
                    );
                    if !t.direction.is_empty() {
                        ui.label(
                            egui::RichText::new(format!("→ pursuing: {}", t.direction))
                                .font(theme::mono(10.0))
                                .color(theme::SCREEN_DIM),
                        );
                    }
                }
                None => {
                    ui.label(
                        egui::RichText::new("(no theory yet — it forms as it interprets)")
                            .font(theme::mono(9.5))
                            .color(theme::SCREEN_FAINT),
                    );
                }
            }
            if let Some(c) = &cand {
                ui.add_space(5.0);
                ui.label(
                    egui::RichText::new(format!(
                        "WORKING {} · gen {} · {}",
                        c.id, c.generation, c.status
                    ))
                    .font(theme::mono_semi(10.0))
                    .color(theme::CYAN),
                );
                if !c.hypothesis.is_empty() {
                    ui.label(
                        egui::RichText::new(&c.hypothesis)
                            .font(theme::serif(13.0))
                            .color(theme::SCREEN_TEXT),
                    );
                }
                if !c.mutation_reason.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("⤳ {} — changed {}", c.mutation_reason, c.changed_traits))
                            .font(theme::mono(9.5))
                            .color(theme::AMBER),
                    );
                }
            }
        });

        ui.add_space(12.0);
        Self::rail_label(ui, "ACTIVITY · TICKER");
        theme::instrument_screen().show(ui, |ui| {
            theme::on_screen(ui);
            activity_feed(ui, &self.snapshot.ticks, now_secs(), &mut self.active_scroll);
        });
    }
    /// The conversation transcript — every ask paired with the familiar's answer, newest
    /// first. This is the durable place an answer lands in text; 🔊 speaks it aloud, and
    /// 👍 / ✍ feed back (refine prefills the ask so the answer can be sharpened).
    fn conversation_panel(&mut self, ui: &mut egui::Ui) {
        // Pair each answer with the text of the request it answered. Cloned up front so the
        // egui closure doesn't hold a borrow of self while the buttons want to mutate it.
        let items: Vec<(Answer, Option<String>)> = self
            .snapshot
            .answers
            .iter()
            .rev()
            .take(30)
            .map(|a| {
                let q = self
                    .snapshot
                    .requests
                    .iter()
                    .find(|r| r.id == a.request_id)
                    .map(|r| r.text.clone());
                (a.clone(), q)
            })
            .collect();

        // An action chosen this frame, applied after the closure (which borrows self).
        enum Act {
            Speak(String),
            Feedback(String, &'static str, Option<String>),
        }
        let mut act: Option<Act> = None;

        egui::Frame::group(ui.style())
            .fill(egui::Color32::from_rgb(24, 28, 36))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("💬 Conversation")
                        .strong()
                        .color(egui::Color32::from_rgb(150, 200, 255)),
                );
                if items.is_empty() {
                    ui.weak("(ask the familiar something above — its answers land here)");
                    return;
                }
                scroll_region(ui, "conversation", 300.0, &mut self.active_scroll, |ui| {
                    for (a, q) in &items {
                        ui.group(|ui| {
                            if let Some(q) = q {
                                ui.label(
                                    egui::RichText::new(format!("🔮 you asked: {q}"))
                                        .strong()
                                        .color(egui::Color32::from_rgb(150, 200, 255)),
                                );
                            }
                            confidence_badge(ui, a.confidence);
                            // Evidence on its own line so it wraps to the column — inside a
                            // horizontal layout egui gives it infinite width and it never wraps,
                            // running past the divider into the right column.
                            if !a.evidence.is_empty() {
                                ui.weak(format!("· {}", a.evidence));
                            }
                            ui.label(&a.body);
                            ui.horizontal(|ui| {
                                if ui.button("🔊 speak").clicked() {
                                    act = Some(Act::Speak(a.body.clone()));
                                }
                                if a.feedback.is_empty() {
                                    if ui.button("👍 helpful").clicked() {
                                        act = Some(Act::Feedback(a.id.clone(), "helpful", None));
                                    }
                                    if ui.button("✍ refine").clicked() {
                                        act =
                                            Some(Act::Feedback(a.id.clone(), "refine", q.clone()));
                                    }
                                }
                            });
                            // The feedback note wraps on its own line, for the same reason.
                            if !a.feedback.is_empty() {
                                ui.weak(format!("✓ you marked this: {}", a.feedback));
                            }
                        });
                    }
                });
            });

        match act {
            Some(Act::Speak(text)) => speak(&text),
            Some(Act::Feedback(id, kind, prefill)) => self.give_feedback(&id, kind, prefill),
            None => {}
        }
    }
    /// Record the observer's reply as an observation — the observer's input channel (the
    /// one place the GUI writes; the familiar's own truth stays read-only).
    fn submit_response(&mut self) {
        let resp = self.response.trim();
        if resp.is_empty() {
            return;
        }
        let q = current_question(&self.data_dir);
        let object: String = resp.chars().take(200).collect();
        let actor = self.observer_handle();
        let obs = Observation::new(
            &actor,
            "needs",
            object,
            format!("q='{q}' response='{resp}'"),
            "observer",
            now_secs(),
            1.0,
        );
        let _ = observation::record(&self.data_dir, obs);
        // close the familiar's open question: mark the latest open thread answered.
        if let Some(open) = self
            .snapshot
            .threads
            .iter()
            .rev()
            .find(|t| t.status == "open")
        {
            let _ = thread::update_status(&self.data_dir, &open.id, "answered");
        }
        // steer: the observer's answer becomes an open thread with their words as the
        // direction, so the familiar pursues what *they* said they need, not just what it
        // inferred.
        let seq = self.snapshot.threads.len() + 1;
        let _ = thread::append(
            &self.data_dir,
            &Thread {
                id: format!("thread-{seq:04}"),
                question: q.clone(),
                theory: format!("{} said: {resp}", self.observer_name()),
                direction: resp.to_string(),
                created_at: now_secs(),
                status: "open".into(),
                origin: "observer".into(),
                actor,
            },
        );
        // mark the registry question answered (it rests, then recurs when appropriate) and
        // clear the active slot so the factory coordinates the next one.
        if let Some(id) = self.active_question_id() {
            let _ = question::record_answered(&self.data_dir, &id, now_secs());
        }
        self.clear_active_question();
        // remember the answered question so it fades out and can't be answered twice;
        // persist it so a restart doesn't re-show it.
        let _ = std::fs::write(self.data_dir.join("last_answered.txt"), &q);
        self.answered_question = Some(q);
        self.response.clear();
        self.refresh();
    }
    /// The id of the question currently on screen (set by the factory when it surfaces one).
    fn active_question_id(&self) -> Option<String> {
        std::fs::read_to_string(self.data_dir.join("active_question.txt"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
    fn clear_active_question(&self) {
        let _ = std::fs::write(self.data_dir.join("active_question.txt"), "");
    }
    /// Dismiss the current question — never answered, never disposed. The familiar records
    /// the dismissal (it rests longer the more it's waved off) and will ask again at a time
    /// it judges right. Law III: an ask is never a demand; declining is always allowed and
    /// always honored.
    fn dismiss_question(&mut self) {
        let q = current_question(&self.data_dir);
        if let Some(id) = self.active_question_id() {
            let _ = question::record_dismissed(&self.data_dir, &id, now_secs(), "");
        }
        self.clear_active_question();
        let _ = std::fs::write(self.data_dir.join("last_answered.txt"), &q);
        self.answered_question = Some(q);
        self.response.clear();
        self.refresh();
    }
}

impl eframe::App for Glass {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply the human's chosen text-zoom (on top of the display's native scale).
        if (ctx.zoom_factor() - self.ui_scale).abs() > 0.001 {
            ctx.set_zoom_factor(self.ui_scale);
        }
        // Occasional availability check: re-probe the LLM providers every few minutes when
        // the gate is open, so a flagged provider is re-tested and rolled back in if it
        // recovers — and the Glass shows current health without the human asking.
        if self.snapshot.boundary.allow_llm
            && self.last_probe.elapsed() >= std::time::Duration::from_secs(300)
        {
            self.probe_llm();
            self.last_probe = std::time::Instant::now();
        }
        // Track the daemon live: reload the snapshot on a throttle (not only when Ian acts),
        // so observations, the activity feed, the chart, and new questions stay current —
        // and a wiped data dir reflects immediately rather than lingering as a stale view.
        // The throttle keeps file reads modest; in-progress edits (ask/response/params) live
        // in separate fields, so a reload never clobbers them.
        if self.last_refresh.elapsed() >= std::time::Duration::from_secs(1) {
            self.refresh();
            self.last_refresh = std::time::Instant::now();
        }
        let running = self.daemon_status.contains("running");

        // ---- titlebar: cockpit chrome ----
        egui::TopBottomPanel::top("titlebar")
            .frame(theme::panel(theme::RAIL_LIGHT).inner_margin(egui::Margin::symmetric(16, 9)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("◉ ◉")
                            .font(theme::mono(11.0))
                            .color(theme::HAIRLINE),
                    );
                    ui.label(
                        egui::RichText::new("THE GLASS")
                            .font(theme::mono_semi(13.0))
                            .color(theme::INK_LABEL),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let dot = if running { theme::GREEN } else { theme::RED };
                        let status = if self.daemon_status.is_empty() {
                            "stopped".to_string()
                        } else {
                            self.daemon_status.clone()
                        };
                        // status on a dark chip — bright text on navy, unmistakably readable
                        theme::instrument_screen()
                            .inner_margin(egui::Margin::symmetric(8, 3))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("●").font(theme::mono(9.0)).color(dot));
                                ui.label(
                                    egui::RichText::new(status)
                                        .font(theme::mono(10.0))
                                        .color(theme::SCREEN_BRIGHT),
                                );
                            });
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new("Local · No network")
                                .font(theme::mono(10.0))
                                .color(theme::INK_LABEL),
                        );
                        ui.add_space(6.0);
                        if ui
                            .button(egui::RichText::new("A+").font(theme::mono(12.0)))
                            .clicked()
                        {
                            self.ui_scale = (self.ui_scale + 0.1).clamp(0.8, 2.4);
                            write_ui_scale(&self.data_dir, self.ui_scale);
                        }
                        if ui
                            .button(egui::RichText::new("A−").font(theme::mono(12.0)))
                            .clicked()
                        {
                            self.ui_scale = (self.ui_scale - 0.1).clamp(0.8, 2.4);
                            write_ui_scale(&self.data_dir, self.ui_scale);
                        }
                        ui.label(
                            egui::RichText::new(format!("{:.0}%", self.ui_scale * 100.0))
                                .font(theme::mono(9.0))
                                .color(theme::INK_MUTED),
                        );
                    });
                });
            });

        // ---- identity strip (once the familiar knows who it serves) ----
        if let Some(name) = self.observer.as_ref().map(|o| o.name.clone()) {
            egui::TopBottomPanel::top("identity")
                .frame(theme::panel(theme::RAIL_LIGHT).inner_margin(egui::Margin::symmetric(18, 6)))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("●")
                                .font(theme::mono(8.0))
                                .color(theme::GREEN_DEEP),
                        );
                        ui.label(
                            egui::RichText::new("known to the familiar as")
                                .font(theme::mono(10.5))
                                .color(theme::INK_LABEL),
                        );
                        ui.label(
                            egui::RichText::new(&name)
                                .font(theme::serif_italic(14.0))
                                .color(theme::INK_HEAD),
                        );
                        ui.label(
                            egui::RichText::new("— names are not forgotten")
                                .font(theme::mono(9.5))
                                .color(theme::INK_MUTED),
                        );
                    });
                });
        }

        // ---- footer: disclosures, calm by default ----
        egui::TopBottomPanel::bottom("footer")
            .frame(theme::panel(theme::RAIL_DARK).inner_margin(egui::Margin::symmetric(16, 7)))
            .show(ctx, |ui| {
                let lbl = |on: bool, s: &str| {
                    egui::RichText::new(s).font(theme::mono(10.0)).color(if on {
                        theme::INK_HEAD
                    } else {
                        theme::INK_LABEL
                    })
                };
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(lbl(self.show_substrate, "▸ INSPECT SUBSTRATE"))
                                .frame(false)
                                .fill(egui::Color32::TRANSPARENT),
                        )
                        .clicked()
                    {
                        self.show_substrate = !self.show_substrate;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(lbl(self.show_settings, "⚙ PARAMETERS"))
                                    .frame(false)
                                    .fill(egui::Color32::TRANSPARENT),
                            )
                            .clicked()
                        {
                            self.show_settings = !self.show_settings;
                        }
                    });
                });
            });

        // ---- left rail (T2) ----
        // Resizable: drag its right edge to set the rail's width. The center column takes
        // whatever is left between the two rails, so its right edge is always the right
        // column's left edge — never the window edge.
        egui::SidePanel::left("rail")
            .resizable(true)
            .default_width(218.0)
            .width_range(160.0..=380.0)
            .frame(theme::panel(theme::RAIL_LIGHT).inner_margin(egui::Margin::same(14)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("rail-scroll")
                    .show(ui, |ui| self.laws_rail(ui, running));
            });

        // ---- right column (T2/T3) ----
        // Resizable: drag its left edge (the boundary with the center) to set its width.
        egui::SidePanel::right("rightcol")
            .resizable(true)
            .default_width(334.0)
            .width_range(240.0..=520.0)
            .frame(theme::panel(theme::RAIL_LIGHT).inner_margin(egui::Margin::same(14)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("rightcol-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.right_column(ui));
            });

        // ---- center (T1): the conversation, on the main dark screen ----
        egui::CentralPanel::default()
            .frame(theme::panel(theme::NAVY).inner_margin(egui::Margin::same(16)))
            .show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            theme::on_screen(ui); // the center is a dark screen — text is bright on it
            if let Some(err) = &self.snapshot.error {
                ui.colored_label(theme::RED, err);
            }

            // --- first run: connect the familiar to a mind (collapses once connected) ---
            ui.add_space(6.0);
            self.connect_panel(ui);

            // --- optional: federate with peer familiars over the tailnet (collapsed once
            // in a group). Sharing only runs while the Mesh gate is open in Capability. ---
            ui.add_space(6.0);
            self.mesh_panel(ui);

            // --- the interaction channel: the familiar asks, the observer answers. Until
            // it knows who it serves, that exchange is learning a name — precise, confirmed,
            // and kept (the familiar's own choice to become familiar). ---
            ui.add_space(6.0);
            if self.observer.is_none() {
                self.name_panel(ui);
            } else {
            let question = current_question(&self.data_dir);
            let already_answered = self.answered_question.as_deref() == Some(question.as_str());
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(28, 34, 44))
                .show(ui, |ui| {
                    if already_answered {
                        // answered or dismissed — fade it out so it isn't acted on twice; the
                        // factory brings the next (or this one again) when it judges the moment.
                        ui.label(
                            egui::RichText::new(
                                "✓ noted — the familiar will ask again when it's useful",
                            )
                            .italics()
                            .color(egui::Color32::from_rgb(150, 205, 150)),
                        );
                        return;
                    }
                    ui.label(
                        egui::RichText::new(&question)
                            .heading()
                            .color(egui::Color32::from_rgb(150, 200, 255)),
                    );
                    ui.add(
                        egui::TextEdit::multiline(&mut self.response)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY)
                            .background_color(theme::CREAM)
                            .text_color(theme::INK)
                            .hint_text(egui::RichText::new("type your answer…").color(theme::INK_MUTED)),
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Send").clicked() {
                            self.submit_response();
                        }
                        // Law III: a question can always be set aside, never forced. A
                        // dismissal isn't discarded — it's tracked, and the familiar asks
                        // again at a time it judges right.
                        if ui.button("Dismiss").clicked() {
                            self.dismiss_question();
                        }
                        ui.label(
                            egui::RichText::new("answer, or dismiss — your call")
                                .weak()
                                .small(),
                        );
                    });
                });
            }

            // --- ask the familiar: a free-form request, answered with everything it has ---
            ui.add_space(6.0);
            let pending_requests = self
                .snapshot
                .requests
                .iter()
                .filter(|r| r.status == "open")
                .count();
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(26, 30, 40))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new("🔮 Ask the familiar")
                            .strong()
                            .color(egui::Color32::from_rgb(150, 200, 255)),
                    );
                    ui.add(
                        egui::TextEdit::multiline(&mut self.ask)
                            .desired_rows(2)
                            .desired_width(f32::INFINITY)
                            .background_color(theme::CREAM)
                            .text_color(theme::INK)
                            .hint_text(
                                egui::RichText::new("e.g. do I have any network-configuration issues?")
                                    .color(theme::INK_MUTED),
                            ),
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Ask").clicked() {
                            self.submit_request();
                        }
                        ui.weak(
                            "answered from what it can verify — labeled known or probable, never a guess",
                        );
                    });
                    if pending_requests > 0 {
                        ui.weak(format!(
                            "⏳ {pending_requests} pending — the familiar is working on it; the \
                             answer appears in the Conversation below"
                        ));
                    }
                });

            // --- the Conversation: the place the familiar answers, in text and (on request)
            // aloud. Every ask is paired with its answer, newest first — wherever the
            // answer came from (verified facts, a reused tool, a freshly written one, the
            // LLM), it lands here. ---
            ui.add_space(6.0);
            self.conversation_panel(ui);

            // The Workshop opens in its own popout window (the brief's window into the work).
            ui.add_space(8.0);
            if ui
                .button("🧪 Open Workshop — watch it theorize, test, and learn")
                .clicked()
            {
                self.workshop_open = true;
            }

            // T5 — the raw substrate, closed by default (toggled from the footer).
            if self.show_substrate {
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(format!("TOOLS · skills it reuses ({})", self.snapshot.tools.len()))
                        .font(theme::mono_semi(11.0))
                        .color(theme::SCREEN_DIM),
                );
                tools_panel(ui, &self.snapshot.tools, &mut self.active_scroll);
                ui.separator();
                ui.label(
                    egui::RichText::new("THEORIES & THREADS")
                        .font(theme::mono_semi(11.0))
                        .color(theme::SCREEN_DIM),
                );
                threads_panel(ui, &self.snapshot.threads, &mut self.active_scroll);
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "LOOPS ({}) · CANDIDATES ({})",
                        self.snapshot.loops.len(),
                        self.snapshot.candidates.len()
                    ))
                    .font(theme::mono_semi(11.0))
                    .color(theme::SCREEN_DIM),
                );
                if self.snapshot.loops.is_empty() {
                    ui.weak("(no loops yet — recurring observations form loops)");
                }
                for lp in &self.snapshot.loops {
                    let n = self
                        .snapshot
                        .candidates
                        .iter()
                        .filter(|c| c.loop_id == lp.id)
                        .count();
                    ui.label(format!(
                        "↻ {}  (x{}, conf {:.2}) — {n} candidate(s)",
                        lp.name, lp.observation_count, lp.confidence
                    ));
                }
                ui.separator();
                ui.label(
                    egui::RichText::new("OBSERVATIONS · the only truth")
                        .font(theme::mono_semi(11.0))
                        .color(theme::SCREEN_DIM),
                );
                if self.snapshot.observations.is_empty() {
                    ui.weak("(no observations yet)");
                }
                egui::Grid::new("obs")
                    .striped(true)
                    .num_columns(4)
                    .show(ui, |ui| {
                        for o in &self.snapshot.observations {
                            let served = service::is_served_facing(o);
                            ui.colored_label(
                                if served { theme::GREEN } else { theme::SCREEN_FAINT },
                                if served { "•" } else { " " },
                            );
                            ui.label(&o.id);
                            ui.label(format!("{} {} {}", o.actor, o.action, o.object));
                            ui.weak(&o.context);
                            ui.end_row();
                        }
                    });
            }

            // T4 — shared parameters, behind the footer toggle.
            if self.show_settings {
                ui.add_space(10.0);
                if settings_panel(ui, &mut self.params_edit, &self.data_dir) {
                    self.snapshot = Snapshot::load(&self.data_dir);
                }
            }
            }); // end ScrollArea
        });

        // The Workshop — a real popout window onto what the familiar is theorizing, testing,
        // and learning. Lives in its own OS window so it can sit beside the Glass.
        if self.workshop_open {
            let mut close = false;
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("familiar-workshop"),
                egui::ViewportBuilder::default()
                    .with_title("The Familiar — Workshop")
                    .with_inner_size([580.0, 760.0]),
                |ctx, _class| {
                    // The Workshop is a dark instrument screen (its labels are the bright
                    // navy-screen colours), so frame it navy — never the beige chassis, which
                    // would put bright text on a light field.
                    egui::CentralPanel::default()
                        .frame(theme::panel(theme::NAVY).inner_margin(egui::Margin::same(16)))
                        .show(ctx, |ui| self.workshop_ui(ui));
                    if ctx.input(|i| i.viewport().close_requested()) {
                        close = true;
                    }
                },
            );
            if close {
                self.workshop_open = false;
            }
        }

        // gentle auto-refresh so the window tracks the familiar as it runs
        ctx.request_repaint_after(std::time::Duration::from_secs(2));
    }

    /// Clear any area the panels don't cover to the chassis colour — never black. (egui
    /// fills the window with the panels, but this guarantees no dark gap on a fast resize.)
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        theme::CHASSIS_DARK.to_normalized_gamma_f32()
    }
}

/// A bounded scroll region that only scrolls once the human has *clicked into it* — so
/// merely hovering it while scrolling the page no longer hijacks the gesture (the trackpad
/// pain with nested scroll areas). Clicking a different region hands focus over; the active
/// region shows a faint accent border. `active` holds the id of the selected region.
fn scroll_region(
    ui: &mut egui::Ui,
    id: &str,
    max_height: f32,
    active: &mut Option<String>,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let is_active = active.as_deref() == Some(id);
    let out = egui::ScrollArea::vertical()
        .max_height(max_height)
        .id_salt(id)
        .enable_scrolling(is_active)
        .show(ui, add_contents);
    // A press anywhere inside the visible region selects it. Detected via the pointer
    // position (not a click interaction) so it never swallows clicks on inner buttons.
    let pressed_inside = ui.input(|i| {
        i.pointer.any_pressed()
            && i.pointer
                .interact_pos()
                .is_some_and(|p| out.inner_rect.contains(p))
    });
    if pressed_inside {
        *active = Some(id.to_string());
    }
    if is_active {
        ui.painter().rect_stroke(
            out.inner_rect,
            egui::CornerRadius::same(3),
            egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 130, 180)),
            egui::StrokeKind::Inside,
        );
    } else {
        ui.weak(egui::RichText::new("click to scroll this panel").small());
    }
}

/// A feed of the most recent *consequential* ticks (skipping quiet ones), newest first —
/// so the human sees the familiar theorize, pursue, test, and promote as it happens.
fn activity_feed(ui: &mut egui::Ui, ticks: &[ActivityTick], now: i64, active: &mut Option<String>) {
    scroll_region(ui, "activity", 170.0, active, |ui| {
        let mut shown = 0;
        for t in ticks.iter().rev() {
            if shown >= 14 {
                break;
            }
            if t.quiet() {
                continue;
            }
            shown += 1;
            let mut parts: Vec<String> = Vec::new();
            if t.theorized {
                parts.push("💭 theorized".into());
            }
            if t.pursued > 0 {
                parts.push(format!("→ pursued {}", t.pursued));
            }
            if t.sensed > 0 {
                parts.push(format!("👁 sensed {}", t.sensed));
            }
            if t.new_candidates > 0 {
                parts.push(format!("✦ {} new", t.new_candidates));
            }
            if t.tested > 0 {
                parts.push(format!("✓ tested {}", t.tested));
            }
            if t.promoted > 0 {
                parts.push(format!("↑ promoted {}", t.promoted));
            }
            if t.mutated > 0 {
                parts.push(format!("⤳ mutated {}", t.mutated));
            }
            if t.archived > 0 {
                parts.push(format!("🗄 archived {}", t.archived));
            }
            if t.reverted > 0 {
                parts.push(format!("↩ reverted {} setting(s)", t.reverted));
            }
            if t.marginalized > 0 {
                parts.push(format!("⛔ marginalized {}", t.marginalized));
            }
            if t.answered > 0 {
                parts.push(format!("🔮 answered {}", t.answered));
            }
            if t.refused > 0 {
                parts.push(format!("⛔ refused {} request(s)", t.refused));
            }
            if t.declined > 0 {
                parts.push(format!("🛑 declined to run {}", t.declined));
            }
            if t.structural_changed {
                parts.push("⚙ world changed".into());
            }
            ui.label(format!("{:>5}  {}", ago(now, t.ts), parts.join("  ·  ")));
        }
        if shown == 0 {
            ui.weak(
                "(no actions yet — the familiar acts as loops form and you answer its questions)",
            );
        }
    });
}

/// A compact relative timestamp like `12s`, `5m`, `2h`.
fn ago(now: i64, then: i64) -> String {
    let d = (now - then).max(0);
    if d < 90 {
        format!("{d}s")
    } else if d < 5400 {
        format!("{}m", d / 60)
    } else {
        format!("{}h", d / 3600)
    }
}

/// The familiar's theories and threads — its questions, interpretations, and the
/// directions it pursued (its own, `llm`; and Ian's answers, `observer`).
fn threads_panel(ui: &mut egui::Ui, threads: &[Thread], active: &mut Option<String>) {
    if threads.is_empty() {
        ui.weak("(no theories yet — they form as the familiar interprets what it observes)");
        return;
    }
    scroll_region(ui, "threads", 220.0, active, |ui| {
        for t in threads.iter().rev().take(20) {
            let color = match t.status.as_str() {
                "open" => egui::Color32::from_rgb(150, 200, 255),
                "pursued" => egui::Color32::from_rgb(180, 180, 140),
                "answered" => egui::Color32::from_rgb(150, 205, 150),
                _ => egui::Color32::from_rgb(176, 184, 198),
            };
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(color, format!("[{}]", t.status));
                    ui.weak(format!("{} · {}", t.id, t.origin));
                });
                if !t.theory.is_empty() {
                    ui.label(format!("💭 {}", t.theory));
                }
                if !t.direction.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("→ {}", t.direction))
                            .small()
                            .color(egui::Color32::from_rgb(160, 190, 160)),
                    );
                }
            });
        }
    });
}

/// The familiar's tool library — the scripts it authored once and now reuses instead of
/// re-asking the LLM (Law I made visible: the future cheaper than the past). Newest first,
/// each with its purpose and how many times it has paid off.
fn tools_panel(ui: &mut egui::Ui, tools: &[Tool], active: &mut Option<String>) {
    if tools.is_empty() {
        ui.weak("(no tools yet — the familiar saves a tool the first time it writes one to run something)");
        return;
    }
    scroll_region(ui, "tools", 220.0, active, |ui| {
        for t in tools.iter().rev() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    let color = if t.last_exit_ok {
                        egui::Color32::from_rgb(150, 200, 255)
                    } else {
                        egui::Color32::from_rgb(200, 120, 80)
                    };
                    ui.colored_label(color, format!("🛠 {}", t.name));
                    ui.weak(if t.uses == 1 {
                        "· 1 use".to_string()
                    } else {
                        format!("· {} uses", t.uses)
                    });
                    if !t.last_exit_ok {
                        ui.colored_label(
                            egui::Color32::from_rgb(200, 120, 80),
                            "· last run failed",
                        );
                    }
                });
                if !t.purpose.is_empty() {
                    ui.label(&t.purpose);
                }
            });
        }
    });
}

/// The shared-parameters editor. Returns true when Ian saved a change. Co-ownership
/// (the familiar reviewing/reverting Ian's edits under the Three Laws) is a later brick;
/// for now these are plain sliders, clamped sane before they reach disk.
fn settings_panel(ui: &mut egui::Ui, params: &mut Parameters, dir: &Path) -> bool {
    ui.label(
        egui::RichText::new(
            "shared parameters. You set these; the familiar co-owns them — a value outside \
             what it can justify under the Three Laws, it puts back (↩, shown in the feed). \
             Within the envelope, the choice is yours. Eventually this becomes view-only.",
        )
        .weak()
        .small(),
    );
    egui::Grid::new("settings").num_columns(3).show(ui, |ui| {
        ui.label("theorize every (s)");
        ui.add(egui::Slider::new(
            &mut params.theorize_every_secs,
            30..=7200,
        ));
        ui.weak("envelope 60–21600");
        ui.end_row();
        ui.label("cadence floor (s)");
        ui.add(egui::Slider::new(&mut params.interval_floor_secs, 5..=600));
        ui.weak("envelope 15–600");
        ui.end_row();
        ui.label("cadence ceiling (s)");
        ui.add(egui::Slider::new(
            &mut params.interval_ceiling_secs,
            60..=3600,
        ));
        ui.weak("envelope 60–3600");
        ui.end_row();
    });
    if params.last_set_by == "familiar" {
        ui.colored_label(
            egui::Color32::from_rgb(180, 180, 140),
            "↩ the familiar last adjusted these — a prior change fell outside the Three Laws.",
        );
    }
    let mut saved = false;
    ui.horizontal(|ui| {
        if ui.button("Save").clicked() {
            params.last_set_by = "observer".into();
            *params = params.clone().sane();
            let _ = params.save(dir);
            saved = true;
        }
        ui.weak("takes effect next tick (the familiar reviews it) / daemon reload");
    });
    saved
}

/// The confidence badge on an answer — the visible promise of no misinformation: a green
/// "known", an amber "probable", or a grey "unknown" (it would rather say so than guess).
fn confidence_badge(ui: &mut egui::Ui, c: Confidence) {
    let (txt, col) = match c {
        Confidence::Known => ("● known", egui::Color32::from_rgb(80, 200, 120)),
        Confidence::Probable => ("◐ probable", egui::Color32::from_rgb(220, 180, 80)),
        Confidence::Unknown => ("○ unknown", egui::Color32::from_rgb(176, 184, 198)),
    };
    ui.colored_label(col, egui::RichText::new(txt).strong());
}

fn data_dir_from_args() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    for w in args.windows(2) {
        if w[0] == "--data-dir" {
            return PathBuf::from(&w[1]);
        }
    }
    PathBuf::from(familiar_kernel::store::DEFAULT_DATA_DIR)
}

fn main() -> eframe::Result<()> {
    let data_dir = data_dir_from_args();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 940.0]),
        ..Default::default()
    };
    eframe::run_native(
        "The Familiar — the Glass",
        options,
        Box::new(|cc| {
            install_theme(&cc.egui_ctx);
            Ok(Box::new(Glass::new(data_dir)))
        }),
    )
}
