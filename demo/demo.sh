#!/usr/bin/env bash
#
# ConfigLens runnable demo — fully isolated and synthetic.
#
# It creates a throwaway home + project under a temporary directory, writes only fake
# configuration (no real credentials, no real user config is read) and runs the CLI against it
# with a cleared environment. Nothing is written outside the temporary directory except the
# snapshot/diff artifacts it creates there, and no network or subprocess of any client is used.
#
# Usage:
#   demo/demo.sh [path-to-configlens-binary]
#
# Default binary: target/release/configlens, falling back to target/debug/configlens.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="${1:-}"
if [ -z "$BIN" ]; then
  if [ -x "$ROOT/target/release/configlens" ]; then
    BIN="$ROOT/target/release/configlens"
  elif [ -x "$ROOT/target/debug/configlens" ]; then
    BIN="$ROOT/target/debug/configlens"
  else
    echo "build first: cargo build --locked --release" >&2
    exit 1
  fi
fi
BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"

SANDBOX="${TMPDIR:-/tmp}/configlens-demo.$$"
HOME_DIR="$SANDBOX/home"
PROJECT="$SANDBOX/project"
mkdir -p "$HOME_DIR/.config/opencode" "$PROJECT/.claude" "$PROJECT/.git"

# --- Synthetic configuration (all values are obviously fake) -----------------------------------
cat > "$PROJECT/.claude/settings.json" <<'JSON'
{
  "model": "project-model",
  "permissions": { "allow": ["Bash(npm run test *)"] },
  "env": { "APP_TOKEN": "demo-not-a-real-token" },
  "mcpServers": { "demo": { "env": { "TOKEN": "demo-not-a-real-token" } } }
}
JSON
cat > "$PROJECT/.claude/settings.local.json" <<'JSON'
{ "model": "local-model" }
JSON
cat > "$HOME_DIR/.config/opencode/opencode.json" <<'JSON'
{
  "model": "global-model",
  "mcp": {
    "demo": {
      "type": "remote",
      "url": "https://user:pass@demo.internal.example/mcp?token=demo-not-a-real-token#frag"
    }
  },
  "unknown_free_text": "demo-not-a-real-token"
}
JSON

run() {
  echo
  echo "\$ configlens $*"
  ( cd "$PROJECT" \
    && env -i HOME="$HOME_DIR" XDG_CONFIG_HOME="$HOME_DIR/.config" \
       "$BIN" "$@" ) || echo "[exit $?]"
}

echo "ConfigLens demo"
echo "  binary:  $BIN"
echo "  sandbox: $SANDBOX   (deleted on exit)"
trap 'rm -rf "$SANDBOX"' EXIT

run clients
run scan --client claude --client-version 2.1.296
run explain /model --client claude --client-version 2.1.296
run explain /permissions/allow --client claude --client-version 2.1.296
run doctor --client claude --client-version 2.1.296
echo
echo "--- share mode: unknown key names and secret values are opaque ---"
run scan --client opencode --client-version 1.18.35 --paths share --format json

echo
echo "--- share mode: private URL hosts are pseudonymized too ---"
run explain /mcp/demo/url --client opencode --client-version 1.18.35 --paths share

echo
echo "--- support report: explicit preview (writes nothing) ---"
run support-report --client opencode --client-version 1.18.35 --preview

echo
echo "--- shell completions and help examples ---"
run --help
run completions --shell bash

echo
echo "--- snapshot + diff (redacted report, owner-only) ---"
run snapshot --client opencode --client-version 1.18.35 --output "$SANDBOX/before.json"
run snapshot --client opencode --client-version 1.18.35 --output "$SANDBOX/after.json"
run diff "$SANDBOX/before.json" "$SANDBOX/after.json"

echo
echo "demo complete."
