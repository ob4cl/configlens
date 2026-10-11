# ConfigLens private-beta compatibility audit

Audit of the bundled compatibility registries and adapters at repo `b2684c7`
(`src/adapters/{mod,claude,opencode}.rs`, `compatibility/{claude,opencode}/registry.{json,md}`,
`docs/spec/ConfigLens_Technical_Specification.md`), checked independently against primary upstream
sources. **Audit date (UTC): 2026-10-10.** No repository file outside `docs/research/` was changed.

This document is deliberately about *gaps and truthful support*, not marketing. Where the engine and a
primary source disagree, the disagreement is recorded and the claim is downgraded, not "fixed by
fixture".

---

## 0. Method, and the three evidence classes

Every rule below is cited to a **primary source with an exact location** (URL + section, or a pinned
file + symbol). Nothing is asserted from ConfigLens's own tests alone.

The audit uses exactly three evidence classes and never conflates them:

| Class | Meaning here | What backs it |
|---|---|---|
| **source-reviewed** | A primary upstream source — published docs, or the *pinned loader source* read at an exact commit — was read and the rule is transcribed from it. | URLs + pinned SHA (sources file) |
| **fixture-verified** | The **ConfigLens release binary** was run against the synthetic fixtures in `docs/research/fixtures/`, and the observed output is captured. This describes the *engine's* behaviour; it is *not* proof of upstream parity. | `docs/research/evidence/fixture-run.txt` |
| **upstream runtime verified** | The *upstream* loader code was executed on synthetic inputs. **Not performed in this audit.** Attempts and their real, blocking outcomes are recorded in `docs/research/evidence/upstream-loader-attempt.txt` (Codex: `openssl-sys` needs `libssl-dev`, absent; OpenCode: loader is TypeScript and `bun`/deps are absent; running the shipped clients is forbidden by spec §7 and by this task). | — |

Nothing in this audit claims `upstream_conformance_verified`. The registries themselves say the same
(`conformance_status = "fixture_verified_max"`), and that honesty is correct.

Provenance for every version/SHA check (raw command output): `docs/research/evidence/provenance-checks.txt`.

---

## 1. Verdict summary

### 1.1 Claimed versions and revisions — independently re-verified

| Registry claim (registry.json / registry.md) | Independent check | Result |
|---|---|---|
| Claude Code tested/documented `2.1.296`; npm `modified` 2026-10-09 | npm `@anthropic-ai/claude-code` `dist-tags.latest = 2.1.296`, version published 2026-10-09T16:58:10.924Z, `modified` 2026-10-09T19:31:26.129Z | **Confirmed** |
| Claude GitHub release `v2.1.296` published `2026-10-09T19:28:59Z` | `releases[0] = v2.1.296, published_at 2026-10-09T19:28:59Z` | **Confirmed exact** |
| Claude repo last `README.md` commit `b640d94a…`, 2026-01-12 | commit `b640d94a49629ef004ecceb5e2bd6d8aebf067ce`, committer 2026-01-12T23:12:24Z, "docs: update installation instructions in README" | **Confirmed** |
| Claude `pinned_commit: null` (closed source, no loader SHA) | `anthropics/claude-code` publishes docs/examples, not the settings loader; HEAD is 2301018b… (the v2.1.296 tag) | **Correct** |
| OpenCode tested/documented `1.18.35`; npm `modified` 2026-10-09 | npm `opencode-ai` `dist-tags.latest = 1.18.35`, published 2026-10-06T20:21:30.784Z, `modified` 2026-10-09T22:55:42.817Z | **Confirmed** |
| OpenCode repo HEAD `7b3d4ce3a7dbd2a6d3637722a0d5f22a7d086937`, `2026-10-10T15:29:34Z` | HEAD sha + committer date match exactly (`sst/opencode` now redirects to `anomalyco/opencode`) | **Confirmed exact** |
| OpenCode commit `44b6938b2ad2895bdd89d5d5cf753fb01ca8e3dd` "removed packages/opencode", `2026-07-21T01:54:22Z` | commit exists, message "removed packages/opencode", 300 files, committer 2026-07-21T01:54:22Z | **Confirmed** |
| OpenCode `pinned_commit: null` ("no replacement loader SHA re-pinned") | A replacement loader **does** exist at HEAD (`packages/core/src/config.ts`); see §3.1 | **Stale — see O1/O2** |

