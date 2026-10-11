# ConfigLens research fixtures (bounded, synthetic, offline)

These fixtures exist for the private-beta compatibility audit. They are **synthetic**: every value is
obviously fake (`fixture-not-a-real-token`, `example.invalid`, `claude-*` placeholder model names).
No fixture is copied from a real machine, and nothing here reads credentials, spawns a client, loads
a plugin or calls the network.

Layout:

    fixtures/
      claude/                       # Claude Code settings files (JSON, strict)
        home/.claude/settings.json           # user layer
        project/.claude/settings.json        # shared project layer
        project/.claude/settings.local.json  # project local layer
        project/.git/                         # repo-root marker (empty dir)
        managed/managed-settings.json        # managed file tier (NOT reachable by ConfigLens CLI:
        managed/managed-settings.d/*.json    #   the system path is fixed at /etc/claude-code)
      opencode/
        home/.config/opencode/opencode.json  # global layer
        project/opencode.json                # project layer (JSON)
        project/opencode.jsonc               # project layer (JSONC, second dialect)
        project/.git/                        # repo-root marker
        managed/opencode.json                # managed file tier (fixed system path /etc/opencode)
      codex/                        # inputs a future Codex adapter would need (NOT read by 0.1)
        home/.codex/config.toml              # user layer
        project/.codex/config.toml           # trusted-project layer
        requirements.toml                    # admin-enforced constraints
        managed_config.toml                  # legacy managed defaults
      context-claude-trusted.json    } supplied input-context documents (spec §4)
      context-claude-untrusted.json  }
      context-opencode-trusted.json  }

Run the engine against them (read-only, offline, isolated homedir):

    bash docs/research/run-fixtures.sh > docs/research/evidence/fixture-run.txt 2>&1

`evidence/fixture-run.txt` is the captured, unmodified output of that command.

## What these fixtures do and do not establish

* They demonstrate the **engine's** behaviour (which layer wins, how trust/coverage are reported, how
  redaction is applied). That is `fixture_verified`-class evidence for ConfigLens itself.
* They are **not** evidence of upstream parity. Each behaviour they exercise is cited to a primary
  upstream source in `../compatibility-audit.md`; where the engine and the primary source disagree,
  the audit records the disagreement rather than the fixture passing.
* The Claude `managed-settings.json` / `managed-settings.d/` fixtures are provided for completeness
  but are **not read** by the CLI, because ConfigLens discovers the managed tier only under the fixed
  system directories (`/etc/claude-code` on Linux/WSL, `/Library/Application Support/ClaudeCode` on
  macOS, `C:\Program Files\ClaudeCode` on Windows). Creating files there is outside this audit's
  "write only under docs/research" constraint, so that tier is verified by documented order + the
  in-repo unit test `managed_file_and_directory_entries_are_merged_in_documented_order`, not by a CLI
  run here.
