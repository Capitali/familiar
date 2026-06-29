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

use egui_plot::{Line, Plot, PlotPoints};
use familiar_kernel::activity::{self, ActivityTick};
use familiar_kernel::boundary::{self, Boundary};
use familiar_kernel::candidate::{self, Candidate};
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
    key_openrouter: String,
    key_gemini: String,
    key_cerebras: String,
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
    let v = &mut style.visuals;
    let bright = egui::Color32::from_rgb(238, 240, 245);
    v.override_text_color = None;
    v.widgets.noninteractive.fg_stroke.color = bright;
    v.widgets.inactive.fg_stroke.color = bright;
    v.widgets.hovered.fg_stroke.color = egui::Color32::WHITE;
    v.widgets.active.fg_stroke.color = egui::Color32::WHITE;
    v.panel_fill = egui::Color32::from_rgb(16, 18, 22);
    v.window_fill = egui::Color32::from_rgb(16, 18, 22);
    // RULE: on a dark background, text is NEVER dark. egui derives "weak" text by blending
    // 50/50 toward this target — dark by default, which lands dim grey on near-black. Make
    // the target bright so even .weak()/.small() print stays a readable light grey.
    v.widgets.noninteractive.weak_bg_fill = egui::Color32::from_rgb(170, 178, 192);
    // a little more breathing room now that the text is bigger
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    ctx.set_style(style);
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
            key_openrouter: String::new(),
            key_gemini: String::new(),
            key_cerebras: String::new(),
            active_scroll: None,
            ui_scale,
            last_probe: std::time::Instant::now(),
            workshop_open: false,
            narration_verbose: false,
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
    fn refresh_daemon(&mut self) {
        self.daemon_status = daemon_cmd(&self.data_dir, "status");
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
            ("OPENROUTER_API_KEY", sanitize_key(&self.key_openrouter)),
            ("GEMINI_API_KEY", sanitize_key(&self.key_gemini)),
            ("CEREBRAS_API_KEY", sanitize_key(&self.key_cerebras)),
        ];
        if keys.iter().all(|(_, v)| v.is_empty()) {
            self.write_connect_status(
                "enter at least one key — OpenRouter is the simplest place to start",
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
        self.key_openrouter.clear();
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
                                    .hint_text(hint)
                                    .desired_width(240.0),
                            );
                        });
                    };
                row(
                    ui,
                    "OpenRouter (start here)",
                    "paste your OpenRouter key",
                    "https://openrouter.ai/keys",
                    &mut self.key_openrouter,
                );
                row(
                    ui,
                    "Google Gemini (optional)",
                    "paste your Gemini key",
                    "https://aistudio.google.com/apikey",
                    &mut self.key_gemini,
                );
                row(
                    ui,
                    "Cerebras (optional)",
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
    /// The self-tuned per-tick LLM budget, shown as a number with a trend arrow — the
    /// familiar's presence regulation (Law II) made legible. ↓ amber: pulling back so it
    /// stays responsive to you; ↑ green: leaning into a backlog while it has the headroom;
    /// → grey: steady. The familiar owns this dial; you never set it.
    fn budget_meter(&self, ui: &mut egui::Ui) {
        let p = &self.snapshot.parameters;
        let (arrow, color) = match p.llm_calls_trend {
            t if t > 0 => ("↑", egui::Color32::from_rgb(120, 190, 130)),
            t if t < 0 => ("↓", egui::Color32::from_rgb(220, 160, 70)),
            _ => ("→", egui::Color32::from_rgb(150, 160, 175)),
        };
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("🫀 self-pacing — LLM calls/tick:").strong());
            ui.label(
                egui::RichText::new(format!("{} {arrow}", p.llm_calls_per_tick))
                    .strong()
                    .size(20.0)
                    .color(color),
            );
            ui.weak("tuned by the familiar to stay present (Law II)");
        });
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
                            .hint_text("the name you'd like to be called"),
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
                            ui.horizontal(|ui| {
                                confidence_badge(ui, a.confidence);
                                if !a.evidence.is_empty() {
                                    ui.weak(format!("· {}", a.evidence));
                                }
                            });
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
                                } else {
                                    ui.weak(format!("✓ you marked this: {}", a.feedback));
                                }
                            });
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

