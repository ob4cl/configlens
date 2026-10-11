# OpenCode adapter — compatibility and evidence registry

Adapter id: `opencode`
`adapter_revision`: `0.1.0`
Client under description: OpenCode (`opencode-ai` on npm)

## Evidence snapshot

| Field | Value |
|---|---|
| Primary source URL | https://docs.opencode.ai/docs/config/ (the **v1** config page) |
| Retrieved (UTC) | 2026-10-10 |
| Second source | Repository metadata, `sst/opencode` tag `v1.18.35` → commit `53d1eabb61e21162157817bf677da0a4ad3332e3` (via GitHub REST API) |
| Newest published client version at retrieval | `opencode-ai` **1.18.35** (npm `modified` 2026-10-09) |
| Pinned loader commit | **`53d1eabb61e21162157817bf677da0a4ad3332e3`** — the `v1.18.35` tag, i.e. the loader for the documented v1 line: `packages/opencode/src/config/config.ts`. The v2 dev line (`@opencode/cli` 2.0.26, HEAD `7b3d4ce3a7dbd2a6d3637722a0d5f22a7d086937`) is a **different product line** and is **out of scope** for this adapter; do not pair a v1 version with a v2 SHA. |
| Consequence | No rule below may claim `upstream_conformance_verified`. Maximum support in this release is `fixture_verified` (our fixtures vs. the documented v1 order) or `documented`. |

## Documented precedence order (lowest → highest)

Verbatim structure of the upstream "Precedence order" section; later sources override earlier ones,
and *"Configuration files are merged together, not replaced… Later configs override earlier ones
only for conflicting keys."*

| # | Layer id | Documented location | Observable offline by ConfigLens? |
|---|---|---|---|
| 1 | `oc.remote.wellknown` | remote config from `.well-known/opencode` | **No** — expected, never fetched |
| 2 | `oc.global` | `~/.config/opencode/opencode.json` (or `.jsonc`) | Yes |
| 3 | `oc.custom_path` | `$OPENCODE_CONFIG` | Yes (env allowlist) |
| 4 | `oc.project` | `opencode.json` in cwd, "traverse up to the nearest Git directory" | Yes |
| 5 | `oc.opencode_dirs` | `.opencode` directories (agents, commands, plugins) | Inventory only — not a settings document |
| 6 | `oc.inline` | `$OPENCODE_CONFIG_CONTENT` | Yes (env allowlist), content treated as sensitive |
| 7 | `oc.managed_file` | Linux `%` → `/etc/opencode/`, macOS `/Library/Application Support/opencode/`, Windows `%ProgramData%\opencode`; file `opencode.json`/`opencode.jsonc` | Yes (existence + parse) |
| 8 | `oc.managed_macos_prefs` | `/Library/Managed Preferences/<user>/ai.opencode.managed.plist`, `/Library/Managed Preferences/ai.opencode.managed.plist` | **Present-but-unparsed** — plist parsing not implemented |

Additional documented source, **position not fully specified by upstream**:
`$OPENCODE_CONFIG_DIR` — *"The custom directory is loaded after the global config and `.opencode`
directories, so it can override their settings."* Its relation to project/inline/managed layers is
not stated, so any key it defines produces a `oc.custom_dir_placement_unverified` coverage gap.

### Workspace trust — not an OpenCode concept

OpenCode documents **no** trust gate for project config, and the pinned v1 loader carries no
workspace-trust machinery (a content grep for `trust` finds only incidental uses — a TLS/CA comment,
a GitLab OAuth app, prompt text). ConfigLens therefore makes `oc.project` and every other OpenCode
source **unconditionally applicable** and never emits a `trust_unknown` / `workspace_untrusted` gap
or a per-source trust diagnostic for this client. (A Claude-derived trust gate used to be applied
here; audit finding O4.)

## Supported keys

Operator vocabulary: `replace` (scalar, higher layer wins), `object-merge` (recursive by key),
`inventory-only` (no merge claim), `interpolation`.

