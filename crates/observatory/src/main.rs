//! The Observatory — a visual window onto the factory.
//!
//! This is the primary human interface (the CLI remains for scripting/headless use).
//! It watches the factory's truth (observations) and the law-signals derived from it,
//! and it controls the daemon. It does **not** mutate the factory's own derived state
//! — with one principled exception: the **observer-input channel**, where Ian's reply
//! to the factory's question is recorded as an observation. That is the observer being
//! a first-class initiator (Input Parity), not the factory editing its own truth.
//!
//! It lives in its own crate so the kernel stays minimal-dependency and
//! `#![forbid(unsafe_code)]`; the GUI's heavier dependencies are isolated here.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use substrate_kernel::boundary::{self, Boundary};
use substrate_kernel::candidate::{self, Candidate};
use substrate_kernel::loops::{self, Loop};
use substrate_kernel::observation::{self, Observation};
use substrate_kernel::presence::{self, PresenceSignal};
use substrate_kernel::service::{self, ServiceSignal};

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A snapshot of the factory's state, recomputed on refresh.
struct Snapshot {
    observations: Vec<Observation>,
    loops: Vec<Loop>,
    candidates: Vec<Candidate>,
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
            error,
        }
    }
}

struct Observatory {
    data_dir: PathBuf,
    snapshot: Snapshot,
    /// Ian's in-progress reply to the factory's question.
    response: String,
    /// Last daemon status line (refreshed on actions and on load).
    daemon_status: String,
}

/// Path to the sibling `substrate` binary (same target dir as this GUI).
fn substrate_bin() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("substrate")))
        .unwrap_or_else(|| PathBuf::from("substrate"))
}

/// Run `substrate daemon <sub> --data-dir <dir>` and return its trimmed output.
fn daemon_cmd(dir: &Path, sub: &str) -> String {
    match Command::new(substrate_bin())
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
        Err(e) => format!("daemon: could not run substrate ({e})"),
    }
}

/// The question the factory is currently posing (it may write `question.txt`; the
/// default is the seed's standing question).
fn current_question(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("question.txt"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "What do you need most today?".to_string())
}

impl Observatory {
    fn new(data_dir: PathBuf) -> Self {
        let snapshot = Snapshot::load(&data_dir);
        let daemon_status = daemon_cmd(&data_dir, "status");
        Observatory {
            data_dir,
            snapshot,
            response: String::new(),
            daemon_status,
        }
    }
    fn refresh(&mut self) {
        self.snapshot = Snapshot::load(&self.data_dir);
    }
    fn refresh_daemon(&mut self) {
        self.daemon_status = daemon_cmd(&self.data_dir, "status");
    }
    /// Record Ian's reply as an observation — the observer's input channel (the one
    /// place the GUI writes; the factory's own truth stays read-only).
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

impl eframe::App for Observatory {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.heading("Substrate — Observatory");
            ui.label(
                egui::RichText::new(
                    "A factory whose survival is defined by its service to humanity.",
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

            // --- the interaction channel: the factory asks, Ian answers ---
            ui.add_space(6.0);
            let question = current_question(&self.data_dir);
            egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgb(28, 34, 44))
                .show(ui, |ui| {
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

        // gentle auto-refresh so the window tracks the factory as it runs
        ctx.request_repaint_after(std::time::Duration::from_secs(2));
    }
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
                    egui::RichText::new("the human's lever; the factory can't widen it")
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
    PathBuf::from(substrate_kernel::store::DEFAULT_DATA_DIR)
}

fn main() -> eframe::Result<()> {
    let data_dir = data_dir_from_args();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Substrate Observatory",
        options,
        Box::new(|_cc| Ok(Box::new(Observatory::new(data_dir)))),
    )
}
