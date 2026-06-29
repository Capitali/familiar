# Packaging — `Familiar.app` and the installer

Turns the workspace binaries into a signed, notarized macOS installer: a double-click
`.pkg` that drops **`Familiar.app`** in `/Applications` and sets the familiar up to live there —
the metabolism running at boot, the **marble** in the menu bar as the way into the Glass.

## What's here

| File | Role |
|---|---|
| `Info.plist` | bundle metadata: menu-bar accessory (`LSUIElement`), `marble` as entry point, `NSCameraUsageDescription` |
| `entitlements.plist` | hardened-runtime entitlements (camera) — required for notarization |
| `build-app.sh` | release build → assemble `dist/Familiar.app` → sign (hardened runtime + entitlements) |
| `build-pkg.sh` | `dist/Familiar.app` → `dist/Familiar-<ver>.pkg` (sign + notarize) |
| `scripts/postinstall` | runs at install time: makes the per-user data dir, installs + loads the LaunchAgents |

The four binaries (`marble`, `glass`, `familiar`, `familiar-eye`) live together in
`Contents/MacOS`, so the marble's sibling-resolution finds them inside the bundle — and the
macOS **camera grant attaches to `Familiar.app`**, not to a terminal.

## Build it

```bash
packaging/build-app.sh      # → dist/Familiar.app   (ad-hoc signed by default)
packaging/build-pkg.sh      # → dist/Familiar-0.1.0.pkg
```

Both work with no Apple account (ad-hoc / unsigned) for **local** testing. The ad-hoc app
runs on this Mac but (a) Gatekeeper blocks it elsewhere and (b) its identity changes each
rebuild, so the camera permission must be re-approved after a rebuild.

## Make it distributable (needs an Apple Developer account)

Prerequisites — the three things only you can obtain:

1. **Apple Developer Program** membership ($99/yr).
2. In your login keychain: a **Developer ID Application** cert (signs the app) and a
   **Developer ID Installer** cert (signs the pkg).
3. A stored notarization credential:
   ```bash
   xcrun notarytool store-credentials familiar-notary \
     --apple-id you@example.com --team-id TEAMID --password <app-specific-password>
   ```

Then the same scripts produce a notarized, anyone-can-run installer:

```bash
APP_IDENTITY="Developer ID Application: Your Name (TEAMID)"   packaging/build-app.sh
INSTALLER_IDENTITY="Developer ID Installer: Your Name (TEAMID)" \
NOTARY_PROFILE="familiar-notary"                              packaging/build-pkg.sh
```

`build-pkg.sh` signs the pkg, submits it to `notarytool --wait`, and staples the ticket.

## What the installer sets up (postinstall)

For the logged-in user:

- **Data dir:** `~/Library/Application Support/Familiar/data` (per-user; agents log there).
- **`io.river.familiar.daemon`** — `KeepAlive` LaunchAgent running `familiar run --daemon`:
  the always-on metabolism, supervised by launchd, restarted if it dies.
- **`io.river.familiar.marble`** — `RunAtLoad` LaunchAgent running `marble run`: the menu-bar
  presence at every login.

Per-user agents load automatically at each login; the postinstall also bootstraps them
immediately so the marble appears without a re-login.

## Open items

- **Daemon camera attribution** — capture runs in the `familiar` daemon. That the TCC grant
  attaches to `Familiar.app` (not the bare binary) needs a real-world check once the app is
  signed with a stable Developer ID identity. Likely fine because the daemon binary is the
  bundle's signed copy, but verify on first signed install.
- **App icon** — no `.icns` yet; Finder/the installer show a generic icon. The marble's
  menu-bar icon is drawn procedurally and is unaffected.
- **`com.apple.provenance` `._` payload files** — system-added xattr we can't strip; benign,
  notarization tolerates it.
