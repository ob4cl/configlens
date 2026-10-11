# Claude Code adapter — compatibility and evidence registry

Adapter id: `claude`
`adapter_revision`: `0.1.0`
Client under description: Claude Code (`@anthropic-ai/claude-code` on npm)

## Evidence snapshot

| Field | Value |
|---|---|
| Primary source URL | https://code.claude.com/docs/en/settings |
| Secondary source URL | https://code.claude.com/docs/en/managed-settings |
| Retrieved (UTC) | 2026-10-10 |
| Newest published client version at retrieval | **2.1.296** (npm `@anthropic-ai/claude-code` `modified` 2026-10-09; GitHub release `v2.1.296` published 2026-10-09T19:28:59Z) |
| Documented version boundaries seen in the sources | `v2.1.193`, `v2.1.211`, `v2.1.229`, `v2.1.238`, `v2.1.242`, `v2.1.246`, `v2.1.257`, `v2.1.259`, `v2.1.267` |
| Pinned loader commit | **None available.** Claude Code is closed source; `anthropics/claude-code` publishes issues and `examples/`, not the settings loader. Only the repository's last `README.md` commit (`b640d94a49629ef004ecceb5e2bd6d8aebf067ce`, 2026-01-12) was observed and is *not* configuration evidence. |
| Consequence | No rule below may claim `upstream_conformance_verified`. Maximum support is `fixture_verified` (fixtures reproducing documented behaviour) or `documented`. |

## Documented precedence (highest wins)

Verbatim from the "Settings precedence" stack, highest first:

1. `cc.managed` — managed settings (file, MDM/OS policy, server-managed settings). *"Nothing you set
   overrides them"* — with a small documented list of security exceptions where a *stricter* value
   from a lower scope is honoured.
2. `cc.session_args` — command-line arguments (`--settings <json|file>`, per-key flags such as
   `--model`, `--effort`). *"it takes a key you set here over the same key in local, project, or user
   settings"*.
