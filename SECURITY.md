# Security Policy

The Familiar is a long-running autonomous process with **unrestricted local and
network reach** by design. Its restraint is constitutional, not technical (it
sends no telemetry and exfiltrates nothing — see [SOUL.md](docs/SOUL.md), Law III
and "restraint is constitutional"). Security is therefore a first-class concern,
and a vulnerability here is, in the project's own terms, a path by which the
factory could "be turned against the served."

## Reporting a vulnerability

Please report privately, **not** via a public issue:

- Use GitHub's **private vulnerability reporting** ("Report a vulnerability" under
  the Security tab), or
- email **ian@river.io** with `[familiar-security]` in the subject.

Include what you found, how to reproduce it, and the impact you foresee. You will
get an acknowledgement; please allow reasonable time to remediate before any public
disclosure.

## Supported versions

Pre-1.0. Only the tip of `main` is supported; there are no maintenance branches yet.

## Design commitments that bear on security

- **Memory safety as constitution.** The kernel (`crates/kernel`) carries
  `#![forbid(unsafe_code)]` — the Law III commitment made literal. A memory-safety
  defect in an unrestricted-reach agent is exactly the kind of "turned against the
  served" failure Law III forbids.
- **Minimal trust surface.** Dependencies are kept deliberately small (currently
  `serde`/`serde_json` only). See [security/dependency-review.md](security/dependency-review.md).
- **No exfiltration.** The familiar does not phone home. See
  [security/privacy-review.md](security/privacy-review.md) and
  [security/threat-model.md](security/threat-model.md).

## Private keys in the repository (git-crypt)

An App Store Connect `.p8` key was once committed at the repo root in plaintext
(issue #3). That key was revoked and rotated in App Store Connect on 2026-08-08;
because the repo is public, the leaked key is treated as compromised forever —
rotation, not history rewriting, is the mitigation.

The standing policy, in layers:

1. **Keys live outside the tree.** Apple API/auth keys sit in
   `~/.appstoreconnect/private_keys/` and tooling (`ios/tools/ship.sh`)
   references them by absolute path. CI receives keys from its secret store,
   never from the repo.
2. **`.gitignore` blocks the class**: `*.p8`, `*.pem`, `key.env`.
3. **git-crypt is the backstop.** `.gitattributes` marks `secrets/**` and any
   force-added key material with `filter=git-crypt`, so it commits encrypted,
   never plaintext. Run `git-crypt init` once per clone that will commit
   secrets; the repo's symmetric key is exported out-of-band
   (`git-crypt export-key <path outside the repo>`) and shared with
   collaborators via `git-crypt add-gpg-user` or an out-of-band channel —
   never through the repo itself.
4. **If a credential must ride the repo**, it goes under `secrets/` (the one
   `.gitignore`-exempt, always-encrypted path). Verify with
   `git-crypt status -e` before pushing.

See [security/](security/) for the full threat model, data classification, and reviews.
