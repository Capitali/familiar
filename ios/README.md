# The Familiar's consoles & device agents (Swift)

Everything human-facing lives here: the **FamiliarMac** sphere console, the
**iPhone/iPad agent** (which hosts the same sphere console), and the **watch app**.
Devices join the familiar's mesh through the lighthouse automatically (a one-scan
invite is the shortcut, not the requirement — ADR-0026), then push **derived
observations** (never raw data) to `POST /mesh/observe`, signed with the same
ed25519 trust the mesh uses.

## Layout

- `MacApp/` — **FamiliarMac**, the macOS sphere console: a WKWebView hosting the
  shared web bundle (`MacApp/Resources/sphere/index.html` — satellite globe,
  hologram screens, the joining address on the Device screen) over a native MKMapView
  street layer. A **peer** — it enrols itself and reads the worldview over the mesh,
  like the iOS shells (ADR-0018). It no longer reads a local daemon over loopback.
  Everything the web bundle needs is vendored in `sphere/vendor/`, so the console draws
  itself with no network; re-fetch with `tools/vendor-sphere-assets.sh`.
- `App/` — the SwiftUI iPhone/iPad agent: enroll (scan/paste), consent switches,
  sensing (CoreLocation + CoreMotion + optional voice/face), and the same sphere
  console rendered from the shared web bundle (worldview read over the mesh).
- `Watch/` — the watchOS companion (its own covenant node — heart rate, motion,
  gyro, GPS → derived observations; enrols via the paired phone's identity). It is
  **embedded inside the iPhone agent**, not shipped or installed on its own:
  `project.yml` has the `FamiliarAgent` target embed the `FamiliarWatch` target
  (`embed: true`). So building/installing the iPhone app carries the watch app with
  it, iOS registers it as the companion (`WCSession.isWatchAppInstalled` → true, the
  address hand-off flows), and it rides along through TestFlight. A watch app
  installed standalone never links to the phone, so this embedding is required.
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

The console does **not** need a daemon on the same Mac. On first launch it greets you,
finds a reachable mesh through the lighthouse, and joins as a guest; the guided path on
its own screen — say a name, be vouched for or sponsored by a member, or redeem a
one-scan invite — carries it to membership (ADR-0026: two-filter admission; the welcome
is a greeting, not a gate). Run a local daemon (`familiar daemon install` — see the
[root README](../README.md#install--run)) only if you want this machine to *be* a familiar
as well as see one; the two are separate mesh nodes and the roster nests the console
under its host machine.

A member console's Welcome screen mints **one-scan invites**: any warranted member is a
door (ADR-0026 Phase 4), and admission is judged by the two filters wherever the knock
lands — the lighthouse is the door that is always home, not the only door.

## Build & test (agents)

```sh
# crypto/wire unit tests + Rust conformance (headless, no device):
cd FamiliarMesh && swift test

# generate the Xcode project and build for the simulator (no signing needed).
# No -sdk flag: the scheme embeds the watch app, and forcing every target onto the
# iPhone SDK makes actool reject the watchOS-only AppIcon ("no applicable content").
xcodegen
xcodebuild -project FamiliarAgent.xcodeproj -scheme FamiliarAgent \
  -destination 'generic/platform=iOS Simulator' \
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
