//! **The declaration is the consent** (ADR-0032, worn by MCP).
//!
//! A partner server exists to the familiar because a human wrote it into `mcp/servers.json`
//! in the data dir — the same rule actuators live under. Nothing here is discovered into
//! existence: a server the human never wrote down cannot be reached however loudly it
//! announces itself, and a tool the declaration does not name cannot be *called* however
//! plainly the server offers it.
//!
//! Discovery and permission are deliberately different things. `tools/list` is a question the
//! familiar may always ask a declared server — knowing what is offered is how a human decides
//! what to allow, and refusing to look would only make the declaration harder to write. What
//! `call` requires is the human's own list.
//!
//! The credential never appears here. The declaration names a **key file** — an env-format
//! file beside it, mode 0600 — so a `servers.json` can be read, diffed, or pasted into a
//! message without leaking anything, and the file that holds the secret is the one file that
//! must never be.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::{Error, Url};

/// Where the declaration lives inside the data dir.
pub const SERVERS_FILE: &str = "mcp/servers.json";

/// One declared MCP server.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Server {
    /// The handle a human and the CLI use. Unique within the declaration.
    pub name: String,
    /// Absolute URL of the MCP endpoint.
    pub url: String,
    /// Path (relative to the data dir) of an env-format file holding the bearer token, and
    /// the key to read from it. Absent means an unauthenticated server.
    #[serde(default)]
    pub key_file: String,
    #[serde(default)]
    pub key_name: String,
    /// The tools the human permits calling. **Empty means discovery only** — the familiar may
    /// ask what this server offers and may invoke nothing.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Free-text note for the human's own benefit; never read by any decision.
    #[serde(default)]
    pub note: String,
}

impl Server {
    pub fn endpoint(&self) -> Result<Url, Error> {
        Url::parse(&self.url)
    }

    /// May this tool be invoked? Discovery is not permission.
    pub fn may_call(&self, tool: &str) -> bool {
        self.tools.iter().any(|t| t == tool)
    }

    /// The bearer token, read at the moment it is needed and never stored on the struct — so
    /// a debug-printed declaration cannot spill it.
    pub fn token(&self, dir: &Path) -> Result<Option<String>, Error> {
        if self.key_file.is_empty() {
            return Ok(None);
        }
        let path = dir.join(&self.key_file);
        let raw = std::fs::read_to_string(&path).map_err(|e| {
            Error::Undeclared(format!(
                "{} names key file {} which cannot be read: {e}",
                self.name,
                path.display()
            ))
        })?;
        let key = if self.key_name.is_empty() {
            "TOKEN"
        } else {
            &self.key_name
        };
        for line in raw.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == key {
                    return Ok(Some(v.trim().trim_matches('"').to_string()));
                }
            }
        }
        Err(Error::Undeclared(format!(
            "{} is not in {}",
            key,
            path.display()
        )))
    }
}

/// Every declared server, by name.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerSet {
    #[serde(default)]
    pub servers: Vec<Server>,
}

impl ServerSet {
    /// Load the declaration. An **absent** file is an empty set — no partners, which is the
    /// correct default and not an error. A **malformed** file is an error: a human who wrote
    /// a declaration and got silence would think their server was reachable.
    pub fn load(dir: &Path) -> Result<Self, Error> {
        let path = dir.join(SERVERS_FILE);
        let raw = match std::fs::read_to_string(&path) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(Error::Io(format!("{}: {e}", path.display()))),
        };
        let set: ServerSet = serde_json::from_str(&raw)
            .map_err(|e| Error::Undeclared(format!("{} is malformed: {e}", path.display())))?;
        let mut seen: BTreeMap<&str, ()> = BTreeMap::new();
        for s in &set.servers {
            if seen.insert(&s.name, ()).is_some() {
                return Err(Error::Undeclared(format!(
                    "{} declares two servers named {}",
                    path.display(),
                    s.name
                )));
            }
            s.endpoint()?;
        }
        Ok(set)
    }

    pub fn get(&self, name: &str) -> Result<&Server, Error> {
        self.servers
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| Error::Undeclared(name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, json: &str) {
        std::fs::create_dir_all(dir.join("mcp")).unwrap();
        std::fs::write(dir.join(SERVERS_FILE), json).unwrap();
    }

    /// Absent is empty, malformed is loud, duplicates refuse, and a bad URL is caught at load
    /// rather than at the moment someone is waiting for an answer.
    #[test]
    fn the_declaration_is_read_strictly_and_absence_is_not_an_error() {
        let d = familiar_kernel::testing::temp_root("mcp_decl");
        assert!(ServerSet::load(&d).unwrap().servers.is_empty());

        write(
            &d,
            r#"{"servers":[{"name":"ucf","url":"https://example.test/mcp"}]}"#,
        );
        let set = ServerSet::load(&d).unwrap();
        assert_eq!(set.get("ucf").unwrap().url, "https://example.test/mcp");
        assert!(set.get("nope").is_err());

        write(&d, "{ not json");
        assert!(ServerSet::load(&d).is_err());
        write(
            &d,
            r#"{"servers":[{"name":"a","url":"https://x.test/mcp"},{"name":"a","url":"https://y.test/mcp"}]}"#,
        );
        assert!(ServerSet::load(&d).is_err(), "two servers, one name");
        write(&d, r#"{"servers":[{"name":"a","url":"not-a-url"}]}"#);
        assert!(ServerSet::load(&d).is_err());
        // An invented field is refused: the shape is the contract, as everywhere else.
        write(
            &d,
            r#"{"servers":[{"name":"a","url":"https://x.test/mcp","allow_all":true}]}"#,
        );
        assert!(ServerSet::load(&d).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Discovery is not permission, and the token lives in a file the declaration only points
    /// at — so the declaration itself is safe to read aloud.
    #[test]
    fn a_tool_is_callable_only_if_the_human_named_it() {
        let d = familiar_kernel::testing::temp_root("mcp_perm");
        write(
            &d,
            r#"{"servers":[{"name":"ucf","url":"https://example.test/mcp",
                 "key_file":"mcp/ucf.env","key_name":"UCF_TOKEN","tools":["list_products"]}]}"#,
        );
        std::fs::write(
            d.join("mcp/ucf.env"),
            "# a comment\nUCF_SERVER=https://example.test\nUCF_TOKEN=ucfk_secret\n",
        )
        .unwrap();
        let set = ServerSet::load(&d).unwrap();
        let s = set.get("ucf").unwrap();
        assert!(s.may_call("list_products"));
        assert!(!s.may_call("place_order"), "undeclared stays uncallable");
        assert_eq!(s.token(&d).unwrap().as_deref(), Some("ucfk_secret"));
        // The struct never carries the secret, so a debug print cannot spill it.
        assert!(!format!("{s:?}").contains("ucfk_secret"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
