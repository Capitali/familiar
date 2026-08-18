//! `catscan` — a CAT scan of United Cat Foods.
//!
//! Ian asked for a status screen for the UCF game that is **not** part of the TestFlight
//! distribution: a CLI that dynamically shows what is going on across every interface to UCF.
//! This is that, and then he asked for it in full cat, which it turns out the domain was
//! begging for — the boundary really is a cat flap and the MCP handshake really is two
//! creatures touching noses.
//!
//! It is its own binary rather than a `familiar` subcommand and rather than anything the
//! device shells embed, because it is an *instrument*: a cat watches the flap, it does not go
//! through it.
//!
//! What it shows, in the order a question about the flap actually gets answered:
//!
//! 1. **COLLAR** — what the human allowed (`mcp/servers.json`), re-read every prowl.
//! 2. **CAT FLAP** — the `allow_network` verdict on this origin, asked the way the client asks.
//! 3. **NOSE BOOP** — the handshake, and drift between what is *offered* and what is *collared*.
//! 4. **The world** — the purr, the perches, the bowls, the yowls, the tomcats, the hauls.
//! 5. **PAW PRINTS** — whether the familiar itself is hunting here, counted from local
//!    evidence rather than asserted.
//!
//! It writes nothing and sheds no fur. Every call it makes is read-only and passes the same
//! boundary any other client would, so a latched flap blanks the world panels and says why —
//! which is the boundary working, not an outage.

mod catch;
mod groom;
mod sniff;

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use familiar_mcp::Session;
use groom::{Fur, Whiskers};

const USAGE: &str = "\
catscan — a CAT scan of United Cat Foods, live

usage:
  catscan [options]

options:
  --server NAME       server from mcp/servers.json (default: ucf)
  --data-dir DIR      the familiar's litter tray (default: the familiar's own)
  --interval SECONDS  how often to prowl (default: 15)
  --once              one look, then back to the sunbeam — for piping, or a quick check
  --plain             no colour, no cursor tricks (also honours NO_COLOR)
  -h, --help          this

It looks. It never touches, and it cannot widen anything: the callable tools come from
the human's collar and every reach passes the cat flap. ^C to stop.
";

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Debug)]
struct Args {
    server: String,
    tray: PathBuf,
    interval: u64,
    once: bool,
    plain: bool,
}

/// Which litter tray to sniff when the human did not say.
///
/// `store::data_dir(None)` is the relative `familiar_data`, which is right for a process
/// launched from a checkout and wrong for a cat carried into an unfamiliar room. "Does the
/// relative dir exist" is too weak a test: a stray empty `familiar_data` in a home directory
/// shadows the real one and the cat looks perfectly content while watching nothing.
///
/// So pick by the thing this tool actually needs — a collar to read. First candidate holding
/// `mcp/servers.json` wins; otherwise the installed per-user tray, whose emptiness the screen
/// will then report honestly against a named path.
fn default_tray() -> PathBuf {
    let installed = familiar_kernel::store::user_data_dir();
    let candidates = [familiar_kernel::store::data_dir(None), installed.clone()];
    candidates
        .into_iter()
        .find(|d| d.join(familiar_mcp::declaration::SERVERS_FILE).is_file())
        .unwrap_or(installed)
}

fn parse(argv: &[String]) -> Result<Args, String> {
    let mut a = Args {
        server: "ucf".into(),
        tray: default_tray(),
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
            "--data-dir" => a.tray = PathBuf::from(value("--data-dir")?),
            "--interval" => {
                let v = value("--interval")?;
                // A cat that paws at a partner's door every tenth of a second is not a
                // monitor, it is 4am. One second is already far below the world's 300s purr.
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
            eprintln!("catscan: {e}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let fur = Fur {
        colour: !args.plain,
    };
    let mut whiskers = Whiskers::new();
    let mut session: Option<Session> = None;
    let mut prowls: u64 = 0;

    loop {
        prowls += 1;
        let p = sniff::prowl(&args.tray, &args.server, &mut session, now());
        let t = groom::twitches(&whiskers, &p);
        let body = groom::draw(&p, &t, &fur, prowls, args.interval);
        groom::remember(&mut whiskers, &p);

        if args.once {
            print!("{body}");
            // One honest exit code: a screen that never got through the flap should not look
            // like success to a script that piped it.
            return match p.purr.got() {
                Some(_) => ExitCode::SUCCESS,
                None => ExitCode::FAILURE,
            };
        }

        // Home the cursor and clear forward rather than clearing the whole screen: the redraw
        // does not flicker, scrollback survives, and ^C leaves a readable terminal.
        let mut out = std::io::stdout().lock();
        if args.plain {
            let _ = writeln!(out, "{body}");
        } else {
            let _ = write!(out, "\x1b[H\x1b[J{body}");
            let _ = write!(
                out,
                "\n  {}\n",
                fur.dim(&format!(
                    "⟳ next prowl in {}s · ^C to stop (or knock it off the table)",
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

    /// Being 4am at a partner's server is a Law III failure, not a personality trait.
    #[test]
    fn the_interval_never_falls_below_a_second() {
        let a = parse(&["--interval".to_string(), "0".into()]).unwrap();
        assert_eq!(a.interval, 1);
    }
}
