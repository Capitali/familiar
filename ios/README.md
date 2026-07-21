# The Familiar — Apple shells (consoles + device agents)

The SwiftUI apps for every Apple platform — since 2026-07-17 the **primary human interface**
to the familiar (superseding the egui Glass; see
[ADR-0007](../docs/decision-records/0007-one-core-many-shells.md)). Each app enrolls into the
familiar's mesh by the covenant handshake, pushes **derived observations** (never raw data) to
`POST /mesh/observe`, and reads the familiar's **worldview** (`/mesh/worldview`, loopback
`/local/worldview`) — the standard dark console: the Three Laws as live meters, the roster +
mesh map, the Roadmap board, the Metal sphere/orb, and dialog with the familiar + gate
control. All signed with the same ed25519 trust the mesh uses.

Still ahead: HealthKit, audio/imagery, a voice-first UI, and ADR-0007 phase 1 (replacing the
Swift wire reimplementation with UniFFI bindings over the Rust core).

## Layout

- `FamiliarMesh/` — a Swift package (macOS + iOS + watchOS + tvOS) with the crypto + wire
  logic: CryptoKit ed25519 keypair, membership-cert minting (byte-matched to the Rust
  `CertBody` canonicalization), the `/mesh/observe` + worldview clients, and the shared
  `FamiliarSphereView` (SceneKit marble/mesh/globe). **Unit-tested on macOS** — no device needed.
- `App/` — the iPhone/iPad SwiftUI app: enroll / consent / status, the `SensingCoordinator`
  (CoreLocation + CoreMotion → derived `ObsRecord`s), and the universal console.
- `MacApp/` — the native macOS console (FamiliarMac).
- `Watch/` — the watchOS companion (enrolls via WatchConnectivity address handoff).
- `Shared/` — the canonical design system (`FamiliarUI.swift`: dark-sphere theme,
  width-driven layout, Marble/Panel/CycleRing/MeshConstellation).
- `project.yml` — the [XcodeGen](https://github.com/yonaskolb/XcodeGen) spec. The `.xcodeproj` is
  generated, not hand-maintained (and git-ignored). TestFlight notes: [TESTFLIGHT.md](TESTFLIGHT.md).

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
