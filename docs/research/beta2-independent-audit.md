# Beta 2 — independent release / security / source audit

- Auditor: `researcher` agent (independent; not the author of the OpenCode adapter, the CI
  workflows, or the release scripts).
- Date: 2026-10-11 (UTC).
- Target: `$REPO`, dirty working tree at `HEAD 35ad132`
  (`git rev-parse HEAD` = `35ad132fc1190124f30d609ff1d1afaa659691db`). The tree contains
  uncommitted modifications (`src/`, `tests/`, `compatibility/opencode/*`,
  `.github/workflows/*`, `CHANGELOG.md`, `.gitignore`) and untracked additions
  (`deny.toml`, `fuzz/`, `scripts/generate_sbom.py`, `scripts/test_generate_sbom.py`,
  `docs/research/evidence/opencode-config-dir-20261011.txt`, `docs/demo/`, `demo/record*.sh|py`).
  All findings below were checked against the working tree as it stands, not against `HEAD` alone.
- Scope: read-only. No repository file other than this one was created or modified; no release,
  tag, push, contact or other external write was performed. No heavy `cargo` build was run
  (the parent/coder validate builds); no real configuration or secret was read.
- This report is a **static, source-level audit**. It makes **no runtime conformance claim**:
  nothing here asserts that ConfigLens behaves like a live OpenCode/Claude client at run time.

## 0. Method and commands actually executed

Independent verification used only read-only network fetches, `git`, and the already-installed
`cargo-deny 0.20.2`:

```
# upstream opencode at the pinned commit (fetched fresh, not from the repo)
curl -sSL -o paths.ts  https://raw.githubusercontent.com/sst/opencode/53d1eabb61e21162157817bf677da0a4ad3332e3/packages/opencode/src/config/paths.ts   # HTTP 200
curl -sSL -o config.ts https://raw.githubusercontent.com/sst/opencode/53d1eabb61e21162157817bf677da0a4ad3332e3/packages/opencode/src/config/config.ts  # HTTP 200
sha256sum paths.ts config.ts
#   cd86a34461b27caf1042f8cba140fbbed47790c4f30cd9691f87298e8d4d4444  paths.ts   (45 lines)
#   87a9071af1ddb04d65947dba49be3fb4c94ecf63e120f17ce46ee81ff25dd45a  config.ts  (701 lines)

# tag -> commit and npm versions
curl -sSL https://api.github.com/repos/sst/opencode/git/refs/tags/v1.18.35   # -> sha 53d1eabb..., repo now "anomalyco/opencode"
curl -sSL https://registry.npmjs.org/opencode-ai        # dist-tags.latest = 1.19.0
curl -sSL https://registry.npmjs.org/@opencode/cli      # dist-tags.latest = 2.0.26

# live upstream docs (both hostnames return identical bytes)
curl -sSL https://opencode.ai/docs/config/          # HTTP 200, 202320 bytes
curl -sSL https://docs.opencode.ai/docs/config/     # HTTP 200, 202320 bytes

# dependency policy (cargo-deny is not a build; it does not alter Cargo.lock)
export PATH="$HOME/.cargo/bin:$PATH"
cargo-deny check licenses bans sources   # -> bans ok, licenses ok, sources ok   (exit 0)
cargo-deny check advisories              # -> advisories ok                     (exit 0)
```

## 1. Upstream OpenCode — the `$OPENCODE_CONFIG_DIR` claims (the core verification)

