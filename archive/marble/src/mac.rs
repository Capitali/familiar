//! macOS menu-bar implementation: an NSStatusItem (the marble) driven by a windowless
//! winit event loop, with `tray-icon` for the status item + menu.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant, SystemTime};

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::window::WindowId;

const LAUNCHD_LABEL: &str = "io.river.marble";
const DEFAULT_DATA_DIR: &str = "familiar_data";
const PIDFILE: &str = "daemon.pid";
/// How often the marble re-checks whether the familiar is alive, to keep its icon honest.
const POLL: Duration = Duration::from_secs(3);
/// While the familiar is alive the marble breathes: a gentle wake cadence (the animation
/// frame) and the period of one full inhale→exhale. Asleep, it sits steady and dim, waking
/// only on the slower [`POLL`] to notice when the familiar comes back.
const PULSE_STEP: Duration = Duration::from_millis(120);
const PULSE_PERIOD_SECS: f32 = 2.6;
/// Binaries the marble copies to the stable path so the login item survives `cargo clean`.
/// `familiar-eye` is the Swift camera helper (absent if the Swift toolchain wasn't present at
/// build time); the copy loop skips any that don't exist, so that's harmless.
const STABLE_BINS: [&str; 4] = ["marble", "glass", "familiar", "familiar-eye"];
/// A durable home for the installed binaries, outside the build tree.
const STABLE_SUBDIR: &str = "Library/Application Support/Familiar/bin";

/// Events forwarded from the tray/menu callbacks into the winit loop so it wakes.
enum UserEvent {
    Tray(TrayIconEvent),
    Menu(MenuEvent),
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("install") => report("install", install(&data_dir(&args))),
        Some("uninstall") => report("uninstall", uninstall()),
        Some("run") | None => run_tray(&args),
        Some(other) => eprintln!("marble: unknown command '{other}' (run|install|uninstall)"),
    }
}

fn report(what: &str, r: std::io::Result<String>) {
    match r {
        Ok(msg) => println!("marble {what}: {msg}"),
        Err(e) => eprintln!("marble {what}: {e}"),
    }
}

/// The `--data-dir` value, or the default. The marble passes this through to the Glass
/// and to `familiar daemon` so all three agree on which familiar they're looking at.
fn data_dir(args: &[String]) -> String {
    args.windows(2)
        .find(|w| w[0] == "--data-dir")
        .map(|w| w[1].clone())
        // No explicit dir (e.g. the app double-clicked from Finder): use the absolute
        // per-user path, not the relative default. A launchd/Finder-launched marble runs
        // with cwd `/`, where "familiar_data" resolves under the read-only system volume —
        // the Glass it opens would then fail every write with EROFS.
        .unwrap_or_else(|| {
            std::env::var("HOME")
                .map(|h| format!("{h}/Library/Application Support/Familiar/data"))
                .unwrap_or_else(|_| DEFAULT_DATA_DIR.to_string())
        })
}

/// A binary that lives next to this one (the workspace builds `marble`, `glass`, and
/// `familiar` into the same directory).
fn sibling(name: &str) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(name)))
        .unwrap_or_else(|| PathBuf::from(name))
}

/// Where this marble's workspace builds `name` under a given Cargo profile. The path is
/// baked in at compile time from the build location, so even the frozen login-item marble
/// can find the live build tree it came from. (On a machine that only has the install,
/// this path won't exist and is simply skipped.)
fn workspace_target(profile: &str, name: &str) -> Option<PathBuf> {
    // CARGO_MANIFEST_DIR is `<root>/crates/marble`; the workspace target dir is `<root>/target`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("target").join(profile).join(name))
}

/// Resolve a sibling binary the marble drives (`glass`, `familiar`) to the copy that was
/// built most recently. Candidates: the sibling next to the running marble (the stable
/// install that survives `cargo clean`) and this marble's own build tree (so a fresh
/// `cargo build` is launched immediately, instead of the snapshot copied at install time).
/// Whichever was modified most recently wins; if none can be stat'd, fall back to sibling.
fn resolve_bin(name: &str) -> PathBuf {
    let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    let mut candidates = vec![sibling(name)];
    candidates.extend(["release", "debug"].iter().filter_map(|p| workspace_target(p, name)));

    let mut best: Option<(SystemTime, PathBuf)> = None;
    for path in candidates {
        if let Some(t) = mtime(&path) {
            if best.as_ref().is_none_or(|(bt, _)| t > *bt) {
                best = Some((t, path));
            }
        }
    }
    best.map(|(_, p)| p).unwrap_or_else(|| sibling(name))
}

// --- the running marble ---------------------------------------------------------

