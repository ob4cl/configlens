# OpenCode adapter — compatibility and evidence registry

Adapter id: `opencode`
`adapter_revision`: `0.1.0`
Client under description: OpenCode (`opencode-ai` on npm)

## Evidence snapshot

| Field | Value |
|---|---|
| Primary source URL | https://docs.opencode.ai/docs/config/ (the **v1** config page) |
| Retrieved (UTC) | 2026-10-10 |
| Second source | Repository metadata, `sst/opencode` tag `v1.18.35` → commit `53d1eabb61e21162157817bf677da0a4ad3332e3` (via GitHub REST API). Note (2026-10-11): the `sst/opencode` org has been renamed and now redirects to **`anomalyco/opencode`**; the same tag → commit still resolves, so a future re-check should not be confused by the org rename. |
| Tested client version | `opencode-ai` **1.18.35** (npm `modified` 2026-10-09) — the version the fixtures were reproduced against |
| Documented client versions | `1.18.35`, **`1.19.0`** (re-checked 2026-10-11: npm dist-tag `latest` is `1.19.0` and the live v1 config page still specifies the identical order, so `1.19.0` is *documented* but not *tested*) |
| Pinned loader commit | **`53d1eabb61e21162157817bf677da0a4ad3332e3`** — the `v1.18.35` tag, i.e. the loader for the documented v1 line: `packages/opencode/src/config/config.ts`. The v2 dev line (`@opencode/cli` 2.0.26, HEAD `7b3d4ce3a7dbd2a6d3637722a0d5f22a7d086937`) is a **different product line** and is **out of scope** for this adapter; do not pair a v1 version with a v2 SHA. |
| Consequence | No rule below may claim `upstream_conformance_verified`. Maximum support in this release is `fixture_verified` (our fixtures vs. the documented v1 order) or `documented`. |

## Version scope (read before reporting a bug)

ConfigLens models the **documented v1 order** below. A claimed client version outside the sets above
is reported `unsupported` with a note, rather than resolved on a guess.

The `@opencode/cli` **2.x** line is a *different product line*, still published as a beta whose docs
warn that data may be wiped. Its config page (`https://v2.opencode.ai/docs/config`) describes a
**different merge model**: it searches from the current directory to the filesystem root, merges
direct `opencode.json(c)` files from the farthest directory to the closest, then merges `.opencode`
directories in the same order, so **every `.opencode` config overrides every direct config**. That
model is **not implemented** here. Do not pair a v1 version with a v2 behaviour claim.

## Documented precedence order (lowest → highest)

Rows 1–5 and 7–9 mirror the upstream "Precedence order" section (which lists **8** documented steps
and has no custom-directory step); later sources override earlier ones, and *"Configuration files are
merged together, not replaced… Later configs override earlier ones only for conflicting keys."* Row 6
(`oc.custom_dir`) is **not** one of those 8 documented rows — it is **source-verified** from the
pinned loader (see below) and inserted between `.opencode` directories and inline content.

| # | Layer id | Documented location | Observable offline by ConfigLens? |
|---|---|---|---|
| 1 | `oc.remote.wellknown` | remote config from `.well-known/opencode` | **No** — expected, never fetched |
| 2 | `oc.global` | `~/.config/opencode/opencode.json` (or `.jsonc`) | Yes |
| 3 | `oc.custom_path` | `$OPENCODE_CONFIG` | Yes (env allowlist) |
| 4 | `oc.project` | every `opencode.json`/`opencode.jsonc` from the working directory up to the project boundary (`--project-root` when supplied, else the nearest Git directory), merged **outermost-first** so the nearest file wins | Yes |
| 5 | `oc.opencode_dirs` | `.opencode` directories (agents, commands, plugins); the `opencode.json(c)` settings files *inside* them are **not read** | Inventory only — the settings files inside `.opencode` directories are not implemented (machine reason: `not_a_settings_document`) |
| 6 | `oc.custom_dir` | `opencode.json` / `opencode.jsonc` **inside** `$OPENCODE_CONFIG_DIR` | Yes (env allowlist, directory value) — placement source-verified |
| 7 | `oc.inline` | `$OPENCODE_CONFIG_CONTENT` | Yes (env allowlist), content treated as sensitive |
| 8 | `oc.managed_file` | Linux `%` → `/etc/opencode/`, macOS `/Library/Application Support/opencode/`, Windows `%ProgramData%\opencode`; file `opencode.json`/`opencode.jsonc` | Yes (existence + parse) |
| 9 | `oc.managed_macos_prefs` | `/Library/Managed Preferences/<user>/ai.opencode.managed.plist`, `/Library/Managed Preferences/ai.opencode.managed.plist` | **Present-but-unparsed** — plist parsing not implemented |

### `$OPENCODE_CONFIG_DIR` — a directory, placement source-verified

The config page says the custom directory *"is loaded after the global config and `.opencode`
directories, so it can override their settings"*; the position relative to the project, inline and
managed layers was not stated, and the adapter used to carry a `custom_dir_placement_unverified`
coverage gap for it.

The pinned loader settles it (evidence `OC-CONFIGDIR-SOURCE-20261011`, transcript
`docs/research/evidence/opencode-config-dir-20261011.txt`, commit
`53d1eabb61e21162157817bf677da0a4ad3332e3`):

- `packages/opencode/src/config/paths.ts` — `directories()` returns
  `unique([Global.Path.config, ...project .opencode dirs, ...home .opencode, ...(Flag.OPENCODE_CONFIG_DIR ? [Flag.OPENCODE_CONFIG_DIR] : [])])`:
  the environment value is a **directory**, appended **last**.