**Verified TRUE.** Reading the pinned files fetched above (not the repo's evidence file):

- `config/paths.ts:23-41` — `directories()` returns
  `unique([ Global.Path.config, ...(project .opencode dirs), ...(home .opencode), ...(Flag.OPENCODE_CONFIG_DIR ? [Flag.OPENCODE_CONFIG_DIR] : []) ])`.
  The env value is a **directory**, appended **last**, and omitted entirely when unset. ⇒ the
  registry's `OC-CONFIGDIR-SOURCE-20261011` description of `paths.ts` is accurate.
- `config/config.ts:439-447` — inside the directories loop:
  `if (dir.endsWith(".opencode") || dir === Flag.OPENCODE_CONFIG_DIR)` then
  `for (const file of ["opencode.json", "opencode.jsonc"])` → `path.join(dir, file)` merged in that
  order. ⇒ the sources are the two files **inside** the directory, and **`opencode.jsonc` wins**
  because it is merged second. A file-valued variable contributes nothing.
- Merge order (`config.ts`): global `412-413` → `OPENCODE_CONFIG` `415-418` → project files
  `420-424` → directories loop `438-480` (`.opencode` dirs, then `OPENCODE_CONFIG_DIR`) → inline
  `482-490`. ⇒ ConfigLens ranks are correct: `oc.project=30 < oc.opencode_dirs=40 <
  oc.custom_dir=45 < oc.inline=50`, with `oc.global=10 < oc.custom_path=20 < oc.project=30`.

Independent corroboration of the version claims: the `v1.18.35` tag resolves to commit
`53d1eabb61e21162157817bf677da0a4ad3332e3`; npm `opencode-ai` `dist-tags.latest` is `1.19.0`; npm
`@opencode/cli` `latest` is `2.0.26`. The live v1 config page still lists the eight documented
sources in the same order the adapter mirrors (Remote → Global → Custom path → Project →
`.opencode` dirs → Inline → Managed files → macOS managed prefs), and it still contains the exact
sentence the registry quotes: *"The custom directory is loaded after the global config and
`.opencode` directories, so it can override their settings."* So the registry's evidence file
`docs/research/evidence/opencode-config-dir-20261011.txt` is faithful to the upstream source I
fetched (line numbers 415/420/432/437/439/440/441/482 all match).

### A1 (Low) — `registry.md` calls an adapted table "Verbatim"

- `compatibility/opencode/registry.md:33` — "Verbatim structure of the upstream 'Precedence order'
  section; later sources override earlier ones…" introduces a **9-row** table.
- But the upstream v1 page's "Precedence order" section has **8** entries and does **not** contain a
  custom-directory step at all (that lives in a separate "Custom directory" subsection). The table
  inserts `oc.custom_dir` as row 6, shifting inline/managed/macOS off the document's numbering.
- Impact: a reader can mistake a source-verified adapter layer for a documented one. The 8-step
  claim itself is correct (`registry.json:36` says "the same 8-step order" — true).
- Remedy (docs only): relabel `registry.md:33` e.g. "Rows 1–5 and 7–9 mirror the upstream 8-step
  order; row 6 (`oc.custom_dir`) is **source-verified** from the pinned loader, not documented."
  No code change and no change to support levels.

### A2 (Low) — `oc.opencode_dirs` "not_a_settings_document" is imprecise

- `compatibility/opencode/registry.json:66-71` models `oc.opencode_dirs` (rank 40) with
  `"observable": false`, `"unobservable_reason": "not_a_settings_document"`, location
  ".opencode directories (agents, commands, plugins)".
- Upstream `config.ts:439-447` **does** load `opencode.json`/`opencode.jsonc` from any `.opencode`
  directory, i.e. a `.opencode` directory can hold a settings document. The reason string therefore
  contradicts the pinned source even though the layer is honestly disclosed as a gap and
  `unsupported` in the support summary.
- Impact: cosmetic/accuracy only (the gap is still emitted, no wrong merge is performed), but the
  wording is an unsupported claim.
- Remedy: reason → `"opencode_dirs_settings_not_implemented"` (or similar), and tighten the registry
  text to say the *settings files inside* `.opencode` directories are not read, rather than calling
  the directory "not a settings document". No behaviour change.

### A3 (Info / lower confidence) — project layer reads one directory; upstream may merge many

- `src/adapters/opencode.rs:216-245` (`project_config_dir`) returns only the **nearest** ancestor
  directory that contains an `opencode.json(c)` and reads that directory's files.
- Upstream `paths.ts:10-21` (`ConfigPaths.files`) collects matches for
  `targets: ["opencode.jsonc","opencode.json"]` while walking `start → worktree` and then
  `.toReversed()`s them, i.e. it appears to merge `opencode.json(c)` from **every** ancestor
  directory up to the worktree, not just the nearest one.
- I did **not** confirm this by reading `FSUtil.up` (out of the requested scope), so this is a
  source-observed divergence, not a proven defect: a repo with `opencode.json` at the root and a
  different `opencode.json` in a nested cwd would be modelled from the nested file only.
- Remedy (verify first): either merge all ancestor project files, or record an explicit coverage gap
  for "multiple ancestor project files" rather than implying full project-file parity. Confirm
  against `FSUtil.up` before changing behaviour.

## 2. Release workflow — the "7 assets" layout

**Verified consistent.** `.github/workflows/prerelease.yml`:

- build job uploads `dist/*` per target = 3 archives (`*.tar.gz`/`*.zip`) **and** 3 `*.sha256`
  (`package_release.py:53-56` writes a sibling checksum);
- sbom job uploads 3 `*.cdx.json`;
- publish job (lines 166-179) stages `archives=(3)`, `sums=(3)`, `boms=(3)`, concatenates the three
  `*.sha256` into a single `SHA256SUMS`, verifies every checksum, then
  `gh release create … "${archives[@]}" "${boms[@]}" SHA256SUMS` and asserts
  `(.assets | length) == 7`.
- 3 archives + 3 SBOMs + 1 `SHA256SUMS` = **7**. The arithmetic in the workflow and in
  `docs/verification-log.md:248` ("seven-asset layout") is correct. The individual `*.sha256` files
  are deliberately not published as separate assets. Two independent paths
  (`prerelease.yml:170` and `:179`) assert the same count.

### B1 (Medium) — the runbook's release mechanism does not match the workflow that exists

- `docs/beta/release-runbook.md:35-40` instructs `git tag -a vX.Y.Z && git push origin vX.Y.Z` and
  `:43` says "The tag-triggered **`release.yml`** builds the matrix".
- There is **no `release.yml`** (`.github/workflows/` contains only `ci.yml` and `prerelease.yml`).
  The actual workflow is **manual** (`workflow_dispatch`, `prerelease.yml:4-9`) and requires an
  **existing reviewed tag**; the publish job itself runs `--verify-tag`. So the runbook describes a
  different trigger than the one implemented.
- Also `release-runbook.md:45` claims "Each built binary is smoke-tested (`--version`, `clients`,
  `schema --kind report`) inside the workflow before upload", but `prerelease.yml`'s build job
  (lines 73-83) has **no smoke-test step** — it builds and packages only. (`milestone-checklist.md`
  X2 still lists this as an unmet acceptance criterion, consistent with it being absent.)
