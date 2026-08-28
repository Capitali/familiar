//! Materialize a validated candidate's manifest into a scratch tree.
//!
//! The manifest names files by path + sha256 digest; the bytes come from a
//! content-addressed store the generation adapter populated. Materialization
//! writes each file and **re-verifies** its digest, so a store that returns
//! the wrong bytes for a digest is caught here rather than run. Paths were
//! already validated traversal-free by `manifest::validate`, but we re-check
//! before writing — defense in depth, since this is where bytes hit the disk.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use familiar_workshop::manifest::{digest_bytes, Manifest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterializeError {
    /// The store has no bytes for a digest the manifest names.
    MissingArtifact {
        path: String,
        digest: String,
    },
    /// The store's bytes for a digest do not hash to that digest.
    DigestMismatch {
        path: String,
    },
    /// A path escaped the destination (should have been caught by manifest
    /// validation; re-checked here).
    UnsafePath(String),
    Io(String),
}

impl std::fmt::Display for MaterializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MaterializeError::MissingArtifact { path, digest } => {
                write!(f, "no artifact bytes for {path} (digest {digest})")
            }
            MaterializeError::DigestMismatch { path } => {
                write!(f, "artifact bytes for {path} do not match their digest")
            }
            MaterializeError::UnsafePath(p) => write!(f, "unsafe path refused: {p}"),
            MaterializeError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for MaterializeError {}

fn safe_join(dest: &Path, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() || rel.starts_with('/') || rel.contains('\\') || rel.contains(':') {
        return None;
    }
    let mut out = dest.to_path_buf();
    for seg in rel.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return None;
        }
        out.push(seg);
    }
    Some(out)
}

/// Write every file the manifest names into `dest`, verifying digests. The
/// `artifacts` map is digest → bytes (the content-addressed store).
pub fn materialize(
    manifest: &Manifest,
    artifacts: &BTreeMap<String, Vec<u8>>,
    dest: &Path,
) -> Result<(), MaterializeError> {
    std::fs::create_dir_all(dest).map_err(|e| MaterializeError::Io(e.to_string()))?;
    for entry in &manifest.files {
        let bytes =
            artifacts
                .get(&entry.digest)
                .ok_or_else(|| MaterializeError::MissingArtifact {
                    path: entry.path.clone(),
                    digest: entry.digest.clone(),
                })?;
        if digest_bytes(bytes) != entry.digest {
            return Err(MaterializeError::DigestMismatch {
                path: entry.path.clone(),
            });
        }
        let target = safe_join(dest, &entry.path)
            .ok_or_else(|| MaterializeError::UnsafePath(entry.path.clone()))?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| MaterializeError::Io(e.to_string()))?;
        }
        std::fs::write(&target, bytes).map_err(|e| MaterializeError::Io(e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use familiar_workshop::manifest::{FileEntry, FileRole};

    fn artifact(map: &mut BTreeMap<String, Vec<u8>>, bytes: &[u8]) -> String {
        let d = digest_bytes(bytes);
        map.insert(d.clone(), bytes.to_vec());
        d
    }

    #[test]
    fn it_writes_verified_files() {
        let dir = std::env::temp_dir().join(format!("familiar-mat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut store = BTreeMap::new();
        let src = artifact(&mut store, b"print('driver')\n");
        let test = artifact(&mut store, b"assert True\n");
        let manifest = Manifest {
            files: vec![
                FileEntry {
                    path: "sp548e.py".into(),
                    digest: src,
                    role: FileRole::Source,
                },
                FileEntry {
                    path: "tests/test_frames.py".into(),
                    digest: test,
                    role: FileRole::SelfTest,
                },
            ],
        };
        materialize(&manifest, &store, &dir).expect("materialize");
        assert_eq!(
            std::fs::read_to_string(dir.join("sp548e.py")).unwrap(),
            "print('driver')\n"
        );
        assert!(dir.join("tests/test_frames.py").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_artifact_is_refused() {
        let dir = std::env::temp_dir().join(format!("familiar-mat2-{}", std::process::id()));
        let manifest = Manifest {
            files: vec![FileEntry {
                path: "x.py".into(),
                digest: digest_bytes(b"nope"),
                role: FileRole::Source,
            }],
        };
        let err = materialize(&manifest, &BTreeMap::new(), &dir).unwrap_err();
        assert!(matches!(err, MaterializeError::MissingArtifact { .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tampered_bytes_are_caught_by_the_digest() {
        let dir = std::env::temp_dir().join(format!("familiar-mat3-{}", std::process::id()));
        let mut store = BTreeMap::new();
        // Claim a digest but store different bytes under it.
        let claimed = digest_bytes(b"honest");
        store.insert(claimed.clone(), b"TAMPERED".to_vec());
        let manifest = Manifest {
            files: vec![FileEntry {
                path: "x.py".into(),
                digest: claimed,
                role: FileRole::Source,
            }],
        };
        let err = materialize(&manifest, &store, &dir).unwrap_err();
        assert!(matches!(err, MaterializeError::DigestMismatch { .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