fn run_tray(args: &[String]) {
    let data = data_dir(args);
    let open_on_start = !args.iter().any(|a| a == "--no-open");

    let event_loop = {
        let mut builder = EventLoop::<UserEvent>::with_user_event();
        // Accessory == menu-bar app: present in the menu bar, absent from the Dock.
        builder.with_activation_policy(ActivationPolicy::Accessory);
        builder.build().expect("event loop")
    };
    event_loop.set_control_flow(ControlFlow::Wait);

    // Route tray/menu callbacks (which fire on their own) through the loop's proxy so a
    // Wait-blocked loop wakes to handle them — no busy polling for an always-on item.
    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |e| {
        let _ = proxy.send_event(UserEvent::Tray(e));
    }));
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |e| {
        let _ = proxy.send_event(UserEvent::Menu(e));
    }));

    let mut app = App {
        data,
        open_on_start,
        tray: None,
        ids: None,
        glass: None,
        daemon_up: false,
        pulse: 0.0,
        last_tick: Instant::now(),
        last_check: Instant::now(),
    };
    let _ = event_loop.run_app(&mut app);
}

struct Ids {
    open: MenuId,
    start: MenuId,
    stop: MenuId,
    quit: MenuId,
}

struct App {
    data: String,
    open_on_start: bool,
    tray: Option<TrayIcon>,
    ids: Option<Ids>,
    glass: Option<Child>,
    /// Last observed daemon liveness — drives whether the marble is bright or dim.
    daemon_up: bool,
    /// Phase of the breathing pulse, in radians (advanced by real elapsed time).
    pulse: f32,
    /// When the pulse was last advanced — so the breath keeps a steady wall-clock pace even
    /// if a frame is late.
    last_tick: Instant,
    /// When liveness was last re-checked — paced by [`POLL`], not by every animation frame.
    last_check: Instant,
}

impl App {
    fn build_tray(&mut self) {
        let open = MenuItem::new("Open the Glass", true, None);
        let start = MenuItem::new("Start the familiar", true, None);
        let stop = MenuItem::new("Stop the familiar", true, None);
        let quit = MenuItem::new("Quit the marble", true, None);
        self.ids = Some(Ids {
            open: open.id().clone(),
            start: start.id().clone(),
            stop: stop.id().clone(),
            quit: quit.id().clone(),
        });
        let menu = Menu::new();
        let _ = menu.append(&open);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&start);
        let _ = menu.append(&stop);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit);

        self.daemon_up = daemon_alive(&self.data);
        match TrayIconBuilder::new()
            .with_tooltip(tooltip(self.daemon_up))
            .with_icon(marble_icon(32, !self.daemon_up, 0.0))
            .with_menu(Box::new(menu))
            .build()
        {
            Ok(tray) => self.tray = Some(tray),
            Err(e) => eprintln!("marble: could not create the menu-bar item: {e}"),
        }
    }

    /// Re-check the daemon and, if its liveness changed, update the tooltip. The icon itself
    /// is repainted every frame by [`Self::animate`], so this only owns the state + tooltip.
    fn check_liveness(&mut self) {
        let up = daemon_alive(&self.data);
        if up != self.daemon_up {
            self.daemon_up = up;
            if let Some(tray) = &self.tray {
                let _ = tray.set_tooltip(Some(tooltip(up)));
            }
        }
    }

    /// Force an immediate liveness re-check and repaint — used right after a menu action
    /// (start/stop) so the marble responds at once instead of on the next poll.
    fn refresh_status(&mut self) {
        self.check_liveness();
        self.last_check = Instant::now();
        self.paint();
    }

    /// One animation frame: advance the breath by real elapsed time, re-check liveness on the
    /// slow [`POLL`] cadence, and repaint.
    fn animate(&mut self) {
        let dt = self.last_tick.elapsed().as_secs_f32();
        self.last_tick = Instant::now();
        self.pulse =
            (self.pulse + dt / PULSE_PERIOD_SECS * std::f32::consts::TAU) % std::f32::consts::TAU;
        if self.last_check.elapsed() >= POLL {
            self.check_liveness();
            self.last_check = Instant::now();
        }
        self.paint();
    }

    /// Paint the marble at the current breath: alive → a soft glow swelling 0→1→0; asleep →
    /// steady and dim (no glow).
    fn paint(&self) {
        let glow = if self.daemon_up {
            0.5 + 0.5 * self.pulse.sin()
        } else {
            0.0
        };
        if let Some(tray) = &self.tray {
            let _ = tray.set_icon(Some(marble_icon(32, !self.daemon_up, glow)));
        }
    }

    /// Schedule the next wake: the breathing cadence while alive, the slower poll while asleep.
    fn schedule(&self, event_loop: &ActiveEventLoop) {
        let next = if self.daemon_up { PULSE_STEP } else { POLL };
        event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + next));
    }

    /// Open the Glass — or, if one we launched is still up, raise it to the front rather
    /// than stacking a second window.
    fn open_glass(&mut self) {
        if let Some(child) = &mut self.glass {
            if matches!(child.try_wait(), Ok(None)) {
                focus_pid(child.id());
                return;
            }
        }
        let exe = resolve_bin("glass");
        match Command::new(&exe).arg("--data-dir").arg(&self.data).spawn() {
            Ok(c) => self.glass = Some(c),
            Err(e) => eprintln!("marble: could not open the Glass ({}): {e}", exe.display()),
        }
    }

    fn daemon(&self, sub: &str) {
        let exe = resolve_bin("familiar");
        let _ = Command::new(&exe)
            .args(["daemon", sub, "--data-dir", &self.data])
            .status();
    }
}

