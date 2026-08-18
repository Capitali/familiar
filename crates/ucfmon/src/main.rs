//! `ucfmon` — a developer's window on the UCF seam.
//!
//! Ian asked for a status screen for the UCF exchange that is **not** part of the TestFlight
//! distribution: a CLI that dynamically shows what is going on across every interface to UCF.
//! This is that. It is its own binary rather than a `familiar` subcommand and rather than
//! anything the device shells embed, because it is an *instrument* — it watches the seam and
//! never participates in it.
//!
//! What it shows, in the order a question about the seam actually gets answered:
//!
//! 1. **DECLARATION** — what the human allowed (`mcp/servers.json`), re-read every round.
//! 2. **BOUNDARY** — the `allow_network` verdict on this origin, asked the way the client asks.
//! 3. **WIRE** — the handshake, and drift between what is *offered* and what is *declared*.
//! 4. **The world** — the exchange's own clock, stations, market, news, carriers, freight.
//! 5. **FAMILIAR→UCF** — whether the familiar itself is acting here, counted from local
//!    evidence rather than asserted.
//!
//! It writes nothing. Every call it makes is read-only and passes the same boundary any other
//! client would, so a shut gate blanks the world panels and says why — which is the boundary
//! working, not an outage.

mod probe;
mod render;
mod world;

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use familiar_mcp::Session;
use render::{Ink, Memory};

const USAGE: &str = "\
ucfmon — a live window on the familiar's UCF seam

usage:
  ucfmon [options]

options:
  --server NAME       server from mcp/servers.json (default: ucf)
  --data-dir DIR      the familiar's data dir (default: the familiar's own)
  --interval SECONDS  how often to look (default: 15)
  --once              draw one screen and exit — for piping, or a quick look
  --plain             no colour, no cursor tricks (also honours NO_COLOR)
  -h, --help          this

It reads. It never writes, and it cannot widen anything: the callable tools come from
the human's declaration and every reach passes the boundary. ^C to stop.
";

/// Where to look when the human did not say.
///
/// `store::data_dir(None)` is the relative `familiar_data`, which is right for a process
/// launched from a checkout and wrong for an instrument run from anywhere. "Does the relative
/// dir exist" is too weak a test: a stray empty `familiar_data` in a home directory shadows
/// the real one and the monitor looks healthy while watching nothing.
///
/// So pick by the thing this tool actually needs — a declaration to read. First candidate
/// holding `mcp/servers.json` wins; otherwise the installed per-user dir, whose emptiness the
/// screen will then report honestly against a named path.
fn default_dir() -> PathBuf {
    let installed = familiar_kernel::store::user_data_dir();
    let candidates = [familiar_kernel::store::data_dir(None), installed.clone()];
    candidates
        .into_iter()
        .find(|d| d.join(familiar_mcp::declaration::SERVERS_FILE).is_file())
        .unwrap_or(installed)
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Debug)]
struct Args {
    server: String,
    dir: PathBuf,
    interval: u64,
    once: bool,
    plain: bool,
}

fn parse(argv: &[String]) -> Result<Args, String> {
    let mut a = Args {
        server: "ucf".into(),
        dir: default_dir(),
        interval: 15,
        once: false,
        plain: std::env::var_os("NO_COLOR").is_some(),
    };
    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        let mut value = |what: &str| -> Result<String, String> {
            i += 1;
            argv.get(i)
                .cloned()
                .ok_or_else(|| format!("{what} needs a value"))
        };
        match arg {
            "--server" => a.server = value("--server")?,
            "--data-dir" => a.dir = PathBuf::from(value("--data-dir")?),
            "--interval" => {
                let v = value("--interval")?;
                // A monitor that hammers a partner's server is a bad guest; one second is
                // already far below the world's 300s tick.
                a.interval = v
                    .parse::<u64>()
                    .map_err(|_| format!("--interval wants whole seconds, not '{v}'"))?
                    .max(1);
            }
            "--once" => a.once = true,
            "--plain" => a.plain = true,
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown option '{other}'")),
        }
        i += 1;
    }
    Ok(a)
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse(&argv) {
        Ok(a) => a,
        Err(e) => {
            if e.is_empty() {
                print!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            eprintln!("ucfmon: {e}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let ink = Ink { color: !args.plain };
    let mut mem = Memory::new();
    let mut session: Option<Session> = None;
    let mut rounds: u64 = 0;

    loop {
        rounds += 1;
        let r = probe::round(&args.dir, &args.server, &mut session, now());
        let changes = render::diff(&mem, &r);
        let body = render::screen(&r, &changes, &ink, rounds, args.interval);
        render::absorb(&mut mem, &r);

        if args.once {
            print!("{body}");
            // One honest exit code: a screen that could not reach the seam should not look
            // like success to a script that piped it.
            return match r.status.ok() {
                Some(_) => ExitCode::SUCCESS,
                None => ExitCode::FAILURE,
            };
        }

        // Home the cursor and clear forward rather than clearing the whole screen: the
        // redraw does not flicker, and scrollback survives, so ^C leaves a readable terminal.
        let mut out = std::io::stdout().lock();
        if args.plain {
            let _ = writeln!(out, "{body}");
        } else {
            let _ = write!(out, "\x1b[H\x1b[J{body}");
            let _ = write!(
                out,
                "\n  {}\n",
                ink.dim(&format!(
                    "⟳ looking again in {}s · ^C to stop",
                    args.interval
                ))
            );
        }
        let _ = out.flush();

        std::thread::sleep(Duration::from_secs(args.interval));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_ones_a_human_would_assume() {
        let a = parse(&[]).unwrap();
        assert_eq!(a.server, "ucf");
        assert_eq!(a.interval, 15);
        assert!(!a.once);
    }

    #[test]
    fn options_parse_and_bad_ones_are_refused_by_name() {
        let argv: Vec<String> = ["--server", "x", "--interval", "30", "--once", "--plain"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let a = parse(&argv).unwrap();
        assert_eq!(a.server, "x");
        assert_eq!(a.interval, 30);
        assert!(a.once && a.plain);

        assert!(parse(&["--nope".to_string()])
            .unwrap_err()
            .contains("--nope"));
        assert!(parse(&["--interval".to_string()])
            .unwrap_err()
            .contains("needs a value"));
        assert!(parse(&["--interval".to_string(), "soon".into()])
            .unwrap_err()
            .contains("whole seconds"));
    }

    /// Being a bad guest on a partner's server is a Law III failure, not a preference.
    #[test]
    fn the_interval_never_falls_below_a_second() {
        let a = parse(&["--interval".to_string(), "0".into()]).unwrap();
        assert_eq!(a.interval, 1);
    }
}
