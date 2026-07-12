# Familiar Agent (iOS)

A lightweight **mesh sensor agent** for the [familiar](../familiar). It enrolls into the familiar's
mesh with a scanned/pasted join key, then pushes **derived observations** (never raw data) to the
familiar's `POST /mesh/observe` endpoint, signed with the same ed25519 trust the mesh uses.

Phase 1 (this scaffold): iPhone, **location (home/away)** + **motion (walking/driving/still)**.
Later phases add HealthKit, Apple Watch, audio/imagery, and a voice + iconographic UI. See the plan:
`~/.claude/plans/tingly-foraging-quail.md`.

## Layout

- `FamiliarMesh/` — a Swift package (macOS + iOS + watchOS) with the crypto + wire logic:
  CryptoKit ed25519 keypair, membership-cert minting (byte-matched to the Rust `CertBody`
  canonicalization), the `/mesh/observe` client. **Unit-tested on macOS** — no device needed.
- `App/` — the SwiftUI app: enroll / consent / status + the `SensingCoordinator` (CoreLocation +
  CoreMotion → derived `ObsRecord`s).
- `project.yml` — the [XcodeGen](https://github.com/yonaskolb/XcodeGen) spec. The `.xcodeproj` is
  generated, not hand-maintained (and git-ignored).

## Build & test

```sh
# crypto/wire unit tests + Rust conformance (headless, no device):
cd FamiliarMesh && swift test

# generate the Xcode project and build for the simulator (no signing needed):
xcodegen
xcodebuild -project FamiliarAgent.xcodeproj -scheme FamiliarAgent \
  -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' \
  CODE_SIGNING_ALLOWED=NO build

# to run on your device: open FamiliarAgent.xcodeproj, pick your device, Run.
```

Provisioning: team **8GHXL328AR**, bundle **io.river.familiar.ios**, automatic signing (set in
`project.yml`). Phase 1 needs only Location (Always) + Motion — no special capabilities. HealthKit
(Phase 2) will add the HealthKit capability + entitlement.

## Enroll a device

1. On the familiar host: `familiar mesh accept-observations on` (default on), then `familiar mesh qr`
   (prints the enrollment payload; renders a scannable QR if `qrencode` is installed).
2. In the app, paste the payload and tap **Enroll**. The device mints its membership cert locally
   from the group secret; nothing is sent until you toggle a sensor on.
3. Walk around → `phone at location:away`, `phone motion:walking` appear in the familiar's
   observations, tagged `source=mesh:<device-node-id>`.
4. Lost device? Revoke it by `node_id` in the familiar's `mesh/revoked.json`.

## The wire contract (what FamiliarMesh implements)

- ed25519 (CryptoKit `Curve25519.Signing`); `node_id = hex(SHA256(pubkey)[..8])`.
- Membership cert = group-secret signature over the **compact** JSON
  `{"node_id","node_pubkey","issued","expiry","group_id"}` (that field order, integers unquoted) —
  the one thing that must byte-match Rust. Pinned by `CertConformanceTests` against a golden vector
  from `cargo run -p familiar-mesh --example cert_vector`.
- Batch = `ObserveEnvelope{node,membership,ts,nonce,observations}` POSTed as JSON; the ed25519
  signature over the **raw body bytes** goes in the `X-Familiar-Sig` header (so there's no payload
  canonicalization to match). Server enforces a ±5-min `ts` window + nonce anti-replay.
