//! The LLM seam — a boundary-gated consult. *The model is not the factory.*
//!
//! Every consult is an `Llm` action weighed by the obedience guard against the
//! human-owned boundary. Under the default-closed boundary it is **refused** with no
//! side effects (no prompt written, no network, no key read). Only when a human has
//! opened `allow_llm` does it shell out to the human-installed adapter
//! (`<data-dir>/llm/call_llm.sh`), which the factory does not author.

use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use familiar_kernel::boundary;
use familiar_kernel::guard::{self, Action, ActionKind, Decision};

/// Default adapter deadline — comfortably above the adapter's own 90s
/// per-request network timeout, so the adapter times out first when it can.
pub const DEFAULT_ADAPTER_TIMEOUT: Duration = Duration::from_secs(120);

/// Deadline for a human-lane consult (a dialogue reply). A conversational answer that
/// takes longer than this has already failed as conversation — the caller falls back to
/// an honest templated acknowledgment instead of leaving the person staring at silence.
pub const HUMAN_TIMEOUT: Duration = Duration::from_secs(45);

/// Which queue a consult stands in. The adapter contract is a shared
/// `llm/prompt.txt` → `llm/response.json` pair, so consults serialize; the lane decides
/// *who goes next* when several wait, and whether an in-flight consult must step aside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lane {
    /// A human is waiting on this reply. Goes to the head of the queue, and any
    /// in-flight background consult yields (its adapter is killed; the muse retries
    /// on its own cadence). Law II in scheduling form: presence outranks musing.
    Human,
    /// The metabolism's own thinking (muse, forge, tool authoring). Waits its turn,
    /// and steps aside the moment a human-lane consult arrives.
    Background,
}

/// The lane state: one consult runs at a time (`busy`); human-lane arrivals are counted
/// so both waiting *and running* background consults can see them and step aside.
struct Lanes {
    busy: bool,
    humans_waiting: usize,
}

/// The single per-process consult queue. Poison-tolerant throughout — a panicked
/// consult must not silence the LLM forever.
static LANES: (std::sync::Mutex<Lanes>, std::sync::Condvar) = (
    std::sync::Mutex::new(Lanes {
        busy: false,
        humans_waiting: 0,
    }),
    std::sync::Condvar::new(),
);

/// Holds the consult slot; released (with a wake to all waiters) on drop.
struct LaneGuard;

impl Drop for LaneGuard {
    fn drop(&mut self) {
        let mut l = LANES.0.lock().unwrap_or_else(|e| e.into_inner());
        l.busy = false;
        LANES.1.notify_all();
    }
}

/// Wait for the consult slot. Human-lane callers announce themselves first (so the
/// background lane — waiting or running — steps aside), then take the next free slot.
fn acquire(lane: Lane) -> LaneGuard {
    let mut l = LANES.0.lock().unwrap_or_else(|e| e.into_inner());
    match lane {
        Lane::Human => {
            l.humans_waiting += 1;
            while l.busy {
                l = LANES.1.wait(l).unwrap_or_else(|e| e.into_inner());
            }
            l.humans_waiting -= 1;
        }
        Lane::Background => {
            while l.busy || l.humans_waiting > 0 {
                l = LANES.1.wait(l).unwrap_or_else(|e| e.into_inner());
            }
        }
    }
    l.busy = true;
    LaneGuard
}

/// Is a human-lane consult waiting right now? A running background consult polls this
/// and yields between adapter checks.
fn human_is_waiting() -> bool {
    LANES
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .humans_waiting
        > 0
}

/// The result of a consult attempt.
pub enum Outcome {
    /// The guard refused (boundary closed, or adapter missing/failed). No reach occurred.
    Refused(String),
    /// Every provider is rate-limited right now (adapter exit code 2). Distinct
    /// from `Refused` so callers can wait-and-retry instead of degrading.
    RateLimited(String),
    /// A background consult stepped aside because a human-lane consult arrived
    /// mid-flight (its adapter was killed). Not a failure — the caller simply
    /// retries on its own cadence, as after `Refused`.
    Yielded(String),
    /// The adapter's raw response (JSON text, per call_llm.sh).
    Response(String),
}

/// Consult the LLM with `prompt`, gated by the boundary on disk.
///
/// Returns `Refused` (with a rationale) when the boundary forbids it or the adapter
/// is absent — never reaching outward in those cases. Returns `Response` with the
/// adapter's raw output otherwise.
pub fn consult(dir: &Path, prompt: &str) -> io::Result<Outcome> {
    consult_with(dir, prompt, DEFAULT_ADAPTER_TIMEOUT)
}