- Remedy (docs): rewrite §1–§2 of the runbook to the real `workflow_dispatch` prerelease flow, or
  add the missing smoke-test step to `prerelease.yml` before the runbook claims it.

### B2 (Low) — runbook checksum and pointer drift

- `release-runbook.md:52` documents `sha256sum configlens-<target>.tar.gz > SHA256SUMS`; the
  workflow instead writes a per-asset `*.sha256` then `cat -- "${sums[@]}" > SHA256SUMS`
  (`prerelease.yml:171-172`). Equivalent result, different procedure.
- `release-runbook.md:71` links "supported keys in `docs/compatibility.md`" — that file **does not
  exist** (confirmed; also flagged as absent/avoid in `docs/maintaining.md:65-67`,
  `docs/implementation-status.md:52,215`). Dangling reference.
- Remedy: point at `compatibility/opencode/registry.md` / the README support table; align §3 wording.

## 3. Scripts — `package_release.py` / `generate_sbom.py`

Reviewed `scripts/package_release.py` (70 lines), `scripts/test_package_release.py`,
`scripts/generate_sbom.py` (219 lines), `scripts/test_generate_sbom.py`.

Good: both enforce the `vMAJOR.MINOR.PATCH-beta.N` tag regex (`package_release.py:19`,
`generate_sbom.py:182`); archives are reproducible (zip fixed date `1980-01-01`,
`package_release.py:38`; gzip `mtime=0`/tar `mtime=0`, `:44-52`); notices are limited to
`README.md`, `LICENSE-MIT`, `LICENSE-APACHE` (`:15`); the SBOM writer is create-new (`"x"`,
`generate_sbom.py:203`); the tool version is fail-closed (`:105-111`).

