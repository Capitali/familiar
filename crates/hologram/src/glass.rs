//! The Glass — the egui layout the holographic pass composites (UI-DESIGN-BRIEF §2–§9).
//!
//! Tiering: T1 conversation owns the center; T2 trust/state is the always-visible top strip;
//! T3 (roster, goals, theories, activity) lives in the right rail; T4 controls (gates) and
//! T5 diagnostics (raw feed) sit behind disclosure. Everything renders from the polled
//! worldview — honest about staleness, no fabricated liveness (Law III).

use egui::{Color32, RichText};
use familiar_mesh::members::{Member, MemberKind};
use familiar_mesh::worldview::Worldview;

/// UI state that survives between frames (the worldview itself lives in `client::Shared`).
pub struct GlassState {
    pub input: String,
    pub zoom: f32,
    pub show_controls: bool,
    pub show_diagnostics: bool,
    /// The last question we cued attention for — so the spike fires once per new question.
    pub cued_question: String,
    /// What the holographic pass should amplify this frame: 0 calm … 1 full attention.
    pub attention_target: f32,
    /// A gate the human just flipped, for the main loop to send to the daemon.
    pub pending_gate: Option<(String, bool)>,
}

impl Default for GlassState {
    fn default() -> Self {
        Self {
            input: String::new(),
            zoom: 1.0,
            show_controls: false,
            show_diagnostics: false,
            cued_question: String::new(),
            attention_target: 0.0,
            pending_gate: None,
        }
    }
}

/// One frame of UI. Returns text the human submitted (an answer/utterance), if any.
pub fn draw(
    ctx: &egui::Context,
    state: &mut GlassState,
    view: Option<&Worldview>,
    error: Option<&str>,
    stale_secs: Option<u64>,
) -> Option<String> {
    ctx.set_zoom_factor(state.zoom);
    let mut submitted = None;
    let now = now_secs();

    // The familiar wants the human: a pending question or an alarm drives the holographic
    // attention spike; calm otherwise. (The cue must be differentiable from ambient motion.)
    let question = view.and_then(|v| {
        let q = v.question.trim();
        (!q.is_empty()).then(|| q.to_string())
    });
    let withdrawn = view.map(|v| v.withdrawn).unwrap_or(false);
    state.attention_target = if withdrawn {
        1.0
    } else if question.is_some() {
        0.7
    } else {
        0.0
    };

    top_strip(ctx, state, view, error, stale_secs, withdrawn);
    right_rail(ctx, view, now);
    conversation(ctx, state, view, question.as_deref(), &mut submitted, now);
    submitted
}

/// T2 — trust & state, always visible, compact.
fn top_strip(
    ctx: &egui::Context,
    state: &mut GlassState,
    view: Option<&Worldview>,
    error: Option<&str>,
    stale_secs: Option<u64>,
    withdrawn: bool,
) {
    egui::TopBottomPanel::top("t2-strip")
        .frame(panel_frame())
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                let title = view.map(|v| v.group_label.as_str()).unwrap_or("the familiar");
                ui.label(RichText::new(title).color(ACCENT).strong().size(16.0));

                if let Some(v) = view {
                    signal(ui, "service", v.service);
                    signal(ui, "presence", v.presence);
                    signal(ui, "capacity", v.capacity);
                    let online = v
                        .members
                        .iter()
                        .filter(|m| m.status == "online" || (m.status.is_empty() && m.online))
                        .count();
                    ui.label(dim(format!("mesh {online}/{} online", v.members.len())));
                    {
                        let g = &v.gates;
                        let open: Vec<&str> = [
                            ("llm", g.llm),
                            ("camera", g.camera),
                            ("net", g.network),
                            ("mesh", g.mesh),
                            ("exec", g.execute),
                            ("agent", g.agent),
                            ("tools", g.tool_install),
                        ]
                        .iter()
                        .filter(|(_, on)| *on)
                        .map(|(n, _)| *n)
                        .collect();
                        ui.label(dim(format!("gates open: {}", if open.is_empty() { "none".into() } else { open.join(" ") })));
                    }
                }

                if withdrawn {
                    ui.label(RichText::new("⚠ WITHDRAWN").color(ALARM).strong());
                }
                if let Some(e) = error {
                    ui.label(RichText::new(format!("⚠ {e}")).color(ALARM));
                } else if let Some(s) = stale_secs {
                    if s > 15 {
                        ui.label(RichText::new(format!("data {s}s stale")).color(WARN));
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("A+").clicked() {
                        state.zoom = (state.zoom + 0.1).min(2.0);
                    }
                    if ui.button("A−").clicked() {
                        state.zoom = (state.zoom - 0.1).max(0.7);
                    }
                    ui.toggle_value(&mut state.show_diagnostics, "diagnostics");
                    ui.toggle_value(&mut state.show_controls, "controls");
                });
            });
        });
}