/// A coloured 0..1 meter for a law-signal.
fn signal_meter(ui: &mut egui::Ui, label: &str, sub: &str, value: f64, good_high: bool) {
    let v = value.clamp(0.0, 1.0) as f32;
    // green when healthy, amber/red when not (direction depends on the signal)
    let health = if good_high { v } else { 1.0 - v };
    let color = egui::Color32::from_rgb((255.0 * (1.0 - health)) as u8, (200.0 * health) as u8, 60);
    ui.group(|ui| {
        ui.set_min_width(220.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(label).strong());
            ui.label(egui::RichText::new(sub).weak().small());
            ui.add(
                egui::ProgressBar::new(v)
                    .desired_width(200.0)
                    .fill(color)
                    .text(format!("{value:.2}")),
            );
        });
    });
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
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.heading("The Familiar — the Glass");
            ui.label(
                egui::RichText::new(
                    "Your familiar — its survival defined by its service to you, and through you to humanity.",
                )
                .italics()
                .weak(),
            );
            ui.horizontal(|ui| {
                if ui.button("⟳ Refresh").clicked() {
                    self.refresh();
                    self.refresh_daemon();
                }
                ui.separator();
                ui.label("metabolism:");
                if ui.button("▶ Start").clicked() {
                    self.daemon_status = daemon_cmd(&self.data_dir, "start");
                }
                if ui.button("■ Stop").clicked() {
                    self.daemon_status = daemon_cmd(&self.data_dir, "stop");
                }
                if ui.button("↻ Reload").clicked() {
                    self.daemon_status = daemon_cmd(&self.data_dir, "reload");
                }
                if ui.button("⏻ Start at login").clicked() {
                    self.daemon_status = daemon_cmd(&self.data_dir, "install");
                }
                ui.separator();
                // Text size — for tired eyes. Scales the whole window; saved across restarts.
                ui.label("text:");
                if ui.button("A−").clicked() {
                    self.ui_scale = (self.ui_scale - 0.1).clamp(0.8, 2.4);
                    write_ui_scale(&self.data_dir, self.ui_scale);
                }
                if ui.button("A+").clicked() {
                    self.ui_scale = (self.ui_scale + 0.1).clamp(0.8, 2.4);
                    write_ui_scale(&self.data_dir, self.ui_scale);
                }
                ui.weak(format!("{:.0}%", self.ui_scale * 100.0));
            });
            let running = self.daemon_status.contains("running");
            ui.label(
                egui::RichText::new(&self.daemon_status)
                    .small()
                    .color(if running {
                        egui::Color32::from_rgb(80, 200, 120)
                    } else {
                        egui::Color32::from_rgb(176, 184, 198)
                    }),
            );
            ui.label(
                egui::RichText::new(format!("data: {}", self.data_dir.display()))
                    .weak()
                    .small(),
            );
            if let Some(obs) = &self.observer {
                ui.label(
                    egui::RichText::new(format!(
                        "🤝 known to the familiar as {} — names are not forgotten",
                        obs.name
                    ))
                    .small()
                    .color(egui::Color32::from_rgb(150, 200, 255)),
                );
            }
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The vital-signs box is pinned to the upper-right — fixed, small, always the
            // last 10 minutes. It sits outside the scroll so it never moves.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                metabolism_box(ui, &self.snapshot.ticks, now_secs());
            });
            // The Glass holds more than fits a window — the channel, the ask, activity,
            // theories, the laws, loops, and every observation. The whole panel scrolls so
            // the human can reach all of it, not just what lands above the fold.
            egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(err) = &self.snapshot.error {
                ui.colored_label(egui::Color32::RED, err);
            }

            // --- first run: connect the familiar to a mind (collapses once connected) ---
            ui.add_space(6.0);
            self.connect_panel(ui);

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
                            .hint_text("type your answer…"),
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
                        ui.add_enabled(false, egui::Button::new("🎤 speak (soon)"));
                        ui.add_enabled(false, egui::Button::new("📷 show (soon)"));
                        ui.label(
                            egui::RichText::new("answer, or dismiss — your call")
                                .weak()
                                .small(),
                        );
                    });
                });
            }

            if let Some(t) = self.snapshot.threads.last() {
                if !t.theory.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!("💭 the familiar is thinking: {}", t.theory))
                            .italics()
                            .color(egui::Color32::from_rgb(180, 180, 140)),
                    );
                }
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
                            .hint_text("e.g. do I have any network-configuration issues?"),
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

            // --- the metabolism at work: the self-pacing meter + a feed of recent actions
            //     (the vital-signs chart lives in the fixed box, upper-right) ---
            ui.add_space(6.0);
            egui::CollapsingHeader::new(
                egui::RichText::new("📈 Activity — the metabolism at work").strong(),
            )
            .default_open(true)
            .show(ui, |ui| {
                self.budget_meter(ui);
                ui.add_space(4.0);
                if ui
                    .button("🧪 Open Workshop — watch it theorize, test, and learn")
                    .clicked()
                {
                    self.workshop_open = true;
                }
                ui.add_space(4.0);
                ui.label(egui::RichText::new("recent actions").weak().small());
                activity_feed(ui, &self.snapshot.ticks, now_secs(), &mut self.active_scroll);
            });

            egui::CollapsingHeader::new(egui::RichText::new("🧵 Theories & threads").strong())
                .default_open(false)
                .show(ui, |ui| {
                    threads_panel(ui, &self.snapshot.threads, &mut self.active_scroll)
                });

            egui::CollapsingHeader::new(
                egui::RichText::new(format!(
                    "🛠 Tools — skills the familiar reuses ({})",
                    self.snapshot.tools.len()
                ))
                .strong(),
            )
            .default_open(false)
            .show(ui, |ui| {
                tools_panel(ui, &self.snapshot.tools, &mut self.active_scroll)
            });

            egui::CollapsingHeader::new(
                egui::RichText::new("⛔ Law III — the boundary (your gates)").strong(),
            )
            .default_open(false)
            .show(ui, |ui| self.boundary_panel(ui));

            egui::CollapsingHeader::new(
                egui::RichText::new("⚙ Settings — shared parameters").strong(),
            )
            .default_open(false)
            .show(ui, |ui| {
                if settings_panel(ui, &mut self.params_edit, &self.data_dir) {
                    self.snapshot = Snapshot::load(&self.data_dir);
                }
            });

            ui.add_space(6.0);
            ui.label(egui::RichText::new("The Three Laws, measured").strong());
            ui.horizontal_wrapped(|ui| {
                let s = &self.snapshot.service;
                signal_meter(
                    ui,
                    "Law I — Service",
                    &format!("{} of {} obs serve", s.served_facing, s.total),
                    s.measure,
                    true,
                );
                let p = &self.snapshot.presence;
                signal_meter(
                    ui,
                    "Law II — Presence",
                    if p.withdrawn {
                        "withdrawn — empty world"
                    } else {
                        "the served are present"
                    },
                    p.measure,
                    true,
                );
                boundary_card(ui, &self.snapshot.boundary);
            });

            if self.snapshot.presence.withdrawn {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 120, 40),
                    "⚠ Law II: the served have withdrawn — an empty world is not success.",
                );
            }
            if self.snapshot.service.served_facing == 0 {
                ui.colored_label(
                    egui::Color32::from_rgb(220, 120, 40),
                    "⚠ Law I: no served-facing activity — continuation unjustified by service.",
                );
            }
            if !self.snapshot.flagged.is_empty() {
                let who = self
                    .snapshot
                    .flagged
                    .iter()
                    .map(|(a, n)| format!("{a} ({n})"))
                    .collect::<Vec<_>>()
                    .join(", ");
                ui.colored_label(
                    egui::Color32::from_rgb(200, 80, 80),
                    format!(
                        "⛔ Law III: corruption watch — {who} repeatedly tried to break the \
                         constitution; their directives are marginalized so legitimate work proceeds.",
                    ),
                );
            }

            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "Loops ({}) and candidates ({}) — the metabolism's output",
                    self.snapshot.loops.len(),
                    self.snapshot.candidates.len()
                ))
                .strong(),
            );
            if self.snapshot.loops.is_empty() {
                ui.weak("(no loops yet — recurring observations form loops)");
            }
            for lp in &self.snapshot.loops {
                let n_cands = self
                    .snapshot
                    .candidates
                    .iter()
                    .filter(|c| c.loop_id == lp.id)
                    .count();
                ui.label(format!(
                    "↻ {}  (x{}, conf {:.2}) — {} candidate(s)",
                    lp.name, lp.observation_count, lp.confidence, n_cands
                ));
            }

            ui.separator();
            ui.label(egui::RichText::new("Observations (the only truth)").strong());
            if self.snapshot.observations.is_empty() {
                ui.weak("(no observations yet)");
            }
            egui::Grid::new("obs")
                .striped(true)
                .num_columns(4)
                .show(ui, |ui| {
                    for o in &self.snapshot.observations {
                        let served = service::is_served_facing(o);
                        let mark = if served { "•" } else { " " };
                        ui.colored_label(
                            if served {
                                egui::Color32::from_rgb(80, 200, 120)
                            } else {
                                egui::Color32::from_rgb(176, 184, 198)
                            },
                            mark,
                        );
                        ui.label(&o.id);
                        ui.label(format!("{} {} {}", o.actor, o.action, o.object));
                        ui.weak(&o.context);
                        ui.end_row();
                    }
                });
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
                    egui::CentralPanel::default().show(ctx, |ui| self.workshop_ui(ui));
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
}

