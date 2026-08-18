//! Typed views of what the UCF exchange answers.
//!
//! **Deliberately lenient, and that is a divergence worth naming.** `familiar-mcp` parses
//! strictly — a server that answers something other than what MCP describes gets an
//! `Error::Protocol` rather than a guess, because the client acts on what it reads. This
//! crate only *draws*, so every field is `#[serde(default)]` and unknown fields are ignored:
//! when Jeff adds a column, the monitor keeps showing the ten things it already understood
//! instead of blanking the screen. An instrument that goes dark on an unfamiliar field is
//! worse than one that admits it is showing a subset.

use serde::Deserialize;
use serde_json::Value;

/// `ucf_status` — the world's own clock.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Status {
    pub world_name: String,
    pub world_seed: String,
    pub tick: i64,
    pub tick_duration_sec: i64,
    pub next_tick_at: String,
    pub epoch_unix_seconds: i64,
    pub content_version: i64,
    pub state_hash: String,
}

/// `ucf_stations` — the places that trade.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Station {
    pub id: String,
    pub display_name: String,
    pub body: String,
    pub role: String,
    pub station_class: String,
    pub trades_goods: bool,
    pub sells_fuel: bool,
}

/// `ucf_prices` — one row per good per station.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Price {
    pub good: String,
    pub station: String,
    pub mid: f64,
    pub stock: i64,
}

impl Price {
    /// The identity of a price row across rounds — what makes a change a *change* rather
    /// than a different row.
    pub fn key(&self) -> String {
        format!("{}@{}", self.good, self.station)
    }
}

/// `ucf_news` — events the world announces before they bite.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct News {
    pub headline: String,
    /// `announced` / `in-effect` / `expired`, as the exchange words it.
    pub status: String,
    /// `confirmed` / `likely` — the exchange's own confidence, carried, never re-judged.
    pub tier: String,
    pub announced_at_tick: i64,
    pub effective_at_tick: i64,
    pub expires_at_tick: i64,
}

/// `ucf_carriers` — haulers, and where they are.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Carrier {
    pub carrier_id: String,
    pub display_name: String,
    pub operator_name: String,
    /// A station id when docked, or `-> station` while under way.
    pub location: String,
    pub in_service: bool,
    pub active_load: String,
    pub arrive_tick: i64,
}

impl Carrier {
    pub fn under_way(&self) -> bool {
        self.location.starts_with("->")
    }
}

/// `ucf_loadboard` — freight on offer, and freight taken.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Load {
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
    /// The exchange's own flag for "this one belongs to the participant holding the token".
    /// It is the single most direct answer the wire can give to *is the familiar acting
    /// here* — so the monitor reads it rather than inferring participation from call counts.
    pub mine: bool,
}

/// Unwrap an MCP `tools/call` answer into the JSON the tool actually returned.
///
/// The exchange answers in the protocol's content-block shape with the payload as a JSON
/// *string* inside `content[0].text`, so there are two parses, not one. `isError: true`
/// carries the server's own words out as an `Err` — the monitor shows what the server said
/// rather than rendering an empty panel that looks like "nothing is happening".
pub fn payload(answer: &Value) -> Result<Value, String> {
    let text = answer
        .get("content")
        .and_then(Value::as_array)
        .and_then(|b| b.first())
        .and_then(|b| b.get("text"))
        .and_then(Value::as_str)
        .ok_or_else(|| "answer carried no text content block".to_string())?;

    if answer.get("isError").and_then(Value::as_bool) == Some(true) {
        return Err(text.to_string());
    }
    serde_json::from_str(text).map_err(|e| format!("unreadable payload: {e}"))
}

/// Parse a payload into `T`, naming the tool when it fails so a broken panel says which
/// wire went wrong.
pub fn decode<T: for<'de> Deserialize<'de>>(answer: &Value, tool: &str) -> Result<T, String> {
    let p = payload(answer)?;
    serde_json::from_value(p).map_err(|e| format!("{tool}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn block(text: &str) -> Value {
        json!({"content": [{"type": "text", "text": text}]})
    }

    #[test]
    fn a_status_payload_parses_through_both_layers() {
        let a = block(
            r#"{"contentVersion":25,"epochUnixSeconds":1785258645,"tick":5836,
                "tickDurationSec":300,"worldName":"PROD","worldSeed":"212332557",
                "nextTickAt":"2026-08-17T23:35:45Z","stateHash":"d8f695"}"#,
        );
        let s: Status = decode(&a, "ucf_status").unwrap();
        assert_eq!(s.tick, 5836);
        assert_eq!(s.world_name, "PROD");
        assert_eq!(s.tick_duration_sec, 300);
    }

    /// The lenience that is the point of this module: a field the monitor has never seen
    /// must not cost it the fields it understands.
    #[test]
    fn an_unknown_field_does_not_blank_the_panel() {
        let a = block(
            r#"[{"good":"catnip","station":"cannery-row","mid":21,"stock":444,
                          "somethingJeffAddedLater":{"deeply":["nested"]}}]"#,
        );
        let p: Vec<Price> = decode(&a, "ucf_prices").unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].key(), "catnip@cannery-row");
        assert_eq!(p[0].stock, 444);
    }

    /// A missing field is absence, not corruption — the monitor draws the rest.
    #[test]
    fn a_missing_field_defaults_rather_than_failing() {
        let a = block(r#"[{"loadId":"L744","good":"grain"}]"#);
        let l: Vec<Load> = decode(&a, "ucf_loadboard").unwrap();
        assert_eq!(l[0].load_id, "L744");
        assert!(!l[0].mine);
        assert_eq!(l[0].units, 0);
    }

    /// `isError` is the server speaking. Carry its words; never swallow them into a blank.
    #[test]
    fn a_tool_error_carries_the_servers_own_words() {
        let mut a = block("station is required");
        a["isError"] = json!(true);
        let e = payload(&a).unwrap_err();
        assert_eq!(e, "station is required");
    }

    #[test]
    fn a_carrier_under_way_is_told_from_one_docked() {
        let moving = Carrier {
            location: "-> whisker-hollow".into(),
            ..Default::default()
        };
        let docked = Carrier {
            location: "clawson-drift".into(),
            ..Default::default()
        };
        assert!(moving.under_way());
        assert!(!docked.under_way());
    }
}
