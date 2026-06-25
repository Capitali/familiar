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

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use egui_plot::{Legend, Line, Plot, PlotPoints};
use familiar_kernel::activity::{self, ActivityTick};
use familiar_kernel::boundary::{self, Boundary};
use familiar_kernel::candidate::{self, Candidate};
use familiar_kernel::corruption;
use familiar_kernel::loops::{self, Loop};
use familiar_kernel::observation::{self, Observation};
use familiar_kernel::parameters::Parameters;
use familiar_kernel::presence::{self, PresenceSignal};
use familiar_kernel::request::{self, Answer, Confidence, Request};
use familiar_kernel::service::{self, ServiceSignal};
use familiar_kernel::thread::{self, Thread};

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
    /// Ian's in-progress free-form request to the familiar ("ask the familiar").
    ask: String,
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

/// The question the familiar is currently posing (it may write `question.txt`; the
/// default is the seed's standing question).
fn current_question(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("question.txt"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "What do you need most today?".to_string())
}

impl Glass {
    fn new(data_dir: PathBuf) -> Self {
        let snapshot = Snapshot::load(&data_dir);
        let daemon_status = daemon_cmd(&data_dir, "status");
        let answered_question = read_answered(&data_dir);
        let params_edit = snapshot.parameters.clone();
        Glass {
            data_dir,
            snapshot,
            response: String::new(),
            daemon_status,
            answered_question,
            params_edit,
            ask: String::new(),
        }
    }
    fn refresh(&mut self) {
        self.snapshot = Snapshot::load(&self.data_dir);
    }
    fn refresh_daemon(&mut self) {
        self.daemon_status = daemon_cmd(&self.data_dir, "status");
    }
    /// Record Ian's free-form request; the cycle answers it on the next tick.
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
                actor: "ian".into(),
                text: text.to_string(),
                created_at: now_secs(),
                status: "open".into(),
            },
        );
        self.ask.clear();
        self.refresh();
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
    /// Record Ian's reply as an observation — the observer's input channel (the one
    /// place the GUI writes; the familiar's own truth stays read-only).
    fn submit_response(&mut self) {
        let resp = self.response.trim();
        if resp.is_empty() {
            return;
        }
        let q = current_question(&self.data_dir);
        let object: String = resp.chars().take(200).collect();
        let obs = Observation::new(
            "ian",
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
        // steer: Ian's answer becomes an open thread with his words as the direction,
        // so the familiar pursues what *he* said he needs — not just what it inferred.
        let seq = self.snapshot.threads.len() + 1;
        let _ = thread::append(
            &self.data_dir,
            &Thread {
                id: format!("thread-{seq:04}"),
                question: q.clone(),
                theory: format!("Ian said: {resp}"),
                direction: resp.to_string(),
                created_at: now_secs(),
                status: "open".into(),
                origin: "observer".into(),
                actor: "ian".into(),
            },
        );
        // remember the answered question so it fades out and can't be answered twice;
        // persist it so a restart doesn't re-show it.
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
            });
            let running = self.daemon_status.contains("running");
            ui.label(
                egui::RichText::new(&self.daemon_status)
                    .small()
                    .color(if running {
                        egui::Color32::from_rgb(80, 200, 120)
                    } else {
                        egui::Color32::GRAY
                    }),
            );
            ui.label(
                egui::RichText::new(format!("data: {}", self.data_dir.display()))
                    .weak()
                    .small(),
            );
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(err) = &self.snapshot.error {
                ui.colored_label(egui::Color32::RED, err);
            }

            // --- the interaction channel: the familiar asks, Ian answers ---
            ui.add_space(6.0);
            let question = current_question(&self.data_dir);
            let already_answered = self.answered_question.as_deref() == Some(question.as_str());
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(28, 34, 44))
                .show(ui, |ui| {
                    if already_answered {
                        // the question has been answered — fade it out so it isn't
                        // answered twice; it returns when the familiar asks something new.
                        ui.label(
                            egui::RichText::new(
                                "✓ answered — the familiar will ask again as it learns",
                            )
                            .italics()
                            .color(egui::Color32::from_rgb(110, 140, 110)),
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
                        ui.add_enabled(false, egui::Button::new("🎤 speak (soon)"));
                        ui.add_enabled(false, egui::Button::new("📷 show (soon)"));
                        ui.label(
                            egui::RichText::new("your reply is recorded as an observation")
                                .weak()
                                .small(),
                        );
                    });
                });

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

            // --- ask the familiar: a free-form request, answered known-vs-probable ---
            ui.add_space(6.0);
            let latest_answer = self.snapshot.answers.last().cloned();
            let pending_requests = self
                .snapshot
                .requests
                .iter()
                .filter(|r| r.status == "open")
                .count();
            let asked_text = latest_answer.as_ref().and_then(|a| {
                self.snapshot
                    .requests
                    .iter()
                    .find(|r| r.id == a.request_id)
                    .map(|r| r.text.clone())
            });
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
                            "⏳ {pending_requests} request(s) pending — answered on the next tick"
                        ));
                    }
                    if let Some(a) = latest_answer {
                        ui.separator();
                        if let Some(q) = &asked_text {
                            ui.weak(format!("you asked: {q}"));
                        }
                        ui.horizontal(|ui| {
                            confidence_badge(ui, a.confidence);
                            if !a.evidence.is_empty() {
                                ui.weak(format!("· grounded in: {}", a.evidence));
                            }
                        });
                        ui.label(&a.body);
                        if a.feedback.is_empty() {
                            ui.horizontal(|ui| {
                                if ui.button("👍 helpful").clicked() {
                                    self.give_feedback(&a.id, "helpful", None);
                                }
                                if ui.button("✍ refine").clicked() {
                                    self.give_feedback(&a.id, "refine", asked_text.clone());
                                }
                            });
                        } else {
                            ui.weak(format!("✓ you marked this: {}", a.feedback));
                        }
                    }
                });

            // --- the metabolism at work: a signals chart + a feed of recent actions ---
            ui.add_space(6.0);
            egui::CollapsingHeader::new(
                egui::RichText::new("📈 Activity — the metabolism at work").strong(),
            )
            .default_open(true)
            .show(ui, |ui| {
                signals_chart(ui, &self.snapshot.ticks);
                ui.add_space(4.0);
                ui.label(egui::RichText::new("recent actions").weak().small());
                activity_feed(ui, &self.snapshot.ticks, now_secs());
            });

            egui::CollapsingHeader::new(egui::RichText::new("🧵 Theories & threads").strong())
                .default_open(false)
                .show(ui, |ui| threads_panel(ui, &self.snapshot.threads));

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
            egui::ScrollArea::vertical().show(ui, |ui| {
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
                                    egui::Color32::GRAY
                                },
                                mark,
                            );
                            ui.label(&o.id);
                            ui.label(format!("{} {} {}", o.actor, o.action, o.object));
                            ui.weak(&o.context);
                            ui.end_row();
                        }
                    });
            });
        });

        // gentle auto-refresh so the window tracks the familiar as it runs
        ctx.request_repaint_after(std::time::Duration::from_secs(2));
    }
}

