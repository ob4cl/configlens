#!/usr/bin/env bash
# ConfigLens research fixture runner (read-only, isolated, no network, no credentials).
# Runs the released ConfigLens binary against the synthetic fixtures in fixtures/.
# Captured output is the *engine* behaviour; every rule it exercises is also cited
# to a primary upstream source in compatibility-audit.md. This is fixture-level
# evidence only (see compatibility-audit.md "evidence classes").
set -u
ROOT="/home/thai/configlens"
BIN="$ROOT/target/release/configlens"
FIX="$ROOT/docs/research/fixtures"

claude_env() {
  env -u CLAUDE_CONFIG_DIR -u OPENCODE_CONFIG -u OPENCODE_CONFIG_DIR -u OPENCODE_CONFIG_CONTENT \
      HOME="$FIX/claude/home" XDG_CONFIG_HOME="$FIX/claude/home/.config" "$@"
}
oc_env() {
  env -u CLAUDE_CONFIG_DIR -u OPENCODE_CONFIG -u OPENCODE_CONFIG_DIR -u OPENCODE_CONFIG_CONTENT \
      HOME="$FIX/opencode/home" XDG_CONFIG_HOME="$FIX/opencode/home/.config" "$@"
}
run() { printf '$ %s\n' "$*"; "$@"; printf '\n'; }

echo "### A. claude: user + shared project + local project, trust UNKNOWN"
run claude_env "$BIN" scan --client claude --cwd "$FIX/claude/project" \
    --project-root "$FIX/claude/project" --format text --paths local

echo "### B. claude: explain /model with trust UNKNOWN (project layers conditional)"
run claude_env "$BIN" explain /model --client claude --cwd "$FIX/claude/project" \
    --project-root "$FIX/claude/project" --client-version 2.1.296 --format text

echo "### C. claude: explain /model with supplied context project_trust=trusted"
run claude_env "$BIN" explain /model --client claude --cwd "$FIX/claude/project" \
    --project-root "$FIX/claude/project" --input-context "$FIX/context-claude-trusted.json" --format text

echo "### D. claude: explain /model with supplied context project_trust=untrusted"
run claude_env "$BIN" explain /model --client claude --cwd "$FIX/claude/project" \
    --project-root "$FIX/claude/project" --input-context "$FIX/context-claude-untrusted.json" --format text

echo "### E. claude: explain /permissions/allow (list concat provenance, not authorization)"
run claude_env "$BIN" explain /permissions/allow --client claude --cwd "$FIX/claude/project" \
    --project-root "$FIX/claude/project" --input-context "$FIX/context-claude-trusted.json" --format text

echo "### F. claude: explain /spinnerTipsEnabled (shared project beats user)"
run claude_env "$BIN" explain /spinnerTipsEnabled --client claude --cwd "$FIX/claude/project" \
    --project-root "$FIX/claude/project" --input-context "$FIX/context-claude-trusted.json" --format text

echo "### G. claude: explain /permissions/defaultMode scope exclusion"
run claude_env "$BIN" explain /permissions/defaultMode --client claude --cwd "$FIX/claude/project" \
    --project-root "$FIX/claude/project" --input-context "$FIX/context-claude-trusted.json" --format text

echo "### H. claude: JSON scan (share paths) - redaction check"
run claude_env "$BIN" scan --client claude --cwd "$FIX/claude/project" \
    --project-root "$FIX/claude/project" --format json --paths share

echo "### I. opencode: scan (global + project, both json and jsonc dialects)"
run oc_env "$BIN" scan --client opencode --cwd "$FIX/opencode/project" \
    --project-root "$FIX/opencode/project" --format text --paths local

echo "### J. opencode: explain /model (project overrides global)"
run oc_env "$BIN" explain /model --client opencode --cwd "$FIX/opencode/project" \
    --project-root "$FIX/opencode/project" --client-version 1.18.35 \
    --input-context "$FIX/context-opencode-trusted.json" --format text

echo "### K. opencode: explain /share (enum provenance)"
run oc_env "$BIN" explain /share --client opencode --cwd "$FIX/opencode/project" \
    --project-root "$FIX/opencode/project" --input-context "$FIX/context-opencode-trusted.json" --format text

echo "### L. opencode: explain /mcp/jira/environment (whole map must be redacted)"
run oc_env "$BIN" explain /mcp/jira/environment --client opencode --cwd "$FIX/opencode/project" \
    --project-root "$FIX/opencode/project" --format text
