//! Content-addressed candidate manifests.
//!
//! Every file a generation outcome carries is named by a relative,
//! traversal-free path and a sha256 digest. Once a manifest is accepted the
//! artifacts it names are write-once: the digest is the identity, and every
//! later oracle verdict cites digests so evidence can never drift from what
//! actually ran.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// What a file in a candidate is for. The runner uses roles to decide what is
/// executable at all; everything else is inert data to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileRole {
    /// The module(s) implementing the ordered capability.
    Source,
    /// The candidate's own tests — the bench oracle runs exactly these.
    SelfTest,
    /// Recorded fixtures (frames, transcripts) the tests replay.
    Fixture,
    /// Human-readable notes the candidate ships about itself.
    Doc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Relative, traversal-free path inside the candidate tree.
    pub path: String,
    /// Lowercase hex sha256 of the file's exact bytes.
    pub digest: String,
    pub role: FileRole,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub files: Vec<FileEntry>,
}

/// Why a manifest was refused. Refusal is the normal fate of hostile or
/// sloppy generation output — the workshop never repairs a manifest, it
/// rejects it and the refusal earns a ledger row upstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    EmptyManifest,
    EmptyPath,
    AbsolutePath(String),
    Traversal(String),
    DuplicatePath(String),
    BadDigest(String),
    /// Windows-style separators and drive letters are refused outright
    /// rather than normalized — normalization is where escapes hide.
    ForeignSeparator(String),
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::EmptyManifest => write!(f, "manifest names no files"),
            ManifestError::EmptyPath => write!(f, "manifest entry with empty path"),
            ManifestError::AbsolutePath(p) => write!(f, "absolute path refused: {p}"),
            ManifestError::Traversal(p) => write!(f, "path traversal refused: {p}"),
            ManifestError::DuplicatePath(p) => write!(f, "duplicate path: {p}"),
            ManifestError::BadDigest(p) => write!(f, "digest is not 64 lowercase hex chars: {p}"),
            ManifestError::ForeignSeparator(p) => {
                write!(f, "foreign path separator or drive refused: {p}")
            }
        }
    }
}

/// sha256 of raw bytes as lowercase hex — the one digest used everywhere in
/// the factory (manifests, outcomes, evidence, proposed declarations).
pub fn digest_bytes(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(64);
    for b in out {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn digest_ok(d: &str) -> bool {
    d.len() == 64 && d.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Validate a manifest fail-closed. Every entry must be a relative forward-
/// slash path with no `.`/`..` segments, no backslashes or drive colons, a
/// well-formed digest, and no path may appear twice.
pub fn validate(m: &Manifest) -> Result<(), ManifestError> {
    if m.files.is_empty() {
        return Err(ManifestError::EmptyManifest);
    }
    let mut seen = std::collections::BTreeSet::new();
    for e in &m.files {
        let p = e.path.as_str();
        if p.is_empty() {
            return Err(ManifestError::EmptyPath);
        }
        if p.starts_with('/') || p.starts_with('~') {
            return Err(ManifestError::AbsolutePath(p.to_string()));
        }
        if p.contains('\\') || p.contains(':') {
            return Err(ManifestError::ForeignSeparator(p.to_string()));
        }
        if p.split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..")
        {
            return Err(ManifestError::Traversal(p.to_string()));
        }
        if !digest_ok(&e.digest) {
            return Err(ManifestError::BadDigest(p.to_string()));
        }
        if !seen.insert(p.to_string()) {
            return Err(ManifestError::DuplicatePath(p.to_string()));
        }
    }
    Ok(())
}

/// The manifest's own identity: sha256 over its canonical JSON.
pub fn manifest_digest(m: &Manifest) -> String {
    let json = serde_json::to_vec(m).unwrap_or_default();
    digest_bytes(&json)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            digest: digest_bytes(path.as_bytes()),
            role: FileRole::Source,
        }
    }

    #[test]
    fn a_clean_manifest_validates() {
        let m = Manifest {
            files: vec![entry("driver/sp548e.py"), entry("tests/test_frames.py")],
        };
        assert!(validate(&m).is_ok());
    }

    #[test]
    fn an_empty_manifest_is_refused() {
        assert_eq!(
            validate(&Manifest::default()),
            Err(ManifestError::EmptyManifest)
        );
    }

    #[test]
    fn traversal_and_absolute_paths_are_refused() {
        for bad in [
            "../escape.py",
            "a/../../b.py",
            "/etc/passwd",
            "~/x.py",
            "a//b.py",
            "./a.py",
            "a/./b.py",
        ] {
            let m = Manifest {
                files: vec![entry(bad)],
            };
            assert!(validate(&m).is_err(), "{bad} should be refused");
        }
    }

    #[test]
    fn foreign_separators_are_refused_not_normalized() {
        for bad in ["a\\b.py", "C:whatever.py"] {
            let m = Manifest {
                files: vec![entry(bad)],
            };
            assert_eq!(
                validate(&m),
                Err(ManifestError::ForeignSeparator(bad.to_string())),
                "{bad}"
            );
        }
    }

    #[test]
    fn duplicate_paths_and_bad_digests_are_refused() {
        let m = Manifest {
            files: vec![entry("a.py"), entry("a.py")],
        };
        assert_eq!(
            validate(&m),
            Err(ManifestError::DuplicatePath("a.py".into()))
        );

        let mut e = entry("b.py");
        e.digest = "DEADBEEF".into();
        let m = Manifest { files: vec![e] };
        assert_eq!(validate(&m), Err(ManifestError::BadDigest("b.py".into())));
    }

    #[test]
    fn digests_are_stable_and_hex() {
        let d = digest_bytes(b"motorlight");
        assert_eq!(d.len(), 64);
        assert_eq!(d, digest_bytes(b"motorlight"));
        assert_ne!(d, digest_bytes(b"motorlite"));
    }
}
