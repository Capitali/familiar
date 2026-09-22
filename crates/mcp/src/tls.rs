//! **Outbound TLS that verifies** — and refuses rather than degrading.
//!
//! `crates/mesh` dials with a deliberately *opportunistic* config: it accepts any server
//! certificate, because in the covenant mesh the authenticity lives in the ed25519 payload
//! signature and TLS is only there to encrypt. That posture is correct there and would be a
//! credential leak here. An MCP request carries a **bearer token** and no signature of its
//! own, so the certificate is the only thing standing between that token and whoever answers
//! the address. Accept-any would hand a partner's key to a middlebox, silently, on the first
//! call.
//!
//! So this module builds a verifying root store from the platform trust bundle, and if it
//! cannot find one it **fails** — it never falls back to the permissive config. A capability
//! that cannot be exercised safely is not exercised: the same shape as `boundary::load`
//! falling back to `closed()`.

use std::sync::Arc;

use crate::Error;

/// Where each platform keeps its CA bundle. Checked in order; the first that parses wins.
const BUNDLES: &[&str] = &[
    "/etc/ssl/cert.pem",                  // macOS, some BSDs
    "/etc/ssl/certs/ca-certificates.crt", // Debian/Ubuntu
    "/etc/pki/tls/certs/ca-bundle.crt",   // RHEL/Fedora
    "/etc/ssl/ca-bundle.pem",             // SUSE
    "/usr/local/etc/ssl/cert.pem",        // homebrew openssl
];

/// A verifying client config built from the platform trust store.
///
/// `Err` when no bundle can be found or none of them yields a usable root — the caller must
/// then refuse the call, not soften the check.
pub fn verifying_config() -> Result<Arc<rustls::ClientConfig>, Error> {
    ensure_crypto_provider();
    let mut roots = rustls::RootCertStore::empty();
    let mut looked = Vec::new();
    for path in BUNDLES {
        looked.push(*path);
        let Ok(raw) = std::fs::read(path) else {
            continue;
        };
        let Ok(entries) = pem::parse_many(&raw) else {
            continue;
        };
        for e in entries {
            if e.tag() != "CERTIFICATE" {
                continue;
            }
            // A single unparseable certificate in a bundle of hundreds is not a reason to
            // refuse the whole store; an EMPTY store is, and that is checked below.
            let _ = roots.add(rustls::pki_types::CertificateDer::from(
                e.contents().to_vec(),
            ));
        }
        if !roots.is_empty() {
            break;
        }
    }
    if roots.is_empty() {
        return Err(Error::NoTrustStore(format!(
            "no usable CA bundle found (looked in {}) — refusing to send a credential to an \
             unverified server",
            looked.join(", ")
        )));
    }
    Ok(Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ))
}

/// rustls 0.23 wants a process-wide crypto provider installed once. Same dance as
/// `mesh::transport::ensure_crypto_provider` — a second install is a no-op, not an error.
fn ensure_crypto_provider() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The platform store is found and is not empty. If this ever fails on a machine, the
    /// right answer is to add its bundle path above — never to relax the verifier.
    #[test]
    fn the_platform_trust_store_is_found_and_verifying() {
        let cfg = verifying_config().expect("a CA bundle on this machine");
        // The config is built with root certificates rather than a custom verifier: rustls
        // gives no accessor for that, so the assertion is structural — this module has no
        // `dangerous()` call in it at all, which is what the next reader needs to know.
        assert!(Arc::strong_count(&cfg) >= 1);
        // Structural, because rustls exposes no accessor for "is this verifying": the
        // permissive path is reached by calling `.dangerous()`, so the assertion is that no
        // such call exists in this module's own code.
        let src = include_str!("tls.rs");
        let code = src.split("#[cfg(test)]").next().unwrap_or(src);
        assert_eq!(
            code.matches(".dangerous(").count(),
            0,
            "this module must never reach for rustls' permissive configuration"
        );
    }
}
