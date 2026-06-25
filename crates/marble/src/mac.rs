//! macOS menu-bar implementation: an NSStatusItem (the marble) driven by a windowless
//! winit event loop, with `tray-icon` for the status item + menu.

use std::path::PathBuf;
use std::process::{Child, Command};

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::window::WindowId;

const LAUNCHD_LABEL: &str = "io.river.marble";
const DEFAULT_DATA_DIR: &str = "familiar_data";

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
        .unwrap_or_else(|| DEFAULT_DATA_DIR.to_string())
}

/// A binary that lives next to this one (the workspace builds `marble`, `glass`, and
/// `familiar` into the same directory).
fn sibling(name: &str) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(name)))
        .unwrap_or_else(|| PathBuf::from(name))
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

        match TrayIconBuilder::new()
            .with_tooltip("The Familiar — open the Glass")
            .with_icon(marble_icon(32))
            .with_menu(Box::new(menu))
            .build()
        {
            Ok(tray) => self.tray = Some(tray),
            Err(e) => eprintln!("marble: could not create the menu-bar item: {e}"),
        }
    }

    /// Open the Glass, unless one we launched is still up (don't stack windows).
    fn open_glass(&mut self) {
        if let Some(child) = &mut self.glass {
            if matches!(child.try_wait(), Ok(None)) {
                return;
            }
        }
        let exe = sibling("glass");
        match Command::new(&exe)
            .arg("--data-dir")
            .arg(&self.data)
            .spawn()
        {
            Ok(c) => self.glass = Some(c),
            Err(e) => eprintln!("marble: could not open the Glass ({}): {e}", exe.display()),
        }
    }

    fn daemon(&self, sub: &str) {
        let exe = sibling("familiar");
        let _ = Command::new(&exe)
            .args(["daemon", sub, "--data-dir", &self.data])
            .status();
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        // The status item must be created after the event loop is running (macOS).
        if cause == StartCause::Init && self.tray.is_none() {
            event_loop.set_control_flow(ControlFlow::Wait);
            self.build_tray();
            if self.open_on_start {
                self.open_glass();
            }
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
                } else if m.id == ids.stop {
                    self.daemon("stop");
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
/// there's no image asset to ship.
fn marble_icon(size: u32) -> Icon {
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
            let base_r = lerp(120.0, 18.0, t);
            let base_g = lerp(185.0, 64.0, t);
            let base_b = lerp(255.0, 150.0, t);
            // specular highlight near (hx, hy)
            let hdx = x as f32 - hx;
            let hdy = y as f32 - hy;
            let hd = (hdx * hdx + hdy * hdy).sqrt();
            let spec = (1.0 - (hd / (r * 0.55)).clamp(0.0, 1.0)).powf(2.2);
            let rr = (base_r + spec * 190.0).min(255.0);
            let gg = (base_g + spec * 190.0).min(255.0);
            let bb = (base_b + spec * 130.0).min(255.0);
            let edge = ((r - dist).clamp(0.0, 1.5)) / 1.5; // soft 1.5px rim
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

fn install(data: &str) -> std::io::Result<String> {
    let exe = std::env::current_exe()?;
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
    let _ = Command::new("launchctl")
        .args(["load", "-w"])
        .arg(&plist)
        .status();
    Ok(format!("the marble will appear at login -> {}", plist.display()))
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
