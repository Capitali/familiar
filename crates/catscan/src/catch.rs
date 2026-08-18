//! What the cat dragged in — typed views of whatever United Cat Foods answered with.
//!
//! **Deliberately lenient, and that is a divergence worth naming.** `familiar-mcp` parses
//! strictly — a server that answers something other than what MCP describes gets an
//! `Error::Protocol` rather than a guess, because the client *acts* on what it reads. This
//! crate only *stares*, so every field is `#[serde(default)]` and unknown fields are ignored:
//! when Jeff adds a column, the cat keeps watching the ten things it already understood
//! instead of stalking off in a huff. An instrument that goes dark on an unfamiliar field is
//! worse than one that admits it is showing a subset.
//!
//! Wire names stay wire names (`stationClass`, `worldName`) so the mapping to Jeff's payload
//! stays legible at a glance. Everything that is *ours* to name is a cat.

use serde::Deserialize;
use serde_json::Value;

/// `ucf_status` — the world's steady purr. Tick, seed, and the hash of everything.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Purr {
    pub world_name: String,
    pub world_seed: String,
    pub tick: i64,
    pub tick_duration_sec: i64,
    pub next_tick_at: String,
    pub epoch_unix_seconds: i64,
    pub content_version: i64,
    pub state_hash: String,
}

/// `ucf_stations` — the perches. High places where trade happens and cats sit.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Perch {
    pub id: String,
    pub display_name: String,
    pub body: String,
    pub role: String,
    pub station_class: String,
    pub trades_goods: bool,
    pub sells_fuel: bool,
}

/// `ucf_prices` — one bowl of one good at one perch, and what it costs to fill.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Kibble {
    pub good: String,
    pub station: String,
    pub mid: f64,
    pub stock: i64,
}

impl Kibble {
    /// Which bowl this is, across prowls — what makes a change a *change* rather than a
    /// different bowl.
    pub fn bowl(&self) -> String {
        format!("{}@{}", self.good, self.station)
    }

    /// An empty bowl. The most upsetting thing in the known universe.
    pub fn empty(&self) -> bool {
        self.stock == 0
    }
}

/// `ucf_news` — a yowl. Something the world announces before it bites.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Yowl {
    pub headline: String,
    /// `announced` / `in-effect` / `expired`, as the exchange words it.
    pub status: String,
    /// `confirmed` / `likely` — the exchange's own confidence, carried, never re-judged.
    pub tier: String,
    pub announced_at_tick: i64,
    pub effective_at_tick: i64,
    pub expires_at_tick: i64,
}

impl Yowl {
    /// Still being yowled about. An expired yowl is yesterday's outrage.
    pub fn live(&self) -> bool {
        self.status != "expired"
    }
}

/// `ucf_carriers` — a tomcat. Roams between perches, occasionally carrying something.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Tomcat {
    pub carrier_id: String,
    pub display_name: String,
    pub operator_name: String,
    /// A perch id when curled up, or `-> perch` while out on the prowl.
    pub location: String,
    pub in_service: bool,
    pub active_load: String,
    pub arrive_tick: i64,
}

impl Tomcat {
    /// Out prowling rather than curled up on a perch.
    pub fn on_the_prowl(&self) -> bool {
        self.location.starts_with("->")
    }
}

/// `ucf_loadboard` — a haul. Freight on offer, and freight already carried off.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Haul {
    pub load_id: String,
    pub good: String,
    pub origin: String,
    pub dest: String,
    pub status: String,
    pub service_class: String,
    pub booked_by: String,
    pub units: i64,
    pub freight_rate: f64,
    pub estimated_net: f64,
    pub posted_at_tick: i64,
    pub expires_at_tick: i64,
    /// The exchange's own flag for "this one belongs to whoever holds the token" — the wire
    /// calls it `mine`, and it is the most direct answer the wire can give to *is the familiar
    /// actually hunting here*. Read, never inferred from call counts.
    #[serde(rename = "mine")]
    pub ours: bool,
}

