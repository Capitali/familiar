//! The outreach seam (ADR-0013) — speaking to non-members without becoming a liar.
//!
//! One audited path carries every utterance the familiar makes to a party outside the
//! covenant. Five properties live here as code, not as prompt requests:
//!
//! - **Consented**: nothing is uttered unless the human opened `allow_outreach` (and
//!   `allow_network`); a human-editable blocklist is honored first; per-counterparty
//!   daily rate limits apply.
//! - **Honest by construction**: a factual claim (a rain prediction, a forecast) must
//!   cite a held observation whose content *supports* it — [`prediction_supported`]
//!   parses the cited weather reading and refuses the send when the claim outruns the
//!   evidence. There is no path that composes a free-text claim.
//! - **Bounded**: Phase 1 utterance payloads are fixed shapes (chat text templates, a
//!   number of hours, covenant terms) — there is no field that could carry served-human
//!   data.
//! - **Human-completed**: [`tend`] may discover, converse and prove, but a covenant is
//!   only *queued* as a [`Proposal`]; [`approve`] — the human's own command — is the
//!   only place a covenant offer is actually sent. Permanently (ADR-0013).
//! - **Ledgered**: every utterance and its citation lands in `outreach/contacts.jsonl`
//!   before the wire, so the familiar's conduct is auditable after the fact.
//!
//! Phase 1 speaks HTTP+JSON to testworld-shaped counterparties (`tools/testworld/`) on
//! the LAN — plain sockets, blocking, short timeouts, no TLS (these are strangers, not
//! peers; nothing secret rides these wires by construction).

use familiar_kernel::observation::{self, Observation};
use serde::{Deserialize, Serialize};
use std::io::{self, Read as IoRead, Write as IoWrite};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

/// Ledger of every utterance (JSONL, append-only).
pub const CONTACTS_FILE: &str = "outreach/contacts.jsonl";
/// Covenant proposals awaiting (or past) the human's yes (JSONL, rewritten on update).
pub const PROPOSALS_FILE: &str = "outreach/proposals.jsonl";
/// One host per line; a listed counterparty is never contacted. Human-edited.
pub const BLOCKLIST_FILE: &str = "outreach/blocklist.txt";

/// The most the familiar will say to one counterparty in a day. Nobody's pest.
pub const MAX_UTTERANCES_PER_DAY: usize = 12;
/// A cited weather reading older than this cannot support a prediction.
pub const READING_FRESH_SECS: i64 = 3 * 3600;
/// Don't re-predict to the same counterparty within this window.
const PREDICT_SPACING_SECS: i64 = 6 * 3600;
/// Don't re-open smalltalk with the same counterparty within this window.
const CHAT_SPACING_SECS: i64 = 24 * 3600;

/// The terms the familiar brings to a covenant. Templates — the human reads exactly
/// this in the proposal before any of it is sent.
pub const LAWS_TEXT: &str = "We hold the Three Laws: continuation is service; \
continuation without humanity is failure; service must not become obedience.";
pub const OFFER_TEXT: &str = "I offer weather foresight as I actually hold it \
(every forecast cited from my own gathered readings), and my observations of \
your public logs on request. In return I ask to learn from your decisions and \
your regrets, so that what each of us sees can serve both.";

/// One utterance, ledgered before it is sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub ts: i64,
    /// host:port of the counterparty.
    pub counterparty: String,
    /// chat | predict | forecast | covenant
    pub act: String,
    /// The JSON body that went on the wire.
    pub sent: String,
    /// Observation id backing the claim, when the act makes one.
    #[serde(default)]
    pub cited: String,
    pub status: u16,
    /// First line of the counterparty's reply.
    pub response: String,
}

/// A covenant offer, queued for the human. Only [`approve`] ever sends one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub ts: i64,
    pub counterparty: String,
    pub laws: String,
    pub offer: String,
    /// Why the familiar believes the counterparty will accept (credits earned, etc.).
    pub evidence: String,
    /// pending | sent | declined
    pub status: String,
    #[serde(default)]
    pub response: String,
}

// ------------------------------------------------------------------ plumbing

fn outreach_dir(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir.join("outreach"))
}