### C1 (Low) — SBOM builder-path redaction misses some shapes

- `scripts/generate_sbom.py:38-47` `HOST_PATH_PATTERNS` = `/home/`, `/Users/`, `/root/`,
  `/private/var/`, `/tmp/`, `\.cargo/registry`, `[A-Za-z]:\\`, `\\?\`.
- Windows `cargo-cyclonedx` output commonly uses **backslashes**, which `[A-Za-z]:\\` covers; but a
  forward-slash Windows path (`C:/Users/…`) would not match, and `/var/`, `/opt/`, `/srv/`,
  `/root` (bare) are also uncovered. `validate()` (`:173-176`) scans `json.dumps(document)`, i.e.
  it only catches what these patterns describe.
- Remedy: broaden to `r"[A-Za-z]:[\\/]"` and add `/var/`, `/opt/`, `/srv/`, plus a generic
  `r"file://[A-Za-z]:/"`; keep the existing test `test_validate_rejects_incomplete_documents`
  ("host path in a component") and add a `C:/…` case.

### C2 (Low) — the "reproducible" test cannot prove real-tool determinism

- `scripts/test_generate_sbom.py:113-135` feeds a **FakeRunner** that writes the same synthetic
  payload twice, then asserts byte-equality. That proves the script's own normalise/serialise path is
  stable, **not** that real `cargo-cyclonedx` output is reproducible.
- `generate_sbom.py:7-9` and `docs/verification-log.md:213` assert real byte-identical output across
  runs; `verification-log.md:247-249` itself lists "release-artefact re-verification" as **not**
  verified. `normalise` sorts `components` and top-level `dependencies` but not each entry's
  `dependsOn` list, so determinism relies on the upstream tool.
- Remedy: either capture the real two-run transcript in `verification-log.md`, or soften the
  docstring to "normalised for reproducibility" and note the real-tool run as pending.

## 4. Dependency and licence policy (`deny.toml`)

**Verified by execution.** `cargo-deny 0.20.2` on this tree:
`bans ok, licenses ok, sources ok` and `advisories ok` (all exit 0). The `allow` list
(`MIT`, `Apache-2.0`, `Unicode-3.0`, `Unlicense`) is sufficient for the current graph; `wildcards =
"deny"`, `allow = []` (bans) and `unknown-registry/unknown-git = "deny"` are coherent with the
comments; `[graph] all-features = true` and the empty advisories `ignore` list match the stated
policy. `Cargo.toml` declares `license = "MIT OR Apache-2.0"`, `rust-version = "1.85"`, and a real
`repository` URL. No finding here. (CI runs the same check in the `dependencies` job,
`ci.yml:72-89`, with `cargo-deny@0.20.2` pinned by commit SHA.)

## 5. Privacy / "support report"

The dedicated privacy command exists: `configlens support-report` (`src/cli.rs:100-106`,
`:223-226`, `:365-392`) always renders share mode regardless of `--paths`
(`cli.rs:369-370`), `--preview` writes nothing, and `--output` uses the atomic `0600` snapshot path
with no automatic upload. The URL-host pseudonymization the docs describe is real in the tree:
`redact::url_host` (`src/security/redact.rs:252-270`) and `pseudonymize_url_host` (`:276-281`), and
the test `share_mode_pseudonymizes_url_hosts_but_local_mode_keeps_them_for_diagnosis`
(`tests/security_leaks.rs:361-406`) asserts the host is *shown* in local mode and *replaced* by
`host_*` in share mode (text and JSON).

### D1 (High, docs) — `verification-log.md` still calls a fixed privacy issue an open gap

- `docs/verification-log.md:131-139` states the URL-host assertion "was removed … Share-mode host
  pseudonymization for URL authorities **remains a documented gap** (see
  `docs/implementation-status.md`)."
- That is **stale and contradicted** by: the implemented code (`redact.rs:252-281`), the passing
  test file (`tests/security_leaks.rs:361-406`), `docs/implementation-status.md:155`
  (`Share mode pseudonymizes URL hosts | [x]`), `docs/beta/milestone-checklist.md:58` (P3 "DONE",
  commit `1fae794`), `CHANGELOG.md:95-96`, and `SECURITY.md:59-61`. `docs/engineering-verification.md:36-43`
  also records the fix. The §10 gap list in `implementation-status.md` does **not** list it.
- Impact: the one document a reviewer trusts for "commands actually run" tells a reader a privacy
  hole is still open when it is closed (or vice-versa depending on which doc they read). For a
  privacy-facing tool this is the most misleading single line found.
- Remedy (docs): replace `verification-log.md:131-139` with the resolved state (the host gap was
  closed; local vs share behaviour is pinned by `share_mode_pseudonymizes_url_hosts_but_local_mode_keeps_them_for_diagnosis`).

### D2 (Medium, docs) — README does not document `support-report` (or `completions`)

- The README "Commands" table (`README.md:92-100`) and the options list (`:107-112`) omit both
  implemented subcommands: `support-report` (`src/cli.rs:100-106`) and `completions`
  (`src/cli.rs` `Command::Completions`). The README's privacy section (`:29-31`) tells users how to
  make a pasteable report without naming the command built for exactly that.
- Remedy (docs): add `support-report` (always share mode; `--preview` | `--output`) and `completions`
  to the README table/options.

## 6. Stale documentation and unsupported claims (consolidated)

Each row is a documentation statement that no longer matches the tree. None of these are code
defects; all are cheap doc fixes. (Line numbers are from the current working tree.)

| # | Sev | Location | Stale / unsupported statement | Reality / remedy |
|---|---|---|---|---|
| S1 | Med | `README.md:184-186` | "the declared MSRV is not yet verified" | `docs/verification-log.md:145-153` (§8.1) records `cargo +1.85.0 check … -> exit 0`, and `implementation-status.md:15` agrees. Update the README to "verified". |
| S2 | Med | `implementation-status.md:214` (§10.13) | "MSRV 1.85 is declared but not built" | Contradicts the same file's own env table (`:15`) and §8.1. Remove/correct item 13. |
| S3 | Low | `engineering-verification.md:190` | "MSRV 1.85 is declared but not built" | Same as S1/S2. |
| S4 | Med | `implementation-status.md:194-196` (§10.2) | "OpenCode loader commit … no replacement loader SHA was re-pinned" | Contradicted by `compatibility/opencode/registry.json:37` and `registry.md:16`, which pin `53d1eabb…` and cite `paths.ts`/`config.ts` (I fetched both at that commit). Remove/rewrite item 2. |
| S5 | Med | `milestone-checklist.md:159` (F3) | "OpenCode loader re-pin — Not re-pinned after upstream restructure" | Same as S4: it is re-pinned now. |
| S6 | Med | `milestone-checklist.md:74` (X4) | "Dependency audit + license check job — absent" | `deny.toml` + the `dependencies` CI job exist (`ci.yml:72-89`); my `cargo-deny` run passes. |
| S7 | Med | `milestone-checklist.md:97` (R5) | "SBOM / provenance — none" | `scripts/generate_sbom.py` + the `sbom` job (`prerelease.yml:92-125`) exist. |
| S8 | Low | `milestone-checklist.md:200` | "296 tests green … (no `cargo-fuzz` targets)" | `fuzz/` targets exist (`fuzz/{parse_documents,resolve_precedence,hostile_inputs}` per `CHANGELOG.md:24`); count conflicts with S10. |
| S9 | Low | `implementation-status.md:51` | "all five jobs green (run 38099457727)" | The current `ci.yml` adds `dependencies` and `fuzz` jobs that have **never** run in CI (`verification-log.md:247-249` says so). The cited run predates them. |
| S10 | Med | `CHANGELOG.md:97` (296) vs `verification-log.md:53-62` & `implementation-status.md:27` (270) vs `engineering-verification.md:159` (286) | Three different test totals, and per-target counts that understate the current files (`verification-log.md:60` "security_leaks 6" vs 13 `#[test]` in `tests/security_leaks.rs`; `:59` "rules_opencode 26" vs 34; `:56` "cli_contract 21" vs 24; `:58` "rules_claude 22" vs 25) | Counts drifted across sessions and were not re-synced. Reconcile from one fresh `cargo test` run (parent/coder to execute — I did not run it). |
| S11 | Low | `docs/beta/README.md:45-54` | "Truth baseline at 2026-10-10": CI workflows empty, `LICENSE-*`/`CHANGELOG.md`/`CONTRIBUTING.md` absent, "one local commit (`b2684c7`)" | Dated snapshot; all now present and `HEAD` is `35ad132`. Mark it as a historical baseline. |
| S12 | Low | `README.md:14-16` | "no benchmarks yet" | `benches/resolution.rs` runs and recorded numbers are in `implementation-status.md:181`; `milestone-checklist.md:219` says benchmarks are no longer open. |
| S13 | Info | `compatibility/opencode/registry.json:22-24` / `registry.md:13` | evidence URL `api.github.com/repos/**sst**/opencode/…` | `sst/opencode` now redirects to **`anomalyco/opencode`** (the same tag/commit resolves). Note the org rename so a future re-check is not confused. |