/// Let the cat out of the bag: unwrap an MCP `tools/call` answer into the JSON the tool really
/// returned.
///
/// The exchange answers in the protocol's content-block shape with the payload as a JSON
/// *string* inside `content[0].text`, so there are two bags, not one. `isError: true` carries
/// the server's own words out as an `Err` — the cat reports what it was hissed at with, rather
/// than presenting an empty panel that looks like a peaceful afternoon.
pub fn open_the_bag(answer: &Value) -> Result<Value, String> {
    let text = answer
        .get("content")
        .and_then(Value::as_array)
        .and_then(|b| b.first())
        .and_then(|b| b.get("text"))
        .and_then(Value::as_str)
        .ok_or_else(|| "the bag had no text in it".to_string())?;

    if answer.get("isError").and_then(Value::as_bool) == Some(true) {
        return Err(text.to_string());
    }
    serde_json::from_str(text).map_err(|e| format!("unchewable payload: {e}"))
}

/// Have a taste. Names the tool when it disagrees with the cat, so a spoiled panel says which
/// bowl it came out of.
pub fn taste<T: for<'de> Deserialize<'de>>(answer: &Value, tool: &str) -> Result<T, String> {
    let p = open_the_bag(answer)?;
    serde_json::from_value(p).map_err(|e| format!("{tool}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bag(text: &str) -> Value {
        json!({"content": [{"type": "text", "text": text}]})
    }

    #[test]
    fn a_purr_parses_through_both_bags() {
        let a = bag(
            r#"{"contentVersion":25,"epochUnixSeconds":1785258645,"tick":5836,
                "tickDurationSec":300,"worldName":"PROD","worldSeed":"212332557",
                "nextTickAt":"2026-08-17T23:35:45Z","stateHash":"d8f695"}"#,
        );
        let s: Purr = taste(&a, "ucf_status").unwrap();
        assert_eq!(s.tick, 5836);
        assert_eq!(s.world_name, "PROD");
        assert_eq!(s.tick_duration_sec, 300);
    }

    /// The lenience that is the point of this module: a field the cat has never sniffed must
    /// not cost it the fields it already knows.
    #[test]
    fn an_unknown_field_does_not_scare_the_cat_off() {
        let a = bag(
            r#"[{"good":"catnip","station":"cannery-row","mid":21,"stock":444,
                        "somethingJeffAddedLater":{"deeply":["nested"]}}]"#,
        );
        let k: Vec<Kibble> = taste(&a, "ucf_prices").unwrap();
        assert_eq!(k.len(), 1);
        assert_eq!(k[0].bowl(), "catnip@cannery-row");
        assert_eq!(k[0].stock, 444);
        assert!(!k[0].empty());
    }

    /// A missing field is absence, not corruption — the cat watches the rest.
    #[test]
    fn a_missing_field_defaults_rather_than_sulking() {
        let a = bag(r#"[{"loadId":"L744","good":"grain"}]"#);
        let h: Vec<Haul> = taste(&a, "ucf_loadboard").unwrap();
        assert_eq!(h[0].load_id, "L744");
        assert!(!h[0].ours);
        assert_eq!(h[0].units, 0);
    }

    /// `isError` is the server hissing. Carry its words; never swallow them into a blank.
    #[test]
    fn a_hiss_carries_the_servers_own_words() {
        let mut a = bag("station is required");
        a["isError"] = json!(true);
        assert_eq!(open_the_bag(&a).unwrap_err(), "station is required");
    }

    #[test]
    fn a_tomcat_on_the_prowl_is_told_from_one_curled_up() {
        let out = Tomcat {
            location: "-> whisker-hollow".into(),
            ..Default::default()
        };
        let home = Tomcat {
            location: "clawson-drift".into(),
            ..Default::default()
        };
        assert!(out.on_the_prowl());
        assert!(!home.on_the_prowl());
    }

    #[test]
    fn an_expired_yowl_is_yesterdays_outrage() {
        let fresh = Yowl {
            status: "in-effect".into(),
            ..Default::default()
        };
        let stale = Yowl {
            status: "expired".into(),
            ..Default::default()
        };
        assert!(fresh.live());
        assert!(!stale.live());
    }

    /// The bowl is empty. This is a five-alarm emergency and must be detectable.
    #[test]
    fn an_empty_bowl_is_never_missed() {
        let empty = Kibble {
            stock: 0,
            ..Default::default()
        };
        assert!(empty.empty());
    }
}
