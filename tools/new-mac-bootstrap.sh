#!/bin/bash
# Bootstrap a fresh Mac for familiar development.
#
#   bash tools/new-mac-bootstrap.sh            # toolchain + clone + build
#   bash tools/new-mac-bootstrap.sh --ios      # ...plus iOS/watch simulator builds
#   bash tools/new-mac-bootstrap.sh --daemon   # install + start the daemon LaunchAgent
#                                              # (run AFTER the transfer bundle is unpacked —
#                                              #  the daemon is mute without llm/call_llm.sh)
#
# Idempotent: every step checks before it acts, so re-running after a failure is safe.
# What this cannot do (hand-carry from the old Mac; see the transfer bundle):
#   - the Developer ID / Apple Development signing identities (Keychain Access export)
#   - ~/.appstoreconnect/private_keys/AuthKey_*.p8 (TestFlight uploads)
#   - the LLM adapter (llm/call_llm.sh + key.env — carries provider keys)
#   - Tailscale login, and the decision: fresh mesh node vs migrating the old data dir.

set -euo pipefail

REPO_SSH="git@github.com:Capitali/familiar.git"
DEV_DIR="$HOME/Development"
REPO_DIR="$DEV_DIR/familiar"
APP_SUPPORT="$HOME/Library/Application Support/Familiar"
LABEL="io.river.familiar"

step() { printf '\n\033[1m== %s ==\033[0m\n' "$*"; }
have() { command -v "$1" >/dev/null 2>&1; }

MODE="tools"
case "${1:-}" in
  --ios) MODE="ios" ;;
  --daemon) MODE="daemon" ;;
esac

# ---------------------------------------------------------------- toolchain
if [ "$MODE" != "daemon" ]; then
  step "Xcode command-line tools"
  if ! xcode-select -p >/dev/null 2>&1; then
    xcode-select --install
    echo "Finish the CLT installer, then re-run this script."; exit 1
  fi
  # First build fails cryptically until the license is accepted.
  if ! xcodebuild -version >/dev/null 2>&1; then
    echo "Accepting the Xcode license (needs your password)…"
    sudo xcodebuild -license accept
  fi

  step "Homebrew"
  if ! have brew; then
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  fi
  # Apple Silicon installs to /opt/homebrew, which is not on PATH until eval'd.
  if [ -x /opt/homebrew/bin/brew ]; then eval "$(/opt/homebrew/bin/brew shellenv)"; fi
  if [ -x /usr/local/bin/brew ] && ! have brew; then eval "$(/usr/local/bin/brew shellenv)"; fi

  step "brew packages: git gh xcodegen node"
  for pkg in git gh xcodegen node; do
    brew list "$pkg" >/dev/null 2>&1 || brew install "$pkg"
  done

  step "Rust (rustup, stable)"
  if ! have rustup && [ ! -x "$HOME/.cargo/bin/rustup" ]; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  fi
  # shellcheck disable=SC1091
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
  rustup update stable >/dev/null

  step "git identity"
  git config --global user.name  >/dev/null 2>&1 || git config --global user.name "Ian Schlueter"
  git config --global user.email >/dev/null 2>&1 || git config --global user.email "ian@river.io"

  step "SSH key for GitHub"
  if [ ! -f "$HOME/.ssh/id_ed25519_github" ] && ! ssh -o BatchMode=yes -T git@github.com 2>&1 | grep -q "successfully authenticated"; then
    if [ ! -f "$HOME/.ssh/id_ed25519" ]; then
      ssh-keygen -t ed25519 -C "ian@$(hostname -s)" -f "$HOME/.ssh/id_ed25519" -N ""
    fi
    echo "Add this public key at https://github.com/settings/keys (or copy id_ed25519_github from the transfer bundle):"
    cat "$HOME/.ssh/id_ed25519.pub"
  fi

  step "clone $REPO_SSH"
  mkdir -p "$DEV_DIR"
  if [ ! -d "$REPO_DIR/.git" ]; then
    git clone "$REPO_SSH" "$REPO_DIR" || {
      echo "SSH clone failed — after 'gh auth login', retrying over https…"
      gh repo clone Capitali/familiar "$REPO_DIR"
    }
  fi

  step "cargo build --release (first build takes a while)"
  (cd "$REPO_DIR" && cargo build --release)

  step "quick test sanity (kernel + llm)"
  (cd "$REPO_DIR" && cargo test -q -p familiar-kernel -p familiar-llm)

  step "generate the Xcode project"
  (cd "$REPO_DIR/ios" && xcodegen)

  echo
  echo "Toolchain ready. Next:"
  echo "  1. Unpack the transfer bundle from the old Mac (signing certs, ASC keys, LLM adapter)."
  echo "  2. Xcode → Settings → Accounts: sign in; import the Certificates .p12 (double-click it)."
  echo "  3. bash tools/new-mac-bootstrap.sh --ios      # verify the app + watch build"
  echo "  4. bash tools/new-mac-bootstrap.sh --daemon   # if this Mac should run a familiar"
  echo "  5. Install Tailscale (brew install --cask tailscale) and log in, if this Mac joins the tailnet."