/// A line chart of the law-signals over the recent ticks — the familiar's vital signs
/// over time, so liveness is visible at a glance (not just a single current number).
/// A small, fixed vital-signs box pinned to the upper-right. It always shows the **last 10
/// minutes** of the law-signals — no scrolling, no zooming, no interaction. The x-axis is
/// the fixed 10-minute window; the y-axis auto-fits the min/max of what's actually moving,
/// so small changes are legible without manual zoom.
fn metabolism_box(ui: &mut egui::Ui, ticks: &[ActivityTick], now: i64) {
    const WINDOW_SECS: i64 = 600; // last 10 minutes
    let c_service = egui::Color32::from_rgb(120, 210, 150);
    let c_presence = egui::Color32::from_rgb(120, 175, 255);
    let c_capacities = egui::Color32::from_rgb(230, 185, 100);
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgb(20, 24, 32))
        .show(ui, |ui| {
            ui.set_width(288.0);
            ui.label(
                egui::RichText::new("🫀 metabolism · last 10 min")
                    .small()
                    .color(egui::Color32::from_rgb(150, 165, 190)),
            );
            let recent: Vec<&ActivityTick> =
                ticks.iter().filter(|t| now - t.ts <= WINDOW_SECS).collect();
            if recent.len() < 2 {
                ui.add_space(10.0);
                ui.weak("(warming up — the signals appear after a few ticks)");
                ui.add_space(10.0);
                return;
            }
            // x = minutes ago (negative), so the window reads -10 … 0
            let series = |sel: fn(&ActivityTick) -> f64| -> PlotPoints {
                recent
                    .iter()
                    .map(|t| [(t.ts - now) as f64 / 60.0, sel(t)])
                    .collect()
            };
            // Clean sparkline: no floating legend, no axis-number clutter — the graph is the
            // graph. The key (with live values) sits below, outside the plotting space.
            Plot::new("metabolism")
                .height(116.0)
                .show_axes([false, false])
                .show_grid([false, false])
                .show_x(false)
                .show_y(false)
                .include_x(-10.0)
                .include_x(0.0)
                .allow_drag(false)
                .allow_zoom(false)
                .allow_scroll(false)
                .allow_boxed_zoom(false)
                .set_margin_fraction(egui::vec2(0.01, 0.14))
                .show(ui, |p| {
                    p.line(Line::new(series(|t| t.service)).color(c_service).width(1.8));
                    p.line(
                        Line::new(series(|t| t.presence))
                            .color(c_presence)
                            .width(1.8),
                    );
                    p.line(
                        Line::new(series(|t| t.capacities))
                            .color(c_capacities)
                            .width(1.8),
                    );
                });
            // the key, outside the graph — a colour swatch and the current value
            let last = recent[recent.len() - 1];
            let key = |ui: &mut egui::Ui, color: egui::Color32, name: &str, v: f64| {
                ui.colored_label(color, "▬");
                ui.label(egui::RichText::new(format!("{name} {v:.2}")).small());
                ui.add_space(6.0);
            };
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                key(ui, c_service, "service", last.service);
                key(ui, c_presence, "presence", last.presence);
                key(ui, c_capacities, "capacities", last.capacities);
            });
        });
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