/// Is the familiar daemon running? Reads its pidfile (the same one `familiar daemon`
/// writes) and asks the OS whether that pid is alive — a stale pidfile reads as down.
fn daemon_alive(data: &str) -> bool {
    let pid = std::fs::read_to_string(Path::new(data).join(PIDFILE))
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok());
    match pid {
        Some(pid) => Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false),
        None => false,
    }
}

fn tooltip(up: bool) -> &'static str {
    if up {
        "The Familiar — running (click to open the Glass)"
    } else {
        "The Familiar — asleep (click to open the Glass)"
    }
}

/// Raise the process with this pid to the front. Used to focus an already-open Glass
/// instead of spawning another window.
fn focus_pid(pid: u32) {
    let script =
        format!("tell application \"System Events\" to set frontmost of (first process whose unix id is {pid}) to true");
    let _ = Command::new("osascript").arg("-e").arg(script).status();
}

impl ApplicationHandler<UserEvent> for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        // The status item must be created after the event loop is running (macOS).
        if cause == StartCause::Init && self.tray.is_none() {
            self.build_tray();
            if self.open_on_start {
                self.open_glass();
            }
            self.last_tick = Instant::now();
            self.last_check = Instant::now();
            self.schedule(event_loop);
        } else if matches!(cause, StartCause::ResumeTimeReached { .. }) {
            // The periodic wake: breathe, and keep liveness in step with the daemon.
            self.animate();
            self.schedule(event_loop);
        }
    }

    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(&mut self, _el: &ActiveEventLoop, _id: WindowId, _event: WindowEvent) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            // A left-click on the marble opens the Glass directly.
            UserEvent::Tray(TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }) => self.open_glass(),
            UserEvent::Tray(_) => {}
            UserEvent::Menu(m) => {
                let Some(ids) = &self.ids else { return };
                if m.id == ids.open {
                    self.open_glass();
                } else if m.id == ids.start {
                    self.daemon("start");
                    self.refresh_status();
                    self.schedule(event_loop);
                } else if m.id == ids.stop {
                    self.daemon("stop");
                    self.refresh_status();
                    self.schedule(event_loop);
                } else if m.id == ids.quit {
                    event_loop.exit();
                }
            }
        }
    }
}