fi

# ---------------------------------------------------------------- simulator builds
if [ "$MODE" = "ios" ]; then
  step "iOS app (simulator)"
  (cd "$REPO_DIR/ios" && xcodebuild -project FamiliarAgent.xcodeproj -scheme FamiliarAgent \
    -destination 'generic/platform=iOS Simulator' -configuration Debug build CODE_SIGNING_ALLOWED=NO | tail -2)
  step "watch app (simulator)"
  (cd "$REPO_DIR/ios" && xcodebuild -project FamiliarAgent.xcodeproj -scheme FamiliarWatch \
    -destination 'generic/platform=watchOS Simulator' -configuration Debug build CODE_SIGNING_ALLOWED=NO | tail -2)
  step "Mac console"
  (cd "$REPO_DIR/ios" && xcodebuild -project FamiliarAgent.xcodeproj -scheme FamiliarMac \
    -configuration Debug build CODE_SIGNING_ALLOWED=NO | tail -2)
fi

# ---------------------------------------------------------------- daemon install
if [ "$MODE" = "daemon" ]; then
  step "daemon install ($LABEL)"
  [ -x "$REPO_DIR/target/release/familiar" ] || (cd "$REPO_DIR" && cargo build --release)
  mkdir -p "$APP_SUPPORT/bin" "$APP_SUPPORT/data"
  cp "$REPO_DIR/target/release/familiar" "$APP_SUPPORT/bin/familiar"

  if [ ! -f "$APP_SUPPORT/data/llm/call_llm.sh" ]; then
    echo "NOTE: no LLM adapter at data/llm/call_llm.sh — the familiar runs, but its mind is"
    echo "      closed until you copy call_llm.sh + key.env from the transfer bundle."
  fi

  PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
  if [ ! -f "$PLIST" ]; then
    mkdir -p "$HOME/Library/LaunchAgents"
    cat > "$PLIST" <<PL
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>$LABEL</string>
  <key>ProgramArguments</key><array>
    <string>$APP_SUPPORT/bin/familiar</string>
    <string>run</string><string>--daemon</string>
    <string>--interval</string><string>60</string>
    <string>--data-dir</string><string>$APP_SUPPORT/data</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><false/>
  <key>StandardOutPath</key><string>$APP_SUPPORT/data/daemon.log</string>
  <key>StandardErrorPath</key><string>$APP_SUPPORT/data/daemon.log</string>
</dict></plist>
PL
    launchctl bootstrap "gui/$(id -u)" "$PLIST"
  fi
  # kickstart -k, never unload -w/load -w (that pattern leaves the agent registered-but-stalled).
  launchctl kickstart -k "gui/$(id -u)/$LABEL"
  sleep 3
  code=$(curl -ksf -o /dev/null -w '%{http_code}' https://127.0.0.1:47100/mesh/hello || true)
  echo "daemon mesh door: HTTP $code (200 = alive; the mesh port is TLS)"
  echo
  echo "This machine is a NEW mesh node (fresh keys minted on first run)."
  echo "Enroll it via the join door — scan an invite QR from an existing member device."
  echo "To instead MIGRATE the old Mac's identity + memory, stop the daemon and copy the"
  echo "old '$APP_SUPPORT/data' over this one before first launch (see docs/handoff)."
fi