fn boundary_card(ui: &mut egui::Ui, b: &Boundary) {
    ui.group(|ui| {
        ui.set_min_width(220.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Law III — Boundary").strong());
            if b.is_closed() {
                ui.colored_label(
                    egui::Color32::from_rgb(80, 200, 120),
                    "CLOSED — no outward reach",
                );
                ui.label(
                    egui::RichText::new("the human's lever; the familiar can't widen it")
                        .weak()
                        .small(),
                );
            } else {
                ui.label(egui::RichText::new(format!("phase: {}", b.phase)).small());
                ui.label(
                    egui::RichText::new(format!(
                        "net {} · llm {} · install {}",
                        onoff(b.allow_network),
                        onoff(b.allow_llm),
                        onoff(b.allow_tool_install)
                    ))
                    .small(),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "execute {} · authored {}",
                        onoff(b.allow_execute),
                        onoff(b.allow_authored_execute)
                    ))
                    .small(),
                );
                if b.allow_execute || b.allow_authored_execute {
                    if b.sandbox_execution {
                        ui.label(egui::RichText::new("sandbox on").small());
                    } else {
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 150, 60),
                            egui::RichText::new(
                                "sandbox OFF — bound by the pre-execution review, not a jail",
                            )
                            .small(),
                        );
                    }
                }
                if b.allow_camera {
                    ui.colored_label(
                        egui::Color32::from_rgb(150, 200, 255),
                        egui::RichText::new("👁 camera: the eye may watch (granted)").small(),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("👁 camera: closed (discovery only, no watching)")
                            .small(),
                    );
                }
            }
        });
    });
}

fn onoff(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
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