/// A line chart of the law-signals over the recent ticks — the familiar's vital signs
/// over time, so liveness is visible at a glance (not just a single current number).
fn signals_chart(ui: &mut egui::Ui, ticks: &[ActivityTick]) {
    if ticks.len() < 2 {
        ui.weak("(the signals chart appears once the metabolism has ticked a few times)");
        return;
    }
    let start = ticks.len().saturating_sub(120); // last ~120 ticks
    let recent = &ticks[start..];
    let series = |sel: fn(&ActivityTick) -> f64| -> PlotPoints {
        recent
            .iter()
            .enumerate()
            .map(|(i, t)| [i as f64, sel(t)])
            .collect()
    };
    Plot::new("signals")
        .height(160.0)
        .legend(Legend::default())
        .include_y(0.0)
        .include_y(1.0)
        .show(ui, |p| {
            p.line(Line::new(series(|t| t.service)).name("service (I)"));
            p.line(Line::new(series(|t| t.presence)).name("presence (II)"));
            p.line(Line::new(series(|t| t.capacities)).name("capacities (II)"));
        });
}

/// A feed of the most recent *consequential* ticks (skipping quiet ones), newest first —
/// so the human sees the familiar theorize, pursue, test, and promote as it happens.
fn activity_feed(ui: &mut egui::Ui, ticks: &[ActivityTick], now: i64) {
    egui::ScrollArea::vertical()
        .max_height(170.0)
        .id_salt("activity")
        .show(ui, |ui| {
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
fn threads_panel(ui: &mut egui::Ui, threads: &[Thread]) {
    if threads.is_empty() {
        ui.weak("(no theories yet — they form as the familiar interprets what it observes)");
        return;
    }
    egui::ScrollArea::vertical()
        .max_height(220.0)
        .id_salt("threads")
        .show(ui, |ui| {
            for t in threads.iter().rev().take(20) {
                let color = match t.status.as_str() {
                    "open" => egui::Color32::from_rgb(150, 200, 255),
                    "pursued" => egui::Color32::from_rgb(180, 180, 140),
                    "answered" => egui::Color32::from_rgb(110, 140, 110),
                    _ => egui::Color32::GRAY,
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
        Confidence::Unknown => ("○ unknown", egui::Color32::GRAY),
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
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "The Familiar — the Glass",
        options,
        Box::new(|_cc| Ok(Box::new(Glass::new(data_dir)))),
    )
}