| Pointer | Type | Operator | Rule ID | Support | Evidence |
|---|---|---|---|---|---|
| `/model` | string | replace | `opencode.scalar.layer-override` | `fixture_verified` | Docs "Precedence order", "Models" |
| `/small_model` | string | replace | `opencode.scalar.layer-override` | `fixture_verified` | Docs "Models" |
| `/default_agent` | string | replace | `opencode.scalar.layer-override` | `fixture_verified` | Docs "Default agent" |
| `/share` | `manual\|auto\|disabled` | replace | `opencode.scalar.layer-override` | `fixture_verified` | Docs "Share" |
| `/autoupdate` | bool \| `"notify"` | replace | `opencode.scalar.layer-override` | `fixture_verified` | Docs "Autoupdate" |
| `/snapshot` | bool | replace | `opencode.scalar.layer-override` | `fixture_verified` | Docs "Snapshot" |
| `/shell` | string | replace | `opencode.scalar.layer-override` | `documented` | Docs "Shell" |
| `/compaction/{auto,prune,reserved}` | bool,int | replace | `opencode.scalar.layer-override` | `fixture_verified` | Docs "Compaction" |
| `/attachment/image/{auto_resize,max_width,max_height,max_base64_bytes}` | bool,int | replace | `opencode.scalar.layer-override` | `documented` | Docs "Attachment image" |
| `/server/{port,hostname,mdns,mdnsDomain}` | int,string,bool | replace | `opencode.scalar.layer-override` | `fixture_verified` | Docs "Server" |
| `/server/cors` | array | inventory-only | `opencode.list.inventory-only` | `inventory_only` | Merge semantics not documented |
| `/tools/<name>` | bool | object-merge | `opencode.object.recursive-merge` | `fixture_verified` | Docs "Tools" |
| `/permission/<tool>[/<pattern>]` | enum | object-merge | `opencode.object.recursive-merge` | `fixture_verified` | Docs "Permissions". **Not** an authorization decision |
| `/mcp/<name>/{enabled,type,url}` | bool,string | object-merge | `opencode.object.recursive-merge` | `fixture_verified` | Docs "MCP servers" |
| `/mcp/<name>/environment` | object | object-merge | `opencode.object.recursive-merge` | `fixture_verified` | **Whole map redacted** (spec §7) |
| `/provider/…`, `/agent/…`, `/experimental/…`, `/formatter`, `/lsp`, `/watcher/ignore`, `/instructions`, `/plugin`, `/disabled_providers`, `/enabled_providers` | any | inventory-only | `opencode.list.inventory-only` / `opencode.unknown.inventory-only` | `inventory_only` | Values may be sensitive; merge rules not verified |
| any other pointer | any | inventory-only | `opencode.unknown.inventory-only` | `inventory_only` | Spec §3 rule 6 |
| `{env:NAME}` in a supported string | string | interpolation | `opencode.value.interpolation-env` | `fixture_verified` | Docs "Variables → Env vars" |
| `{file:path}` in a supported string | string | interpolation | `opencode.value.interpolation-file` | `documented` | Docs "Variables → Files"; read gated by `--allow-read-root` |

## Rule → test IDs

| Rule ID | Positive / boundary / negative tests |
|---|---|
| `opencode.scalar.layer-override` | `oc_scalar_project_overrides_global`, `oc_managed_file_overrides_project`, `oc_inline_content_overrides_project`, `oc_scalar_boundary_highest_layer_null_is_not_missing`, `oc_scalar_negative_unknown_layer_does_not_win` |
| `opencode.object.recursive-merge` | `oc_object_nested_merge_retains_unrelated_keys`, `oc_object_child_events_retained`, `oc_object_negative_no_merge_for_unknown_root` |
| `opencode.list.inventory-only` | `oc_list_inventory_only_no_merge_claim` |
| `opencode.unknown.inventory-only` | `oc_unknown_key_inventory_only` |
| `opencode.value.interpolation-env` | `oc_env_interpolation_tainted`, `oc_env_interpolation_unresolved_without_context` |
| `opencode.value.interpolation-file` | `oc_file_interpolation_requires_allow_read_root` |
| coverage | `oc_remote_and_mdm_reported_unobservable`, `oc_json_and_jsonc_coexistence_partial`, `oc_custom_dir_placement_gap` |

## Support-state summary

| State | Keys |
|---|---|
| `upstream_conformance_verified` | *(none — no loader harness)* |
| `fixture_verified` | the scalar/object/interpolation rows marked above |
| `documented` | `/shell`, `/attachment/image/*`, `{file:}` interpolation, managed file paths |
| `inventory_only` | every list/unknown/instruction/provider/agent key |
| `unsupported` | remote defaults content, macOS MDM preferences content, `.opencode` directory settings |
