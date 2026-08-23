//! The signed, expiring boundary projection (ADR-0045 §5).
//!
//! Purr owns no independently editable boundary. Three layers: the ONE human-owned root
//! boundary; this lease — a signed projection of it a ship process may hold; and
//! ship-local per-capability grants that can only narrow (future ADR-0044 machinery).
//! Stale, malformed, wrongly-signed, or missing → every consequential call fails closed.
//!
//! The signature covers the exact serialized lease bytes, carried verbatim — no
//! canonicalization step to disagree about (the 1-ULP longitude lesson, 2026-08-13).

use serde::{Deserialize, Serialize};

use crate::Error;
use familiar_kernel::boundary::Boundary;
use familiar_mesh::node::{NodeIdentity, NodeKey};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryLease {
    /// Which world instance this projection was issued to.
    pub instance: String,
    /// The projection itself: the root boundary's truth at issue time. Shared truth,
    /// instance-scoped authority — the ship may only ever narrow it, never widen.
    pub boundary: Boundary,
    pub issued_at: i64,
    /// Hard stop. After this instant the lease authorizes nothing.
    pub expires_at: i64,
}

/// A lease as it travels and rests: the exact signed bytes plus the signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedLease {
    /// `BoundaryLease` as JSON — the byte-exact message that was signed.
    pub lease_json: String,
    /// Hex ed25519 signature by the household node key.
    pub sig: String,
}

/// Issue a lease over the CURRENT root boundary. `ttl_secs` is clamped to at least 1 —
/// a non-expiring lease is not a lease.
pub fn issue(
    root: &Boundary,
    instance: &str,
    ttl_secs: i64,
    now: i64,
    signer: &NodeKey,
) -> Result<SignedLease, Error> {
    let lease = BoundaryLease {
        instance: instance.to_string(),
        boundary: root.clone(),
        issued_at: now,
        expires_at: now + ttl_secs.max(1),
    };
    let lease_json = serde_json::to_string(&lease).map_err(|e| Error::Io(e.to_string()))?;
    let sig = signer.sign(lease_json.as_bytes());
    Ok(SignedLease { lease_json, sig })
}

/// Verify a lease against the household issuer and the clock. Everything wrong —
/// signature, parse, expiry, wrong instance — is a refusal; the caller's only honest
/// reaction is to not act.
pub fn verify(
    signed: &SignedLease,
    issuer: &NodeIdentity,
    instance: &str,
    now: i64,
) -> Result<BoundaryLease, Error> {
    issuer
        .verify(signed.lease_json.as_bytes(), &signed.sig)
        .map_err(|e| Error::Refused(format!("lease signature: {e}")))?;
    let lease: BoundaryLease = serde_json::from_str(&signed.lease_json)
        .map_err(|e| Error::Refused(format!("lease is malformed: {e}")))?;
    if lease.instance != instance {
        return Err(Error::Refused(
            "lease was issued to another instance".into(),
        ));
    }
    if now > lease.expires_at {
        return Err(Error::Refused(
            "lease expired — refresh before acting".into(),
        ));
    }
    Ok(lease)
}

/// The fail-closed floor a ship process actually calls before a consequential act: no
/// lease, bad lease, stale lease, or a shut gate all answer the same word. `gate` reads
/// the projected boundary (e.g. `|b| b.allow_network`).
pub fn permits(
    signed: Option<&SignedLease>,
    issuer: &NodeIdentity,
    instance: &str,
    now: i64,
    gate: impl Fn(&Boundary) -> bool,
) -> bool {
    match signed {
        Some(s) => match verify(s, issuer, instance, now) {
            Ok(lease) => gate(&lease.boundary),
            Err(_) => false,
        },
        None => false,
    }
}