### 1.2 Rule/behaviour findings

| # | Client | Finding | Severity | Class |
|---|---|---|---|---|
| C1 | Claude | `cc.managed.hkcu` has `rank 90` — the **highest** precedence layer — but upstream ranks HKCU as the **lowest** managed source. The rank is inverted vs. both the docs and registry.md's own table. | correctness (inert while unobservable) | source-reviewed |
| C2 | Claude | `registry.json` `cc.project_shared.location` = `<project root>/.claude/settings.json`, but upstream reads shared project settings from the **session's primary working directory**, not the repo root. registry.md states it correctly. | wording/consistency | source-reviewed |
| C3 | Claude | The managed tier is modelled as a plain merge stack. Upstream default is **"first-wins"** across managed sources, plus a fixed set of cross-source keys, plus `managedSourcesBehavior`/`policyHelper`. None of that is modelled. | substantive gap | source-reviewed |
| C4 | Claude | Trust gating is **per-source**, applied to the whole shared *and* local project file. Upstream gates only `permissions.allow`/`additionalDirectories`/`extraKnownMarketplaces`/most `env`; `deny`/`ask` apply immediately, and local-file allow rules apply without trust while untracked. Visible in fixture D (both files dropped whole). | over-broad; disclose | source-reviewed + fixture |
| C5 | Claude | `registry.json` declares a `/modelPicker` rule (`ordered-replace`, `documented`) that `registry.md` omits and its summary calls *unsupported*; upstream additionally restricts `modelPicker` to managed/`--settings`/user (ignored in project/local), which is not modelled. | registry-internal + missing scope | source-reviewed |
| O1 | OpenCode | The tested version (`opencode-ai` 1.18.35, the **v1** line) and the only pinned SHA (`7b3d4ce3`, the **v2** dev line that ships as `@opencode/cli` 2.0.26) are different product lines. The v1 line's own tag is `v1.18.35 → 53d1eabb…`. | provenance | source-reviewed |
| O2 | OpenCode | At the pinned SHA the loader's precedence does **not** match the documented order the registry encodes (no separate `custom_path` rank, no `custom_dir`, no file-based managed tier). | substantive gap | source-reviewed |
| O3 | OpenCode | v1 "managed settings" = file `/etc/opencode` + macOS `ai.opencode.managed`; v2 "managed" = Console/cloud-compiled policy statements (`packages/core/src/managed-policy.ts`). The registry models only the v1 shape, against a v2 SHA. | substantive gap | source-reviewed |
| O4 | OpenCode | ConfigLens trust-gates `oc.project`, but upstream OpenCode has **no** workspace-trust concept in either the docs or the pinned tree (`grep -ri trust` finds none). The gate is a Claude-derived assumption. Visible in fixture I ("not unconditional (trust_unknown)"). | over-modelled | source-reviewed + fixture |
| O5 | OpenCode | `custom_dir` placement gap and `placement_verified:false` are correct and honest; upstream only says "after the global config and `.opencode` directories". | none (honest) | source-reviewed |

Confirmed-correct items (do not over-correct): Claude precedence order, strict-JSON dialect, list
concat+dedupe, `defaultMode` scope exclusion, env-pair semantics, `managed-settings.d/` merge order;
OpenCode local precedence for the **v1** docs, JSON/JSONC acceptance, `{env:}`/`{file:}`
interpolation, remote/`.opencode`-dir/macOS-prefs as unobservable.

---

## 2. Claude Code (`client: claude`)

### 2.1 Layer model vs. primary source

Primary source: `https://code.claude.com/docs/en/settings` § "Settings precedence" and
`https://code.claude.com/docs/en/managed-settings` § "How Claude Code combines managed sources".

