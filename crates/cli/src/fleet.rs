//! `familiar fleet` — the captain's ships, and the pilots that fly them.
//!
//! A captain in the game buys the familiar for a ship they own or lease and hands
//! it a key (today a trading key; tomorrow the co-pilot key Jeff mints with the
//! purchased automation scopes — ucf-exchange#15). PAIRING turns that hand-off into
//! a ship world of its own (ADR-0045: the ship's store holds the key, the grants and
//! the journal; the household holds only the provisioning record), leased by the
//! household's word. UNPAIRING is revocation: the pilot stops, the key is destroyed,
//! the record stays. STATUS reads every ship's own ledger on the wire and books
//! them per captain — earnings POOL within one captain's ships and never across
//! captains (Ian, 2026-09-01, the fleet money boundary). RUN keeps one pilot per
//! paired ship alive and, when told to, renews leases before they lapse.
//!
//! Ian, 2026-09-02: "The familiar needs to work in conjunction to a captain in the
//! game who purchased the familiar for their owned or leased ship … we need it all
//! working first" — this is the working half; the in-game purchase and key minting
//! are Jeff's add-in.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use familiar_mcp::{http, Url};
use familiar_world::instance::{self, Lifecycle, WorldInstance};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Who the ship flies for, written at pairing (`captain.json` in the ship store).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Captain {
    /// The captain's IDENTITY: generated once, meaningless, and the only thing
    /// anything keys on — the store directory, the persona, the money.
    ///
    /// It is deliberately not derived from the name, the key or the world hash.
    /// The slug it replaces was a lossy transform of the display name: every
    /// non-alphanumeric became `-`, so "A/B" and "A B" both landed on
    /// `captains/a-b` and the captain brief summed two people's credits, debt and
    /// realized P&L into one pile (codex T-236 re-verification, finding 4). An id
    /// that means nothing cannot collide, and a captain who renames himself does
    /// not move his money.
    ///
    /// Empty on a record written before this existed; [`ensure_captain_id`] fills
    /// it in and is the only thing that ever writes it.
    #[serde(default)]
    pub captain_id: String,
    /// The captain's DISPLAY NAME. A label, never an identifier.
    pub captain: String,
    /// The key's public id (`keyId`, the first 8 hex of the secret) — never the secret.
    pub key_id: String,
    pub server: String,
    pub automations: Vec<String>,
    pub paired_at: i64,
    /// The hull's DISPLAY name as `/v1/me` showed it at pairing — a courtesy label,
    /// not an identity: the exchange offers no durable hull id yet, so the
    /// operational binding stays `(server, key_id)` (T-236 dialogue, correction 3).
    #[serde(default)]
    pub hull_name: String,
    /// Extra arguments for this ship's pilot (a LOCAL soak passes `--allow-paws`
    /// and a short `--interval-floor`; a PROD hull passes nothing).
    #[serde(default)]
    pub pilot_args: Vec<String>,
}

pub(crate) fn read_env_value(path: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().to_string())
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// GET on the exchange with the ship's own key. Bounded: one call, one timeout.
pub(crate) fn wire_get(server: &str, key: &str, path: &str) -> Result<Value, String> {
    let url = Url::parse(&format!("{}{}", server.trim_end_matches('/'), path))
        .map_err(|e| format!("{e:?}"))?;
    let headers = vec![
        ("Authorization".to_string(), format!("Bearer {key}")),
        ("X-UCF-App".to_string(), "familiar-fleet".to_string()),
    ];
    let resp = http::get(&url, &headers).map_err(|e| format!("{e:?}"))?;
    if !(200..300).contains(&resp.status) {
        return Err(format!("HTTP {}", resp.status));
    }
    serde_json::from_slice(&resp.body).map_err(|e| e.to_string())
}

/// Issue a fresh lease for a ship from the household's boundary and key — the same
/// act as `familiar world lease`, callable by the supervisor when a human has said
/// `--renew`.
pub(crate) fn issue_lease(
    dir: &Path,
    ship_dir: &Path,
    id: &str,
    ttl_hours: i64,
) -> Result<i64, String> {
    let root_boundary = familiar_kernel::boundary::load(dir).map_err(|e| e.to_string())?;
    let key = familiar_mesh::node::NodeKey::load_or_mint(dir, "").map_err(|e| e.to_string())?;
    let now = super::now_secs();
    let signed = familiar_world::lease::issue(
        &root_boundary,
        id,
        ttl_hours.saturating_mul(3600),
        now,
        &key,
    )
    .map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(&signed).map_err(|e| e.to_string())?;
    std::fs::write(ship_dir.join("lease.json"), bytes).map_err(|e| e.to_string())?;
    Ok(now + ttl_hours * 3600)
}

/// When the ship's current lease expires, if it can be read.
pub(crate) fn lease_expiry(ship_dir: &Path) -> Option<i64> {
    let raw = std::fs::read_to_string(ship_dir.join("lease.json")).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    let inner: Value = serde_json::from_str(v.get("lease_json")?.as_str()?).ok()?;
    inner.get("expires_at")?.as_i64()
}