/// T3 — the mesh's life: roster (with the full metadata), roadmap, theories.
fn right_rail(ctx: &egui::Context, view: Option<&Worldview>, now: i64) {
    egui::SidePanel::right("t3-rail")
        .frame(panel_frame())
        .default_width(340.0)
        .show(ctx, |ui| {
            let Some(v) = view else {
                ui.label(dim("waiting for the familiar…".to_string()));
                return;
            };
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::CollapsingHeader::new(RichText::new("roster").color(ACCENT).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        for m in &v.members {
                            member_row(ui, m, now);
                        }
                    })
                    .header_response
                    .on_hover_text("every mesh participant, classified");
                ui.separator();

                egui::CollapsingHeader::new(RichText::new("roadmap").color(ACCENT).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        if v.goals.is_empty() {
                            ui.label(dim("no shared goals".into()));
                        }
                        for g in &v.goals {
                            ui.label(format!("{} {}", status_dot_word(&g.status), g.description));
                            let mut dates = vec![format!("{} {}", g.status, fmt_date(pick(g.status_at, g.updated_at)))];
                            if g.last_worked_at > 0 {
                                dates.push(format!("worked {}", fmt_date(g.last_worked_at)));
                            }
                            if g.completed_at > 0 {
                                dates.push(format!("completed {}", fmt_date(g.completed_at)));
                            }
                            if g.ended_at > 0 {
                                dates.push(format!("ended {}", fmt_date(g.ended_at)));
                            }
                            if !g.owner.is_empty() {
                                dates.push(format!("owner {}", g.owner));
                            }
                            ui.label(dim(dates.join(" · ")));
                            ui.add_space(4.0);
                        }
                    });
                ui.separator();

                egui::CollapsingHeader::new(RichText::new("theories").color(ACCENT).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        let theories = &v.theories;
                        if theories.is_empty() {
                            ui.label(dim("no live theories".into()));
                        }
                        for t in theories.iter().take(8) {
                            ui.label(&t.theory);
                            let mut dates = vec![format!("{} {}", t.status, fmt_date(pick(t.status_at, t.created_at)))];
                            if t.created_at > 0 {
                                dates.push(format!("born {}", fmt_date(t.created_at)));
                            }
                            if t.last_worked_at > 0 {
                                dates.push(format!("worked {}", fmt_date(t.last_worked_at)));
                            }
                            ui.label(dim(dates.join(" · ")));
                            ui.add_space(4.0);
                        }
                    });
            });
        });
}

/// One roster row: status dot, identity, then the full metadata block the roster was missing.
fn member_row(ui: &mut egui::Ui, m: &Member, now: i64) {
    let (dot, color) = match m.status.as_str() {
        "online" => ("●", OK),
        "away" => ("●", WARN),
        _ => ("○", DIM),
    };
    let kind = match m.kind {
        MemberKind::SelfNode => "self",
        MemberKind::GossipPeer => "peer",
        MemberKind::DevicePeer => "device",
        MemberKind::DeviceAgent => "agent",
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(dot).color(color));
        ui.label(RichText::new(&m.label).strong());
        ui.label(dim(format!("{kind} · {}", short(&m.node_id))));
        if m.ai {
            ui.label(RichText::new("ai").color(ACCENT).small());
        }
        if !m.trust.is_empty() && m.trust != "trusted" {
            ui.label(RichText::new(&m.trust).color(ALARM).small());
        }
    });
    let seen = if m.status == "online" {
        "now".to_string()
    } else {
        format!("{} ago", fmt_span(now - m.last_seen))
    };
    ui.label(dim(format!(
        "{} · seen {seen} · joined {}",
        m.status,
        fmt_date(m.first_seen)
    )));
    let mut line = String::new();
    if m.session_start > 0 {
        line.push_str(&format!("session {} · ", fmt_span(now - m.session_start)));
    }
    if m.total_online_secs > 0 {
        line.push_str(&format!("total online {} · ", fmt_span(m.total_online_secs)));
    }
    line.push_str(&format!(
        "{}{} · v{}",
        m.os,
        if m.os_version.is_empty() { String::new() } else { format!(" {}", m.os_version) },
        if m.familiar_version.is_empty() { "?" } else { &m.familiar_version }
    ));
    ui.label(dim(line));
    ui.label(dim(format!(
        "interactive {} {}",
        if m.interactive { "yes" } else { "no" },
        if m.human.is_empty() { String::new() } else { format!("· serves {}", m.human) }
    )));
    ui.add_space(6.0);
}