## 7. Things independently confirmed as correct (no action)

- Pinned commit ↔ tag ↔ files: `v1.18.35` → `53d1eabb61e21162157817bf677da0a4ad3332e3`;
  both `paths.ts` (45 lines) and `config.ts` (701 lines) exist there and match the registry's
  narrative and the `docs/research/evidence/opencode-config-dir-20261011.txt` transcript.
- The four ranks `30/40/45/50` and the json-then-jsonc / directory semantics are source-verified
  (section 1).
- Version claims: npm `opencode-ai` latest `1.19.0`, `@opencode/cli` latest `2.0.26`; the live v1
  docs page still lists the same order and still contains the registry's quoted custom-directory
  sentence.
- No rule claims `upstream_conformance_verified`: `registry.json:38` is `fixture_verified_max`;
  `README.md:145-148` and `implementation-status.md:192-193` say so. The **no runtime conformance
  claim** requirement is satisfied.
- The 7-asset prerelease layout is internally consistent (section 2).
- `cargo-deny` advisories / bans / licenses / sources all pass (section 4).
- Actions in both workflows are pinned by commit SHA and `permissions: contents: read` is set
  (write only in the publish job); `persist-credentials: false` throughout.
- `.gitignore` excludes `/dist`, `*.cdx.json`, `fuzz/artifacts`, `fuzz/Cargo.lock` and any
  `pilot-log.md` (tester personal data) — consistent with `generator`/`packager` not committing
  release material or tester records.
