# The Familiar's consoles & device agents (Swift)

Everything human-facing lives here: the **FamiliarMac** sphere console, the
**iPhone/iPad agent** (which hosts the same sphere console), and the **watch app**.
Devices enroll into the familiar's mesh by scanning a QR, then push **derived
observations** (never raw data) to `POST /mesh/observe`, signed with the same
ed25519 trust the mesh uses.

## Layout

- `MacApp/` — **FamiliarMac**, the macOS sphere console: a WKWebView hosting the
  shared web bundle (`MacApp/Resources/sphere/index.html` — satellite globe,
  hologram screens, the invite QR on the Device screen) over a native MKMapView
  street layer. Talks to the local daemon on the loopback seam (`:47101`).
- `App/` — the SwiftUI iPhone/iPad agent: enroll (scan/paste), consent switches,
  sensing (CoreLocation + CoreMotion + optional voice/face), and the same sphere
  console rendered from the shared web bundle (worldview read over the mesh).
- `Watch/` — the watchOS companion (enrols via the paired phone's identity).
- `FamiliarMesh/` — a Swift package (macOS + iOS + watchOS) with the crypto +
  wire logic: CryptoKit ed25519, membership-cert minting (byte-matched to the
  Rust `CertBody` canonicalization), the `/mesh/observe` client.
  **Unit-tested on macOS** — no device needed.
- `project.yml` — the [XcodeGen](https://github.com/yonaskolb/XcodeGen) spec.
  The `.xcodeproj` is generated, not hand-maintained (and git-ignored).

## Install FamiliarMac (the macOS console)

```sh
brew install xcodegen         # once
cd ios && xcodegen            # generates FamiliarAgent.xcodeproj
xcodebuild -project FamiliarAgent.xcodeproj -scheme FamiliarMac \
  -configuration Release build
# then copy the built app into place, e.g.:
#   cp -R build/Release/FamiliarMac.app /Applications/   (or drag from Xcode's Products)
open /Applications/FamiliarMac.app
```

The console expects the daemon running on the same Mac (`familiar daemon install`
— see the [root README](../README.md#install--run)). The Device screen renders the
**invite QR** new devices scan to join.

## Build & test (agents)

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

Provisioning: team **8GHXL328AR**, bundle **io.river.familiar.ios**, automatic
signing (set in `project.yml`). TestFlight uploads: `tools/testflight.sh`
(bump `CURRENT_PROJECT_VERSION` first — see [TESTFLIGHT.md](TESTFLIGHT.md)).

## Enroll a device

1. Open the FamiliarMac console's **Device** screen (or any enrolled member's
   "Show join QR") — it renders the enrollment QR. Headless alternative:
   `familiar mesh qr` on the host.
2. In the app, scan the QR (or paste the payload) and tap **Request**. You accept
   the device on the familiar itself; nothing is sent until you toggle a sensor on.
3. Walk around → `phone at location:away`, `phone motion:walking` appear in the
   familiar's observations, tagged `source=mesh:<device-node-id>`.
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