/// T1 — the conversation, center stage; T4/T5 behind disclosure beneath it.
fn conversation(
    ctx: &egui::Context,
    state: &mut GlassState,
    view: Option<&Worldview>,
    question: Option<&str>,
    submitted: &mut Option<String>,
    now: i64,
) {
    egui::CentralPanel::default()
        .frame(panel_frame())
        .show(ctx, |ui| {
            // The familiar's open question — the single most important element on the Glass.
            if let Some(q) = question {
                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgba_unmultiplied(20, 60, 70, 160))
                    .show(ui, |ui| {
                        ui.label(RichText::new("the familiar asks").color(ACCENT).small());
                        ui.label(RichText::new(q).size(20.0).strong());
                    });
                ui.add_space(8.0);
            }

            // The reply/utterance channel — always available, not only when asked.
            ui.horizontal(|ui| {
                let edit = egui::TextEdit::singleline(&mut state.input)
                    .hint_text(if question.is_some() { "answer…" } else { "tell the familiar…" })
                    .desired_width(ui.available_width() - 70.0);
                let r = ui.add(edit);
                let send = ui.button("send").clicked()
                    || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                if send && !state.input.trim().is_empty() {
                    *submitted = Some(state.input.trim().to_string());
                    state.input.clear();
                }
            });
            ui.add_space(8.0);

            // The dialogue trail + ambient activity (T3): served-facing lines bright,
            // the rest of the metabolism dim beneath them.
            let Some(v) = view else { return };
            egui::ScrollArea::vertical()
                .max_height(ui.available_height() - 140.0)
                .show(ui, |ui| {
                    for o in &v.recent {
                        let served = o.context == "console"
                            || o.action.contains("told")
                            || o.action.contains("asked")
                            || o.action.contains("answered");
                        let line = format!(
                            "{} · {} {} {}",
                            fmt_span(now - o.ts),
                            o.actor,
                            o.action,
                            o.object
                        );
                        if served {
                            ui.label(RichText::new(line).color(Color32::from_rgb(220, 240, 245)));
                        } else {
                            ui.label(dim(line));
                        }
                    }
                });

            // T4 — controls behind disclosure: the boundary gates as the human's own switches.
            if state.show_controls {
                ui.separator();
                ui.label(RichText::new("gates").color(ACCENT));
                if let Some(g) = view.map(|v| v.gates.clone()) {
                    ui.horizontal_wrapped(|ui| {
                        for (name, key, on) in [
                            ("llm", "allow_llm", g.llm),
                            ("camera", "allow_camera", g.camera),
                            ("network", "allow_network", g.network),
                            ("mesh", "allow_mesh", g.mesh),
                            ("execute", "allow_execute", g.execute),
                            ("agent", "allow_agent", g.agent),
                            ("tools", "allow_tool_install", g.tool_install),
                        ] {
                            let mut v = on;
                            if ui.toggle_value(&mut v, name).changed() {
                                state.pending_gate = Some((key.to_string(), v));
                            }
                        }
                    });
                }
            }

            // T5 — diagnostics: the raw substrate, on demand only.
            if state.show_diagnostics {
                ui.separator();
                if let Some(v) = view {
                    ui.label(dim(format!(
                        "observations {} · tick {} · uptime {} · theory quality {:.2}",
                        v.observation_count,
                        v.tick,
                        fmt_span(v.uptime_secs),
                        v.theory_quality
                    )));
                    ui.label(dim(format!("node {}", v.node_id)));
                    if !v.hosts.is_empty() {
                        ui.label(dim(format!("answers at {}", v.hosts.join(", "))));
                    }
                }
            }
        });
}

// ---- helpers ----------------------------------------------------------------------

const ACCENT: Color32 = Color32::from_rgb(102, 230, 255);
const OK: Color32 = Color32::from_rgb(80, 250, 180);
const WARN: Color32 = Color32::from_rgb(255, 200, 80);
const ALARM: Color32 = Color32::from_rgb(255, 110, 110);
const DIM: Color32 = Color32::from_rgb(130, 155, 165);

fn dim(s: String) -> RichText {
    RichText::new(s).color(DIM).small()
}

fn signal(ui: &mut egui::Ui, name: &str, value: f64) {
    ui.label(dim(name.to_string()));
    ui.add(
        egui::ProgressBar::new(value as f32)
            .desired_width(60.0)
            .fill(ACCENT.gamma_multiply(0.7)),
    );
}

fn panel_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(Color32::from_rgba_unmultiplied(6, 16, 22, 215))
        .inner_margin(10.0)
}

fn status_dot_word(status: &str) -> &'static str {
    match status {
        "done" => "✓",
        "failed" | "blocked" => "✗",
        "in_progress" | "claimed" => "▶",
        _ => "•",
    }
}

fn short(id: &str) -> String {
    id.chars().take(8).collect()
}

fn pick(a: i64, b: i64) -> i64 {
    if a > 0 {
        a
    } else {
        b
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// "3m", "2h 10m", "4d 6h" — coarse and honest.
fn fmt_span(secs: i64) -> String {
    let s = secs.max(0);
    let (d, h, m) = (s / 86_400, (s % 86_400) / 3_600, (s % 3_600) / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{s}s")
    }
}

/// Unix secs → "2026-07-21" (Howard Hinnant's civil-from-days; no chrono dependency).
fn fmt_date(ts: i64) -> String {
    if ts <= 0 {
        return "—".into();
    }
    let z = ts.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}