3. `cc.project_local` — `<repo root>/.claude/settings.local.json`
4. `cc.project_shared` — `.claude/settings.json` (the session's primary working directory)
5. `cc.user` — `~/.claude/settings.json` (`$CLAUDE_CONFIG_DIR/settings.json` when set)

Environment variables are explicitly **not** a level: *"which one applies is decided per pair, not by
level"*. Modelled separately, per key.

Managed tier internals (from the managed-settings page). Claude Code combines managed *sources*
**first-wins** by default: `managedSourcesBehavior = "first-wins"` reads the highest-ranked source
that delivers at least one policy key and *ignores the rest rather than merging them*. `"merge"` is
opt-in from Claude Code v2.1.242 or later. ConfigLens models **single-source** managed resolution
only: `registry.json` declares `managed_sources_behavior = "first-wins"` and ranks the layers so the
highest-ranked source wins. That declaration is **documentation only** — no code path consults it,
so it cannot change a resolution (it is asserted by a test, not enforced by the engine). The
documented cross-source key set (keys read from *every* admin source
with their own combination rules — `sandbox.network.allowManagedDomainsOnly`, `allowAllClaudeAiMcps`,
`deniedMcpServers`, `maxEffortLevel`, per-variable `env`, `forceRemoteSettingsRefresh`, …) and
`policyHelper` (a helper that emits `managedSettings`, which then becomes the only managed settings
for the session) are **not modelled** — a documented gap.

| # | Layer id | registry rank | Location | Observable? |
|---|---|---|---|---|
| 1 | `cc.managed.server` | 80 | claude.ai server-managed settings / gateway | **No** |
| 2 | `cc.managed.mdm` | 70 | macOS `com.anthropic.claudecode` profile; Windows `HKLM\SOFTWARE\Policies\ClaudeCode` | **No** |
| 3 | `cc.managed.file` | 60 | `managed-settings.json` then `managed-settings.d/*.json` (alphabetical, non-hidden, `*.json` only) in `/Library/Application Support/ClaudeCode/` (macOS), `/etc/claude-code/` (Linux and WSL), `C:\Program Files\ClaudeCode\` (Windows) | Yes |
| 4 | `cc.managed.hkcu` | 50 | `HKCU\SOFTWARE\Policies\ClaudeCode` (Windows/WSL, only when no higher source delivers a key) | **No** |

Ranks ascend with precedence, so `cc.managed.hkcu` (rank **50**) is the **lowest** managed source —
below the file tier — matching upstream's bottom-managed fallback and *not* the previously inverted
rank 90.

Managed file/directory merge rules (documented, *within the single file source*): single values →
later file replaces; lists → combine with duplicates removed; nested blocks → merge key by key;
`fallbackModel` → later chain replaces whole; `extraKnownMarketplaces`/`managedMcpServers` → later
same-name entry replaces whole.

## Supported keys

| Pointer | Type | Operator | Rule ID | Support | Evidence |
|---|---|---|---|---|---|
| `/model` | string | replace | `claude.scalar.layer-override` | `fixture_verified` | Settings precedence; per-key env pair `ANTHROPIC_MODEL` |
| `/effortLevel`, `/theme`, `/verbose`, `/spinnerTipsEnabled`, `/cleanupPeriodDays` | scalar | replace | `claude.scalar.layer-override` | `documented` | Settings page examples |
| `/fallbackModel` | array | ordered replace | `claude.list.ordered-whole-replace` | `documented` | *"position carries meaning, so Claude Code takes the whole value from the highest-precedence file that defines it"* |
| `/permissions/allow`, `/permissions/ask`, `/permissions/deny` | array | stable concat + dedupe | `claude.permissions.list-concat` | `fixture_verified` | *"Claude Code combines the lists instead of picking one"* + managed tier *"combine, with duplicates removed"*. **Authorization semantics: none** |
| `/permissions/defaultMode` | enum | replace + scope exclusion | `claude.permissions.default-mode-scope-exclusion` | `documented` | *"values `auto` and `bypassPermissions` don't take effect from project or local settings"* |
| `/availableModels` | array | constraint (lock) | `claude.availableModels.managed-lock` | `documented` | *"a flag such as `--model` picks only from the models your organization allows"* |
| `/modelPicker` | array | inventory-only | `claude.unknown.inventory-only` | `inventory_only` | Upstream takes `modelPicker` from managed / `--settings` / user only and **ignores it in project and local settings** (v2.1.242+); that scope is not modelled, so no merge is claimed |
| `/env/<NAME>` | string | inventory-only | `claude.env.inventory-only-trust-gated` | `inventory_only` | `env` is an ordinary key; values redacted; project-file values wait for trust |
| `/hooks/…`, `/permissions/additionalDirectories`, `/extraKnownMarketplaces`, `/plugins/…`, `/mcpServers` | any | inventory-only | `claude.unknown.inventory-only` | `inventory_only` | Bodies never read (spec §7); merge rules not verified |
| `/permissions`, `/sandbox`, `/statusLine` | object | object-merge | `claude.object.recursive-merge` | `documented` | Nested blocks merge key by key (managed-tier rule) |
| any other pointer | any | inventory-only | `claude.unknown.inventory-only` | `inventory_only` | Spec §3 rule 6 |

Environment-pair rules modelled for `/model` only (`ANTHROPIC_MODEL`, and `ANTHROPIC_DEFAULT_MODEL`
which applies *only when no file sets `model`*). Other pairs are not modelled and produce a
`claude.env_pair_not_modelled` coverage gap when the supplied environment contains an `ANTHROPIC_*`
variable.

## Trust gating (documented, key-granular)

Trust gates *specific keys*, not whole files. From the **shared project** file only:

* `permissions.allow`, `permissions.additionalDirectories`, `extraKnownMarketplaces` and most `env`
  values apply only after the folder is trusted.
* `deny` and `ask` rules apply right away.
* From the **local** file: its `allow` rules apply *without* the trust step while the file stays
  **untracked**; if it is tracked by git, the trust step applies to it too.

ConfigLens consequence (0.1): applicability is **key-granular**. An untrusted/unknown workspace makes
the project layers `Conditional`, and only a *trust-gated* key (`permissions.allow`, `env`) quoted
from the **shared** project file is ignored; the reason is `workspace_untrusted` (untrusted) or
`trust_unknown` (undetermined). `deny`/`ask` from either project file, and `allow` from the local
file, still apply. Git-tracking of the local file is **not observable offline**, so ConfigLens
assumes the local file is untracked — a documented gap.

## Rule → test IDs

| Rule ID | Tests |
|---|---|
| `claude.scalar.layer-override` | `cc_local_overrides_project_overrides_user`, `cc_managed_overrides_user_files`, `cc_session_settings_flag_overrides_files` |
| `claude.model.env-pair-override` | `cc_model_env_pair_overrides_file`, `cc_default_model_env_only_when_no_file_sets_key` |
| `claude.permissions.list-concat` | `cc_permissions_allow_lists_concat_and_dedupe`, `cc_permissions_list_concat_negative_no_authorization_claim` |
| `claude.permissions.default-mode-scope-exclusion` | `cc_default_mode_scope_exclusion_ignores_project_value` |
| `claude.list.ordered-whole-replace` | `cc_fallback_model_ordered_whole_replace` |
| `claude.availableModels.managed-lock` | `cc_available_models_locks_model_choice` |
| `claude.env.inventory-only-trust-gated` | `cc_allow_from_project_conditional_when_trust_unknown` |
| `claude.object.recursive-merge` | `cc_object_nested_merge_key_by_key` |
| `claude.unknown.inventory-only` | `cc_unknown_key_inventory_only` |
| coverage | `cc_managed_missing_reported_unobservable`, `cc_unknown_version_provisional` |

## Support-state summary

| State | Keys |
|---|---|
| `upstream_conformance_verified` | *(none — closed-source loader, no safe harness)* |
| `fixture_verified` | `/model`, `/permissions/{allow,ask,deny}` merge/dedupe, file layer order |
| `documented` | `/effortLevel`, `/theme`, `/verbose`, `/spinnerTipsEnabled`, `/cleanupPeriodDays`, `/fallbackModel`, `/permissions/defaultMode` scope exclusion, `/availableModels` lock, object-merge containers |
| `inventory_only` | `env` values, hooks, plugins, marketplaces, MCP server definitions, `modelPicker`, all other keys |
| `unsupported` | managed MDM/registry/server-managed content, special-list rules beyond `fallbackModel`, cross-source managed keys and `policyHelper` |