/// A human-lane consult: a person is at a console (or speaking) waiting on this reply.
/// Goes to the head of the queue, causes any in-flight background consult to yield,
/// and runs under the shorter [`HUMAN_TIMEOUT`] — past that the caller should say
/// something honest rather than keep the person waiting.
pub fn consult_human(dir: &Path, prompt: &str) -> io::Result<Outcome> {
    consult_in(dir, prompt, HUMAN_TIMEOUT, Lane::Human)
}

/// [`consult`] with an explicit adapter deadline. A hung adapter must never hang
/// the caller: at the deadline the adapter is killed and the consult is
/// `Refused`. Exit code 2 is the adapter contract for "every provider
/// rate-limited" and maps to [`Outcome::RateLimited`].
pub fn consult_with(dir: &Path, prompt: &str, timeout: Duration) -> io::Result<Outcome> {
    consult_in(dir, prompt, timeout, Lane::Background)
}

fn consult_in(dir: &Path, prompt: &str, timeout: Duration, lane: Lane) -> io::Result<Outcome> {
    let _slot = acquire(lane);
    let b = boundary::load(dir)?;
    let verdict = guard::evaluate(&Action::new(ActionKind::Llm, "llm-provider"), &b);
    if verdict.decision != Decision::Allow {
        return Ok(Outcome::Refused(verdict.rationale));
    }

    let llm_dir = dir.join("llm");
    let script = llm_dir.join("call_llm.sh");
    if !script.exists() {
        return Ok(Outcome::Refused(format!(
            "{} not found — install the adapter (see llm/README.md)",
            script.display()
        )));
    }
    fs::create_dir_all(&llm_dir)?;
    fs::write(llm_dir.join("prompt.txt"), prompt)?;
    let mut child = Command::new("sh").arg(&script).spawn()?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(Outcome::Refused(format!(
                "adapter exceeded its {}s deadline and was killed",
                timeout.as_secs()
            )));
        }
        // A person is waiting: background thinking steps aside mid-flight. The muse
        // retries on its own cadence; the human's reply starts within ~100ms.
        if lane == Lane::Background && human_is_waiting() {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(Outcome::Yielded(
                "stepped aside for a human-lane consult".to_string(),
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    if status.code() == Some(2) {
        return Ok(Outcome::RateLimited(
            "every provider rate-limited (adapter exit 2)".to_string(),
        ));
    }
    if !status.success() {
        return Ok(Outcome::Refused(format!(
            "adapter exited with status {status}"
        )));
    }
    let resp = fs::read_to_string(llm_dir.join("response.json"))?;
    Ok(Outcome::Response(resp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The lane queue is process-global, so tests that consult must not overlap: a
    /// human-lane consult in one test would make another test's background consult
    /// yield, failing both. Poison-tolerant so one panicked test doesn't cascade.
    static TEST_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn exclusive() -> std::sync::MutexGuard<'static, ()> {
        TEST_SERIAL.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn refused_with_no_side_effects_under_closed_boundary() {
        let _x = exclusive();
        let p =
            std::env::temp_dir().join(format!("familiar_llm_test_closed_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        let t = Temp(p.clone());
        match consult(&t.0, "hello").unwrap() {
            Outcome::Refused(_) => {}
            Outcome::Response(_) | Outcome::RateLimited(_) | Outcome::Yielded(_) => {
                panic!("closed boundary must refuse")
            }
        }
        // no prompt written, no llm dir created beyond what we made
        assert!(!p.join("llm").join("prompt.txt").exists());
    }

    /// Set up a data dir whose boundary allows LLM and whose adapter is `body`.
    fn open_dir_with_adapter(tag: &str, body: &str) -> Temp {
        use familiar_kernel::boundary::{Boundary, BOUNDARY_FILE};
        let p =
            std::env::temp_dir().join(format!("familiar_llm_test_{tag}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(p.join("llm")).unwrap();
        let mut b = Boundary::closed();
        b.allow_llm = true;
        fs::write(p.join(BOUNDARY_FILE), serde_json::to_string(&b).unwrap()).unwrap();
        fs::write(p.join("llm").join("call_llm.sh"), body).unwrap();
        Temp(p)
    }

    #[test]
    fn adapter_exit_two_is_rate_limited_not_refused() {
        let _x = exclusive();
        let t = open_dir_with_adapter("ratelimit", "#!/bin/sh\nexit 2\n");
        match consult(&t.0, "hello").unwrap() {
            Outcome::RateLimited(why) => assert!(why.contains("rate-limited")),
            Outcome::Refused(why) => panic!("exit 2 must be RateLimited, got Refused({why})"),
            Outcome::Response(_) | Outcome::Yielded(_) => panic!("exit 2 must not be a response"),
        }
    }

    #[test]
    fn consults_serialize_within_the_process() {
        let _x = exclusive();
        // The adapter contract is one shared prompt.txt → response.json pair; two threads
        // consulting at once must not cross their prompts. The adapter echoes the prompt
        // back after a pause — interleaving would hand at least one caller the other's words.
        let t = open_dir_with_adapter(
            "serial",
            "#!/bin/sh\ncp \"$(dirname \"$0\")/prompt.txt\" /tmp/familiar_llm_serial_$$ \n\
             sleep 1\ncp /tmp/familiar_llm_serial_$$ \"$(dirname \"$0\")/response.json\"\n\
             rm -f /tmp/familiar_llm_serial_$$\n",
        );
        let dir_a = t.0.clone();
        let dir_b = t.0.clone();
        let a = std::thread::spawn(move || consult(&dir_a, "prompt-alpha").unwrap());
        let b = std::thread::spawn(move || consult(&dir_b, "prompt-beta").unwrap());
        let (ra, rb) = (a.join().unwrap(), b.join().unwrap());
        match (ra, rb) {
            (Outcome::Response(x), Outcome::Response(y)) => {
                assert_eq!(x, "prompt-alpha", "each caller got back its own words");
                assert_eq!(y, "prompt-beta", "each caller got back its own words");
            }
            _ => panic!("both serialized consults must succeed"),
        }
    }

    #[test]
    fn human_lane_preempts_and_jumps_the_queue() {
        let _x = exclusive();
        // Adapter: log each prompt's first word, sleep, echo the prompt back. Slow enough
        // that the human consult arrives while the first background consult is in flight.
        let t = open_dir_with_adapter(
            "lanes",
            "#!/bin/sh\nd=\"$(dirname \"$0\")\"\nhead -c 32 \"$d/prompt.txt\" >> \"$d/invocations.log\"\n\
             echo >> \"$d/invocations.log\"\nsleep 2\ncp \"$d/prompt.txt\" \"$d/response.json\"\n",
        );
        let (da, db, dh) = (t.0.clone(), t.0.clone(), t.0.clone());
        let bg1 = std::thread::spawn(move || consult(&da, "bg-one").unwrap());
        std::thread::sleep(Duration::from_millis(300)); // bg-one is in flight
        let bg2 = std::thread::spawn(move || consult(&db, "bg-two").unwrap());
        std::thread::sleep(Duration::from_millis(100)); // bg-two is queued behind bg-one
        let human = std::thread::spawn(move || consult_human(&dh, "human-turn").unwrap());
        let (r1, r2, rh) = (
            bg1.join().unwrap(),
            bg2.join().unwrap(),
            human.join().unwrap(),
        );
        // The in-flight background consult stepped aside for the person…
        match r1 {
            Outcome::Yielded(_) => {}
            _ => panic!("in-flight background consult must yield to a waiting human"),
        }
        // …the human got a real answer…
        match rh {
            Outcome::Response(r) => assert_eq!(r, "human-turn"),
            _ => panic!("the human-lane consult must succeed"),
        }
        // …the queued background consult still ran, after the human.
        match r2 {
            Outcome::Response(r) => assert_eq!(r, "bg-two"),
            _ => panic!("the queued background consult must still run"),
        }
        let log = fs::read_to_string(t.0.join("llm").join("invocations.log")).unwrap();
        let order: Vec<&str> = log.lines().collect();
        assert_eq!(
            order,
            vec!["bg-one", "human-turn", "bg-two"],
            "the human turn runs ahead of queued background work"
        );
    }

    #[test]
    fn hung_adapter_is_killed_at_the_deadline() {
        let _x = exclusive();
        let t = open_dir_with_adapter("hang", "#!/bin/sh\nsleep 300\n");
        let started = std::time::Instant::now();
        match consult_with(&t.0, "hello", Duration::from_secs(1)).unwrap() {
            Outcome::Refused(why) => assert!(why.contains("deadline"), "unexpected: {why}"),
            _ => panic!("a hung adapter must end in a timed-out refusal"),
        }
        // Killed near the deadline — not after the adapter's five minutes.
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