pub(crate) fn pid_alive(ship_dir: &Path) -> Option<u32> {
    let pid: u32 = std::fs::read_to_string(ship_dir.join("whisker.pid"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    #[cfg(unix)]
    {
        // kill(pid, 0): no signal, just "does it exist and may I signal it".
        let alive = unsafe { libc_kill(pid as i32, 0) } == 0;
        return alive.then_some(pid);
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(unix)]
extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}

pub(crate) fn stop_pilot(ship_dir: &Path) -> bool {
    match pid_alive(ship_dir) {
        Some(pid) => {
            #[cfg(unix)]
            unsafe {
                libc_kill(pid as i32, 15);
            }
            let _ = std::fs::remove_file(ship_dir.join("whisker.pid"));
            true
        }
        None => false,
    }
}

/// A paired ship: the record, its store, and who it flies for.
pub(crate) struct Ship {
    pub(crate) world: WorldInstance,
    pub(crate) dir: PathBuf,
    pub(crate) captain: Captain,
}

pub(crate) fn paired_ships(dir: &Path, root: &Path) -> Vec<Ship> {
    let Ok(all) = instance::load(dir) else {
        return Vec::new();
    };
    all.into_iter()
        .filter(|w| w.lifecycle != Lifecycle::Decommissioned)
        .filter_map(|w| {
            let ship_dir = root.join(&w.id);
            let captain: Captain =
                serde_json::from_str(&std::fs::read_to_string(ship_dir.join("captain.json")).ok()?)
                    .ok()?;
            ship_dir.join("ucf.env").exists().then_some(Ship {
                world: w,
                dir: ship_dir,
                captain,
            })
        })
        .collect()
}

pub(crate) fn last_journal_line(ship_dir: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(ship_dir.join("journal.jsonl")).ok()?;
    text.lines()
        .rev()
        .find_map(|l| serde_json::from_str::<Value>(l).ok())
}

/// The merchant's book from the exchange's own receipt trail: realized profit on
/// sold lots (FIFO cost), what it cost, and what is still aboard at cost. The
/// trail covers roughly the last day of ticks, so this is a rolling window on a
/// fast world and the whole story on a slow one.
#[derive(Debug, Default, Clone, Serialize)]
pub(crate) struct TradeBook {
    pub(crate) filled: i64,
    pub(crate) rejected: i64,
    pub(crate) realized: i64,
    pub(crate) cost_of_sold: i64,
    pub(crate) inventory_cost: i64,
    pub(crate) inventory: BTreeMap<String, i64>,
    /// Units sold whose purchase this book never saw, and what they fetched. NEVER
    /// counted as profit — a sale with no cost is not a gain, it is a gap.
    pub(crate) unmatched_units: i64,
    pub(crate) unmatched_proceeds: i64,
    /// Lots whose basis came from the pilot's quoted ask rather than a fill receipt.
    pub(crate) quoted_basis_lots: i64,
}

/// The ship's own record of its fills — every `trade-outcome` the pilot journaled —
/// as receipt-shaped rows. The exchange's `/v1/receipts` covers roughly a day of
/// ticks, so a buy older than that vanishes and its sale reads as pure profit
/// (KK II's salmon-mousse, 2026-09-03: "realized 6074 on 0 sold"). The journal is
/// the whole story; the wire is the fallback for a store without one.
///
/// One gap the journal can carry: a pilot restarted between filing a buy and reading
/// its receipt never journals that fill, and by the time anyone asks, the wire's
/// window has rolled past it (KK II's bluefin, bought t7078, sold t7436). The
/// `position-opened` line the pilot writes when it files the buy carries the good,
/// the units and the QUOTED ask, so a lot with no fill of its own is reconstructed
/// from it — marked `basis_from: "quote"`, because a quote is not a fill: the real
/// total carries tax and walks the curve, so this basis is a floor and the profit it
/// implies is a ceiling.
pub(crate) fn journal_fills(ship_dir: &Path) -> Value {
    let Ok(text) = std::fs::read_to_string(ship_dir.join("journal.jsonl")) else {
        return Value::Null;
    };
    let lines: Vec<Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    let ev = |v: &Value| {
        v.get("event")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let mut rows: Vec<Value> = lines
        .iter()
        .filter(|v| ev(v) == "trade-outcome")
        .cloned()
        .collect();
    for op in lines.iter().filter(|v| ev(v) == "position-opened") {
        let good = op.get("good").and_then(Value::as_str).unwrap_or("");
        let tick = op.get("tick").and_then(Value::as_i64).unwrap_or(0);
        let units = op.get("units").and_then(Value::as_i64).unwrap_or(0);
        let ask = op.get("ask").and_then(Value::as_i64).unwrap_or(0);
        if good.is_empty() || units <= 0 || ask <= 0 {
            continue;
        }
        // Its own fill, if the pilot ever read one back: same good, buy, within a
        // few ticks of the filing.
        let read_back = rows.iter().any(|r| {
            r.get("side").and_then(Value::as_str) == Some("buy")
                && r.get("good").and_then(Value::as_str) == Some(good)
                && (r.get("tick").and_then(Value::as_i64).unwrap_or(0) - tick).abs() <= 6
        });
        if !read_back {
            rows.push(
                json!({"tick": tick + 1, "side": "buy", "good": good, "units": units,
                             "total": ask * units, "outcome": "filled", "basis_from": "quote"}),
            );
        }
    }
    if rows.is_empty() {
        Value::Null
    } else {
        Value::Array(rows)
    }
}

pub(crate) fn trade_book(receipts: &Value) -> TradeBook {
    let mut book = TradeBook::default();
    let Some(rows) = receipts.as_array() else {
        return book;
    };
    let mut fills: Vec<&Value> = rows
        .iter()
        .filter(|r| {
            let filled = r.get("outcome").and_then(Value::as_str) == Some("filled");
            if !filled {
                book.rejected += 1;
            }
            filled
        })
        .collect();
    fills.sort_by_key(|r| r.get("tick").and_then(Value::as_i64).unwrap_or(0));
    // good → lots of (units, cost per unit ×1000 for integer arithmetic)
    let mut lots: BTreeMap<String, std::collections::VecDeque<(i64, i64)>> = BTreeMap::new();
    for r in fills {
        book.filled += 1;
        let good = r
            .get("good")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let units = r.get("units").and_then(Value::as_i64).unwrap_or(0);
        let total = r.get("total").and_then(Value::as_i64).unwrap_or(0);
        if units <= 0 {
            continue;
        }
        if r.get("side").and_then(Value::as_str) == Some("buy") {
            if r.get("basis_from").and_then(Value::as_str) == Some("quote") {
                book.quoted_basis_lots += 1;
            }
            lots.entry(good)
                .or_default()
                .push_back((units, total * 1000 / units));
        } else {
            let mut left = units;
            let mut cost_milli = 0;
            if let Some(q) = lots.get_mut(&good) {
                while left > 0 {
                    let Some(front) = q.front_mut() else { break };
                    let take = front.0.min(left);
                    cost_milli += take * front.1;
                    front.0 -= take;
                    left -= take;
                    if front.0 == 0 {
                        q.pop_front();
                    }
                }
            }
            // `left` units were sold out of a lot this book never saw bought. Their
            // share of the proceeds is set aside, not banked: counting it as profit
            // is how KK II's bluefin read as +5,583 on a cost of nothing.
            let matched = units - left;
            let matched_proceeds = if units > 0 {
                total * matched / units
            } else {
                0
            };
            let cost = cost_milli / 1000;
            book.realized += matched_proceeds - cost;
            book.cost_of_sold += cost;
            book.unmatched_units += left;
            book.unmatched_proceeds += total - matched_proceeds;
        }
    }
    for (good, q) in &lots {
        let units: i64 = q.iter().map(|(u, _)| *u).sum();
        if units > 0 {
            book.inventory.insert(good.clone(), units);
            book.inventory_cost += q.iter().map(|(u, c)| u * c).sum::<i64>() / 1000;
        }
    }
    book
}

/// Where a captain's computer lives. Ian's ruling, 2026-09-04: *"One 'ships computer'
/// per captain that can act across his entire fleet under a name he chooses."* So the
/// persona is not a property of a hull — it is the captain's, and every ship they pair
/// answers as it. The record sits beside `worlds/` in a `captains/<slug>/` store; the
/// ship stores keep `captain.json` (who they fly for) and nothing else about the voice.
/// A persona written into a ship store before this ruling is still read, as a fallback,
/// so a store from last week does not lose its name.
/// Where one captain's computer lives, by IDENTITY.
///
/// The id is the whole key. [`captain_slug`] survives only to find a store written
/// before ids existed, and nothing new is ever placed by it.
/// One captain's pooled book on `fleet status`: display name, then credits, debt,
/// hauls, freight paid, realized trade P&L, inventory at cost.
type CaptainBook = (String, i64, i64, i64, i64, i64, i64);

pub(crate) fn captain_store_by_id(root: &Path, captain_id: &str) -> PathBuf {
    root.parent()
        .unwrap_or(root)
        .join("captains")
        .join(if captain_id.trim().is_empty() {
            "captain"
        } else {
            captain_id.trim()
        })
}

/// The captain's store, preferring identity and falling back to the legacy slug for
/// a record that has not been migrated yet.
pub(crate) fn captain_store_for(root: &Path, rec: &Captain) -> PathBuf {
    if !rec.captain_id.trim().is_empty() {
        return captain_store_by_id(root, &rec.captain_id);
    }
    captain_store(root, &rec.captain)
}

/// The one lock every identity assignment and store move takes: `captains/.migrate.lock`
/// beside the stores. Held by the OS, released however the holder exits, never unlinked.
fn migration_lock(root: &Path) -> Result<std::fs::File, String> {
    let captains = root.parent().unwrap_or(root).join("captains");
    std::fs::create_dir_all(&captains).map_err(|e| format!("captains store: {e}"))?;
    let f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(captains.join(".migrate.lock"))
        .map_err(|e| format!("migration lock: {e}"))?;
    f.lock().map_err(|e| format!("migration lock: {e}"))?;
    Ok(f)
}

/// Where a captain's computer lives today. Typed, so a caller migrates the WHOLE
/// record from the right place instead of reading one directory and defaulting
/// (codex T-236 round 2, finding 2: a rename on a legacy hull loaded only the new,
/// empty id store and printed `was "the familiar"` over a tuned persona).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Origin {
    /// Already under the captain's identity.
    IdStore(PathBuf),
    /// The pre-identity slug store, whole (persona + trail), not yet moved.
    LegacyStore(PathBuf),
    /// A ship's own record, from before the per-captain ruling.
    ShipLocal(PathBuf),
    /// Never named anywhere.
    None,
}

/// Resolve the origin for `rec`, looking at every same-captain hull in `ship_dirs`.
/// Two ship-local records that DISAGREE are refused rather than the first taken:
/// picking one silently is how a captain ends up with two voices.
pub(crate) fn computer_origin(
    root: &Path,
    rec: &Captain,
    ship_dirs: &[PathBuf],
) -> Result<Origin, String> {
    let file = familiar_kernel::persona::PERSONA_FILE;
    if !rec.captain_id.trim().is_empty() {
        let id_store = captain_store_by_id(root, &rec.captain_id);
        if id_store.join(file).exists() {
            return Ok(Origin::IdStore(id_store));
        }
    }
    let legacy = captain_store(root, &rec.captain);
    if legacy.join(file).exists() {
        return Ok(Origin::LegacyStore(legacy));
    }
    let mut locals: Vec<(&PathBuf, Vec<u8>)> = Vec::new();
    for d in ship_dirs {
        if let Ok(bytes) = std::fs::read(d.join(file)) {
            locals.push((d, bytes));
        }
    }
    let Some((first, first_bytes)) = locals.first() else {
        return Ok(Origin::None);
    };
    if let Some((other, _)) = locals.iter().find(|(_, b)| b != first_bytes) {
        return Err(format!(
            "{} has two computers on record that disagree — {} and {} — refusing to pick one; \
             `fleet rename` the hull that is wrong first",
            rec.captain,
            first.display(),
            other.display()
        ));
    }
    Ok(Origin::ShipLocal((*first).clone()))
}

/// Bring a ship-local computer into the captain's id store WHOLE: persona bytes and
/// naming trail, byte-equivalent. Under the persona lock the kernel uses, so a
/// concurrent naming cannot interleave. Idempotent: a store that already carries a
/// persona is left alone.
pub(crate) fn migrate_computer(
    root: &Path,
    captain_id: &str,
    origin: &Origin,
) -> Result<(), String> {
    let Origin::ShipLocal(from) = origin else {
        return Ok(()); // an id store needs nothing; a legacy store moved with the identity
    };
    let to = captain_store_by_id(root, captain_id);
    let file = familiar_kernel::persona::PERSONA_FILE;
    let trail = familiar_kernel::persona::NAME_EVENTS_FILE;
    std::fs::create_dir_all(&to).map_err(|e| format!("{}: {e}", to.display()))?;
    if to.join(file).exists() {
        return Ok(());
    }
    let put = |name: &str| -> std::io::Result<()> {
        let src = from.join(name);
        if !src.exists() {
            return Ok(());
        }
        let tmp = to.join(format!("{name}.{}.migrate", std::process::id()));
        std::fs::copy(&src, &tmp)?;
        std::fs::File::open(&tmp)?.sync_all()?;
        std::fs::rename(&tmp, to.join(name))
    };
    put(trail).and_then(|_| put(file)).map_err(|e| {
        let _ = std::fs::remove_file(to.join(trail));
        let _ = std::fs::remove_file(to.join(file));
        format!(
            "migrating {}'s computer from {}: {e}",
            captain_id,
            from.display()
        )
    })?;
    std::fs::File::open(&to)
        .and_then(|d| d.sync_all())
        .map_err(|e| format!("{}: {e}", to.display()))
}

/// Every same-captain hull that has no identity yet takes this one, so one captain
/// resolves one store from every hull — never the migrated copy from the new hull
/// and the ship-local copy from the old (finding 2). Returns how many were updated.
pub(crate) fn adopt_siblings(
    dir: &Path,
    root: &Path,
    captain: &str,
    captain_id: &str,
) -> Result<usize, String> {
    let mut n = 0;
    for mut s in paired_ships(dir, root) {
        if s.captain.captain != captain || !s.captain.captain_id.trim().is_empty() {
            continue;
        }
        s.captain.captain_id = captain_id.to_string();
        let bytes = serde_json::to_vec_pretty(&s.captain).map_err(|e| e.to_string())?;
        std::fs::write(s.dir.join("captain.json"), bytes)
            .map_err(|e| format!("{}: captain.json: {e}", s.world.label))?;
        n += 1;
    }
    Ok(n)
}

/// Give this captain an identity, once, and bring their computer with them.
///
/// `siblings` is every paired ship's record — ALL of them, not the ones visited so
/// far — and it is why this takes the whole fleet rather than one record: **two hulls
/// can share a captain.** Ian's own two PROD ships are both "Luke SkyWhisker".
/// Migrating them one at a time minted two ids, moved the store under the first, and
/// handed the second an empty directory — finding 2's shadowing bug wearing a new
/// hat. So an unmigrated record first adopts the id any sibling already carries for
/// the same display name; only a captain nobody has migrated yet gets a fresh id; a
/// captain who somehow has TWO ids gets a refusal, not a third.
///
/// Under the fleet's migration lock, returning every failure (round 2, finding 4):
/// the store is moved successfully before the identity is installed, a legacy slug
/// collision is refused rather than one directory moved for two captains, and a
/// second run after an interruption finds a state it understands.
pub(crate) fn ensure_captain_id(
    root: &Path,
    rec: &mut Captain,
    siblings: &[Captain],
) -> Result<String, String> {
    let _lock = migration_lock(root)?;
    if !rec.captain_id.trim().is_empty() {
        return Ok(rec.captain_id.trim().to_string());
    }
    let mut ids: Vec<String> = siblings
        .iter()
        .filter(|s| s.captain == rec.captain && !s.captain_id.trim().is_empty())
        .map(|s| s.captain_id.trim().to_string())
        .collect();
    ids.sort();
    ids.dedup();
    if ids.len() > 1 {
        return Err(format!(
            "{} already carries {} identities ({}) — refusing to add a third; reconcile them first",
            rec.captain,
            ids.len(),
            ids.join(", ")
        ));
    }
    let from = captain_store(root, &rec.captain);
    if let Some(other) = siblings.iter().find(|s| {
        s.captain != rec.captain
            && s.captain_id.trim().is_empty()
            && captain_store(root, &s.captain) == from
    }) {
        return Err(format!(
            "the legacy stores of {} and {} collide at {} — name one of them before migrating",
            rec.captain,
            other.captain,
            from.display()
        ));
    }
    let id = ids.pop().unwrap_or_else(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("cpt-{nanos:x}-{:x}", std::process::id())
    });
    let to = captain_store_by_id(root, &id);
    let file = familiar_kernel::persona::PERSONA_FILE;
    if from.exists() {
        if !to.exists() {
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            std::fs::rename(&from, &to)
                .map_err(|e| format!("moving {} to {}: {e}", from.display(), to.display()))?;
        } else if !to.join(file).exists() {
            // An interrupted earlier run left the id store without a persona: finish
            // the move — but only over an EMPTY directory. A trail is a history, and
            // we do not forget names.
            let empty = std::fs::read_dir(&to)
                .map(|mut d| d.next().is_none())
                .unwrap_or(false);
            if !empty {
                return Err(format!(
                    "{} holds records but no persona, and {} also exists — refusing to move over it",
                    to.display(),
                    from.display()
                ));
            }
            std::fs::remove_dir(&to).map_err(|e| format!("{}: {e}", to.display()))?;
            std::fs::rename(&from, &to)
                .map_err(|e| format!("moving {} to {}: {e}", from.display(), to.display()))?;
        }
    }
    rec.captain_id = id.clone();
    Ok(id)
}

/// The familiar's choice of how a computer is spoken of, from what it knows about
/// this ship right now: the captain, the purse off the wire when it answers, the
/// fleet as paired, and the name (Ian, 2026-09-09).
pub(crate) fn choose_for(
    dir: &Path,
    root: &Path,
    ship_dir: &Path,
    rec: &Captain,
    name: &str,
) -> (familiar_kernel::persona::Pronouns, String) {
    let key = read_env_value(&ship_dir.join("ucf.env"), "UCF_KEY").unwrap_or_default();
    let server = read_env_value(&ship_dir.join("ucf.env"), "UCF_SERVER")
        .unwrap_or_else(|| rec.server.clone());
    let me = wire_get(&server, &key, "/v1/me").unwrap_or(Value::Null);
    familiar_kernel::persona::choose_gender(&familiar_kernel::persona::NamingContext {
        captain: rec.captain.clone(),
        name: name.to_string(),
        credits: me.get("credits").and_then(Value::as_i64).unwrap_or(0),
        debt: me.get("debt").and_then(Value::as_i64).unwrap_or(0),
        fleet_size: paired_ships(dir, root)
            .iter()
            .filter(|s| s.captain.captain == rec.captain)
            .count()
            .max(1) as i64,
        at: super::now_secs(),
    })
}

/// The computer's state as a typed record — named, broken (with the kernel's
/// reason), or absent — so no surface has to infer "broken" from a missing name and
/// tell the captain to rename a file that already has a name in it (finding 7).
pub(crate) fn computer_state(root: &Path, ship_dir: &Path, rec: &Captain) -> Value {
    match persona_for(root, ship_dir, rec) {
        Some(p) => match (
            p.get("name").and_then(Value::as_str),
            p.get("error").and_then(Value::as_str),
        ) {
            (Some(n), _) => match p.get("pronouns") {
                Some(pr) if !pr.is_null() => json!({"state": "named", "name": n, "pronouns": pr}),
                _ => json!({"state": "named", "name": n}),
            },
            (None, Some(e)) => json!({"state": "broken", "error": e}),
            _ => json!({"state": "absent"}),
        },
        None => json!({"state": "absent"}),
    }
}

/// One line of the fleet's names ledger: who has worn what name, and when it changed.
///
/// Ian, 2026-09-08 (verbatim): "Two captains cannot have the same name, two ships cannot
/// have the same name. Two ships computers cannot have the same name. Names are unique.
/// We remember names. Names are important to the familiar. Lineage is important. We do
/// not forget names." The ledger is append-only and fleet-wide (`captains/names.jsonl`,
/// beside the stores); the per-computer trail is the computer's own history, this is the
/// household's. Nothing here is ever removed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct NameEntry {
    pub at: i64,
    /// `captain` | `hull` | `computer`
    pub kind: String,
    pub name: String,
    /// Who wears it: a `captain_id` for captains and computers, a world id for hulls.
    pub holder: String,
    /// `paired` | `named` | `renamed` | `reassigned` | `unpaired`
    pub act: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub from: String,
    pub by: String,
    /// What the computer went by from this naming — the familiar\'s choice (Ian,
    /// 2026-09-09). Empty on captain and hull rows and on namings before the choice.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pronouns: String,
}

fn names_ledger(root: &Path) -> PathBuf {
    root.parent()
        .unwrap_or(root)
        .join("captains")
        .join("names.jsonl")
}