// --- the glassy marble icon (procedural, no asset file) -------------------------

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// A small glassy blue marble: radial blue gradient (lighter at the core), a soft
/// specular highlight up-left, and an anti-aliased rim — generated as raw RGBA so
/// there's no image asset to ship. When `dim` (the familiar is asleep) the marble
/// desaturates toward grey and goes translucent, so liveness reads at a glance. `glow`
/// (0..1) is the breath — it swells the core and highlight, so an alive marble visibly
/// pulses; pass 0.0 for a still marble.
fn marble_icon(size: u32, dim: bool, glow: f32) -> Icon {
    let n = (size * size * 4) as usize;
    let mut rgba = vec![0u8; n];
    let c = (size as f32 - 1.0) / 2.0;
    let r = c; // marble fills the icon
    let hx = c - r * 0.35; // highlight centre, up and to the left
    let hy = c - r * 0.35;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - c;
            let dy = y as f32 - c;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > r {
                continue; // transparent outside the circle
            }
            let i = ((y * size + x) * 4) as usize;
            let t = (dist / r).clamp(0.0, 1.0); // 0 core .. 1 rim
            // The breath lifts the inner glass most, fading to nothing at the rim.
            let lift = glow * 36.0 * (1.0 - t);
            let base_r = lerp(120.0, 18.0, t) + lift;
            let base_g = lerp(185.0, 64.0, t) + lift;
            let base_b = lerp(255.0, 150.0, t) + lift * 0.4;
            // specular highlight near (hx, hy), itself brightened by the breath
            let hdx = x as f32 - hx;
            let hdy = y as f32 - hy;
            let hd = (hdx * hdx + hdy * hdy).sqrt();
            let spec = (1.0 - (hd / (r * 0.55)).clamp(0.0, 1.0)).powf(2.2);
            let mut rr = (base_r + spec * (190.0 + glow * 70.0)).min(255.0);
            let mut gg = (base_g + spec * (190.0 + glow * 70.0)).min(255.0);
            let mut bb = (base_b + spec * (130.0 + glow * 50.0)).min(255.0);
            let mut edge = ((r - dist).clamp(0.0, 1.5)) / 1.5; // soft 1.5px rim
            if dim {
                // Blend each channel toward a muted grey and drop the opacity — asleep.
                let grey = (0.30 * rr + 0.59 * gg + 0.11 * bb) * 0.55;
                rr = lerp(rr, grey, 0.7);
                gg = lerp(gg, grey, 0.7);
                bb = lerp(bb, grey, 0.7);
                edge *= 0.5;
            }
            rgba[i] = rr as u8;
            rgba[i + 1] = gg as u8;
            rgba[i + 2] = bb as u8;
            rgba[i + 3] = (edge * 255.0) as u8;
        }
    }
    Icon::from_rgba(rgba, size, size).expect("valid marble icon")
}

// --- launchd login agent --------------------------------------------------------

fn launch_agent_plist() -> std::io::Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME not set"))?;
    Ok(PathBuf::from(home)
        .join("Library/LaunchAgents")
        .join(format!("{LAUNCHD_LABEL}.plist")))
}

/// The durable bin directory the login items point at, so a `cargo clean` (which wipes
/// `target/`) can't break them. Built outside the workspace under Application Support.
fn stable_bin_dir() -> std::io::Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME not set"))?;
    Ok(PathBuf::from(home).join(STABLE_SUBDIR))
}

/// Copy this marble and its siblings (`glass`, `familiar`) from the build output into the
/// stable bin directory, and return the stable marble path. Skips the copy if we're
/// already running from there (a reinstall in place). The build dir is wherever the
/// running binary lives — so `cargo build --release` then `target/release/marble install`
/// installs the release binaries.
fn install_stable_binaries() -> std::io::Result<(PathBuf, Vec<&'static str>)> {
    let src = std::env::current_exe()?;
    let src_dir = src
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no build dir"))?;
    let bin = stable_bin_dir()?;
    std::fs::create_dir_all(&bin)?;
    let in_place = std::fs::canonicalize(src_dir).ok() == std::fs::canonicalize(&bin).ok();
    let mut copied = Vec::new();
    if !in_place {
        for name in STABLE_BINS {
            let from = src_dir.join(name);
            if from.exists() {
                std::fs::copy(&from, bin.join(name))?;
                copied.push(name);
            }
        }
    }
    Ok((bin.join("marble"), copied))
}

fn install(data: &str) -> std::io::Result<String> {
    let (exe, copied) = install_stable_binaries()?;
    let plist = launch_agent_plist()?;
    // Absolute data dir so the agent works regardless of launchd's working directory.
    let data_abs = std::fs::canonicalize(data)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| data.to_string());
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LAUNCHD_LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>run</string>
    <string>--data-dir</string>
    <string>{data_abs}</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><false/>
</dict>
</plist>
"#,
        exe = exe.display(),
    );
    if let Some(parent) = plist.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&plist, xml)?;
    // Unload any prior copy so launchd picks up the (possibly new) stable path, then load.
    let _ = Command::new("launchctl")
        .args(["unload", "-w"])
        .arg(&plist)
        .status();
    let _ = Command::new("launchctl")
        .args(["load", "-w"])
        .arg(&plist)
        .status();
    let where_ = if copied.is_empty() {
        "(already at the stable path)".to_string()
    } else {
        format!("installed {} -> {}", copied.join(", "), exe.display())
    };
    Ok(format!(
        "the marble will appear at login; {where_}; agent {}",
        plist.display()
    ))
}

fn uninstall() -> std::io::Result<String> {
    let plist = launch_agent_plist()?;
    if !plist.exists() {
        return Ok("was not installed".to_string());
    }
    let _ = Command::new("launchctl")
        .args(["unload", "-w"])
        .arg(&plist)
        .status();
    std::fs::remove_file(&plist)?;
    Ok("removed the login item".to_string())
}