- `packages/opencode/src/config/config.ts` — inside the directories loop it loads `opencode.json`
  then `opencode.jsonc` from every directory that `endsWith(".opencode")` or
  `=== Flag.OPENCODE_CONFIG_DIR`. That loop runs **after** the project files are merged and
  **before** `$OPENCODE_CONFIG_CONTENT`.

Consequences, all encoded in the registry and covered by tests:

1. The sources are `<dir>/opencode.json` and `<dir>/opencode.jsonc`, and `.jsonc` wins because it is
   merged second. The environment value is **never** read as a configuration file.
2. Rank 45 (between `oc.opencode_dirs` = 40 and `oc.inline` = 50) is source-verified, so
   `placement_verified` is `true` and no placement gap is emitted.
3. A **file**-valued `$OPENCODE_CONFIG_DIR` contributes nothing, because the loader would look for
   `<value>/opencode.json`.
4. As with every other path, a directory outside the approved read roots is listed as a source with
   an explicit `outside_approved_roots` status and a diagnostic — never silently skipped. Use
   `--allow-read-root <dir>` to include it.

Two real Windows beta observations drove this (v0.1.0-beta.1, OpenCode 1.18.35): a directory value
was reported `not_a_regular_file` (so the correct usage did nothing), and a file value was accepted
and made the winner (a claim upstream does not support). Both are fixed and regression-tested.

### Workspace trust — not an OpenCode concept

OpenCode documents **no** trust gate for project config, and the pinned v1 loader carries no
workspace-trust machinery (a content grep for `trust` finds only incidental uses — a TLS/CA comment,
a GitLab OAuth app, prompt text). ConfigLens therefore makes `oc.project` and every other OpenCode
source **unconditionally applicable** and never emits a `trust_unknown` / `workspace_untrusted` gap
or a per-source trust diagnostic for this client. (A Claude-derived trust gate used to be applied
here; audit finding O4.)

### Project layer — every ancestor file, nearest wins (finding A3, fixed)

`oc.project` reads **every** `opencode.json`/`opencode.jsonc` from the working directory up to the
project boundary, inclusive, and emits them outermost-first (boundary first, working directory last),
so the nearest file is last in definition order and wins. The boundary is `--project-root` when
supplied, otherwise the nearest Git directory; with no boundary only the working directory is read
and the run keeps the `project_boundary_uncertain` signal.

This matches upstream. At the pinned commit `53d1eabb61e21162157817bf677da0a4ad3332e3`
(evidence `OC-PROJECT-SOURCE-20261011`):

- `packages/opencode/src/config/paths.ts` — `ConfigPaths.files(name, directory, worktree?)` calls
  `FSUtil.up({ targets: [`${name}.jsonc`, `${name}.json`], start: directory (the working directory),
  stop: worktree (the repository root) }).toReversed()`.
- `packages/core/src/fs-util.ts` — `up()` pushes a match at **every** level from `start` upward while
  `options.stop !== current` (and stops at the filesystem root when no stop is reached), so `files()`
  returns every ancestor config from the working directory up to the worktree, outermost first,
  working directory last.
- `packages/opencode/src/config/config.ts` — merges that list in order, so **the nearest file wins**
  and **every ancestor contributes**.

Consequences, encoded in the registry and covered by tests:

1. A conflicting scalar resolves to the nearest (working-directory) file; the ancestor definition is
   still reported, as a loser with reason `replaced_by_later_layer`.
2. A key defined only in an ancestor resolves from that ancestor (not `absent`/`unsupported`).
3. `opencode.jsonc` wins over `opencode.json` at the same level (`.jsonc` is merged second), at every
   level including ancestors.
4. When the working directory **is** the boundary, exactly one file per directory is read — unchanged
   from the previous behaviour.
5. A config **above** the boundary is never read.

Previously ConfigLens used `--project-root` (or the cwd) as the *search start* and read only the first
directory containing a config, so a working directory below the repository root could be resolved from
the wrong (ancestor) file and every intermediate ancestor was dropped. That was the wrong-winner path
recorded as audit finding A3 in
[`docs/research/project-config-discovery-finding-20261011.md`](../../docs/research/project-config-discovery-finding-20261011.md);
it is now fixed and regression-tested.

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
| coverage | `oc_remote_and_mdm_reported_unobservable`, `oc_json_and_jsonc_coexistence_partial`, `oc_project_nested_configs_nearest_wins_and_every_level_is_a_source`, `oc_project_ancestor_only_key_resolves_from_the_ancestor`, `oc_project_cwd_equal_to_the_boundary_reads_a_single_source`, `oc_project_explicit_boundary_never_reads_above_it`, `oc_project_without_a_boundary_reads_the_cwd_only`, `oc_project_jsonc_beats_json_at_an_ancestor_level`, `oc_custom_dir_reads_the_config_inside_the_directory`, `oc_custom_dir_jsonc_wins_over_json_inside_the_directory`, `oc_custom_dir_ignores_a_file_valued_environment_variable`, `oc_custom_dir_without_config_files_contributes_nothing`, `oc_custom_dir_stays_below_inline_content`, `oc_custom_dir_outside_the_read_roots_is_reported_as_blocked_not_ignored`, `oc_custom_dir_is_read_from_windows_shaped_paths`, `oc_custom_dir_absent_from_the_environment_is_a_clean_run` |

## Support-state summary

| State | Keys |
|---|---|
| `upstream_conformance_verified` | *(none — no loader harness)* |
| `fixture_verified` | the scalar/object/interpolation rows marked above |
| `documented` | `/shell`, `/attachment/image/*`, `{file:}` interpolation, managed file paths |
| `inventory_only` | every list/unknown/instruction/provider/agent key |
| `unsupported` | remote defaults content, macOS MDM preferences content, `.opencode` directory settings |