- `src/adapters/opencode.rs:105-133` implements exactly the source-verified rule: `oc.custom_dir`
  candidates are `<dir>/opencode.json` then `<dir>/opencode.jsonc`, and a file-valued variable
  yields nothing; `:198-207` makes OpenCode trust-independent (no `trust_unknown` gap), which
  matches the pinned loader having no workspace-trust concept.

## 8. Limitations of this audit

- Static only. I did **not** run `cargo test`, `cargo build`, `cargo fmt`, `cargo clippy`, the demo,
  or `actionlint` (heavy/out of scope; the parent and coder validate). `cargo-deny` was the only
  crate-adjacent tool executed.
- Test-count discrepancies (S10) are from counting `#[test]` attributes, which is **not** the same
  as a `cargo test` pass count (cfg-gated / platform tests differ). Treat S10 as "the recorded
  numbers disagree with the tree and with each other", not as the true totals.
- The `oc.project` multi-ancestor observation (A3) is inferred from `config.ts`/`paths.ts` only; I did
  not read `FSUtil.up`, so it is flagged for confirmation, not asserted as a defect.
- I could not observe a real GitHub Actions run: `prerelease.yml` has never run
  (`verification-log.md:247-249`), so all workflow reasoning is source-level.
- No real configuration, credential, or secret was read, and nothing in this audit was executed
  against a live OpenCode/Claude client.