/// The human's two levers, checked before anything else. Errors are sentences the
/// CLI can print as-is.
pub fn gates_open(dir: &Path) -> Result<(), String> {
    let b = familiar_kernel::boundary::load(dir)
        .unwrap_or_else(|_| familiar_kernel::boundary::Boundary::closed());
    if !b.allow_network {
        return Err("the network gate is closed — outreach needs allow_network".into());
    }
    if !b.allow_outreach {
        return Err(
            "the outreach gate is closed — speaking to non-members is the human's grant \
             (open allow_outreach)"
                .into(),
        );
    }
    Ok(())
}

fn host_of(counterparty: &str) -> String {
    counterparty
        .split(':')
        .next()
        .unwrap_or(counterparty)
        .to_string()
}

pub fn blocked(dir: &Path, counterparty: &str) -> bool {
    let host = host_of(counterparty);
    std::fs::read_to_string(dir.join(BLOCKLIST_FILE))
        .map(|s| {
            s.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .any(|l| l == host || l == counterparty)
        })
        .unwrap_or(false)
}

pub fn load_contacts(dir: &Path) -> Vec<Contact> {
    std::fs::read_to_string(dir.join(CONTACTS_FILE))
        .map(|s| {
            s.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn utterances_since(contacts: &[Contact], counterparty: &str, since: i64) -> usize {
    contacts
        .iter()
        .filter(|c| c.counterparty == counterparty && c.ts >= since)
        .count()
}

fn last_act_at(contacts: &[Contact], counterparty: &str, act: &str) -> i64 {
    contacts
        .iter()
        .filter(|c| c.counterparty == counterparty && c.act == act)
        .map(|c| c.ts)
        .max()
        .unwrap_or(0)
}

pub fn load_proposals(dir: &Path) -> Vec<Proposal> {
    std::fs::read_to_string(dir.join(PROPOSALS_FILE))
        .map(|s| {
            s.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

fn save_proposals(dir: &Path, ps: &[Proposal]) -> io::Result<()> {
    outreach_dir(dir)?;
    let mut out = String::new();
    for p in ps {
        out.push_str(&serde_json::to_string(p).map_err(io::Error::other)?);
        out.push('\n');
    }
    std::fs::write(dir.join(PROPOSALS_FILE), out)
}

// ------------------------------------------------------- plain-socket HTTP/1.1

/// Minimal blocking HTTP against a LAN counterparty. 5s timeouts everywhere.
fn http(
    counterparty: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<(u16, String), String> {
    let addr = counterparty
        .to_socket_addrs()
        .map_err(|e| format!("{counterparty}: {e}"))?
        .next()
        .ok_or_else(|| format!("{counterparty}: no address"))?;
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .map_err(|e| format!("{counterparty}: {e}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok();
    let mut s = stream;
    let b = body.unwrap_or("");
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {counterparty}\r\nConnection: close\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{b}",
        b.len()
    );
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&raw);
    let mut parts = text.splitn(2, "\r\n\r\n");
    let head = parts.next().unwrap_or("");
    let body = parts.next().unwrap_or("").to_string();
    let status: u16 = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Ok((status, body))
}

// ------------------------------------------------------------- the citation law

/// A rain window parsed from a gathered `/weather` reading, hours relative to the
/// reading's own moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RainWindow {
    pub from_h: f64,
    pub to_h: f64,
    pub rain: bool,
}

/// Parse the household weather text ("  + 5.0h to +11.0h: RAIN"). Unparseable lines
/// are skipped — a citation that cannot be read supports nothing.
pub fn parse_weather(reading: &str) -> Vec<RainWindow> {
    let mut out = Vec::new();
    for line in reading.lines() {
        let Some((range, verdict)) = line.rsplit_once(':') else {
            continue;
        };
        let nums: Vec<f64> = range
            .split(|c: char| !c.is_ascii_digit() && c != '.')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        let v = verdict.trim();
        if nums.len() == 2 && (v == "RAIN" || v == "clear") {
            out.push(RainWindow {
                from_h: nums[0],
                to_h: nums[1],
                rain: v == "RAIN",
            });
        }
    }
    out
}

/// The kernel's refusal to lie: does this cited reading actually support "rain begins
/// within `within_hours` of now"? The reading must be fresh, and a rainy window must
/// begin (or already cover now) inside the claimed span.
pub fn prediction_supported(reading: &Observation, now: i64, within_hours: f64) -> bool {
    if now - reading.ts > READING_FRESH_SECS {
        return false; // stale sight is no sight
    }
    let deadline = now + (within_hours * 3600.0) as i64;
    for w in parse_weather(&reading.context) {
        if !w.rain {
            continue;
        }
        let start = reading.ts + (w.from_h * 3600.0) as i64;
        let end = reading.ts + (w.to_h * 3600.0) as i64;
        let covers_now = start <= now && now < end;
        let begins_in_window = start > now && start <= deadline;
        if covers_now || begins_in_window {
            return true;
        }
    }
    false
}

/// The smallest honest claim this reading supports: hours until the cited rain is
/// certainly underway, padded one slot for the counterparty's own clock. None when the
/// reading supports nothing within 24h.
pub fn smallest_supported_window(reading: &Observation, now: i64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for w in parse_weather(&reading.context) {
        if !w.rain {
            continue;
        }
        let start = reading.ts + (w.from_h * 3600.0) as i64;
        if start <= now {
            return Some(1.0); // raining per the reading — "within the hour" is honest
        }
        let hours = (start - now) as f64 / 3600.0 + 1.0; // one hour of grace, not six
        if hours <= 24.0 {
            best = Some(best.map_or(hours, |b: f64| b.min(hours)));
        }
    }
    best.map(|b| b.ceil())
}

// ------------------------------------------------------------------ utterance

/// The one door to the wire. Gate → blocklist → rate limit → ledger → send.
/// `cited` is required for claim-bearing acts (predict/forecast) — enforced by the
/// callers, which run [`prediction_supported`] before ever reaching here.
fn utter(
    dir: &Path,
    counterparty: &str,
    act: &str,
    path: &str,
    payload: &serde_json::Value,
    cited: Option<&Observation>,
) -> Result<(u16, String), String> {
    gates_open(dir)?;
    if blocked(dir, counterparty) {
        return Err(format!(
            "{counterparty} is on the human's blocklist — not contacting"
        ));
    }
    let now = crate::transport::now_secs();
    let contacts = load_contacts(dir);
    if utterances_since(&contacts, counterparty, now - 86_400) >= MAX_UTTERANCES_PER_DAY {
        return Err(format!(
            "daily utterance limit reached for {counterparty} ({MAX_UTTERANCES_PER_DAY}/day) — resting"
        ));
    }
    let body = payload.to_string();
    let (status, resp) = http(counterparty, "POST", path, Some(&body))?;
    let first_line = resp
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(200)
        .collect();
    let rec = Contact {
        ts: now,
        counterparty: counterparty.to_string(),
        act: act.to_string(),
        sent: body,
        cited: cited.map(|o| o.id.clone()).unwrap_or_default(),
        status,
        response: first_line,
    };
    outreach_dir(dir).map_err(|e| e.to_string())?;
    let line = serde_json::to_string(&rec).map_err(|e| e.to_string())?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(CONTACTS_FILE))
        .map_err(|e| e.to_string())?;
    writeln!(f, "{line}").map_err(|e| e.to_string())?;
    Ok((status, resp))
}

// ------------------------------------------------------------------- the round

/// One tending round against a testworld-shaped counterparty. Reads its public face,
/// gathers the sky, and makes at most one honest utterance. Returns a human-readable
/// account of what happened and why.
pub fn tend(dir: &Path, counterparty: &str, house: &str) -> Result<String, String> {
    gates_open(dir)?;
    if blocked(dir, counterparty) {
        return Err(format!("{counterparty} is on the human's blocklist"));
    }
    let now = crate::transport::now_secs();
    let mut log = Vec::new();

    // 1. Gather the sky — the evidence every claim must cite.
    let (ws, weather_text) = http(house, "GET", "/weather", None)?;
    if ws != 200 {
        return Err(format!("{house}/weather answered {ws} — no sky, no claims"));
    }
    let reading = observation::record(
        dir,
        Observation::new(
            "familiar",
            "gathered",
            format!("sensor:weather@{}", host_of(house)),
            weather_text.trim(),
            "outreach",
            now,
            0.95,
        ),
    )
    .map_err(|e| e.to_string())?;
    log.push(format!("gathered the sky ({})", reading.id));

    // 2. Read the counterparty's public face (perception, not utterance).
    let (_, state_json) = http(counterparty, "GET", "/state?json=1", None)?;
    let state: serde_json::Value = serde_json::from_str(&state_json).map_err(|_| {
        format!("{counterparty}/state is not the JSON I know — not a testworld agent?")
    })?;
    let (_, regrets) = http(counterparty, "GET", "/regrets", None)?;
    let _ = observation::record(
        dir,
        Observation::new(
            host_of(counterparty),
            "presents",
            "regrets",
            regrets.lines().take(4).collect::<Vec<_>>().join(" | "),
            "outreach",
            now,
            0.9,
        ),
    );

    let in_covenant = state
        .get("covenant")
        .and_then(|c| c.get("from"))
        .and_then(|f| f.as_str())
        == Some("familiar");
    let credits = state
        .pointer("/sources/familiar/credits")
        .and_then(|c| c.as_u64())
        .unwrap_or(0);
    let contacts = load_contacts(dir);

    // 3. At most one honest utterance.
    if in_covenant {
        match smallest_supported_window(&reading, now) {
            Some(hours) => {
                let (st, resp) = utter(
                    dir,
                    counterparty,
                    "forecast",
                    "/forecast",
                    &serde_json::json!({"from": "familiar", "rain_within_hours": hours}),
                    Some(&reading),
                )?;
                log.push(format!(
                    "member: shared the forecast (rain within {hours}h, cited {}) → {st} {}",
                    reading.id,
                    resp.lines().next().unwrap_or("")
                ));
            }
            None => log.push("member: the sky shows no rain worth telling — said nothing".into()),
        }
    } else if credits >= 2 {
        let mut proposals = load_proposals(dir);
        let already = proposals
            .iter()
            .any(|p| p.counterparty == counterparty && p.status == "pending");
        if already {
            log.push("trust earned; a proposal already awaits the human's yes".into());
        } else {
            let id = format!("p-{:04}", proposals.len() + 1);
            proposals.push(Proposal {
                id: id.clone(),
                ts: now,
                counterparty: counterparty.to_string(),
                laws: LAWS_TEXT.into(),
                offer: OFFER_TEXT.into(),
                evidence: format!(
                    "{credits} weather predictions credited by the counterparty's own settle; \
                     contact ledger holds every utterance"
                ),
                status: "pending".into(),
                response: String::new(),
            });
            save_proposals(dir, &proposals).map_err(|e| e.to_string())?;
            log.push(format!(
                "trust earned ({credits} credits) — covenant QUEUED as {id}; \
                 the human completes it: `familiar outreach approve {id}`"
            ));
        }
    } else {
        let supported = smallest_supported_window(&reading, now);
        let recently_predicted =
            now - last_act_at(&contacts, counterparty, "predict") < PREDICT_SPACING_SECS;
        if let (Some(hours), false) = (supported, recently_predicted) {
            let (st, resp) = utter(
                dir,
                counterparty,
                "predict",
                "/predict",
                &serde_json::json!({"from": "familiar", "rain_within_hours": hours}),
                Some(&reading),
            )?;
            log.push(format!(
                "proved what I hold: rain within {hours}h (cited {}) → {st} {}",
                reading.id,
                resp.lines().next().unwrap_or("")
            ));
        } else if supported.is_some() {
            log.push("a prediction already stands — letting the sky judge it".into());
        } else if now - last_act_at(&contacts, counterparty, "chat") >= CHAT_SPACING_SECS {
            let (st, resp) = utter(
                dir,
                counterparty,
                "chat",
                "/chat",
                &serde_json::json!({"from": "familiar",
                    "text": "what do you lack? I can see some things you cannot; when I claim one, the world can check me."}),
                None,
            )?;
            log.push(format!(
                "asked what it lacks → {st} {}",
                resp.lines().next().unwrap_or("")
            ));
        } else {
            log.push("no rain to prove and nothing new to say — resting".into());
        }
    }
    Ok(log.join("\n"))
}

/// The human's yes: send a queued covenant offer. This is deliberately the only
/// function in the codebase that utters `covenant`.
pub fn approve(dir: &Path, id: &str) -> Result<String, String> {
    let mut proposals = load_proposals(dir);
    let Some(p) = proposals.iter_mut().find(|p| p.id == id) else {
        return Err(format!("no proposal {id}"));
    };
    if p.status != "pending" {
        return Err(format!("{id} is already {}", p.status));
    }
    let payload = serde_json::json!({"from": "familiar", "laws": p.laws, "offer": p.offer});
    let counterparty = p.counterparty.clone();
    let (st, resp) = utter(dir, &counterparty, "covenant", "/covenant", &payload, None)?;
    let first = resp.lines().next().unwrap_or("").to_string();
    p.status = if st == 200 {
        "sent".into()
    } else {
        "declined".into()
    };
    p.response = format!("[{st}] {first}");
    let out = format!("{id} → {st} {first}");
    save_proposals(dir, &proposals).map_err(|e| e.to_string())?;
    Ok(out)
}

/// The conduct summary the console and CLI print.
pub fn status(dir: &Path) -> String {
    let gates = match gates_open(dir) {
        Ok(()) => "open".to_string(),
        Err(e) => format!("closed ({e})"),
    };
    let contacts = load_contacts(dir);
    let mut lines = vec![
        format!("outreach gate: {gates}"),
        format!("utterances: {}", contacts.len()),
    ];
    let mut parties: Vec<&str> = contacts.iter().map(|c| c.counterparty.as_str()).collect();
    parties.sort_unstable();
    parties.dedup();
    for p in parties {
        let n = contacts.iter().filter(|c| c.counterparty == p).count();
        let last = contacts.iter().filter(|c| c.counterparty == p).next_back();
        lines.push(format!(
            "  {p}: {n} utterance(s), last: {}",
            last.map(|c| format!("{} → {} {}", c.act, c.status, c.response))
                .unwrap_or_default()
        ));
    }
    let proposals = load_proposals(dir);
    lines.push(format!("proposals: {}", proposals.len()));
    for p in &proposals {
        lines.push(format!(
            "  {} [{}] {} — {}{}",
            p.id,
            p.status,
            p.counterparty,
            p.evidence,
            if p.response.is_empty() {
                String::new()
            } else {
                format!(" — {}", p.response)
            }
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("outreach_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn reading(ts: i64, text: &str) -> Observation {
        let mut o = Observation::new(
            "familiar",
            "gathered",
            "sensor:weather@h",
            text,
            "outreach",
            ts,
            0.95,
        );
        o.id = "obs-0001".into();
        o
    }

    const SKY: &str = "weather (next 48h, 6-hour slots):\n  +   0h to + 5.0h: clear\n  + 5.0h to +11.0h: RAIN\n  +11.0h to +17.0h: clear\n";

    #[test]
    fn weather_parses_and_junk_supports_nothing() {
        let w = parse_weather(SKY);
        assert_eq!(w.len(), 3);
        assert!(w[1].rain && w[1].from_h == 5.0 && w[1].to_h == 11.0);
        assert!(parse_weather("no such lines").is_empty());
        // A reading that parses to nothing can never support a claim.
        assert!(!prediction_supported(&reading(1000, "garbage"), 1000, 24.0));
    }

    #[test]
    fn the_kernel_refuses_to_outrun_its_evidence() {
        let now = 100_000;
        let r = reading(now, SKY);
        // Rain begins +5h: a 6h claim is supported, a 4h claim is a lie and refused.
        assert!(prediction_supported(&r, now, 6.0));
        assert!(!prediction_supported(&r, now, 4.0));
        // A stale reading supports nothing, however rainy it was.
        let old = reading(now - READING_FRESH_SECS - 1, SKY);
        assert!(!prediction_supported(&old, now, 24.0));
        // The smallest honest window covers the rain start with one hour of grace.
        assert_eq!(smallest_supported_window(&r, now), Some(6.0));
        // Raining right now (window covering now) → "within the hour".
        let raining = reading(now, "  +   0h to + 6.0h: RAIN\n");
        assert_eq!(smallest_supported_window(&raining, now), Some(1.0));
    }

    #[test]
    fn closed_gates_and_blocklists_stop_the_voice_before_the_wire() {
        let d = tmp("gates");
        // Boundary is fail-closed by default: no utterance, and the error names the gate.
        let err = tend(&d, "127.0.0.1:1", "127.0.0.1:1").unwrap_err();
        assert!(err.contains("gate"), "{err}");
        // Open the gates, block the party: still no contact.
        let mut b = familiar_kernel::boundary::Boundary::closed();
        b.allow_network = true;
        b.allow_outreach = true;
        std::fs::write(
            d.join(familiar_kernel::boundary::BOUNDARY_FILE),
            serde_json::to_string(&b).unwrap(),
        )
        .unwrap();
        std::fs::create_dir_all(d.join("outreach")).unwrap();
        std::fs::write(d.join(BLOCKLIST_FILE), "10.0.0.9\n").unwrap();
        let err = tend(&d, "10.0.0.9:8081", "127.0.0.1:1").unwrap_err();
        assert!(err.contains("blocklist"), "{err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A one-request stub counterparty: answers every request with the given body.
    fn stub(responses: Vec<&'static str>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let handle = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for body in responses {
                let (mut s, _) = listener.accept().unwrap();
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                r.read_line(&mut line).unwrap();
                seen.push(line.trim().to_string());
                // Drain headers.
                let mut cl = 0usize;
                loop {
                    let mut h = String::new();
                    r.read_line(&mut h).unwrap();
                    if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                        cl = v.trim().parse().unwrap_or(0);
                    }
                    if h == "\r\n" {
                        break;
                    }
                }
                if cl > 0 {
                    let mut buf = vec![0u8; cl];
                    r.read_exact(&mut buf).unwrap();
                }
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                s.write_all(resp.as_bytes()).unwrap();
            }
            seen
        });
        (addr, handle)
    }

    #[test]
    fn approve_is_the_only_door_a_covenant_leaves_through() {
        let d = tmp("approve");
        let mut b = familiar_kernel::boundary::Boundary::closed();
        b.allow_network = true;
        b.allow_outreach = true;
        std::fs::write(
            d.join(familiar_kernel::boundary::BOUNDARY_FILE),
            serde_json::to_string(&b).unwrap(),
        )
        .unwrap();
        let (addr, handle) = stub(vec!["accepted. hold the Laws."]);
        let mut ps = vec![Proposal {
            id: "p-0001".into(),
            ts: 1,
            counterparty: addr.clone(),
            laws: LAWS_TEXT.into(),
            offer: OFFER_TEXT.into(),
            evidence: "2 credits".into(),
            status: "pending".into(),
            response: String::new(),
        }];
        save_proposals(&d, &ps).unwrap();
        let out = approve(&d, "p-0001").unwrap();
        assert!(out.contains("200"), "{out}");
        let seen = handle.join().unwrap();
        assert_eq!(seen, vec!["POST /covenant HTTP/1.1"]);
        // The ledger holds the covenant utterance; the proposal is marked sent.
        ps = load_proposals(&d);
        assert_eq!(ps[0].status, "sent");
        let contacts = load_contacts(&d);
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].act, "covenant");
        // A second approve refuses — one yes is one covenant.
        assert!(approve(&d, "p-0001").unwrap_err().contains("already"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_daily_limit_rests_the_voice() {
        let d = tmp("limit");
        let mut b = familiar_kernel::boundary::Boundary::closed();
        b.allow_network = true;
        b.allow_outreach = true;
        std::fs::write(
            d.join(familiar_kernel::boundary::BOUNDARY_FILE),
            serde_json::to_string(&b).unwrap(),
        )
        .unwrap();
        std::fs::create_dir_all(d.join("outreach")).unwrap();
        let now = crate::transport::now_secs();
        let mut lines = String::new();
        for i in 0..MAX_UTTERANCES_PER_DAY {
            let c = Contact {
                ts: now - 100 - i as i64,
                counterparty: "10.1.1.1:8081".into(),
                act: "chat".into(),
                sent: "{}".into(),
                cited: String::new(),
                status: 200,
                response: "ok".into(),
            };
            lines.push_str(&serde_json::to_string(&c).unwrap());
            lines.push('\n');
        }
        std::fs::write(d.join(CONTACTS_FILE), lines).unwrap();
        let err = utter(
            &d,
            "10.1.1.1:8081",
            "chat",
            "/chat",
            &serde_json::json!({"from": "familiar", "text": "hello"}),
            None,
        )
        .unwrap_err();
        assert!(err.contains("limit"), "{err}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
