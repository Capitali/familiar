//! **Audience classification for a worldview read** (T-217; conduct dialogue Q6, DECIDED).
//!
//! Ian's ruling (2026-08-20, verbatim): *"no more names visible. They still must be present
//! in the data, but it addresses, human names, and internal network names need to only be
//! displayed for devices in the local network or owned by the human."*
//!
//! The membership cert decides that a node may READ (that gate is upstream of this module
//! and unchanged). This module decides what the reader is shown, and it fails closed:
//!
//! - [`Audience::Owned`] — the reading device's record carries an established identity: it
//!   is owned by a human of this household, and sees full names **from any network**.
//! - [`Audience::HouseholdLan`] — an un-established member reading from the door's own
//!   machine (loopback) or a network the household has DECLARED as its LAN
//!   (`household_lan_cidrs`). "LAN" is a configured fact — never an RFC1918 guess, and
//!   never a forwarded header (the proxy-is-not-a-neighbour lesson, `8ecb41b`, kept):
//!   `peer_ip` here is the accepted connection's real source, and with no CIDRs configured
//!   nothing widens.
//! - [`Audience::Federated`] — a valid member satisfying neither: the masked view
//!   (`standing::to_guest_view`), deliberate roles instead of names.
//!
//! MCP partners never reach this seam at all — their surface is the covenant catalog
//! (T-216), structurally, whatever network they connect from. Source network is context:
//! it can widen display AFTER the membership gate passed, never substitute for it.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    Owned,
    HouseholdLan,
    Federated,
}

/// Classify a verified reader. `node_id` has already passed the membership gate; `peer_ip`
/// is the real source address of the accepted connection (never a header).
pub fn classify(dir: &Path, node_id: &str, peer_ip: &str) -> Audience {
    if crate::standing::standing_of(dir, node_id) == crate::standing::Standing::Full {
        return Audience::Owned;
    }
    if peer_ip == "127.0.0.1" || peer_ip == "::1" {
        return Audience::HouseholdLan; // the door's own machine is the household
    }
    let cidrs = crate::config::load(dir)
        .map(|c| c.household_lan_cidrs)
        .unwrap_or_default();
    if cidrs.iter().any(|c| cidr_match(c, peer_ip)) {
        return Audience::HouseholdLan;
    }
    Audience::Federated
}

/// One declared network against one source address. IPv4 `a.b.c.d/n` or an exact address
/// (v4 or v6) — anything unparseable matches nothing, so a typo narrows rather than widens.
fn cidr_match(cidr: &str, ip: &str) -> bool {
    let cidr = cidr.trim();
    let Some((net, bits)) = cidr.split_once('/') else {
        return !cidr.is_empty() && cidr == ip.trim();
    };
    let (Some(n), Some(a)) = (v4(net), v4(ip)) else {
        return false;
    };
    let Ok(bits) = bits.parse::<u32>() else {
        return false;
    };
    if bits > 32 {
        return false;
    }
    let mask = if bits == 0 {
        0
    } else {
        u32::MAX << (32 - bits)
    };
    (n & mask) == (a & mask)
}

fn v4(s: &str) -> Option<u32> {
    s.trim().parse::<std::net::Ipv4Addr>().ok().map(u32::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("familiar_viewer_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Ian's ruling, both halves: an OWNED device keeps the names from any network, and a
    /// merely-private source address never widens an un-established reader.
    #[test]
    fn owned_keeps_names_off_lan_and_a_spoofed_private_address_never_widens() {
        let dir = fresh("classes");
        // An established device (full standing) reading from a foreign network: Owned.
        crate::standing::grant(&dir, "owned-device-0001", "").unwrap();
        assert_eq!(
            classify(&dir, "owned-device-0001", "203.0.113.9"),
            Audience::Owned,
            "owned by the human → full names from any network"
        );
        // An un-established member from an UNDECLARED private range: masked. RFC1918 is
        // not a household.
        assert_eq!(
            classify(&dir, "guest-device-0001", "192.168.1.9"),
            Audience::Federated
        );
        // The same reader once the household DECLARES its LAN: widened.
        let mut cfg = crate::config::load(&dir).unwrap_or_default();
        cfg.household_lan_cidrs = vec!["192.168.1.0/24".into()];
        std::fs::create_dir_all(dir.join("mesh")).unwrap();
        std::fs::write(
            dir.join(crate::config::CONFIG_FILE),
            serde_json::to_string(&cfg).unwrap(),
        )
        .unwrap();
        assert_eq!(
            classify(&dir, "guest-device-0001", "192.168.1.9"),
            Audience::HouseholdLan
        );
        // …and an address outside the declaration stays masked.
        assert_eq!(
            classify(&dir, "guest-device-0001", "192.168.2.9"),
            Audience::Federated
        );
        // The door's own machine is the household.
        assert_eq!(
            classify(&dir, "guest-device-0001", "127.0.0.1"),
            Audience::HouseholdLan
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_declared_lan_matches_and_a_private_range_alone_never_widens() {
        assert!(cidr_match("192.168.108.0/24", "192.168.108.44"));
        assert!(!cidr_match("192.168.108.0/24", "192.168.109.44"));
        assert!(cidr_match("10.1.2.3", "10.1.2.3"), "exact address form");
        // RFC1918 is NOT a household: without a declaration, nothing matches.
        assert!(!cidr_match("", "10.0.0.5"));
        // Unparseable declarations narrow, never widen.
        assert!(!cidr_match("192.168.108.0/oops", "192.168.108.44"));
        assert!(!cidr_match("192.168.108.0/40", "192.168.108.44"));
        assert!(!cidr_match("not-a-net/24", "192.168.108.44"));
    }
}
