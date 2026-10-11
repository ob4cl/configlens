#!/usr/bin/env bash
#
# The recorded terminal demonstration: ConfigLens resolving configuration precedence.
#
# This is the script behind `docs/demo/configlens-demo.cast`. It is a *focused* sibling of
# `demo/demo.sh` (same isolation rules, fewer commands) so the recording stays short enough to watch:
# three verified layers define `model` differently, and the CLI shows which one wins, why, and what
# it refuses to claim.
#
# Layering shown: cc.user (rank 10) → cc.project_shared (rank 20) → cc.project_local (rank 30),
# i.e. three layers the Claude adapter really reads (`configlens clients` prints the matrix).
#
# Isolation and honesty rules (identical to demo/demo.sh):
#   * a throwaway home + project under TMPDIR, deleted on exit;
#   * only obviously fake values are written — no real configuration, no real credential is read;
#   * the environment is cleared for every CLI invocation; no network, no client subprocess.
#
# Record it with:
#   DEMO_PAUSE=1.2 asciinema rec --cols 100 --rows 30 --idle-time-limit 1.5 \
#     --command "DEMO_PAUSE=1.2 bash demo/record.sh" docs/demo/configlens-demo.cast --overwrite
#
# `DEMO_PAUSE` (seconds, default 0) adds a pause between sections so the recording is watchable.
# Recording must happen on a real pty: driven from a non-interactive shell, asciinema flushes the
# whole session at once and the cast would play back instantly (see docs/demo/README.md).
#
# Play it back with:
#   asciinema play docs/demo/configlens-demo.cast

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

SANDBOX="${TMPDIR:-/tmp}/configlens-record.$$"
HOME_DIR="$SANDBOX/home"
PROJECT="$SANDBOX/project"
mkdir -p "$HOME_DIR/.claude" "$PROJECT/.claude" "$PROJECT/.git"

# Three layers, three different values for the same key. Every value is obviously fake.
cat > "$HOME_DIR/.claude/settings.json" <<'JSON'
{ "model": "user-layer-model" }
JSON
cat > "$PROJECT/.claude/settings.json" <<'JSON'
{ "model": "project-layer-model" }
JSON
cat > "$PROJECT/.claude/settings.local.json" <<'JSON'
{ "model": "local-layer-model" }
JSON

trap 'rm -rf "$SANDBOX"' EXIT

# Seconds of pause between sections; 0 keeps a scripted run fast, a recording sets it explicitly.
PAUSE="${DEMO_PAUSE:-0}"

run() {
  printf '\n$ configlens %s\n' "$*"
  ( cd "$PROJECT" \
    && env -i HOME="$HOME_DIR" XDG_CONFIG_HOME="$HOME_DIR/.config" \
       "$BIN" "$@" ) || printf '[exit %s]\n' "$?"
  sleep "$PAUSE"
}

# The JSON envelope for one resolution is long; the recording shows the same command piped through
# `jq` so the trace stays readable on one screen. The filter is printed with the command.
run_json_excerpt() {
  local filter='.resolutions[] | {pointer, status, observed_value, events: [.events[] | {kind, location, value}]}'
  printf '\n$ configlens %s --format json | jq '"'"'%s'"'"'\n' "$*" "$filter"
  ( cd "$PROJECT" \
    && env -i HOME="$HOME_DIR" XDG_CONFIG_HOME="$HOME_DIR/.config" \
       "$BIN" "$@" --format json \
       | jq "$filter" ) || printf '[exit %s]\n' "$?"
}

printf 'ConfigLens — the same key, set in three layers\n'
printf 'sandbox: %s (synthetic values only, deleted on exit)\n' "$SANDBOX"
printf '\n--- the layers, lowest precedence first (cc.user → cc.project_shared → cc.project_local) ---\n'
printf '\n$ cat ~/.claude/settings.json\n'
cat "$HOME_DIR/.claude/settings.json"
printf '$ cat ./.claude/settings.json\n'
cat "$PROJECT/.claude/settings.json"
printf '$ cat ./.claude/settings.local.json\n'
cat "$PROJECT/.claude/settings.local.json"

run explain /model --client claude --client-version 2.1.296

printf '\n--- the same resolution as JSON: every contribution keeps its source, line and reason ---\n'
run_json_excerpt explain /model --client claude --client-version 2.1.296

printf '\ndemo complete — nothing above touched a real configuration file.\n'