Documented precedence, **highest first** (settings § "Settings precedence"):
1. Managed settings (`managed-settings.json`, MDM policy, or server-managed settings)
2. Command-line arguments (`--settings <json|file>`, per-key flags such as `--model`)
3. Project local `.claude/settings.local.json`
4. Shared project `.claude/settings.json` (the **session's primary working directory**)
5. User `~/.claude/settings.json` (`$CLAUDE_CONFIG_DIR/settings.json` when set)

Registry ranks: `cc.user`10 < `cc.project_shared`20 < `cc.project_local`30 < `cc.session_args`40 <
managed 60/70/80/90. **Order confirmed correct** for the five levels (fixture H's JSON shows the
`compatibility` block and `sources`; fixtures A/B show the observed replacement chain
user→shared→local).

Managed-tier **internals** (managed-settings § "How Claude Code combines managed sources"), documented
highest first: **remote/server-managed → MDM(MDM plist · HKLM) → managed settings files
(`managed-settings.json` + `managed-settings.d/*.json`) → HKCU registry**. HKCU is read "only when no
source above it delivers a policy key".

**Finding C1 — registry rank inversion.** `compatibility/claude/registry.json` sets
`cc.managed.file`=60, `cc.managed.mdm`=70, `cc.managed.server`=80, **`cc.managed.hkcu`=90**. Because
`Registry::layers()` sorts ascending and treats later as higher precedence, this makes **HKCU the
top-precedence managed layer**, the opposite of upstream (HKCU is the bottom fallback). registry.md's
own managed-tier table lists the correct order (1 server, 2 MDM, 3 file, 4 HKCU). While
`cc.managed.hkcu` is `observable:false`, the inverted rank is still a latent correctness bug and a
documentation inconsistency. Recommended: give HKCU a rank **below** `cc.managed.file` (e.g. 50), or
model HKCU as an explicit fallback rather than a ranked layer.

**Finding C2 — shared-project location string.** `registry.json` `cc.project_shared` says
`"<project root>/.claude/settings.json"`. Upstream: the shared file is read from the session's
**primary working directory** (settings § "Settings files and who they affect": "the .claude folder
inside your project"; settings § "Where Claude Code keeps the local file…": "Claude Code reads the
shared `.claude/settings.json` from the session's primary working directory"). The adapter code is
already correct (`claude.rs` uses `ctx.cwd`), and registry.md already says "the session's primary
working directory"; only the registry.json `location` string is wrong. Low severity, easy fix.

### 2.2 Managed-file / `managed-settings.d/` merge (confirmed)

Primary source: managed-settings § "Split a file-based policy across teams". Verbatim rules: the file
`managed-settings.json` **first**, then "every `*.json` file in the directory in alphabetical order",
"ignores hidden files and files that don't end in `.json`"; single values → later replaces; **lists →
combine with duplicates removed**; nested blocks → merge key by key; `fallbackModel` → later chain
replaces whole; `extraKnownMarketplaces`/`managedMcpServers` → later same-name entry replaces whole;
`modelPicker` → later lineup replaces whole.

The adapter (`claude.rs::managed_files`) and the unit test
`managed_file_and_directory_entries_are_merged_in_documented_order` implement exactly this ordering
(`managed-settings.json`, then `10-telemetry.json`, `20-security.json`, hidden/non-`.json` ignored).
The fixture set mirrors it. **Confirmed**, with the caveat that this tier is not CLI-visible under the
"write only docs/research" constraint (system path is fixed), so it is documented+unit-test verified,
not fixture-verified by this audit.

### 2.3 Managed-source combination — substantive gap (C3)

Primary source: managed-settings § "How Claude Code combines managed sources" and § "Compose every
managed source". Key documented behaviours **not modelled** by ConfigLens:

* Default `managedSourcesBehavior = "first-wins"`: "Claude Code uses the highest-ranked source that
  delivers at least one policy key and **ignores the rest rather than merging them**". ConfigLens's
  managed tier is a layer stack that would merge contributions from `file`, `mdm`, `server`.
* `managedSourcesBehavior = "merge"` ("Requires Claude Code v2.1.242 or later"): lists union, locks
  take the strictest value, restriction allowlists taken whole from the highest source, etc.
* "Keys read from every admin source" — a documented cross-source set (`sandbox.network.
  allowManagedDomainsOnly`, `allowAllClaudeAiMcps`, `deniedMcpServers`, `maxEffortLevel`, per-variable
  `env` merge, `forceRemoteSettingsRefresh`, …) with their own combination rules.
* `policyHelper` (managed-settings § "Compute the policy with a helper program"): when the selected
  source runs a helper that emits `managedSettings`, that object "is the only managed settings for the
  session".

Consequence: for keys set in more than one managed source, ConfigLens can currently produce a merged
result where upstream would apply one source and ignore the others. Because managed sources are
`observable:false` except the file tier, the practical exposure is limited to the file tier itself;
the honest fix is to model `managedSourcesBehavior` and the cross-source set, or to state in the
registry that only single-source managed resolution is claimed.

### 2.4 Trust gating — over-broad (C4)

Primary sources: settings § "A committed key doesn't reach teammates"; permissions § "Project allow
rules and workspace trust"; settings § "Where Claude Code keeps the local file in a git repository".

Documented: from the **shared project** file, `permissions.allow`, `permissions.additionalDirectories`,
`extraKnownMarketplaces`, and most `env` values "apply only after each teammate trusts the folder";
"`deny` and `ask` rules apply right away". Separately, the **local** file: "Its allow rules don't wait
for trust while the file stays untracked. … If the file is tracked by git, the trust step applies to
it too."

ConfigLens implementation: `adapters/mod.rs::default_applicability` maps both `ProjectShared` and
`ProjectLocal` to `Skipped("workspace_untrusted")` when `Trust::Untrusted` and `Conditional` when
`Unknown`, at **source granularity**. Fixture D shows the whole shared file *and* the whole local file
being dropped when untrusted, including their `deny`/`ask` entries — whereas upstream keeps `deny`/`ask`
and (for an untracked local file) the local `allow` rules. registry.md overstates fidelity when it says
only "those contributions are `Conditional`".

This is conservative (it never *adds* authority), but it is a semantic divergence and must be disclosed
as a known limitation, or refined to key/pair granularity. Recommended wording for the registry: trust
gating in 0.1 is file-granular and therefore can under-report `deny`/`ask` from an untrusted project.

### 2.5 List merges and special keys (mostly confirmed)

* `permissions.allow/ask/deny` → stable concat + dedupe, **provenance only, no authorization**.
  Source: settings § "Lists merge instead of overriding" ("combines the lists instead of picking one")
  and managed-settings § "Split a file-based policy…" ("combine, with duplicates removed"). Fixture E
  reproduces this and prints the "not an authorization decision" caveat. **Confirmed.**
* `fallbackModel` → ordered whole-replace from the highest file that defines it (settings § "Lists
  merge…", bullet on `fallbackModel`). Registry `ordered-replace`. **Confirmed.**
* `permissions.defaultMode` → values `auto` and `bypassPermissions` "don't take effect from project or
  local settings"; "Before v2.1.257, `bypassPermissions` took effect from any file" (settings §
  "A value you set is ignored", and settings-reference). Registry models `scope_excluded_values:
  ["auto","bypassPermissions"]` on `cc.project_shared`/`cc.project_local`. **Confirmed**; the
  pre-v2.1.257 version boundary remains a documented, disclosed gap (implementation-status §10 item 5).
* `availableModels` → managed list applied as-is, user/project/local additions ignored unless an
  embedding app supplies its own list (settings § "Lists merge…"). Registry models this as a
  `constraint`. **Confirmed as a shape**; the sibling keys `availableModelsMatch` (v2.1.291+ behaviour),
  `enforceAvailableModels`, `deniedModels` (v2.1.273+) are not modelled — acceptable inventory gap.
* `ANTHROPIC_MODEL` overrides the `model` key from any file; `ANTHROPIC_DEFAULT_MODEL` applies "only
  when no file sets `model`" (settings § "Settings precedence", environment paragraph). Registry
  `env_pairs` matches word for word. **Confirmed.**

**Finding C5 — `/modelPicker` inconsistency.** `registry.json` declares `/modelPicker` with operator
`ordered-replace`, `support: documented`, `model_name: true`, and cites `S3`. registry.md's
supported-keys table omits `/modelPicker` and its "Support-state summary" lists "`modelPicker`/
`modelSettings` semantics" as **unsupported**. Upstream (settings § "Lists merge…") says `modelPicker`
"takes the whole value from the highest of managed settings, `--settings`, and user settings that
defines it, and **ignores the key in project and local settings**" (v2.1.242+). So the registry.json
rule claim is under-specified (no scope exclusion) and contradicts registry.md. Either downgrade
`/modelPicker` to `inventory_only` in both files, or model its three-source scope as
`scope_excluded_from: [cc.project_shared, cc.project_local]`.

### 2.6 Dialect and version boundaries

* Strict JSON: settings § "Edit a settings file" — "Settings files are strict JSON: a `//` comment or a
  trailing comma is a syntax error". Adapter returns `Syntax::Strict`. **Confirmed.**
* Version boundaries seen in the current docs that are relevant and consistent with the registry's
  list: v2.1.193, v2.1.211 (local-file-at-root), v2.1.223 (`env` per-variable admin merge), v2.1.229,
  v2.1.238, v2.1.242 (`managedSourcesBehavior`, `modelPicker`, `enableArtifact`), v2.1.246, v2.1.257
  (`bypassPermissions` from project/local), v2.1.259, v2.1.267 (`maxEffortLevel`), v2.1.273
  (`allowManagedMcpServersOnly`), v2.1.277, v2.1.282, v2.1.283. registry.md's "documented version
  boundaries" list is a correct subset; only `2.1.211` is behaviourally modelled, which matches the
  disclosed gap.

---

## 3. OpenCode (`client: opencode`)

### 3.1 Provenance: the version and the pinned SHA are on different product lines (O1)

At audit time two OpenCode npm lines are published and the repo carries both tags:

* `opencode-ai` `dist-tags.latest = 1.18.35` — **the v1 line the registry tests.**
  Its Git tag exists: `refs/tags/v1.18.35 → commit 53d1eabb61e21162157817bf677da0a4ad3332e3`.
* `@opencode/cli` `dist-tags.latest = 2.0.26` — **the v2 line**, `repository =
  git+https://github.com/anomalyco/opencode.git`, whose GitHub tags are `v2.0.26 … v2.0.x`.
* The repo HEAD the registry pins, `7b3d4ce3…` (default branch `dev`), belongs to the **v2** line.

So `compatibility/opencode/registry.json` pairs `tested_client_versions: ["1.18.35"]` (v1) with a
`pinned_commit`-adjacent evidence note whose SHA is the **v2** HEAD, while `pinned_commit` is `null`.
The registry.md "Pinned loader commit" sentence creates the impression that no loader exists for the
current layout; in fact a replacement loader exists (`packages/core/src/config.ts`) but it is the *v2*
loader, unrelated to 1.18.35. **Recommended:** either pin `53d1eabb…` (v1) as the loader for the
1.18.35 claim, or re-scope the adapter to the v2 line (`@opencode/cli` 2.0.26) and pin `7b3d4ce3…`
with the v2 doc set. Do not mix.

### 3.2 Documented precedence (v1) vs. the pinned loader (v2) (O2)

Primary source for the registry's model: `https://opencode.ai/docs/config/` (identical content under
`docs.opencode.ai/docs/config/`) § "Precedence order" (later overrides earlier):
remote `.well-known` → global → `OPENCODE_CONFIG` → project → `.opencode` directories → inline
`OPENCODE_CONFIG_CONTENT` → managed files → macOS managed preferences. This is a **v1** page (the site
carries a "New OpenCode v2 is now available" banner).

Pinned loader at `7b3d4ce3` (source-reviewed): `packages/core/src/config.ts::load()` assembles entries
lowest→highest explicitly as:

```
[ ...loadWellknown(), ...globalSupplementary, ...explicit, ...direct, ...projectSupplementary, ...content ]
```

i.e. **remote well-known → global → explicit (`sources.explicit`, the `--config`/file override) →
direct (`opencode.json(c)` files walking up the tree) → project (`.opencode` directories) → content
(`OPENCODE_CONFIG_CONTENT`)**, with `latest()` picking the last entry that defines a key for conflicting
keys (no list union in this module). This matches the **v2** docs
(`https://opencode.ai/v2/docs/config` § "Locations": global → direct files farthest→closest →
`.opencode` files, "every discovered `.opencode` config overrides every direct config") — **not** the
v1 eight-layer order in the registry.

Consequences for the registry, if it keeps the 7b3d4ce3 pin: `$OPENCODE_CONFIG` exists but the loader's
`explicit` is the `--config` option, not the env var; there is no distinct `custom_dir` rank; there is no
file-based managed tier; macOS managed preferences are absent from this module. Registry ranks
(`oc.custom_path`=20, `oc.custom_dir`=45, `oc.managed_file`=60, `oc.managed_macos_prefs`=70) are not
corroborated by the pinned source.

### 3.3 Managed config is a different mechanism in v2 (O3)

Source-reviewed: `packages/core/src/managed-policy.ts` defines the managed model as **policy statements
compiled by the connected OpenCode Console** (`ConfigPolicy.Info[]`, from `experimental.policies` in
config documents plus `organization.statements`), with `decision()` returning allow/deny by matching
`action`/`resource`. This is not the v1 file-based `/etc/opencode` managed config nor the
`ai.opencode.managed` plist. The v2 docs add an "experimental.policies" and "Console-managed policy"
concept. If ConfigLens advertises OpenCode v1 managed files, it must pin v1; if it moves to v2, the
managed layer must be re-described as Console/cloud policy statements (unobservable offline).

### 3.4 Trust is not an upstream OpenCode concept (O4)

The config docs (v1 and v2) document no trust gate for project config, and a **content** grep of the
pinned tree (`packages/core/src` at `7b3d4ce3`) finds only incidental uses of the word — a TLS/CA
"trust" comment, a "trusted GitLab OAuth application", prompt text, and an "untrusted boundary"
comment — with no workspace/project-trust machinery and no `trust_level`/`trusted project` key. Yet
the adapter routes `oc.project` through
`default_applicability` (`SourceClass::ProjectShared`), so an unknown/untrusted project is reported as
conditional/skipped — visible in fixture I and in fixture J's diagnostics. This is an assumption
inherited from the Claude adapter. Recommend removing the trust gate for OpenCode (project config is
not trust-gated upstream) or, if ConfigLens deliberately keeps a user-declared caution, renaming it so
it is not presented as upstream behaviour.

### 3.5 Confirmed OpenCode behaviours

* JSON **and** JSONC accepted, trailing commas allowed: docs § "Format"; loader uses `jsonc-parser`
  `parse(text, errors, { allowTrailingComma: true })` in `packages/core/src/config.ts` and
  `packages/core/src/config/discovery.ts` names `["opencode.json", "opencode.jsonc"]`. Registry
  `Syntax::Jsonc`. **Confirmed** (fixture I discovers both dialects; fixture notes flag coexistence).
* `{env:NAME}` / `{file:path}` interpolation: docs § "Variables"; loader calls
  `ConfigVariable.substitute`. Registry models them. **Confirmed** (fixture-level for taint, per the
  in-repo tests).
* `.opencode` directories = agents/commands/plugins, not a settings document; remote `.well-known`
  never fetched; macOS managed preferences present-but-unparsed: docs § "Precedence order" / "Managed
  settings". Registry marks all three unobservable. **Confirmed correct.**
* Custom directory placement: docs § "Custom directory" only says it "is loaded after the global config
  and `.opencode` directories". Registry's `placement_verified:false` + gap is **honest and correct**.

---

## 4. Registry hygiene notes (both clients)

* `registry.json` vs `registry.md` drift: the `/modelPicker` rule (C5) and the `cc.project_shared`
  location (C2) are the two concrete cases. Both files should be generated from one source or
  cross-checked.
* Every non-`inventory_only` rule in both registries already carries `evidence` and `tests`, and the
  unit test `advertised_support_levels_never_exceed_the_conformance_status` enforces the ceiling. Keep
  that gate; it is the reason no rule is mislabelled `upstream_conformance_verified`.
* `tested_client_versions` and `documented_client_versions` are identical single values in both
  registries. That is honest (no tested version range is published) and matches spec §3.

---

## 5. Executable upstream-conformance approach (design — safe by construction)

The spec (§8) and this task both forbid: real credentials, launching agents/plugins, network model
calls, and running the shipped clients as oracles. The harness below therefore executes **only** pinned
loader code against **synthetic** inputs, in an isolated scratch directory, with network and
credential stores disabled. It is split into "runs today on a suitable host" and "needs a host package".

### 5.1 Codex — the crate's own tests (ready to run; blocked here only by `libssl-dev`)

```
# isolated scratch; no credentials, no network at run time (deps fetched once by cargo)
git clone --depth 1 --branch rust-v0.162.1 https://github.com/openai/codex
cd codex && cargo test -p codex-config --lib        # merge_tests.rs, loader/tests.rs, stack_tests.rs
```

These tests already encode the exact semantics this audit cites: recursive TOML merge, "project layer
loaded but **disabled** when untrusted" (`loader/mod.rs` lines ~132-134, `ProjectTrustContext`), and
`ConfigLayerSource::precedence()` ordering. Add a read-only fixture asserting the numeric ordering in
§`codex-adapter-contract.md` and the ignored-in-project key set. **Real outcome of attempting this in
the audit sandbox:** build failed at `openssl-sys = 0.9.111` for want of `libssl-dev`
(`evidence/upstream-loader-attempt.txt`). Run it on a host that has OpenSSL headers.

### 5.2 OpenCode — pin one line, then run the loader against a temp dir

```
# v1 claim: pin the tag, not HEAD
git clone --depth 1 --branch v1.18.35 https://github.com/anomalyco/opencode   # -> 53d1eabb...
# v2 claim: pin HEAD and the v2 package
git clone --depth 1 --branch dev https://github.com/anomalyco/opencode          # -> 7b3d4ce3...
# then (bun or node with deps installed), drive Config.load over a temp XDG dir with
# OPENCODE_CONFIG_CONTENT set to a fixture, wellknown/credential services stubbed to empty.
```

Assert: (a) layer order equals `load()`'s array order in `packages/core/src/config.ts`; (b) `latest()`
picks the last defining entry; (c) `{env:}`/`{file:}` substitution; (d) no trust gate. **Real outcome
here:** not run — the loader is TypeScript and `bun`/its dependency graph are absent; installing them is
out of scope for a "no network model calls / isolated" audit run.

### 5.3 What the harness must record

For each run: pinned SHA, command, toolchain version, fixture bytes, and a pass/fail per asserted rule,
stored next to the fixtures. Until a rule has an entry, its support label must stay at most
`fixture_verified` (the current ceiling).

---

## 6. Recommended actions (truthful support, no marketing)

1. **C1** Fix the `cc.managed.hkcu` rank (or model HKCU as a fallback). Reconcile registry.json with
   registry.md's managed-tier table.
2. **C2** Correct `cc.project_shared.location`; keep the adapter's cwd-based discovery.
3. **C3** Either model `managedSourcesBehavior` + the cross-source key set, or state in the registry
   that only single-source managed resolution is claimed and mark multi-source managed keys
   `partial`.
4. **C4** Disclose trust as **file-granular** in 0.1 (it can drop `deny`/`ask` and untracked-local
   `allow` from an untrusted project), or refine to key/pair granularity.
5. **C5** Make `/modelPicker` consistent (downgrade to `inventory_only` or model its three-source
   scope).
6. **O1** Pick **one** OpenCode line: pin `53d1eabb…` (v1.18.35) for the current registry, or move to
   `@opencode/cli` 2.0.26 / `7b3d4ce3…` for v2. Do not pair a v1 version with a v2 SHA.
7. **O2/O3** If staying on v2, re-describe layers and managed config to match the pinned loader and the
   v2 docs; if staying on v1, pin and cite the v1 docs and tag.
8. **O4** Remove the OpenCode project trust gate (no upstream concept) or clearly relabel it as a
   ConfigLens caution, not upstream semantics.
9. Keep `conformance_status = "fixture_verified_max"` until §5 actually runs and passes.

---

## 7. Sources (primary, with locations used)

Claude Code
* Settings/precedence/lists/trust/version boundaries — `https://code.claude.com/docs/en/settings`
  (§ Settings precedence; § Lists merge instead of overriding; § A committed key doesn't reach
  teammates; § Exceptions to managed settings precedence; § Edit a settings file; § Where Claude Code
  keeps the local file in a git repository; § A value you set is ignored).
* Managed tier, `managed-settings.d/` merge, managed-source combination, `policyHelper` —
  `https://code.claude.com/docs/en/managed-settings` (§ Split a file-based policy across teams;
  § How Claude Code combines managed sources; § Compose every managed source; § Keys read from every
  admin source; § Compute the policy with a helper program; § Where each mechanism stores the policy).
* Permission/trust — `https://code.claude.com/docs/en/permissions` (§ Project allow rules and
  workspace trust; § When your local settings file needs trust).
* Version/publish facts — npm `@anthropic-ai/claude-code`; GitHub `anthropics/claude-code` releases,
  tags, commits (see `evidence/provenance-checks.txt`).

OpenCode
* v1 docs — `https://opencode.ai/docs/config/` (also `https://docs.opencode.ai/docs/config/`)
  (§ Format; § Locations / Precedence order; § Custom path; § Custom directory; § Managed settings;
  § Variables).
* v2 docs — `https://opencode.ai/v2/docs/config` (§ Locations; § Permissions; § Policies).
* Pinned loader (v2 dev), `7b3d4ce3a7dbd2a6d3637722a0d5f22a7d086937` —
  `packages/core/src/config.ts` (`load()` order, `latest()`), `packages/core/src/config/discovery.ts`
  (`names`, discovery), `packages/core/src/managed-policy.ts` (Console policy statements),
  `packages/cli/src/config/config.ts` (CLI-local `cli.json`).
* Version/tag facts — npm `opencode-ai`, npm `@opencode/cli`; GitHub `anomalyco/opencode` tags/commits
  (`v1.18.35`, HEAD, `44b6938b…`).

Codex (see `codex-adapter-contract.md` for the full citation list)
* `https://developers.openai.com/codex/config-basic`, `.../config-advanced`,
  `.../config-file/config-reference`, `.../agent-approvals-security`,
  `.../enterprise/managed-configuration`.
* Pinned loader `092d3acd6bec3e3a14bdc7e7a2810ab628ab759d` (`rust-v0.162.1`) —
  `codex-rs/config/src/config_layer_source.rs`, `codex-rs/config/src/loader/README.md`,
  `codex-rs/config/src/loader/mod.rs`, `codex-rs/config/src/merge.rs`,
  `codex-rs/config/src/loader/project_discovery.rs`.

Artifact map for this audit:
* `docs/research/compatibility-audit.md` (this file)
* `docs/research/codex-adapter-contract.md`
* `docs/research/fixtures/` + `docs/research/fixtures/README.md`
* `docs/research/run-fixtures.sh`
* `docs/research/evidence/fixture-run.txt` (real engine output)
* `docs/research/evidence/provenance-checks.txt` (real version/SHA checks)
* `docs/research/evidence/upstream-loader-attempt.txt` (real, blocked upstream-load attempts)
