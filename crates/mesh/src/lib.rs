//! `familiar-mesh` — peer federation over the tailnet.
//!
//! When a familiar runs on more than one node, this crate lets those nodes **find each
//! other, prove they belong to the same group, and share what they've learned** — tool
//! solutions, abstract patterns, and (only when separately opted-in) knowledge of the
//! humans they serve. It exists in service of the Three Laws: a wider, corroborated
//! picture of who is served (Laws I/II), better tools spread across nodes (Law I), and a
//! node never turned against people by a bad peer (Law III — trust is cryptographic, not
//! ambient, and the human still owns the `allow_mesh` gate).
//!
//! ## The one place dependency-minimalism is relaxed
//!
//! The rest of the workspace is deliberately serde-only: a small, legible trust surface is
//! part of the Law III commitment. Native mesh transport was the chosen architecture, so
//! **this crate — and only this crate** — takes on a crypto floor (`ed25519-dalek`,
//! `sha2`, `getrandom`) and, in the transport module, an async HTTP stack. The kernel,
//! cycle, and CLI never inherit these: they call this crate through **synchronous** entry
//! points and let the async transport run on its own background thread. The concession is
//! named here so it stays visible, not hidden.
//!
//! ## Transport vs. trust+merge split
//!
//! The constitutional decisions stay inside the synchronous, auditable tick. The async
//! transport verifies signatures at ingress and writes verified briefs to `mesh/inbox`,
//! but **never mutates the canonical stores**; the merge into observations/tools/patterns
//! happens in the tick (`merge::drain_inbox`), governed by the same metabolism and the
//! boundary. See `docs/mesh.md`.

#![forbid(unsafe_code)]

pub mod brief;
pub mod group;
pub mod node;

use std::fmt;

/// The mesh error type. Kept deliberately coarse — callers log the rationale and record it
/// as an observation; there is no fine-grained recovery beyond "reject this brief".
#[derive(Debug)]
pub enum Error {
    /// Filesystem / serialization trouble reading or writing the `mesh/` state.
    Io(std::io::Error),
    /// A malformed or wrong-length key / signature / hex payload.
    Malformed(String),
    /// A signature or membership certificate did not verify, is expired, or is revoked.
    /// The most security-relevant variant: a peer failed to prove it belongs to the group.
    Untrusted(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "mesh io: {e}"),
            Error::Malformed(s) => write!(f, "mesh malformed: {s}"),
            Error::Untrusted(s) => write!(f, "mesh untrusted: {s}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Malformed(format!("json: {e}"))
    }
}

/// Result alias for mesh operations.
pub type Result<T> = std::result::Result<T, Error>;

// ---- small hex helpers (kept dependency-free) --------------------------------------

/// Lower-case hex encoding of a byte slice.
pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    s
}

/// Decode a lower/upper-case hex string into bytes. Rejects odd length / non-hex.
pub(crate) fn hex_decode(s: &str) -> Result<Vec<u8>> {
    let s = s.trim();
    if !s.len().is_multiple_of(2) {
        return Err(Error::Malformed("hex: odd length".into()));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char)
            .to_digit(16)
            .ok_or_else(|| Error::Malformed("hex: non-hex digit".into()))?;
        let lo = (bytes[i + 1] as char)
            .to_digit(16)
            .ok_or_else(|| Error::Malformed("hex: non-hex digit".into()))?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Ok(out)
}

/// Fill a fixed-size buffer with OS randomness (for minting keys).
pub(crate) fn os_random<const N: usize>() -> Result<[u8; N]> {
    let mut buf = [0u8; N];
    getrandom::getrandom(&mut buf).map_err(|e| Error::Malformed(format!("getrandom: {e}")))?;
    Ok(buf)
}

/// Exactly-32-byte view of a slice, or a `Malformed` error.
pub(crate) fn exactly_32(bytes: &[u8], what: &str) -> Result<[u8; 32]> {
    bytes
        .try_into()
        .map_err(|_| Error::Malformed(format!("{what}: expected 32 bytes, got {}", bytes.len())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_and_rejects_junk() {
        let data = [0x00u8, 0x0f, 0xf0, 0xab, 0xff];
        let s = hex_encode(&data);
        assert_eq!(s, "000ff0abff");
        assert_eq!(hex_decode(&s).unwrap(), data);
        assert!(hex_decode("abc").is_err()); // odd length
        assert!(hex_decode("zz").is_err()); // non-hex
    }

    #[test]
    fn os_random_is_nonzero_and_varies() {
        let a: [u8; 32] = os_random().unwrap();
        let b: [u8; 32] = os_random().unwrap();
        assert_ne!(a, [0u8; 32]);
        assert_ne!(a, b, "two draws should differ");
    }
}
