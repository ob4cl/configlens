# Codex adapter contract (draft for ConfigLens 0.2)

Target: the **next** adapter after `claude` and `opencode`. Codex is currently listed as unsupported
(`src/adapters/mod.rs::Client::unsupported` → "0.2 backlog — TOML, trust gating and requirements model
not implemented"). This document is the source-cited contract that adapter must satisfy. It does not
implement anything; it is the evidence base and the acceptance shape.

Audit date (UTC): 2026-10-10. Evidence classes (see `compatibility-audit.md` §0): **source-reviewed**,
**fixture-verified**, **upstream runtime verified (not performed — blocked here, see §9)**.

## 1. Provenance

| Fact | Value | Source |
|---|---|---|
| npm package | `@openai/codex` `dist-tags.latest = 0.162.1` (published 2026-10-09T19:50:30.512Z) | npm registry (`evidence/provenance-checks.txt`) |
| GitHub repo | `openai/codex`, default branch `main` | GitHub API |
| Pin (tag → commit) | `rust-v0.162.1` → commit `092d3acd6bec3e3a14bdc7e7a2810ab628ab759d` (committer 2026-10-09T17:39:05Z) | GitHub API |
| Loader location | Rust workspace `codex-rs/`; config module `codex-rs/config/src/` | pinned tree |

The pinned loader is **open source and buildable**, which is why Codex can aspire to
`upstream_conformance_verified` (higher than either current adapter) once §9 runs.

## 2. Configuration files and locations (documented)

Primary sources: `https://developers.openai.com/codex/config-basic`, `.../config-advanced`,
`.../config-file/config-reference`, `.../enterprise/managed-configuration`.

| Role | Location |
|---|---|
| User config | `~/.codex/config.toml` (relocatable via `CODEX_HOME`) |
| Profile file | `$CODEX_HOME/<profile-name>.config.toml` (selected with `--profile`; top-level keys, not `[profiles.x]`; the legacy `[profiles.x]` table and top-level `profile = "x"` selector were **removed in 0.134.0**) |
| Project config | `.codex/config.toml` in the working directory and each parent up to the project root; **loaded only when the project is trusted**; the file closest to cwd wins |
| System config | `/etc/codex/config.toml` on Unix (docs); a Windows system path exists per the loader's Windows module |
| Managed defaults (legacy) | `/etc/codex/managed_config.toml` (Unix) / `~/.codex/managed_config.toml` (Windows/non-Unix) |
| Admin requirements | `/etc/codex/requirements.toml` (Unix) / `%ProgramData%\OpenAI\Codex\requirements.toml` (Windows) |
| macOS MDM | preference domain `com.openai.codex`, keys `config_toml_base64` (managed defaults) and `requirements_toml_base64` (requirements), base64 TOML |
| Cloud | enterprise-managed `config.toml` bundle and cloud-managed requirements (ChatGPT Business/Enterprise) |

All settings are **TOML** (not JSON/JSONC). `--config key=value` overrides are parsed as TOML; a value
that does not parse as TOML is taken as a string (`config-advanced`).

## 3. Layer precedence

### 3.1 Authoritative order from the pinned loader (source-reviewed)

`codex-rs/config/src/config_layer_source.rs`, `ConfigLayerSource::precedence()` — a setting from a layer
with a **higher** value overrides one with a lower value:

| Precedence | Layer (`ConfigLayerSource`) | Meaning |
|---:|---|---|
| 50 | `LegacyManagedConfigTomlFromMdm` | legacy `managed_config.toml` delivered by MDM |
| 40 | `LegacyManagedConfigTomlFromFile` | `/etc/codex/managed_config.toml` (or Windows path) |
| 30 | `SessionFlags` | `-c key=value` and flags such as `--model` |
| 25 | `Project` | `.codex/config.toml` (disabled while untrusted) |
| 21 | `User { profile: Some(_) }` | user config + selected `--profile` file |
| 20 | `User { profile: None }` | `~/.codex/config.toml` |
| 15 | `EnterpriseManaged` | cloud-managed config bundle |
| 10 | `System` | `/etc/codex/config.toml` |
| 0 | `Mdm` | macOS managed preferences (`config_toml_base64`) |
| -10 | `PackagedDefaults` | defaults shipped with the package |

Corroborated by `codex-rs/config/src/loader/README.md` (same stack, "top overrides bottom") and by
`ConfigLayerStack::layers_high_to_low()` / `origins()` (per-key winning layer).

### 3.2 The public docs describe a subset of the same stack

`config-basic` § "Configuration precedence" (highest first): CLI flags/`--config` → project
`.codex/config.toml` (root→cwd, closest wins, trusted only) → profile files → user config →
`/etc/codex/config.toml` → built-in defaults. This omits the legacy managed layers and the
enterprise-managed layer, which the enterprise page and the loader add. One retrieval of the developer
page also listed a "Cloud-managed `config.toml` defaults" level between user and system; the live page
fetched for this audit listed six ordinary levels and deferred managed layers to the enterprise page.
**The loader source is authoritative; treat the docs as a simplification.**

### 3.3 Conflict to resolve by execution (flagged, not decided)

The enterprise page (`.../enterprise/managed-configuration` § "Precedence and layering") states that
among **managed defaults** the order is "Managed preferences (macOS MDM; highest) → `managed_config.toml`
→ `config.toml`". The pinned loader instead gives `Mdm` precedence **0** (below `System`) and the
legacy `managed_config.toml` layers **40/50** (above `SessionFlags`). These two statements disagree on
where macOS MDM managed defaults sit relative to the legacy file. §9.1 (running the crate's own tests)
is the way to settle it; until then the adapter must not assert a single winner for a key set in both an
MDM preference and the legacy managed file.

## 4. Trust model (source-reviewed)

* `config-basic`: "If you mark a project as untrusted, Codex skips project-scoped `.codex/` layers,
  including project-local config, hooks, and rules. User and system config still load."
* `config-advanced`/`config-reference`: project config applies only when the project is trusted;
  trust is recorded in the **user** config as `[projects."/path"] trust_level = "trusted"`.
* Pinned loader: the project tree layer is "**loaded but disabled when untrusted**"
  (`codex-rs/config/src/loader/mod.rs` doc comment, `ProjectTrustContext`,
  `ProjectTrustDecision::is_trusted()`), i.e. it still appears in UI metadata with a `disabled_reason`
  and is ignored by `effective_config()`.

Adapter consequence: model trust as a **tri-state** like the Claude adapter (`Unknown`/`Trusted`/
`Untrusted`). `Trusted` → Project layer applicable; `Untrusted` → skipped with reason; `Unknown` →
`Conditional`. Because Codex itself loads-and-disables (rather than hiding) the project layer, ConfigLens
should keep the skipped layer in the report as an `Ignored` contribution, not delete it.

## 5. Merge semantics

* `codex-rs/config/src/merge.rs::merge_toml_values(base, overlay)`: recursive; for a key present in both
  as **tables**, merge key by key; otherwise the overlay value **replaces** the base value. **Arrays are
  replaced wholesale — Codex does not concatenate or union config arrays at merge time.**
* Special-cased paths in the pinned merge: `shell_environment_policy` filter representation switches
  (`filters` vs legacy `exclude`/`include_only`) replace each other's fields; `permissions.<n>.network.
  domains` and `application.network.domains` keys are host-normalised; `credentials.env` source arrays
  de-duplicate by source within structured-feature paths. These are client policy, not generic merge —
  keep them in the adapter, not in `resolve::operators` (spec §6, D15).
* Hooks are the documented exception to override semantics: `config-advanced` notes hooks from all
  layers **run concurrently** rather than one winning. If hooks are inventoried at all, they must be
  modelled as a union/`inventory_only`, never as a replace chain.
* Requirements merge differently again: "Tables combine one entry at a time" for
  `allowed_permission_profiles`, and "if an earlier layer sets a field (including an empty list), later
  layers do not override that field" per the enterprise page's field-level rules. Do not reuse the
  config merge for requirements.

## 6. Managed policy

Two distinct mechanisms (enterprise page):

1. **Admin-enforced requirements** (`requirements.toml`) — hard constraints the user cannot override.
   Constrained settings include approval policy, approvals reviewer, sandbox mode, permission profiles,
   web search mode, managed hooks, MCP server allowlist, plugin marketplace sources, and `[features]`
   pins. Delivery/precedence is documented "lower to higher" as: system `requirements.toml` (1) →
   enterprise cloud bundle (2) → legacy `managed_config.toml` fields reinterpreted as requirements (3)
   → macOS MDM `requirements_toml_base64` (4); higher-precedence layers override ordinary scalar/list
   values and tables merge by key. *(Note: older third-party snapshots of the same page state the
   requirements order the other way — "earlier wins", cloud first. The docs are mutable; pin the page
   version in the registry and prefer execution.)*
2. **Managed defaults** (legacy `managed_config.toml`) — starting values merged on top of the user
   `config.toml` and **above** CLI `--config` overrides; the user may change them during a session and
   they are re-applied at next launch. Cloud `config.toml` uses normal configuration precedence, not
   the legacy ordering.

Adapter consequence: treat requirements as a **distinct policy phase** (like `constrained` events), not
a merge layer; treat managed defaults as the two legacy layers with their loader precedence (40/50).

## 7. Key scope restrictions (documented, must not be invented)

`config-reference`/`config-advanced`: a project-local `.codex/config.toml` "can't override
machine-local provider, auth, host-owned app request metadata, notification, configuration profile
selection, or telemetry routing keys". Codex **ignores** these keys when they appear in a project file
and prints a warning:

```
openai_base_url, chatgpt_base_url, apps_mcp_product_sku, model_provider, model_providers,
notify, profile, profiles, experimental_realtime_ws_base_url, otel
```

The adapter should model this as a project-scope exclusion (`scope_excluded_from: [project]`), analogous
to Claude's `defaultMode` exclusion, and report the ignored entry rather than dropping it silently.
(One third-party source additionally names `responses_api_metadata`; it is **not** in the current
official list — do not add it without a primary citation.)

## 8. Proposed ConfigLens adapter contract

* **Client id**: `codex`; add `Client::Codex` to `src/adapters/mod.rs` (and remove the `codex` entry
  from `Client::unsupported` only when the rules below are implemented).
* **Syntax**: `Syntax::Toml` for every layer (a `src/parse/toml.rs` using `toml_edit`, per spec §6).
* **Layers** (rank = loader precedence, ascending), all reported lowest-first:
  `codex.packaged_defaults`(-10, unobservable), `codex.mdm`(-0/0, present-but-unparsed on macOS),
  `codex.system`(10), `codex.enterprise`(15, unobservable → gap),
  `codex.user`(20), `codex.user_profile`(21), `codex.project`(25, trust-gated),
  `codex.session_flags`(30, supplied-only), `codex.legacy_managed_file`(40),
  `codex.legacy_managed_mdm`(50, unobservable).
* **Source classes**: user→`User`; profile→`User`; project→`ProjectShared` (trust-gated);
  session flags→`SessionArgs` (supplied only); system/managed files→`ManagedFile`;
  MDM→`PlatformManaged`; enterprise→unobservable.
* **Operators**: `replace` for scalars, `object-merge` for TOML tables (recursive by key),
  `ordered-replace`/`replace` for arrays (arrays are replaced, not merged), `inventory-only` for
  hooks/MCP/provider/telemetry and every unknown key, `constraint` for `requirements.toml`.
* **Unknown keys**: `inventory_only` (spec §3 rule 6). Never attach a merge operator to an unlisted key.
* **Coverage gaps (always emitted)**: `codex.enterprise` (remote, not fetched); `codex.mdm`
  (macOS plist not parsed); `codex.system` absent if not present; untrusted/unknown project.
* **Redaction**: the MCP `mcp_servers.*.env`/`env_vars` maps and `shell_environment_policy` values are
  secret-bearing → redact subtrees; `model` is a public model-name key; `approval_policy`/
  `sandbox_mode`/`model_reasoning_effort` are public enums.
* **Support ceiling until §9 runs**: `documented` (docs) / `fixture_verified` (this repo's fixtures).
  Then `upstream_conformance_verified` once `cargo test -p codex-config` plus the added ordering/
  trust/scope fixtures pass against the pinned SHA. Note the ConfigLens ladder has **no slot for
  "pinned-source-reviewed"**; spec §3 says documentation alone does not establish runtime equivalence,
  so source review maps to `documented` until executed. Consider stating that explicitly.

## 9. Conformance harness (executable, safe) and its real status here

### 9.1 Codex loader tests (crate's own, synthetic inputs only)

```
git clone --depth 1 --branch rust-v0.162.1 https://github.com/openai/codex
cd codex && cargo test -p codex-config --lib
```

This runs `merge_tests.rs`, `loader/tests.rs`, `loader/stack_tests.rs`, `managed_project_discovery_tests.rs`
— pure functions over literal TOML, no credentials, no network model calls, no client launch. Add
read-only fixtures asserting: `ConfigLayerSource::precedence()` equals §3.1; the project layer is
disabled when `trust_level` is absent/untrusted; the project-scope ignored-key set in §7.

**Real outcome in the audit sandbox:** clone succeeded (128 MB); the build failed at
`openssl-sys = 0.9.111` for want of `libssl-dev` (`libssl.so` and `pkg-config openssl` are both absent
here). Full log in `evidence/upstream-loader-attempt.txt`. Run it on a host with OpenSSL headers — this
is the single cheapest path to a real `upstream_conformance_verified` rule set for any adapter in this
project.

### 9.2 Fixtures for this contract

`docs/research/fixtures/codex/` contains a synthetic user `config.toml` (with a `[projects]` trust
entry), a trusted-project `.codex/config.toml`, a `requirements.toml`, and a legacy
`managed_config.toml`. They are **not read by ConfigLens 0.1** (codex is unsupported); they define the
inputs the 0.2 adapter and its harness must accept, and they double as the shared vocabulary for the
audit. Expected outcomes to assert once the adapter exists are encoded in §3–§7.

## 10. Open questions (must be resolved by execution, not guessed)

1. `Mdm` vs legacy `managed_config.toml` precedence for the same key (§3.3).
2. Requirements precedence direction (system-first vs cloud-first) — page version drift (§6.1).
3. Whether `requirements.toml` live precedence now matches the "higher-precedence layers override"
   wording or the older "earlier wins" wording.
4. Exact Windows system config path and whether it participates offline.
5. Whether `[profiles.*]` in `config.toml` is fully inert (0.134.0+) or still read for back-compat.

## 11. Sources

Documentation (primary)
* `https://developers.openai.com/codex/config-basic` — Config basics (§ Codex configuration file;
  § Configuration precedence; untrusted semantics).
* `https://developers.openai.com/codex/config-advanced` — Advanced Config (§ Profiles; § One-off
  overrides; § Config and state locations; project-scope ignored keys).
* `https://developers.openai.com/codex/config-file/config-reference` — Configuration Reference
  (§ `config.toml`; project-scope ignored keys; § `requirements.toml`).
* `https://developers.openai.com/codex/enterprise/managed-configuration` — Managed configuration
  (§ Admin-enforced requirements; § Locations and precedence; § Managed defaults; § macOS managed
  preferences).
* `https://developers.openai.com/codex/agent-approvals-security` — sandbox/approval model context.

Pinned loader source `092d3acd6bec3e3a14bdc7e7a2810ab628ab759d` (`rust-v0.162.1`)
* `codex-rs/config/src/config_layer_source.rs` — `ConfigLayerSource` + `precedence()` (authoritative
  numbers), `format_config_layer_source`.
* `codex-rs/config/src/loader/README.md` — layering model, public surface, `disabled_reason`.
* `codex-rs/config/src/loader/mod.rs` — `ProjectTrustContext`, trust lookups, project layer
  loaded-but-disabled.
* `codex-rs/config/src/merge.rs` — `merge_toml_values` recursive merge; arrays replace.
* `codex-rs/config/src/loader/project_discovery.rs` — managed sources overlaid for project/trust
  discovery.

Provenance: `docs/research/evidence/provenance-checks.txt`;
execution attempts: `docs/research/evidence/upstream-loader-attempt.txt`.