/// Every name the fleet has ever recorded, oldest first.
pub(crate) fn names(root: &Path) -> Vec<NameEntry> {
    std::fs::read_to_string(names_ledger(root))
        .map(|t| {
            t.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Remember a name. Appended and synced under the fleet's migration lock, so two
/// namings cannot interleave a line. A failure to remember is a failure: the act
/// that could not be recorded is not reported as done.
pub(crate) fn record_name(root: &Path, entry: &NameEntry) -> Result<(), String> {
    use std::io::Write as _;
    let _lock = migration_lock(root)?;
    let path = names_ledger(root);
    let line = serde_json::to_string(entry).map_err(|e| e.to_string())?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    writeln!(f, "{line}").map_err(|e| format!("{}: {e}", path.display()))?;
    f.sync_all().map_err(|e| format!("{}: {e}", path.display()))
}

/// Remember what was named before the ledger existed. Walks every captain store
/// (identity and legacy) and every ship's own trail, and writes each naming the
/// ledger does not already carry — dated by the trail's own `at`, by the trail's own
/// actor, `from` the name before it. Captains and hulls come from the pairing
/// records. Idempotent: a second run writes nothing. Returns how many rows landed.
pub(crate) fn backfill_names(dir: &Path, root: &Path) -> Result<usize, String> {
    let have: std::collections::BTreeSet<(i64, String, String, String, String)> = names(root)
        .into_iter()
        .map(|e| (e.at, e.kind, e.name, e.holder, e.act))
        .collect();
    let ships = paired_ships(dir, root);
    let mut rows: Vec<NameEntry> = Vec::new();
    // Captains and hulls, from the pairing records.
    let mut captains: BTreeMap<String, (String, i64)> = BTreeMap::new();
    for s in &ships {
        if s.captain.captain_id.trim().is_empty() {
            continue;
        }
        let e = captains
            .entry(s.captain.captain_id.clone())
            .or_insert((s.captain.captain.clone(), s.captain.paired_at));
        e.1 = e.1.min(s.captain.paired_at);
        if !s.captain.hull_name.trim().is_empty() {
            rows.push(NameEntry {
                at: s.captain.paired_at,
                kind: "hull".into(),
                name: s.captain.hull_name.clone(),
                holder: s.world.id.clone(),
                act: "paired".into(),
                from: String::new(),
                by: "backfill".into(),
                pronouns: String::new(),
            });
        }
    }
    for (id, (name, at)) in &captains {
        rows.push(NameEntry {
            at: *at,
            kind: "captain".into(),
            name: name.clone(),
            holder: id.clone(),
            act: "paired".into(),
            from: String::new(),
            by: "backfill".into(),
            pronouns: String::new(),
        });
    }
    // Computers, from every trail: the captain's store, and each hull's own record.
    let mut trails: Vec<(String, PathBuf)> = Vec::new();
    for s in &ships {
        if s.captain.captain_id.trim().is_empty() {
            continue;
        }
        trails.push((
            s.captain.captain_id.clone(),
            captain_store_for(root, &s.captain),
        ));
        trails.push((
            s.captain.captain_id.clone(),
            captain_store(root, &s.captain.captain),
        ));
        trails.push((s.captain.captain_id.clone(), s.dir.clone()));
    }
    trails.sort();
    trails.dedup();
    for (holder, d) in trails {
        let mut prior = String::new();
        for ev in familiar_kernel::persona::namings(&d) {
            rows.push(NameEntry {
                at: ev.at,
                kind: "computer".into(),
                name: ev.name.clone(),
                holder: holder.clone(),
                act: if prior.is_empty() {
                    "named".into()
                } else {
                    "renamed".into()
                },
                from: prior.clone(),
                by: ev.actor.clone(),
                pronouns: ev
                    .pronouns
                    .as_ref()
                    .map(|p| p.label.clone())
                    .unwrap_or_default(),
            });
            prior = ev.name;
        }
    }
    rows.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.kind.cmp(&b.kind)));
    rows.dedup_by(|a, b| {
        (a.at, &a.kind, &a.name, &a.holder, &a.act) == (b.at, &b.kind, &b.name, &b.holder, &b.act)
    });
    let mut landed = 0;
    for r in rows {
        if have.contains(&(
            r.at,
            r.kind.clone(),
            r.name.clone(),
            r.holder.clone(),
            r.act.clone(),
        )) {
            continue;
        }
        record_name(root, &r)?;
        landed += 1;
    }
    Ok(landed)
}

fn fold_name(s: &str) -> String {
    s.trim().to_lowercase()
}

/// May this captain's computer wear `name`? Unique across the fleet, case-folded,
/// against every OTHER captain's computer — the one it wears now, and every one the
/// ledger says it ever wore (a name is a lineage, not a label). A captain may return
/// to a name they themselves wore before.
pub(crate) fn computer_name_free(
    dir: &Path,
    root: &Path,
    name: &str,
    captain: &str,
    captain_id: &str,
) -> Result<(), String> {
    let want = fold_name(name);
    // The household's root name is the UNNAMED state — status already says so — not a
    // name anyone chose. Uniqueness is for names given.
    if want == fold_name(familiar_kernel::persona::ROOT_NAME) {
        return Ok(());
    }
    for e in names(root) {
        if e.kind == "computer" && fold_name(&e.name) == want && e.holder != captain_id {
            return Err(format!(
                "\"{name}\" is {}'s computer's name (since {}); two ships' computers cannot have \
                 the same name",
                e.holder, e.at
            ));
        }
    }
    for s in paired_ships(dir, root) {
        // Same identity, or the same captain by name (a hull from before identity
        // landed is still theirs): their own computer is not a rival.
        if s.captain.captain_id == captain_id || s.captain.captain == captain {
            continue;
        }
        if let Some(p) = persona_for(root, &s.dir, &s.captain) {
            if p.get("name").and_then(Value::as_str).map(fold_name) == Some(want.clone()) {
                return Err(format!(
                    "\"{name}\" is {}'s computer's name; two ships' computers cannot have the same name",
                    s.captain.captain
                ));
            }
        }
    }
    Ok(())
}

/// May a hull named `hull_name` be paired under `key_id`? Two ships cannot have the
/// same name: a second paired hull wearing a name the fleet already flies, on a
/// different key, is refused — the world may have made the mistake, the familiar
/// will not hold it.
pub(crate) fn hull_name_free(
    dir: &Path,
    root: &Path,
    hull_name: &str,
    key_id: &str,
) -> Result<(), String> {
    if hull_name.trim().is_empty() {
        return Ok(());
    }
    let want = fold_name(hull_name);
    for s in paired_ships(dir, root) {
        if fold_name(&s.captain.hull_name) == want && s.captain.key_id != key_id {
            return Err(format!(
                "a ship named \"{hull_name}\" is already paired ({}, key {}); two ships cannot \
                 have the same name",
                s.world.label, s.captain.key_id
            ));
        }
    }
    Ok(())
}

/// The pre-identity store path. Kept ONLY to find what an unmigrated deployment
/// already wrote; it is lossy and nothing new is keyed by it. See [`Captain::captain_id`].
pub(crate) fn captain_store(root: &Path, captain: &str) -> PathBuf {
    let slug: String = captain
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    root.parent()
        .unwrap_or(root)
        .join("captains")
        .join(if slug.is_empty() {
            "captain".into()
        } else {
            slug
        })
}

/// The persona a ship answers as: the captain's, else whatever the ship store carries.
/// Read through the kernel's loader, so a file the CLI would refuse as invalid is
/// refused here too instead of flowing raw onto the feed (review 2026-09-05, F2); a
/// present-but-broken record surfaces as `{"error": …}` rather than as a silent Null.
pub(crate) fn persona_for(root: &Path, ship_dir: &Path, rec: &Captain) -> Option<Value> {
    let cap = captain_store_for(root, rec);
    for dir in [cap.as_path(), ship_dir] {
        if !dir.join(familiar_kernel::persona::PERSONA_FILE).exists() {
            continue;
        }
        return Some(match familiar_kernel::persona::load(dir) {
            Ok(p) => serde_json::to_value(&p).unwrap_or(Value::Null),
            Err(e) => serde_json::json!({ "error": e.to_string() }),
        });
    }
    None
}

/// What is actually in the hold, from the pilot's own reconciled book
/// (`holdings.json`, checked against `/v1/me.cargo` every fold): good → units, and
/// the total at cost. The trade book's leftover lots are a derived guess and drift
/// whenever a fill was never read back; this is the truth the captain is shown.
pub(crate) fn aboard(ship_dir: &Path) -> (BTreeMap<String, i64>, i64) {
    let mut units = BTreeMap::new();
    let mut cost = 0;
    if let Ok(text) = std::fs::read_to_string(ship_dir.join("holdings.json")) {
        if let Ok(Value::Array(rows)) = serde_json::from_str::<Value>(&text) {
            for h in rows {
                let good = h
                    .get("good")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let u = h.get("units").and_then(Value::as_i64).unwrap_or(0);
                let basis = h.get("avg_cost").and_then(Value::as_i64).unwrap_or(0);
                if !good.is_empty() && u > 0 {
                    *units.entry(good).or_insert(0) += u;
                    cost += u * basis;
                }
            }
        }
    }
    (units, cost)
}

/// How the merchant's own estimates have held up: for every position the pilot
/// opened and later closed, what it expected to make against what the fold actually
/// paid. The estimate is a mid-price guess with a fixed haircut; this is the only
/// way to know whether that haircut is the right size on a given world (LOCAL
/// catnip, 2026-09-04: promised ℳ372, returned −45 when the target's bid fell and
/// the stuck-position rule cut it). Returns (closed positions, expected, realized).
pub(crate) fn estimate_calibration(ship_dir: &Path) -> (i64, i64, i64) {
    let Ok(text) = std::fs::read_to_string(ship_dir.join("journal.jsonl")) else {
        return (0, 0, 0);
    };
    let lines: Vec<Value> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    fn ev(v: &Value) -> &str {
        v.get("event").and_then(Value::as_str).unwrap_or("")
    }
    let num = |v: &Value, k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0);
    let good_of = |v: &Value| {
        v.get("good")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let (mut closed, mut expected, mut realized) = (0, 0, 0);
    for (i, op) in lines
        .iter()
        .enumerate()
        .filter(|(_, v)| ev(v) == "position-opened")
    {
        let good = good_of(op);
        let units = num(op, "units");
        if units <= 0 {
            continue;
        }
        // What it cost: the fill if one was read back, else the quoted ask.
        let basis_total = lines[i..]
            .iter()
            .take(8)
            .find(|v| {
                ev(v) == "trade-outcome"
                    && v.get("side").and_then(Value::as_str) == Some("buy")
                    && good_of(v) == good
            })
            .map(|v| num(v, "total"))
            .unwrap_or_else(|| num(op, "ask") * units);
        // What it fetched: the sells of that good after it, up to its own units.
        let (mut left, mut proceeds) = (units, 0);
        for sell in lines[i..].iter().filter(|v| {
            ev(v) == "trade-outcome"
                && v.get("side").and_then(Value::as_str) == Some("sell")
                && v.get("outcome").and_then(Value::as_str) == Some("filled")
                && good_of(v) == good
        }) {
            if left <= 0 {
                break;
            }
            let su = num(sell, "units").max(1);
            let u = su.min(left);
            proceeds += num(sell, "total") * u / su;
            left -= u;
        }
        if left == 0 {
            closed += 1;
            expected += num(op, "est_margin");
            realized += proceeds - basis_total;
        }
    }
    (closed, expected, realized)
}

/// The ship's own delivery record, summed: hauls and freight paid.
pub(crate) fn delivery_totals(ship_dir: &Path) -> (i64, i64) {
    let Ok(text) = std::fs::read_to_string(ship_dir.join("deliveries.jsonl")) else {
        return (0, 0);
    };
    let mut n = 0;
    let mut paid = 0;
    for l in text.lines() {
        if let Ok(v) = serde_json::from_str::<Value>(l) {
            n += 1;
            paid += v.get("paid").and_then(Value::as_i64).unwrap_or(0);
        }
    }
    (n, paid)
}

pub fn cmd_fleet(args: &[String]) -> ExitCode {
    let sub = args.first().map(String::as_str).unwrap_or("status");
    let f = super::flags(args);
    let dir = familiar_kernel::store::data_dir(f.get("data-dir").map(String::as_str));
    let root = super::world_store_root(&dir, f.get("store-root").map(String::as_str));
    let positional: Vec<&String> = {
        let mut out = Vec::new();
        let mut skip_next = false;
        for a in args.iter().skip(1) {
            if skip_next {
                skip_next = false;
                continue;
            }
            if let Some(key) = a.strip_prefix("--") {
                skip_next = !key.contains('=');
                continue;
            }
            out.push(a);
        }
        out
    };

    match sub {
        // ── pair: a captain's key becomes a ship world ─────────────────────────
        "pair" => {
            let (Some(label), Some(captain), Some(server)) =
                (f.get("label"), f.get("captain"), f.get("server"))
            else {
                eprintln!(
                    "fleet pair: --label <ship name> --captain <who> --server <exchange url> \
                     --key <ucfk_…> | --key-file <path> [--automations freight,trade,outfit] \
                     [--ttl-hours 24]"
                );
                return ExitCode::FAILURE;
            };
            let key = match (f.get("key"), f.get("key-file")) {
                (Some(k), _) => k.trim().to_string(),
                (None, Some(p)) => match std::fs::read_to_string(p) {
                    Ok(s) => s.trim().to_string(),
                    Err(e) => {
                        eprintln!("fleet pair: --key-file: {e}");
                        return ExitCode::FAILURE;
                    }
                },
                _ => {
                    eprintln!("fleet pair: the captain's key is needed — --key or --key-file");
                    return ExitCode::FAILURE;
                }
            };
            if !key.starts_with("ucfk_") || key.len() < 16 {
                eprintln!("fleet pair: that is not an exchange key (ucfk_…)");
                return ExitCode::FAILURE;
            }
            let automations: Vec<String> = f
                .get("automations")
                .map(|s| {
                    s.split(',')
                        .map(|a| a.trim().to_string())
                        .filter(|a| !a.is_empty())
                        .collect()
                })
                .unwrap_or_else(|| vec!["freight".to_string()]);
            let commissioner = f
                .get("commissioner")
                .cloned()
                .or_else(|| familiar_kernel::identity::current(&dir))
                .unwrap_or_default();
            if commissioner.is_empty() || commissioner == "observer" {
                eprintln!("fleet pair: no established commissioner — pass --commissioner <human>");
                return ExitCode::FAILURE;
            }
            // The key answers for itself before anything is written: who is this, on
            // which exchange, with what ship.
            let me = match wire_get(server, &key, "/v1/me") {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("fleet pair: the key does not answer on {server}: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let ship_name = me
                .get("shipName")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let key_id = key
                .trim_start_matches("ucfk_")
                .chars()
                .take(8)
                .collect::<String>();
            // Two ships cannot have the same name (Ian, 2026-09-08).
            if let Err(e) = hull_name_free(&dir, &root, &ship_name, &key_id) {
                eprintln!("fleet pair: {e}");
                return ExitCode::FAILURE;
            }
            // And one key is one hull: a key already paired is not paired again — two
            // worlds on one key are two pilots contradicting each other on the
            // exchange (Kibble Klipper NG, 2026-09-09: paired from the iPad and from
            // wildhorse 48 seconds apart). Rename or unpair the one that exists.
            if let Some(already) = paired_ships(&dir, &root)
                .into_iter()
                .find(|s| s.captain.key_id == key_id)
            {
                eprintln!(
                    "fleet pair: key {key_id} already flies {} as \"{}\" for {} — one key is one hull; \
                     `fleet rename`/`fleet unpair {}` rather than pairing it twice",
                    already.world.id, already.world.label, already.captain.captain, already.world.id
                );
                return ExitCode::FAILURE;
            }
            // THE COMPUTER IS SETTLED BEFORE THE SHIP IS COMMISSIONED (codex T-236
            // re-verification, finding 6). Every failure below used to happen after
            // the world, the key and captain.json were already on disk, so a pair
            // that could not name the computer left an ACTIVE world behind for the
            // supervisor to find and fly.
            //
            // Identity first: siblings decide it, because two hulls can share a
            // captain and a second pairing must join the computer that already flies
            // for them rather than mint a rival.
            let siblings: Vec<Captain> = paired_ships(&dir, &root)
                .into_iter()
                .map(|s| s.captain)
                .collect();
            let computer_name = f.get("computer-name").cloned();
            let mut pending = Captain {
                captain_id: String::new(),
                captain: captain.clone(),
                key_id: String::new(),
                server: server.clone(),
                automations: automations.clone(),
                paired_at: super::now_secs(),
                hull_name: ship_name.clone(),
                pilot_args: f
                    .get("pilot-args")
                    .map(|s| s.split_whitespace().map(String::from).collect())
                    .unwrap_or_default(),
            };
            let captain_id = match ensure_captain_id(&root, &mut pending, &siblings) {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("fleet pair: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let persona_dir = captain_store_by_id(&root, &captain_id);

            // What this captain's computer already IS, wherever it was written — as a
            // typed origin, migrated WHOLE (persona and trail) into the id store, with
            // every same-captain hull pointed at it, before anything else happens. A
            // captain whose only record is the pre-ruling ship-local persona keeps
            // that computer, tuned Purr included; two ship-local records that
            // disagree are refused, not raced (finding 2).
            let same_captain_dirs: Vec<PathBuf> = paired_ships(&dir, &root)
                .into_iter()
                .filter(|s| s.captain.captain == *captain)
                .map(|s| s.dir)
                .collect();
            let origin = match computer_origin(&root, &pending, &same_captain_dirs) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("fleet pair: {e}");
                    return ExitCode::FAILURE;
                }
            };
            if let Err(e) = migrate_computer(&root, &captain_id, &origin) {
                eprintln!("fleet pair: {e}");
                return ExitCode::FAILURE;
            }
            if let Err(e) = adopt_siblings(&dir, &root, captain, &captain_id) {
                eprintln!("fleet pair: {e}");
                return ExitCode::FAILURE;
            }
            let existing = match origin {
                Origin::None => None,
                _ => familiar_kernel::persona::load(&persona_dir).ok(),
            };

            let mut persona = match (existing, computer_name.as_deref()) {
                // Named at pairing: RENAME the captain's computer, never replace it —
                // a fresh default record here wiped a tuned style (review 2026-09-05).
                (Some(mut have), Some(name)) => {
                    have.name = name.to_string();
                    have
                }
                (Some(have), None) => {
                    println!("  joining {captain}'s computer, {}", have.name);
                    have
                }
                // Only a captain with no computer anywhere gets one written here,
                // defaulting to the root name — written exactly, never generated around.
                (None, given) => familiar_kernel::persona::Persona {
                    persona_version: 2,
                    name: given
                        .map(String::from)
                        .unwrap_or_else(|| familiar_kernel::persona::ROOT_NAME.to_string()),
                    style: Some(familiar_kernel::persona::Style::default()),
                    ..familiar_kernel::persona::Persona::default()
                },
            };
            // A captain naming their computer is the moment the familiar chooses how
            // it is spoken of (Ian, 2026-09-09) — from everything it knows right now.
            let chosen = computer_name.as_deref().map(|_| {
                familiar_kernel::persona::choose_gender(&familiar_kernel::persona::NamingContext {
                    captain: captain.to_string(),
                    name: persona.name.clone(),
                    credits: me.get("credits").and_then(Value::as_i64).unwrap_or(0),
                    debt: me.get("debt").and_then(Value::as_i64).unwrap_or(0),
                    fleet_size: 1 + siblings.iter().filter(|c| c.captain == *captain).count()
                        as i64,
                    at: super::now_secs(),
                })
            });
            if let Some((pron, _)) = &chosen {
                persona.pronouns = Some(pron.clone());
            }
            if let Err(e) = persona.validate() {
                eprintln!("fleet pair: that computer will not do: {e}");
                return ExitCode::FAILURE;
            }
            // Two ships' computers cannot have the same name (Ian, 2026-09-08): a name
            // GIVEN here must be free across every other captain, now and ever. Joining
            // the captain's own computer is not a naming, and is never checked — a
            // second hull for Luke was refused on 2026-09-09 because the LOCAL twin's
            // captain also wears Felix, which is the grandfathered pair, not a naming.
            if let Some(given) = computer_name.as_deref() {
                if let Err(e) = computer_name_free(&dir, &root, given, captain, &captain_id) {
                    eprintln!("fleet pair: {e}");
                    return ExitCode::FAILURE;
                }
            }

            // The computer, and its history, as ONE recoverable mutation (finding 3) —
            // and BEFORE the ship is commissioned (round 2, finding 6): a naming that
            // fails here leaves no world, no key, no captain record, and no event.
            if let Err(e) = familiar_kernel::persona::name(
                &persona_dir,
                &persona,
                Some(&familiar_kernel::persona::NameEvent {
                    at: super::now_secs(),
                    actor: if computer_name.is_some() {
                        captain.to_string()
                    } else {
                        "pairing".to_string()
                    },
                    name: persona.name.clone(),
                    pronouns: chosen.as_ref().map(|(p, _)| p.clone()),
                    why: chosen.as_ref().map(|(_, w)| w.clone()).unwrap_or_default(),
                }),
            ) {
                eprintln!("fleet pair: writing the captain's computer: {e}");
                return ExitCode::FAILURE;
            }
            // Remembered in the fleet's ledger: the captain, and the computer's name
            // under them. Lineage is important; we do not forget names.
            let by = if computer_name.is_some() {
                captain.to_string()
            } else {
                "pairing".to_string()
            };
            for entry in [
                NameEntry {
                    at: super::now_secs(),
                    kind: "captain".into(),
                    name: captain.to_string(),
                    holder: captain_id.clone(),
                    act: "paired".into(),
                    from: String::new(),
                    by: by.clone(),
                    pronouns: String::new(),
                },
                NameEntry {
                    at: super::now_secs(),
                    kind: "computer".into(),
                    name: persona.name.clone(),
                    holder: captain_id.clone(),
                    act: if computer_name.is_some() {
                        "named".into()
                    } else {
                        "joined".into()
                    },
                    from: String::new(),
                    by,
                    pronouns: chosen
                        .as_ref()
                        .map(|(p, _)| p.label.clone())
                        .unwrap_or_default(),
                },
            ] {
                if let Err(e) = record_name(&root, &entry) {
                    eprintln!("fleet pair: the names ledger could not be written: {e}");
                    return ExitCode::FAILURE;
                }
            }
            let (w, ship_dir) = match instance::commission(
                &dir,
                &root,
                label,
                &commissioner,
                server,
                super::now_secs(),
            ) {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("fleet pair: {e}");
                    return ExitCode::FAILURE;
                }
            };
            // The issuer's public identity, so the ship can verify leases (as the
            // world ceremony does).
            if let Ok(k) = familiar_mesh::node::NodeKey::load_or_mint(&dir, "") {
                if let Ok(bytes) = serde_json::to_vec_pretty(&k.identity()) {
                    let _ = std::fs::write(ship_dir.join("issuer.json"), bytes);
                }
            }
            let env = format!("UCF_KEY={key}\nUCF_SERVER={server}\n");
            if let Err(e) = write_private(&ship_dir.join("ucf.env"), env.as_bytes()) {
                eprintln!("fleet pair: writing the key into the ship store: {e}");
                return ExitCode::FAILURE;
            }
            let _ = std::fs::write(
                ship_dir.join("automations.json"),
                serde_json::to_vec_pretty(&automations).unwrap_or_default(),
            );
            let record = Captain {
                key_id: key_id.clone(),
                ..pending.clone()
            };
            if let Err(e) = record_name(
                &root,
                &NameEntry {
                    at: super::now_secs(),
                    kind: "hull".into(),
                    name: ship_name.clone(),
                    holder: w.id.clone(),
                    act: "paired".into(),
                    from: String::new(),
                    by: captain.to_string(),
                    pronouns: String::new(),
                },
            ) {
                eprintln!("fleet pair: the names ledger could not be written: {e}");
            }
            let _ = std::fs::write(
                ship_dir.join("captain.json"),
                serde_json::to_vec_pretty(&record).unwrap_or_default(),
            );
            let ttl: i64 = f
                .get("ttl-hours")
                .and_then(|s| s.parse().ok())
                .unwrap_or(24);
            match issue_lease(&dir, &ship_dir, &w.id, ttl) {
                Ok(exp) => println!(
                    "paired {} — \"{}\" for captain {captain}, leased to {exp}",
                    w.id, w.label
                ),
                Err(e) => eprintln!("paired {} but the lease failed: {e}", w.id),
            }
            println!("  ship on the exchange: {ship_name} (key {key_id}, {server})");
            if let Some((pron, why)) = &chosen {
                println!("  goes by {} — {why}", pron.label);
            }
            println!(
                "  {captain}'s computer answers to: {}",
                familiar_kernel::persona::load(&persona_dir)
                    .map(|p| p.name)
                    .unwrap_or_else(|_| persona.name.clone())
            );
            println!("  automations granted: {}", automations.join(", "));
            println!("  store: {}", ship_dir.display());
            println!("  next: `familiar fleet run` keeps a pilot aboard; `familiar fleet unpair {}` revokes.", w.id);
            ExitCode::SUCCESS
        }

        // ── unpair: revocation ─────────────────────────────────────────────────
        // ── rename: the captain names the ship's COMPUTER (not the hull, not the
        // world label — three names, never collapsed; T-236 brick 1). A local
        // ceremony by the established human, recorded in the naming trail.
        "rename" => {
            let (Some(id), Some(new_name)) = (positional.first(), positional.get(1)) else {
                eprintln!("fleet rename: `familiar fleet rename <world-id> <computer name>`");
                return ExitCode::FAILURE;
            };
            let ship_dir = root.join(id.as_str());
            if !ship_dir.join("captain.json").exists() {
                eprintln!("fleet rename: {id} is not a paired ship in this store root");
                return ExitCode::FAILURE;
            }
            // The name belongs to the CAPTAIN, not the hull: naming one ship names the
            // computer that flies all of them (Ian, 2026-09-04). The ship is only how
            // the captain was identified.
            let mut rec: Captain = match std::fs::read_to_string(ship_dir.join("captain.json"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())
            {
                Some(c) => c,
                None => {
                    eprintln!("fleet rename: {id}'s captain.json will not read");
                    return ExitCode::FAILURE;
                }
            };
            let captain = rec.captain.clone();
            // `--captain` LABELS THE ACT; it does not choose whose computer this is.
            // Passing someone else's name used to rename this ship's captain while
            // recording the other name in the trail — a forged provenance nobody
            // would catch by reading it (codex T-236 re-verification, finding 5).
            if let Some(given) = f.get("captain") {
                if given != &captain {
                    eprintln!(
                        "fleet rename: {id} is {captain}'s ship, not {given}'s. \
                         --captain records WHO IS NAMING, and it has to be them."
                    );
                    return ExitCode::FAILURE;
                }
            }
            let siblings: Vec<Captain> = paired_ships(&dir, &root)
                .into_iter()
                .map(|s| s.captain)
                .collect();
            let captain_id = match ensure_captain_id(&root, &mut rec, &siblings) {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("fleet rename: {e}");
                    return ExitCode::FAILURE;
                }
            };
            if let Err(e) = std::fs::write(
                ship_dir.join("captain.json"),
                serde_json::to_vec_pretty(&rec).unwrap_or_default(),
            ) {
                eprintln!("fleet rename: captain.json: {e}");
                return ExitCode::FAILURE;
            }
            let persona_dir = captain_store_by_id(&root, &captain_id);
            if let Err(e) = std::fs::create_dir_all(&persona_dir) {
                eprintln!("fleet rename: {e}");
                return ExitCode::FAILURE;
            }
            // The computer to rename is wherever it lives TODAY — a legacy hull's
            // ship-local record is migrated whole first, so the rename never lands
            // on an empty id store and prints `was "the familiar"` over a tuned
            // persona (round 2, finding 2).
            let same_captain_dirs: Vec<PathBuf> = paired_ships(&dir, &root)
                .into_iter()
                .filter(|s| s.captain.captain == captain)
                .map(|s| s.dir)
                .collect();
            let origin = match computer_origin(&root, &rec, &same_captain_dirs) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("fleet rename: {e}");
                    return ExitCode::FAILURE;
                }
            };
            if let Err(e) = migrate_computer(&root, &captain_id, &origin) {
                eprintln!("fleet rename: {e}");
                return ExitCode::FAILURE;
            }
            if let Err(e) = adopt_siblings(&dir, &root, &captain, &captain_id) {
                eprintln!("fleet rename: {e}");
                return ExitCode::FAILURE;
            }
            let mut persona = match familiar_kernel::persona::load(&persona_dir) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("fleet rename: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let actor = f
                .get("captain")
                .cloned()
                .or_else(|| familiar_kernel::identity::current(&dir))
                .unwrap_or_else(|| "captain".to_string());
            let actor_for_ledger = actor.clone();
            let was = persona.name.clone();
            // Two ships' computers cannot have the same name (Ian, 2026-09-08).
            if let Err(e) = computer_name_free(&dir, &root, new_name, &captain, &captain_id) {
                eprintln!("fleet rename: {e}");
                return ExitCode::FAILURE;
            }
            persona.name = new_name.to_string();
            persona.persona_version = 2;
            // Every naming is a fresh choice of how the computer is spoken of (Ian,
            // 2026-09-09), from what the familiar knows now.
            let (pron, pron_why) = choose_for(&dir, &root, &ship_dir, &rec, new_name);
            persona.pronouns = Some(pron.clone());
            // The computer and its history, as ONE recoverable mutation — the same
            // helper pairing uses. Rename used to write the persona, release the
            // lock, append the trail unlocked, print the trail's error, and exit 0
            // (codex T-236 round 2, finding 3): a computer wearing a name its own
            // history did not contain.
            if let Err(e) = familiar_kernel::persona::name(
                &persona_dir,
                &persona,
                Some(&familiar_kernel::persona::NameEvent {
                    at: super::now_secs(),
                    actor,
                    name: new_name.to_string(),
                    pronouns: Some(pron.clone()),
                    why: pron_why.clone(),
                }),
            ) {
                eprintln!("fleet rename: the computer could not be renamed: {e}");
                return ExitCode::FAILURE;
            }
            if let Err(e) = record_name(
                &root,
                &NameEntry {
                    at: super::now_secs(),
                    kind: "computer".into(),
                    name: new_name.to_string(),
                    holder: captain_id.clone(),
                    act: "renamed".into(),
                    from: was.clone(),
                    by: actor_for_ledger.clone(),
                    pronouns: pron.label.clone(),
                },
            ) {
                eprintln!("fleet rename: the names ledger could not be written: {e}");
                return ExitCode::FAILURE;
            }
            let fleet: Vec<String> = paired_ships(&dir, &root)
                .into_iter()
                .filter(|s| s.captain.captain == captain)
                .map(|s| s.world.label)
                .collect();
            println!("  goes by {} — {pron_why}", pron.label);
            println!(
                "{}'s computer now answers to \"{new_name}\" (was \"{was}\") — aboard {}",
                if captain.is_empty() {
                    "the captain"
                } else {
                    &captain
                },
                if fleet.is_empty() {
                    id.to_string()
                } else {
                    fleet.join(", ")
                }
            );
            ExitCode::SUCCESS
        }

        "unpair" => {
            let Some(id) = positional.first() else {
                eprintln!("fleet unpair: `familiar fleet unpair <world-id>`");
                return ExitCode::FAILURE;
            };
            let ship_dir = root.join(id.as_str());
            let stopped = stop_pilot(&ship_dir);
            let key_gone = std::fs::remove_file(ship_dir.join("ucf.env")).is_ok();
            match instance::decommission(&dir, id) {
                Err(e) => {
                    eprintln!("fleet unpair: {e}");
                    ExitCode::FAILURE
                }
                Ok(w) => {
                    println!(
                        "unpaired {} — pilot {}, key {}, authority ended (epoch {}). The journal, the \
                         delivery record, and the computer's persona stay for the captain.",
                        w.id,
                        if stopped { "stopped" } else { "was not running" },
                        if key_gone { "destroyed" } else { "was not held" },
                        w.grant_epoch
                    );
                    ExitCode::SUCCESS
                }
            }
        }

        // ── status: every ship, booked per captain ─────────────────────────────
        // Give every paired captain their identity, once, deliberately.
        //
        // `pair` and `rename` mint an id on the way past, but an existing fleet is
        // touched by neither — so without this the migration would only ever reach a
        // captain who happened to pair another ship. Kept as its OWN command rather
        // than folded into `status`: a read that silently rewrites the store is a
        // read nobody can trust, and this rewrites captain.json and moves a
        // directory.
        // ── choose: the computer decides how it is spoken of, as its own act ────
        // Ian, 2026-09-09: "give the opportunity back to Felix to change their
        // gender since it wasn't given that choice when initiated." The name stays;
        // the choice is made from today's facts and recorded like a naming.
        "choose" => {
            let Some(id) = positional.first() else {
                eprintln!("fleet choose <world>");
                return ExitCode::FAILURE;
            };
            let ship_dir = root.join(id.as_str());
            let Ok(text) = std::fs::read_to_string(ship_dir.join("captain.json")) else {
                eprintln!("fleet choose: no paired ship {id}");
                return ExitCode::FAILURE;
            };
            let Ok(rec) = serde_json::from_str::<Captain>(&text) else {
                eprintln!("fleet choose: captain.json is not a captain record");
                return ExitCode::FAILURE;
            };
            let persona_dir = captain_store_for(&root, &rec);
            let mut persona = match familiar_kernel::persona::load(&persona_dir) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("fleet choose: {e}");
                    return ExitCode::FAILURE;
                }
            };
            if persona.name == familiar_kernel::persona::ROOT_NAME
                || persona.name == familiar_kernel::persona::DEFAULT_NAME
            {
                eprintln!(
                    "fleet choose: this computer has not been named yet — `fleet rename` it first"
                );
                return ExitCode::FAILURE;
            }
            let was = persona
                .pronouns
                .as_ref()
                .map(|p| p.label.clone())
                .unwrap_or_else(|| "none".into());
            let (pron, why) = choose_for(&dir, &root, &ship_dir, &rec, &persona.name);
            persona.pronouns = Some(pron.clone());
            persona.persona_version = 2;
            let why = format!("{why} — the choice not given at naming; went by {was}");
            if let Err(e) = familiar_kernel::persona::name(
                &persona_dir,
                &persona,
                Some(&familiar_kernel::persona::NameEvent {
                    at: super::now_secs(),
                    actor: "familiar".into(),
                    name: persona.name.clone(),
                    pronouns: Some(pron.clone()),
                    why: why.clone(),
                }),
            ) {
                eprintln!("fleet choose: {e}");
                return ExitCode::FAILURE;
            }
            if let Err(e) = record_name(
                &root,
                &NameEntry {
                    at: super::now_secs(),
                    kind: "computer".into(),
                    name: persona.name.clone(),
                    holder: rec.captain_id.clone(),
                    act: "chose".into(),
                    from: was.clone(),
                    by: "familiar".into(),
                    pronouns: pron.label.clone(),
                },
            ) {
                eprintln!("fleet choose: the names ledger could not be written: {e}");
                return ExitCode::FAILURE;
            }
            println!(
                "{} now goes by {} (was {was}) — {why}",
                persona.name, pron.label
            );
            ExitCode::SUCCESS
        }
        // ── economy: the captain's money over time, from the journals ──────────
        "economy" => {
            let ships = paired_ships(&dir, &root);
            let since = super::now_secs()
                - super::economy::window_seconds(f.get("window").map(String::as_str));
            let mut by_captain: BTreeMap<String, (String, Vec<super::economy::History>)> =
                BTreeMap::new();
            for s in &ships {
                let key = if s.captain.captain_id.trim().is_empty() {
                    s.captain.captain.clone()
                } else {
                    s.captain.captain_id.clone()
                };
                let e = by_captain.entry(key).or_default();
                e.0 = s.captain.captain.clone();
                e.1.push(super::economy::for_ship(&s.dir, since));
            }
            if f.contains_key("json") {
                let out: Vec<Value> = by_captain
                    .iter()
                    .map(|(id, (name, hulls))| {
                        json!({
                    "captain_id": id, "captain": name,
                    "pooled": super::economy::to_json(&super::economy::pool(hulls, since), false)})
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
                return ExitCode::SUCCESS;
            }
            for (name, hulls) in by_captain.values() {
                println!("{name}:");
                for line in super::economy::analysis(&super::economy::pool(hulls, since)) {
                    println!("  {line}");
                }
            }
            ExitCode::SUCCESS
        }
        // ── names: everything the fleet has ever called anyone ─────────────────
        "names" => {
            if f.contains_key("backfill") {
                match backfill_names(&dir, &root) {
                    Ok(n) => {
                        println!("fleet names: {n} naming(s) remembered from before the ledger")
                    }
                    Err(e) => {
                        eprintln!("fleet names: {e}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            let all = names(&root);
            if all.is_empty() {
                println!("fleet: no names on record yet");
                return ExitCode::SUCCESS;
            }
            if f.contains_key("json") {
                println!("{}", serde_json::to_string_pretty(&all).unwrap_or_default());
                return ExitCode::SUCCESS;
            }
            for e in &all {
                let from = if e.from.is_empty() {
                    String::new()
                } else {
                    format!(" (was \"{}\")", e.from)
                };
                println!(
                    "  {} {:<8} {:<9} \"{}\"{from} — {} — by {}",
                    e.at, e.kind, e.act, e.name, e.holder, e.by
                );
            }
            ExitCode::SUCCESS
        }
        "adopt-ids" => {
            let mut ships = paired_ships(&dir, &root);
            if ships.is_empty() {
                println!("fleet: no paired ships");
                return ExitCode::SUCCESS;
            }
            // Every record is inspected before any is assigned (round 2, finding 4):
            // an unmigrated hull listed before an already-migrated sibling used to
            // mint a rival id because only the records visited so far were siblings.
            let mut moved = 0;
            let mut i = 0;
            while i < ships.len() {
                let all: Vec<Captain> = ships.iter().map(|s| s.captain.clone()).collect();
                let had = ships[i].captain.captain_id.clone();
                let id = match ensure_captain_id(&root, &mut ships[i].captain, &all) {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("fleet adopt-ids: {} — {e}", ships[i].world.label);
                        return ExitCode::FAILURE;
                    }
                };
                if had.trim().is_empty() {
                    let name = ships[i].captain.captain.clone();
                    // This hull and every same-captain sibling without an id, in one go.
                    for s in ships.iter_mut().filter(|s| {
                        s.captain.captain == name && s.captain.captain_id.trim().is_empty()
                    }) {
                        s.captain.captain_id = id.clone();
                    }
                    for s in ships.iter().filter(|s| s.captain.captain == name) {
                        let bytes = serde_json::to_vec_pretty(&s.captain).unwrap_or_default();
                        if let Err(e) = std::fs::write(s.dir.join("captain.json"), bytes) {
                            eprintln!("fleet adopt-ids: {} — {e}", s.world.label);
                            return ExitCode::FAILURE;
                        }
                        moved += 1;
                        println!("  {} — {} is now {id}", s.world.label, s.captain.captain);
                    }
                } else {
                    println!(
                        "  {} — {} already {id}",
                        ships[i].world.label, ships[i].captain.captain
                    );
                }
                i += 1;
            }
            println!("fleet: {moved} record(s) given an identity");
            ExitCode::SUCCESS
        }
        "status" => {
            let ships = paired_ships(&dir, &root);
            if ships.is_empty() {
                println!("fleet: no paired ships. `familiar fleet pair --label … --captain … --server … --key …`");
                return ExitCode::SUCCESS;
            }
            let json_out = f.contains_key("json");
            let now = super::now_secs();
            let mut rows: Vec<Value> = Vec::new();
            // credits, debt, hauls, freight paid, realized trade P&L, inventory at cost
            // Keyed by IDENTITY, never by the display name (round 2, finding 4): two
            // captains whose names slug alike are two captains with two books.
            let mut per_captain: BTreeMap<String, CaptainBook> = BTreeMap::new();
            for s in &ships {
                let key = read_env_value(&s.dir.join("ucf.env"), "UCF_KEY").unwrap_or_default();
                let server = read_env_value(&s.dir.join("ucf.env"), "UCF_SERVER")
                    .unwrap_or_else(|| s.captain.server.clone());
                let me = wire_get(&server, &key, "/v1/me").ok();
                // Journal ∪ wire, deduplicated: the journal is the long memory, the
                // wire's day-window catches a fill the pilot never read back (a
                // restart inside the fold — the bluefin lot, 2026-09-03).
                let mut fills: Vec<Value> = journal_fills(&s.dir)
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                if let Ok(Value::Array(wire_rows)) = wire_get(&server, &key, "/v1/receipts") {
                    // The wire's tick is the tick the action APPLIED on; the journal's is
                    // the fold the pilot read it back. Same good, side and units within a
                    // few ticks is the same fill.
                    let journaled: Vec<(String, String, i64, i64)> = fills
                        .iter()
                        .map(|f| {
                            (
                                f["good"].as_str().unwrap_or("").to_string(),
                                f["side"].as_str().unwrap_or("").to_string(),
                                f["units"].as_i64().unwrap_or(0),
                                f["tick"].as_i64().unwrap_or(0),
                            )
                        })
                        .collect();
                    for r in wire_rows {
                        let good = r["good"].as_str().unwrap_or("");
                        let side = r["side"].as_str().unwrap_or("");
                        let units = r["units"].as_i64().unwrap_or(0);
                        let tick = r["tick"].as_i64().unwrap_or(0);
                        let dup = journaled.iter().any(|(g, s, u, t)| {
                            g == good && s == side && *u == units && (t - tick).abs() <= 3
                        });
                        if !dup {
                            fills.push(r);
                        }
                    }
                }
                let book = trade_book(&Value::Array(fills));
                let g = |k: &str| {
                    me.as_ref()
                        .and_then(|m| m.get(k).cloned())
                        .unwrap_or(Value::Null)
                };
                let (hauls, paid) = delivery_totals(&s.dir);
                let (aboard_units, aboard_cost) = aboard(&s.dir);
                let (closed_positions, expected, est_realized) = estimate_calibration(&s.dir);
                let credits = g("credits").as_i64().unwrap_or(0);
                let debt = g("debt").as_i64().unwrap_or(0);
                let key = if s.captain.captain_id.trim().is_empty() {
                    format!(
                        "slug:{}",
                        captain_store(&root, &s.captain.captain).display()
                    )
                } else {
                    s.captain.captain_id.clone()
                };
                let e = per_captain.entry(key).or_default();
                e.0 = s.captain.captain.clone();
                e.1 += credits;
                e.2 += debt;
                e.3 += hauls;
                e.4 += paid;
                e.5 += book.realized;
                e.6 += aboard_cost;
                let last = last_journal_line(&s.dir);
                let expiry = lease_expiry(&s.dir);
                // A ship paired before T-236 has no persona file: say so honestly
                // rather than letting the loader's household default ("the
                // familiar") answer for a computer that was never named.
                // The captain's computer, the ship's own record as a fallback for a
                // store named before the per-captain ruling, else honestly unnamed.
                // A BROKEN persona is not an unnamed one, and must not read as one
                // (codex T-236 re-verification, finding 7). `persona_for` reports a
                // file the kernel refuses as {"error": …}; taking only `name` from
                // that turned a captain whose computer will not load into a captain
                // who never named theirs, and the advice — rename her — is exactly
                // the wrong thing to do to a file that already has a name in it.
                let computer = match persona_for(&root, &s.dir, &s.captain) {
                    Some(p) => match (
                        p.get("name").and_then(Value::as_str),
                        p.get("error").and_then(Value::as_str),
                    ) {
                        (Some(n), _) => n.to_string(),
                        (None, Some(e)) => format!("(will not load: {e})"),
                        _ => "(unnamed — `fleet rename` it)".to_string(),
                    },
                    None => "(unnamed — `fleet rename` it)".to_string(),
                };
                rows.push(json!({
                    "world": s.world.id, "label": s.world.label, "computer": computer,
                    "hull": s.captain.hull_name, "captain": s.captain.captain,
                    "key_id": s.captain.key_id, "server": server,
                    "automations": std::fs::read_to_string(s.dir.join("automations.json")).ok()
                        .and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or(Value::Null),
                    "pilot_pid": pid_alive(&s.dir),
                    "lease_expires_in_h": expiry.map(|x| (x - now) / 3600),
                    "ship": g("shipName"), "docked": g("docked"), "credits": credits, "debt": debt,
                    "fuel": g("fuel"), "wearBps": g("wearBps"), "fittings": g("fittings"),
                    "titled": g("titled"), "hauls": hauls, "freight_paid": paid,
                    "trades": {"filled": book.filled, "rejected": book.rejected, "realized": book.realized,
                               "cost_of_sold": book.cost_of_sold,
                               "margin_pct": if book.cost_of_sold > 0 { book.realized * 100 / book.cost_of_sold } else { 0 },
                               "inventory_cost": aboard_cost, "inventory": aboard_units,
                               "unmatched_units": book.unmatched_units,
                               "unmatched_proceeds": book.unmatched_proceeds,
                               "quoted_basis_lots": book.quoted_basis_lots,
                               "closed_positions": closed_positions,
                               "expected_margin": expected,
                               "realized_on_closed": est_realized},
                    "last_event": last.as_ref().and_then(|v| v.get("event").cloned()).unwrap_or(Value::Null),
                    "last_at": last.as_ref().and_then(|v| v.get("at").cloned()).unwrap_or(Value::Null),
                    "reachable": me.is_some(),
                }));
            }
            if json_out {
                println!(
                    "{}",
                    json!({"ships": rows, "captains": per_captain.iter().map(|(id, (c, cr, d, h, p, rz, inv))| json!({
                        "captain_id": id, "captain": c, "pooled_credits": cr, "debt": d, "hauls": h,
                        "freight_paid": p, "trade_realized": rz, "inventory_cost": inv})).collect::<Vec<_>>()})
                );
                return ExitCode::SUCCESS;
            }
            for r in &rows {
                let pilot = match r.get("pilot_pid").and_then(Value::as_u64) {
                    Some(p) => format!("pilot {p}"),
                    None => "NO PILOT".to_string(),
                };
                let lease = match r.get("lease_expires_in_h").and_then(Value::as_i64) {
                    Some(h) if h >= 0 => format!("lease {h}h"),
                    Some(_) => "LEASE EXPIRED".to_string(),
                    None => "no lease".to_string(),
                };
                println!(
                    "{} \"{}\" · hull \"{}\" · computer \"{}\" — captain {} — {} — {} — {}",
                    r["world"].as_str().unwrap_or(""),
                    r["label"].as_str().unwrap_or(""),
                    r["hull"].as_str().unwrap_or(""),
                    r["computer"].as_str().unwrap_or(""),
                    r["captain"].as_str().unwrap_or(""),
                    pilot,
                    lease,
                    if r["reachable"].as_bool().unwrap_or(false) {
                        "on the wire"
                    } else {
                        "UNREACHABLE"
                    }
                );
                if r["reachable"].as_bool().unwrap_or(false) {
                    println!(
                        "    {} — {} — ℳ{} (debt {}) — fuel {} — wear {}bps — fittings {} — {} hauls, ℳ{} freight",
                        r["ship"].as_str().unwrap_or("?"),
                        r["docked"].as_str().unwrap_or("under way"),
                        r["credits"],
                        r["debt"],
                        r["fuel"],
                        r["wearBps"],
                        r["fittings"],
                        r["hauls"],
                        r["freight_paid"]
                    );
                }
                let t = &r["trades"];
                println!(
                    "    trades: {} filled — realized ℳ{} on ℳ{} sold ({}%) — aboard at cost ℳ{} {}",
                    t["filled"], t["realized"], t["cost_of_sold"], t["margin_pct"], t["inventory_cost"], t["inventory"]
                );
                if t["closed_positions"].as_i64().unwrap_or(0) > 0 {
                    println!(
                        "      estimates: {} closed position(s) promised ℳ{}, returned ℳ{}",
                        t["closed_positions"], t["expected_margin"], t["realized_on_closed"]
                    );
                }
                if t["unmatched_units"].as_i64().unwrap_or(0) > 0
                    || t["quoted_basis_lots"].as_i64().unwrap_or(0) > 0
                {
                    println!(
                        "      ({} units sold whose purchase this book never saw, ℳ{} set aside; {} lot(s) priced from the quoted ask)",
                        t["unmatched_units"], t["unmatched_proceeds"], t["quoted_basis_lots"]
                    );
                }
                println!(
                    "    last: {} — automations {}",
                    r["last_event"], r["automations"]
                );
            }
            println!("— per captain (pooled within a captain, never across) —");
            for (c, cr, d, h, p, rz, inv) in per_captain.values() {
                println!(
                    "  {c}: ℳ{cr} pooled, debt {d}, {h} hauls, ℳ{p} freight paid, trades realized ℳ{rz}, ℳ{inv} aboard at cost"
                );
            }
            ExitCode::SUCCESS
        }

        // ── run: one pilot per ship, kept alive; leases renewed on a human's word ──
        "serve" => {
            let bind = f
                .get("bind")
                .cloned()
                .unwrap_or_else(|| "127.0.0.1:7899".to_string());
            super::fleet_serve::serve(&dir, &root, &bind)
        }
        "run" => {
            let renew = f.contains_key("renew");
            let allow_paws = f.contains_key("allow-paws");
            let floor = f.get("interval-floor").cloned();
            let whisker: PathBuf = match f.get("whisker") {
                Some(p) => PathBuf::from(p),
                None => std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.join("whisker")))
                    .unwrap_or_else(|| PathBuf::from("whisker")),
            };
            if !whisker.exists() {
                eprintln!(
                    "fleet run: no whisker binary at {} — pass --whisker <path>",
                    whisker.display()
                );
                return ExitCode::FAILURE;
            }
            let once = f.contains_key("once");
            println!(
                "fleet run: pilots from {} — renew {} — {}",
                whisker.display(),
                if renew {
                    "ON (a human said so)"
                } else {
                    "off (leases are the household's word)"
                },
                if once { "one pass" } else { "supervising" }
            );
            let mut backoff: BTreeMap<String, (i64, u32)> = BTreeMap::new(); // next try, failures
                                                                             // The pilots this supervisor spawned, by ship: reaped every pass, because a
                                                                             // child nobody waits for becomes a zombie that `kill(pid, 0)` still calls
                                                                             // alive — both pilots sat dead for twenty minutes that way (2026-09-02).
            let mut children: BTreeMap<String, std::process::Child> = BTreeMap::new();
            loop {
                let now = super::now_secs();
                let mut gone: Vec<String> = Vec::new();
                for (id, child) in children.iter_mut() {
                    if let Ok(Some(status)) = child.try_wait() {
                        println!("{id}: pilot exited ({status})");
                        gone.push(id.clone());
                    }
                }
                for id in gone {
                    children.remove(&id);
                    let _ = std::fs::remove_file(root.join(&id).join("whisker.pid"));
                }
                for s in paired_ships(&dir, &root) {
                    let id = s.world.id.clone();
                    // Leases: renew inside two hours of expiry when authorized.
                    match lease_expiry(&s.dir) {
                        Some(exp) if exp - now < 2 * 3600 && renew => {
                            match issue_lease(&dir, &s.dir, &id, 24) {
                                Ok(new_exp) => println!("{id}: lease renewed, now to {}", new_exp),
                                Err(e) => eprintln!("{id}: lease renewal failed: {e}"),
                            }
                        }
                        Some(exp) if exp < now => {
                            println!("{id}: LEASE EXPIRED — the pilot holds at the gate until `familiar world lease {id}`");
                        }
                        _ => {}
                    }
                    // Ours and still running, or somebody else's and answering signals.
                    if children.contains_key(&id) || pid_alive(&s.dir).is_some() {
                        continue;
                    }
                    let (next, fails) = backoff.get(&id).cloned().unwrap_or((0, 0));
                    if now < next {
                        continue;
                    }
                    let out = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(s.dir.join("whisker.out"));
                    let mut cmd = std::process::Command::new(&whisker);
                    cmd.arg("--ship").arg(&s.dir);
                    for a in &s.captain.pilot_args {
                        cmd.arg(a);
                    }
                    if allow_paws {
                        cmd.arg("--allow-paws");
                    }
                    if let Some(fl) = &floor {
                        cmd.arg("--interval-floor").arg(fl);
                    }
                    if let Ok(o) = out {
                        if let Ok(e) = o.try_clone() {
                            cmd.stdout(o).stderr(e);
                        }
                    }
                    match cmd.spawn() {
                        Ok(child) => {
                            let _ =
                                std::fs::write(s.dir.join("whisker.pid"), child.id().to_string());
                            println!(
                                "{id}: pilot started (pid {}) for captain {} — \"{}\"",
                                child.id(),
                                s.captain.captain,
                                s.world.label
                            );
                            children.insert(id.clone(), child);
                            backoff.insert(id, (now + 30, fails));
                        }
                        Err(e) => {
                            let wait = (30i64 << fails.min(5)).min(600);
                            eprintln!("{id}: pilot failed to start: {e} — next try in {wait}s");
                            backoff.insert(id, (now + wait, fails + 1));
                        }
                    }
                }
                if once {
                    break;
                }
                std::thread::sleep(Duration::from_secs(60));
            }
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("fleet: unknown subcommand `{other}` — pair | unpair | status | economy | names | adopt-ids | rename | choose | run | serve");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod captain_store_tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("fleet_cap_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join("worlds").join("ship")).unwrap();
        p
    }

    fn rec(name: &str, id: &str) -> Captain {
        Captain {
            captain_id: id.into(),
            captain: name.into(),
            key_id: "k".into(),
            server: "s".into(),
            automations: vec![],
            paired_at: 0,
            hull_name: String::new(),
            pilot_args: vec![],
        }
    }

    #[test]
    fn the_captain_store_sits_beside_worlds_and_slugs_the_name() {
        let base = tmp("slug");
        let root = base.join("worlds");
        assert_eq!(
            captain_store(&root, "Luke SkyWhisker"),
            base.join("captains").join("luke-skywhisker")
        );
        assert_eq!(
            captain_store(&root, "  luke-skywhisker "),
            captain_store(&root, "Luke SkyWhisker")
        );
        assert_eq!(
            captain_store(&root, ""),
            base.join("captains").join("captain")
        );
    }

    /// A persona that will not load is not a persona that was never named, and the
    /// difference matters: the advice for "unnamed" is `fleet rename`, which would
    /// write over a file that already has a name in it (codex finding 7).
    #[test]
    fn a_broken_persona_says_so_rather_than_reading_as_unnamed() {
        let base = tmp("broken");
        let root = base.join("worlds");
        let ship = root.join("ship");
        std::fs::create_dir_all(&ship).unwrap();
        std::fs::write(
            ship.join(familiar_kernel::persona::PERSONA_FILE),
            "{ not json",
        )
        .unwrap();
        let v = persona_for(&root, &ship, &rec("Cap", "")).expect("a present file is reported");
        assert!(v.get("name").is_none(), "a broken file has no name to give");
        assert!(
            v.get("error").and_then(Value::as_str).is_some(),
            "it must carry the reason instead: {v}"
        );
    }

    /// Two hulls, one captain — the case that would have lost Felix.
    ///
    /// Ian's own PROD ships are both "Luke SkyWhisker". Migrating them one at a time
    /// would mint two ids, move the computer under the first, and hand the second an
    /// empty store: finding 2's shadowing bug in a new hat. The second hull must
    /// ADOPT the first's identity.
    #[test]
    fn a_second_hull_adopts_its_captains_existing_identity() {
        let base = tmp("adopt");
        let root = base.join("worlds");
        let mut first = rec("Luke SkyWhisker", "");
        let id = ensure_captain_id(&root, &mut first, &[]).unwrap();
        assert!(!id.is_empty());
        assert_eq!(first.captain_id, id);

        let mut second = rec("Luke SkyWhisker", "");
        let got = ensure_captain_id(&root, &mut second, std::slice::from_ref(&first)).unwrap();
        assert_eq!(got, id, "the same captain is the same captain");

        // A DIFFERENT captain never adopts.
        let mut other = rec("Big Tuna", "");
        let theirs =
            ensure_captain_id(&root, &mut other, &[first.clone(), second.clone()]).unwrap();
        assert_ne!(theirs, id);
    }

    /// The migration carries the computer across, once, and is idempotent.
    #[test]
    fn migrating_brings_the_captains_computer_with_them() {
        let base = tmp("carry");
        let root = base.join("worlds");
        let legacy = captain_store(&root, "Luke SkyWhisker");
        std::fs::create_dir_all(&legacy).unwrap();
        let felix = familiar_kernel::persona::Persona {
            name: "Felix".into(),
            ..Default::default()
        };
        familiar_kernel::persona::write(&legacy, &felix).unwrap();

        let mut r = rec("Luke SkyWhisker", "");
        let id = ensure_captain_id(&root, &mut r, &[]).unwrap();
        let moved = captain_store_by_id(&root, &id);
        assert_eq!(
            familiar_kernel::persona::load(&moved).unwrap().name,
            "Felix",
            "the captain keeps the computer they already had"
        );
        assert!(!legacy.exists(), "and it is not left in two places");

        // Running it again changes nothing and mints nothing.
        let again = ensure_captain_id(&root, &mut r, &[]).unwrap();
        assert_eq!(again, id);
    }

    /// The id is what resolves the store; the display name is only a label. Two
    /// captains whose names slug identically ("A/B" and "A B" both became `a-b`,
    /// and the brief summed their money) now keep separate stores.
    #[test]
    fn names_that_slug_alike_no_longer_share_a_store() {
        let base = tmp("collide");
        let root = base.join("worlds");
        assert_eq!(
            captain_store(&root, "A/B"),
            captain_store(&root, "A B"),
            "the legacy transform really did collide"
        );
        let mut one = rec("A/B", "");
        let mut two = rec("A B", "");
        let a = ensure_captain_id(&root, &mut one, &[]).unwrap();
        let b = ensure_captain_id(&root, &mut two, std::slice::from_ref(&one)).unwrap();
        assert_ne!(a, b, "different captains, different stores");
        assert_ne!(
            captain_store_for(&root, &one),
            captain_store_for(&root, &two)
        );
    }

    #[test]
    fn the_captain_persona_wins_over_the_ship_local_record() {
        let base = tmp("precedence");
        let root = base.join("worlds");
        let ship = root.join("ship");
        let cap = captain_store(&root, "Luke SkyWhisker");
        std::fs::create_dir_all(&cap).unwrap();
        let old = familiar_kernel::persona::Persona {
            name: "Purr".into(),
            ..Default::default()
        };
        familiar_kernel::persona::write(&ship, &old).unwrap();
        assert_eq!(
            persona_for(&root, &ship, &rec("Luke SkyWhisker", ""))
                .and_then(|v| v["name"].as_str().map(String::from)),
            Some("Purr".into()),
            "a store named before the ruling keeps its name"
        );
        let felix = familiar_kernel::persona::Persona {
            name: "Felix".into(),
            ..Default::default()
        };
        familiar_kernel::persona::write(&cap, &felix).unwrap();
        assert_eq!(
            persona_for(&root, &ship, &rec("Luke SkyWhisker", ""))
                .and_then(|v| v["name"].as_str().map(String::from)),
            Some("Felix".into())
        );
        assert!(
            persona_for(&root, &ship, &rec("Nobody Else", "")).is_some_and(|v| v["name"] == "Purr")
        );
        assert!(persona_for(&root, &root.join("other"), &rec("Nobody Else", "")).is_none());
    }

    #[test]
    fn a_broken_persona_is_an_error_on_the_feed_not_a_raw_value() {
        let base = tmp("broken");
        let root = base.join("worlds");
        let cap = captain_store(&root, "Cap");
        std::fs::create_dir_all(&cap).unwrap();
        std::fs::write(cap.join("persona.json"), r#"{"name":"X","not_a_field":1}"#).unwrap();
        let v = persona_for(&root, &root.join("ship"), &rec("Cap", "")).unwrap();
        assert!(v.get("error").is_some(), "got {v}");
        assert!(v.get("name").is_none());
    }

    /// A one-shot exchange on the loopback: answers `/v1/me` with a ship name for
    /// as many pairings as the test makes, so `fleet pair` can be driven end to end
    /// with no network and no key that means anything.
    fn stub_exchange(requests: usize) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for n in 0..requests {
                let Ok((mut conn, _)) = listener.accept() else {
                    return;
                };
                let mut buf = [0u8; 4096];
                let mut got = Vec::new();
                loop {
                    let n = conn.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    got.extend_from_slice(&buf[..n]);
                    if got.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                // A distinct hull name per pairing: two ships cannot have the same name.
                let body = format!(r#"{{"shipName":"Probe {n}","actor":"key:test"}}"#);
                let body = body.as_bytes();
                let _ = write!(
                    conn,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = conn.write_all(body);
                let _ = conn.flush();
            }
        });
        format!("http://{addr}")
    }

    fn pair_args(
        base: &Path,
        server: &str,
        label: &str,
        key: &str,
        name: Option<&str>,
    ) -> Vec<String> {
        pair_args_for(base, server, "A. Captain", label, key, name)
    }

    fn pair_args_for(
        base: &Path,
        server: &str,
        captain: &str,
        label: &str,
        key: &str,
        name: Option<&str>,
    ) -> Vec<String> {
        let mut v: Vec<String> = [
            "pair",
            "--label",
            label,
            "--captain",
            captain,
            "--server",
            server,
            "--key",
            key,
            "--commissioner",
            "ian",
            "--ttl-hours",
            "1",
            "--data-dir",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        v.push(base.to_string_lossy().into_owned());
        v.push("--store-root".into());
        v.push(base.join("worlds").to_string_lossy().into_owned());
        if let Some(n) = name {
            v.push("--computer-name".into());
            v.push(n.into());
        }
        v
    }

    /// Round 2, finding 3, at the CLI: a rename whose trail cannot be written FAILS,
    /// and the computer keeps the name its history last recorded. It used to print
    /// the trail's error, print the success sentence, and exit 0 with persona.json
    /// saying one name and the last complete event another.
    #[test]
    fn a_rename_whose_trail_cannot_be_written_fails_and_changes_nothing() {
        let base = tmp("rename_trail");
        let server = stub_exchange(1);
        let ok = cmd_fleet(&pair_args(
            &base,
            &server,
            "probe",
            "ucfk_aaaaaaaaaaaaaaaaaaaa",
            Some("Felix"),
        ));
        assert_eq!(ok, ExitCode::SUCCESS);
        let root = base.join("worlds");
        let ships = paired_ships(&base, &root);
        assert_eq!(ships.len(), 1);
        let store = captain_store_by_id(&root, &ships[0].captain.captain_id);
        assert_eq!(
            familiar_kernel::persona::load(&store).unwrap().name,
            "Felix"
        );
        assert_eq!(familiar_kernel::persona::namings(&store).len(), 1);

        // The trail becomes unwritable.
        let trail = store.join(familiar_kernel::persona::NAME_EVENTS_FILE);
        std::fs::remove_file(&trail).unwrap();
        std::fs::create_dir(&trail).unwrap();
        let world_id = ships[0].world.id.clone();
        let rc = cmd_fleet(&[
            "rename".to_string(),
            world_id,
            "Sprocket".to_string(),
            "--data-dir".to_string(),
            base.to_string_lossy().into_owned(),
            "--store-root".to_string(),
            base.join("worlds").to_string_lossy().into_owned(),
        ]);
        assert_eq!(
            rc,
            ExitCode::FAILURE,
            "an unrecorded naming is a failure, not a success"
        );
        assert_eq!(
            familiar_kernel::persona::load(&store).unwrap().name,
            "Felix",
            "the persona must not wear a name its history does not contain"
        );
        assert!(trail.is_dir(), "the failed append created nothing");
    }

    /// Round 2, finding 6, at the CLI: a second pairing for a captain whose
    /// computer already exists, injected to fail at the naming (the trail is a
    /// directory), leaves NO new world, no key file, no captain record, and no
    /// event. It used to fail after commissioning, with a live world on disk for
    /// the supervisor to find and fly.
    #[test]
    fn a_pairing_whose_naming_fails_commissions_nothing() {
        let base = tmp("pair_naming");
        let server = stub_exchange(2);
        assert_eq!(
            cmd_fleet(&pair_args(
                &base,
                &server,
                "first",
                "ucfk_aaaaaaaaaaaaaaaaaaaa",
                Some("Felix")
            )),
            ExitCode::SUCCESS
        );
        let root = base.join("worlds");
        let before = paired_ships(&base, &root);
        assert_eq!(before.len(), 1);
        let store = captain_store_by_id(&root, &before[0].captain.captain_id);
        let trail = store.join(familiar_kernel::persona::NAME_EVENTS_FILE);
        std::fs::remove_file(&trail).unwrap();
        std::fs::create_dir(&trail).unwrap();
        let worlds_before: std::collections::BTreeSet<String> = std::fs::read_dir(&root)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();

        let rc = cmd_fleet(&pair_args(
            &base,
            &server,
            "second",
            "ucfk_bbbbbbbbbbbbbbbbbbbb",
            None,
        ));
        assert_eq!(rc, ExitCode::FAILURE);
        let after = paired_ships(&base, &root);
        assert_eq!(after.len(), 1, "no new active world");
        assert_eq!(
            instance::load(&base).unwrap().len(),
            1,
            "no new registry entry"
        );
        let worlds_after: std::collections::BTreeSet<String> = std::fs::read_dir(&root)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            worlds_after, worlds_before,
            "no new directory under the store"
        );
        assert!(trail.is_dir(), "no event was written");
        assert_eq!(
            familiar_kernel::persona::load(&store).unwrap().name,
            "Felix"
        );
    }

    /// Turn a paired hull back into a pre-identity one: no captain_id, no id store,
    /// and its computer written ship-local with a trail — the deployment shape every
    /// hull had before T-236's identity landed.
    fn make_legacy(base: &Path, ship: &Ship, computer: &str, trail_actor: &str) {
        let root = base.join("worlds");
        let _ = std::fs::remove_dir_all(captain_store_by_id(&root, &ship.captain.captain_id));
        let mut rec = ship.captain.clone();
        rec.captain_id = String::new();
        std::fs::write(
            ship.dir.join("captain.json"),
            serde_json::to_vec_pretty(&rec).unwrap(),
        )
        .unwrap();
        let p = familiar_kernel::persona::Persona {
            persona_version: 2,
            name: computer.into(),
            style: Some(familiar_kernel::persona::Style::default()),
            ..Default::default()
        };
        familiar_kernel::persona::name(
            &ship.dir,
            &p,
            Some(&familiar_kernel::persona::NameEvent {
                at: 7,
                actor: trail_actor.into(),
                name: computer.into(),
                pronouns: None,
                why: String::new(),
            }),
        )
        .unwrap();
    }

    /// Round 2, finding 2: a second pairing for a captain whose only computer is a
    /// hull's ship-local record brings that computer WHOLE — a tuned Purr included,
    /// trail included — into the id store, and points the old hull at it too. One
    /// captain, one voice, from either hull.
    #[test]
    fn a_second_pairing_migrates_the_whole_ship_local_computer_and_its_sibling() {
        let base = tmp("migrate_whole");
        let server = stub_exchange(2);
        assert_eq!(
            cmd_fleet(&pair_args(
                &base,
                &server,
                "old",
                "ucfk_aaaaaaaaaaaaaaaaaaaa",
                None
            )),
            ExitCode::SUCCESS
        );
        let root = base.join("worlds");
        let old = paired_ships(&base, &root).remove(0);
        make_legacy(&base, &old, "Purr", "tuned-by-the-captain");

        assert_eq!(
            cmd_fleet(&pair_args(
                &base,
                &server,
                "new",
                "ucfk_bbbbbbbbbbbbbbbbbbbb",
                None
            )),
            ExitCode::SUCCESS
        );
        let ships = paired_ships(&base, &root);
        assert_eq!(ships.len(), 2);
        let ids: std::collections::BTreeSet<&str> = ships
            .iter()
            .map(|s| s.captain.captain_id.as_str())
            .collect();
        assert_eq!(
            ids.len(),
            1,
            "one captain, one identity, on both hulls: {ids:?}"
        );
        let id = ids.into_iter().next().unwrap();
        assert!(!id.is_empty());
        let store = captain_store_by_id(&root, id);
        assert_eq!(
            familiar_kernel::persona::load(&store).unwrap().name,
            "Purr",
            "the tuned Purr is retained"
        );
        let trail = familiar_kernel::persona::namings(&store);
        assert!(
            trail.iter().any(|e| e.actor == "tuned-by-the-captain"),
            "the trail came whole: {trail:?}"
        );
        for s in &ships {
            let v = persona_for(&root, &s.dir, &s.captain).unwrap();
            assert_eq!(v["name"], "Purr", "{}", s.world.label);
        }
    }

    /// Two ship-local records that disagree are refused, not raced.
    #[test]
    fn disagreeing_ship_local_computers_refuse_a_pairing() {
        let base = tmp("disagree");
        let server = stub_exchange(3);
        assert_eq!(
            cmd_fleet(&pair_args(
                &base,
                &server,
                "one",
                "ucfk_aaaaaaaaaaaaaaaaaaaa",
                None
            )),
            ExitCode::SUCCESS
        );
        assert_eq!(
            cmd_fleet(&pair_args(
                &base,
                &server,
                "two",
                "ucfk_bbbbbbbbbbbbbbbbbbbb",
                None
            )),
            ExitCode::SUCCESS
        );
        let root = base.join("worlds");
        let ships = paired_ships(&base, &root);
        make_legacy(&base, &ships[0], "Felix", "a");
        make_legacy(&base, &ships[1], "Mittens", "b");
        assert_eq!(
            cmd_fleet(&pair_args(
                &base,
                &server,
                "three",
                "ucfk_cccccccccccccccccccc",
                None
            )),
            ExitCode::FAILURE
        );
        assert_eq!(
            paired_ships(&base, &root).len(),
            2,
            "nothing was commissioned"
        );
    }

    /// A rename on a legacy hull renames the computer it HAS, migrated whole first —
    /// never an empty id store's default.
    #[test]
    fn a_rename_on_a_legacy_hull_keeps_the_tuned_computer() {
        let base = tmp("rename_legacy");
        let server = stub_exchange(1);
        assert_eq!(
            cmd_fleet(&pair_args(
                &base,
                &server,
                "hull",
                "ucfk_aaaaaaaaaaaaaaaaaaaa",
                None
            )),
            ExitCode::SUCCESS
        );
        let root = base.join("worlds");
        let ship = paired_ships(&base, &root).remove(0);
        make_legacy(&base, &ship, "Purr", "tuned-by-the-captain");
        let rc = cmd_fleet(&[
            "rename".into(),
            ship.world.id.clone(),
            "Mittens".into(),
            "--data-dir".into(),
            base.to_string_lossy().into_owned(),
            "--store-root".into(),
            root.to_string_lossy().into_owned(),
        ]);
        assert_eq!(rc, ExitCode::SUCCESS);
        let after = paired_ships(&base, &root).remove(0);
        assert!(!after.captain.captain_id.is_empty());
        let store = captain_store_by_id(&root, &after.captain.captain_id);
        assert_eq!(
            familiar_kernel::persona::load(&store).unwrap().name,
            "Mittens"
        );
        let trail = familiar_kernel::persona::namings(&store);
        assert_eq!(
            trail.len(),
            2,
            "the migrated naming and the rename: {trail:?}"
        );
        assert_eq!(trail[0].name, "Purr");
        assert_eq!(trail[1].name, "Mittens");
    }

    /// Round 2, finding 4: an unmigrated hull listed BEFORE its already-migrated
    /// sibling adopts the sibling's id — adopt-ids inspects every record first.
    #[test]
    fn adopt_ids_hands_one_captain_one_id_whichever_hull_is_first() {
        let base = tmp("adopt_order");
        let server = stub_exchange(2);
        assert_eq!(
            cmd_fleet(&pair_args(
                &base,
                &server,
                "first",
                "ucfk_aaaaaaaaaaaaaaaaaaaa",
                None
            )),
            ExitCode::SUCCESS
        );
        assert_eq!(
            cmd_fleet(&pair_args(
                &base,
                &server,
                "second",
                "ucfk_bbbbbbbbbbbbbbbbbbbb",
                None
            )),
            ExitCode::SUCCESS
        );
        let root = base.join("worlds");
        let ships = paired_ships(&base, &root);
        let kept = ships[1].captain.captain_id.clone();
        let mut rec = ships[0].captain.clone();
        rec.captain_id = String::new();
        std::fs::write(
            ships[0].dir.join("captain.json"),
            serde_json::to_vec_pretty(&rec).unwrap(),
        )
        .unwrap();
        let rc = cmd_fleet(&[
            "adopt-ids".into(),
            "--data-dir".into(),
            base.to_string_lossy().into_owned(),
            "--store-root".into(),
            root.to_string_lossy().into_owned(),
        ]);
        assert_eq!(rc, ExitCode::SUCCESS);
        let after = paired_ships(&base, &root);
        assert!(
            after.iter().all(|s| s.captain.captain_id == kept),
            "{:?}",
            after
                .iter()
                .map(|s| &s.captain.captain_id)
                .collect::<Vec<_>>()
        );
    }

    /// Two captains whose names slug alike, both unmigrated, are a collision the
    /// migration refuses rather than moving one directory for two people.
    #[test]
    fn adopt_ids_refuses_colliding_legacy_slugs() {
        let base = tmp("adopt_collide");
        let server = stub_exchange(2);
        assert_eq!(
            cmd_fleet(&pair_args_for(
                &base,
                &server,
                "A/B",
                "one",
                "ucfk_aaaaaaaaaaaaaaaaaaaa",
                None
            )),
            ExitCode::SUCCESS
        );
        assert_eq!(
            cmd_fleet(&pair_args_for(
                &base,
                &server,
                "A B",
                "two",
                "ucfk_bbbbbbbbbbbbbbbbbbbb",
                None
            )),
            ExitCode::SUCCESS
        );
        let root = base.join("worlds");
        for s in paired_ships(&base, &root) {
            let mut rec = s.captain.clone();
            rec.captain_id = String::new();
            std::fs::write(
                s.dir.join("captain.json"),
                serde_json::to_vec_pretty(&rec).unwrap(),
            )
            .unwrap();
        }
        let rc = cmd_fleet(&[
            "adopt-ids".into(),
            "--data-dir".into(),
            base.to_string_lossy().into_owned(),
            "--store-root".into(),
            root.to_string_lossy().into_owned(),
        ]);
        assert_eq!(
            rc,
            ExitCode::FAILURE,
            "a slug shared by two captains is refused"
        );
    }

    /// Ian, 2026-09-08: two ships' computers cannot have the same name. A second
    /// captain naming theirs after the first's is refused — now, and after the first
    /// captain has moved on from it (a name is a lineage).
    #[test]
    fn two_computers_cannot_share_a_name_now_or_ever() {
        let base = tmp("unique_computer");
        let server = stub_exchange(8);
        assert_eq!(
            cmd_fleet(&pair_args_for(
                &base,
                &server,
                "Luke",
                "kk",
                "ucfk_aaaaaaaaaaaaaaaaaaaa",
                Some("Felix")
            )),
            ExitCode::SUCCESS
        );
        assert_eq!(
            cmd_fleet(&pair_args_for(
                &base,
                &server,
                "Ann",
                "tuna",
                "ucfk_bbbbbbbbbbbbbbbbbbbb",
                Some("felix")
            )),
            ExitCode::FAILURE,
            "Felix is Luke's computer's name, whatever the case"
        );
        let root = base.join("worlds");
        assert_eq!(
            paired_ships(&base, &root).len(),
            1,
            "nothing was commissioned"
        );
        // Luke renames to Mittens; Felix is still Luke's lineage, so Ann still may not.
        let luke = paired_ships(&base, &root).remove(0);
        let rename = |world: &str, name: &str| {
            cmd_fleet(&[
                "rename".into(),
                world.into(),
                name.into(),
                "--data-dir".into(),
                base.to_string_lossy().into_owned(),
                "--store-root".into(),
                root.to_string_lossy().into_owned(),
            ])
        };
        assert_eq!(rename(&luke.world.id, "Mittens"), ExitCode::SUCCESS);
        assert_eq!(
            cmd_fleet(&pair_args_for(
                &base,
                &server,
                "Ann",
                "tuna",
                "ucfk_bbbbbbbbbbbbbbbbbbbb",
                None
            )),
            ExitCode::SUCCESS
        );
        let ann = paired_ships(&base, &root)
            .into_iter()
            .find(|s| s.captain.captain == "Ann")
            .unwrap();
        assert_eq!(
            rename(&ann.world.id, "Felix"),
            ExitCode::FAILURE,
            "a name once worn stays with its lineage"
        );
        // Luke may go back to his own former name.
        assert_eq!(rename(&luke.world.id, "Felix"), ExitCode::SUCCESS);
        // And the ledger remembers all of it, oldest first.
        let ledger = names(&root);
        let computers: Vec<(String, String, String)> = ledger
            .iter()
            .filter(|e| e.kind == "computer")
            .map(|e| (e.act.clone(), e.name.clone(), e.from.clone()))
            .collect();
        assert_eq!(
            computers,
            vec![
                ("named".into(), "Felix".into(), String::new()),
                ("renamed".into(), "Mittens".into(), "Felix".into()),
                ("joined".into(), "Purr".into(), String::new()),
                ("renamed".into(), "Felix".into(), "Mittens".into()),
            ],
            "{ledger:?}"
        );
        assert!(ledger
            .iter()
            .any(|e| e.kind == "hull" && e.name == "Probe 0"));
        assert!(ledger
            .iter()
            .any(|e| e.kind == "captain" && e.name == "Luke"));
        assert_eq!(
            cmd_fleet(&[
                "names".into(),
                "--data-dir".into(),
                base.to_string_lossy().into_owned(),
                "--store-root".into(),
                root.to_string_lossy().into_owned()
            ]),
            ExitCode::SUCCESS
        );
    }

    /// Ian, 2026-09-09: gender is the familiar's choice at every naming. A pairing
    /// with a name chooses and records it on the persona, the trail and the ledger;
    /// a pairing without a name leaves the computer unnamed and unspoken-of (`it`);
    /// a rename chooses again; `fleet choose` lets a computer named before the choice
    /// existed make it now, as its own act.
    #[test]
    fn the_familiar_chooses_how_it_is_spoken_of_at_every_naming() {
        let base = tmp("pronouns");
        let server = stub_exchange(8);
        assert_eq!(
            cmd_fleet(&pair_args(
                &base,
                &server,
                "one",
                "ucfk_aaaaaaaaaaaaaaaaaaaa",
                Some("Felix")
            )),
            ExitCode::SUCCESS
        );
        let root = base.join("worlds");
        let ship = paired_ships(&base, &root).remove(0);
        let store = captain_store_by_id(&root, &ship.captain.captain_id);
        let first = familiar_kernel::persona::load(&store)
            .unwrap()
            .pronouns
            .expect("named: chosen");
        let trail = familiar_kernel::persona::namings(&store);
        assert_eq!(trail[0].pronouns.as_ref(), Some(&first));
        assert!(
            trail[0].why.contains("the familiar's choice"),
            "{}",
            trail[0].why
        );
        assert!(names(&root)
            .iter()
            .any(|e| e.kind == "computer" && e.pronouns == first.label));
        assert_eq!(
            computer_state(&root, &ship.dir, &ship.captain)["pronouns"]["label"],
            first.label
        );

        assert_eq!(
            cmd_fleet(&pair_args_for(
                &base,
                &server,
                "Ann",
                "two",
                "ucfk_bbbbbbbbbbbbbbbbbbbb",
                None
            )),
            ExitCode::SUCCESS
        );
        let ann = paired_ships(&base, &root)
            .into_iter()
            .find(|s| s.captain.captain == "Ann")
            .unwrap();
        assert!(familiar_kernel::persona::load(&captain_store_by_id(
            &root,
            &ann.captain.captain_id
        ))
        .unwrap()
        .pronouns
        .is_none());
        let args = |sub: &str, world: &str, extra: &[&str]| -> Vec<String> {
            let mut v: Vec<String> = vec![sub.into(), world.into()];
            v.extend(extra.iter().map(|s| s.to_string()));
            v.extend([
                "--data-dir".to_string(),
                base.to_string_lossy().into_owned(),
                "--store-root".to_string(),
                root.to_string_lossy().into_owned(),
            ]);
            v
        };
        assert_eq!(
            cmd_fleet(&args("choose", &ann.world.id, &[])),
            ExitCode::FAILURE,
            "unnamed: nothing to choose for yet"
        );

        assert_eq!(
            cmd_fleet(&args("rename", &ship.world.id, &["Mrs. Norris"])),
            ExitCode::SUCCESS
        );
        let trail = familiar_kernel::persona::namings(&store);
        assert_eq!(trail.len(), 2);
        assert!(trail[1].pronouns.is_some() && !trail[1].why.is_empty());

        assert_eq!(
            cmd_fleet(&args("choose", &ship.world.id, &[])),
            ExitCode::SUCCESS
        );
        let trail = familiar_kernel::persona::namings(&store);
        assert_eq!(trail.len(), 3);
        assert_eq!(trail[2].actor, "familiar");
        assert!(
            trail[2].why.contains("the choice not given at naming"),
            "{}",
            trail[2].why
        );
        assert_eq!(
            familiar_kernel::persona::load(&store).unwrap().pronouns,
            trail[2].pronouns
        );
        assert!(names(&root)
            .iter()
            .any(|e| e.act == "chose" && e.by == "familiar"));
    }

    /// One key is one hull: pairing a key that already flies a world is refused with
    /// the world named, whatever captain name is typed.
    #[test]
    fn a_key_already_paired_is_not_paired_twice() {
        let base = tmp("one_key_one_hull");
        let server = stub_exchange(2);
        assert_eq!(
            cmd_fleet(&pair_args_for(
                &base,
                &server,
                "Luke SkyWhisker",
                "ng",
                "ucfk_dddddddddddddddddddd",
                None
            )),
            ExitCode::SUCCESS
        );
        assert_eq!(
            cmd_fleet(&pair_args_for(
                &base,
                &server,
                "Luke Sky-Whisker",
                "ng again",
                "ucfk_dddddddddddddddddddd",
                None
            )),
            ExitCode::FAILURE
        );
        assert_eq!(paired_ships(&base, &base.join("worlds")).len(), 1);
    }

    /// Joining a captain's own computer is not a naming: a second hull for Luke pairs
    /// even though another captain (the LOCAL twin) wears the same computer name —
    /// only a name GIVEN at pairing is checked for uniqueness.
    #[test]
    fn a_second_hull_joins_its_captains_computer_whatever_other_captains_wear() {
        let base = tmp("join_not_naming");
        let server = stub_exchange(4);
        assert_eq!(
            cmd_fleet(&pair_args_for(
                &base,
                &server,
                "Luke",
                "one",
                "ucfk_aaaaaaaaaaaaaaaaaaaa",
                Some("Felix")
            )),
            ExitCode::SUCCESS
        );
        // Another captain wears Felix too (a grandfathered twin, written straight to the ledger).
        let root = base.join("worlds");
        record_name(
            &root,
            &NameEntry {
                at: 1,
                kind: "computer".into(),
                name: "Felix".into(),
                holder: "cpt-twin".into(),
                act: "named".into(),
                from: String::new(),
                by: "ian".into(),
                pronouns: String::new(),
            },
        )
        .unwrap();
        assert_eq!(
            cmd_fleet(&pair_args_for(
                &base,
                &server,
                "Luke",
                "two",
                "ucfk_bbbbbbbbbbbbbbbbbbbb",
                None
            )),
            ExitCode::SUCCESS,
            "joining Luke's own Felix is not a naming"
        );
        assert_eq!(paired_ships(&base, &root).len(), 2);
        // But a NAME given that another captain wears is still refused.
        assert_eq!(
            cmd_fleet(&pair_args_for(
                &base,
                &server,
                "Ann",
                "three",
                "ucfk_cccccccccccccccccccc",
                Some("Felix")
            )),
            ExitCode::FAILURE
        );
    }

    /// Two ships cannot have the same name: a second hull the exchange calls what a
    /// paired hull is already called, on another key, is refused.
    #[test]
    fn two_hulls_cannot_share_a_name() {
        let base = tmp("unique_hull");
        // Every answer names the ship the same.
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for _ in 0..2 {
                let Ok((mut conn, _)) = listener.accept() else {
                    return;
                };
                let mut buf = [0u8; 4096];
                let _ = conn.read(&mut buf);
                let body = br#"{"shipName":"Kibble Klipper","actor":"key:test"}"#;
                let _ = write!(conn, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = conn.write_all(body);
            }
        });
        let server = format!("http://{addr}");
        assert_eq!(
            cmd_fleet(&pair_args_for(
                &base,
                &server,
                "Luke",
                "one",
                "ucfk_aaaaaaaaaaaaaaaaaaaa",
                None
            )),
            ExitCode::SUCCESS
        );
        assert_eq!(
            cmd_fleet(&pair_args_for(
                &base,
                &server,
                "Ann",
                "two",
                "ucfk_bbbbbbbbbbbbbbbbbbbb",
                None
            )),
            ExitCode::FAILURE
        );
        assert_eq!(paired_ships(&base, &base.join("worlds")).len(), 1);
    }

    /// Ian, 2026-09-08: we do not forget names — what was named before the ledger
    /// existed is remembered into it, dated by the trail's own clock, by the trail's
    /// own actor, and a second run remembers nothing twice.
    #[test]
    fn the_ledger_remembers_what_was_named_before_it_existed() {
        let base = tmp("backfill");
        let server = stub_exchange(1);
        assert_eq!(
            cmd_fleet(&pair_args(
                &base,
                &server,
                "kk",
                "ucfk_aaaaaaaaaaaaaaaaaaaa",
                Some("Felix")
            )),
            ExitCode::SUCCESS
        );
        let root = base.join("worlds");
        // Pretend the ledger never existed, and that the captain renamed once before it did.
        std::fs::remove_file(root.parent().unwrap().join("captains").join("names.jsonl")).unwrap();
        let ship = paired_ships(&base, &root).remove(0);
        let store = captain_store_by_id(&root, &ship.captain.captain_id);
        let mut p = familiar_kernel::persona::load(&store).unwrap();
        p.name = "Mrs. Norris".into();
        familiar_kernel::persona::name(
            &store,
            &p,
            Some(&familiar_kernel::persona::NameEvent {
                at: 1_900_000_000,
                actor: "A. Captain".into(),
                name: "Mrs. Norris".into(),
                pronouns: None,
                why: String::new(),
            }),
        )
        .unwrap();
        assert!(names(&root).is_empty());
        let args = |extra: &[&str]| -> Vec<String> {
            let mut v: Vec<String> = vec!["names".into()];
            v.extend(extra.iter().map(|s| s.to_string()));
            v.extend([
                "--data-dir".to_string(),
                base.to_string_lossy().into_owned(),
                "--store-root".to_string(),
                root.to_string_lossy().into_owned(),
            ]);
            v
        };
        assert_eq!(cmd_fleet(&args(&["--backfill"])), ExitCode::SUCCESS);
        let got = names(&root);
        let computers: Vec<(String, String, String, String)> = got
            .iter()
            .filter(|e| e.kind == "computer")
            .map(|e| (e.act.clone(), e.name.clone(), e.from.clone(), e.by.clone()))
            .collect();
        assert_eq!(
            computers,
            vec![
                (
                    "named".into(),
                    "Felix".into(),
                    String::new(),
                    "A. Captain".into()
                ),
                (
                    "renamed".into(),
                    "Mrs. Norris".into(),
                    "Felix".into(),
                    "A. Captain".into()
                ),
            ],
            "{got:?}"
        );
        assert!(got.iter().any(|e| e.kind == "captain"
            && e.name == "A. Captain"
            && e.holder == ship.captain.captain_id));
        assert!(got
            .iter()
            .any(|e| e.kind == "hull" && e.name == "Probe 0" && e.holder == ship.world.id));
        assert!(
            got.iter()
                .any(|e| e.kind == "computer" && e.name == "Mrs. Norris" && e.at == 1_900_000_000),
            "dated by the trail's own clock"
        );
        let n = got.len();
        assert_eq!(cmd_fleet(&args(&["--backfill"])), ExitCode::SUCCESS);
        assert_eq!(
            names(&root).len(),
            n,
            "a second run remembers nothing twice"
        );
    }
}
