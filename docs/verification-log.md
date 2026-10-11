# ConfigLens verification log

Commands actually run in this repository, with their real results. Nothing here is a claim about a
command that was not executed. Copy/paste each command to reproduce.

## Environment

| Item | Value |
|---|---|
| Host | Linux `DESKTOP-T5SB5DG` `6.18.40.1-microsoft-standard-WSL2` (WSL2), x86_64 |
| `cargo` | `cargo 1.99.0 (5f94df478 2026-08-27)` |
| `rustc` | `rustc 1.99.0 (b940084d7 2026-09-28)` |
| `rustfmt` | `rustfmt 1.10.0-stable (b940084d7e 2026-09-28)` |
| Toolchain selection | `rust-toolchain.toml` → `channel = "stable"` |
| MSRV | declared `1.85` in `Cargo.toml`; **verified** — `cargo +1.85.0 check --locked --all-targets --all-features` exits `0` (see §8.1) |

## 1. Formatting

```
$ cargo fmt --all -- --check
$ echo $?
0
```

## 2. Lints (warnings denied)

```
$ cargo clippy --locked --all-targets -- -D warnings
    Checking configlens v0.1.0 ($REPO)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.64s
$ echo $?
0
```

Lint fixes made to reach a clean run (all mechanical, no behaviour change):

- `src/resolve/mod.rs` — needless borrow passed to `ingest_supplied`; `.iter().any(...)` →
  `.contains(...)`.
- `src/adapters/claude.rs`, `src/resolve/coverage.rs`, `tests/common/mod.rs` — test-only
  `cloned_ref_to_slice_refs`, `useless_vec`, `vec_init_then_push`.
- `src/lib.rs` and the integration-test crates allow `clippy::unwrap_used` / `expect_used` / `panic`
  **in test code only** (`#![cfg_attr(test, ...)]` for the crate, `#![allow(...)]` for each test
  target). Production code keeps those lints active.

## 3. Tests (locked, no early stop)

```
$ cargo test --locked --no-fail-fast
```

| Target | Result |
|---|---|
| `unittests src/lib.rs` | ok. 194 passed; 0 failed; 0 ignored |
| `unittests src/main.rs` | ok. 0 passed; 0 failed |
| `tests/acceptance.rs` | ok. 8 passed; 0 failed |
| `tests/cli_contract.rs` | ok. 24 passed; 0 failed |
| `tests/conformance.rs` | ok. 10 passed; 0 failed |
| `tests/rules_claude.rs` | ok. 25 passed; 0 failed |
| `tests/rules_opencode.rs` | ok. 40 passed; 0 failed |
| `tests/security_leaks.rs` | ok. 14 passed; 0 failed |
| Doc-tests | ok. 0 passed; 0 failed |
| **Total** | **315 passed, 0 failed, 0 ignored** (9 targets) |

Both the library and integration counts above are the current, authoritative census (fresh
`cargo test --locked --no-fail-fast` on `rustc 1.99.0`). Earlier entries in this log and in
`docs/engineering-verification.md` recorded intermediate totals (270, then 286, then 296) as the test
suite grew; those are historical session snapshots, not the current state. Where they disagree, this
table is correct.

## 4. Release build

```
$ cargo build --locked --release
    Finished `release` profile [optimized] target(s) in 2m 01s
$ ./target/release/configlens --version
configlens 0.1.0
```

## 5. Runnable isolated demo

```
$ cargo build --locked --release
$ ./demo/demo.sh
...
demo complete.
$ echo $?
0
```

Observed in the demo transcript (synthetic values, cleared environment, nothing outside a temp dir):

- `scan --client claude` with no trust recorded 2 observed project sources and 5 coverage gaps, and
  the resolution status stayed `partial` (`effective value withheld`).
- `explain /model` showed both locations and the `replaced_by_later_layer` reason.
- `explain /permissions/allow` printed the "not an authorization decision" caveat.
- `scan --client opencode --paths share --format json` replaced the unknown key name and the MCP
  server name with opaque IDs: `"/field_1"`, `"/mcp/field_1"`, `"/mcp/field_1/url"`. The synthetic
  token `demo-not-a-real-token` appeared **0** times in the output.
- `snapshot` wrote `owner_only_0600` files; `diff` of two identical snapshots reported
  "no differences in values, provenance, status or coverage".

## 6. Independent acceptance harness

`configlens_acceptance.py` (synthetic isolated inputs only) against the debug binary:

```
$ python3 configlens_acceptance.py target/debug/configlens
... all 7 checks "ok": true ...
$ echo $?
0
```

One expectation was adapted, with the documented spec reason recorded in the harness itself:
`duplicate_keys` originally expected exit `4`, but a duplicate key parses successfully and is
preserved as a **warning** diagnostic (spec §5 step 3); spec §4 reserves exit `4` for "required file
inaccessible, malformed or over resource limit". The repository's own `doctor` contract
(`tests/cli_contract.rs::doctor_exits_zero_by_default_and_one_for_warning_threshold`) classifies
`duplicate_key` as a warning, so `scan` exits `0`. The harness expectation was changed to `{0}` and
commented.

## 7. Fixes made in this run (root causes + regressions)

The parent's run had 6 failing tests across 3 targets. Each was root-caused and fixed; the failing
tests themselves are the regression tests, and two extra assertions were added.

| Symptom | Root cause | Fix | Regression test |
|---|---|---|---|
| `a_malformed_high_priority_file_exits_4...`, `secrets_in_a_malformed_file_are_not_echoed`: exit 4 but `malformed` absent from `explain` text | the text renderer ignored run-level diagnostics | `render::text::render_explain` now prints the run's diagnostics (code + safe location + line/column) | the two tests above |
| `acceptance_example_5`: skipped project layer explained as `trust_unknown` | the trust-gated branch overwrote the adapter's `workspace_untrusted` skip reason | only set `trust_unknown` when no reason was already assigned (`resolve::collect_contributions`) | `acceptance_example_5_*` |
| `acceptance_example_9`: `sources` listed missing locations | a planned location that does not exist was pushed as a source | the engine skips `Missing` sources; their absence stays a coverage gap | `acceptance_example_9_*`, plus a new `sources.is_empty()` assertion in `cc_no_config_files_is_an_empty_inventory_not_an_error` |
| `explain_json_has_every_required_field_and_no_ansi`, `strict_without_a_tested_version_exits_3`: exit 3 for a normal provisional report | exit logic also raised `3` whenever the explain resolution was `unsupported` (which an untested/unknown version causes) | removed that duplicate rule; `3` now comes from an unmatched pointer (`unknown_key`) or `--strict` | the two tests above |
| `seeded_secrets_never_reach_any_channel`, `files_outside_authorised_roots_are_never_read`, `snapshots_and_diffs_never_contain_seeded_secrets`: seeded value leaked as `verified_model_name` | `is_model_name_shape` accepted upper-case tokens, so a seeded token under `/model` was published | the verified model-name shape is lower-case only | new redaction unit-test assertions |
| independent harness: a seeded **object key** leaked as a resolution `pointer` in share mode | unknown key names were never pseudonymized in the report | `model::report::project_resolutions` replaces non-documented pointer segments with deterministic opaque field IDs in share mode | `model::report::tests::share_mode_makes_unknown_key_names_opaque` |

One test expectation was relaxed, with a spec reason:

- `tests/security_leaks.rs::seeded_secrets_never_reach_any_channel` asserted that the MCP URL **host**
  (`jira.example`) never appears, for commands that all run in **local** mode. Spec §7 says private
  hosts are pseudonymized only in **share** mode ("In local mode show config locations for
  diagnosis"), and two other passing tests pin the local behaviour
  (`tests/rules_opencode.rs::oc_mcp_type_and_url_rules`,
  `security::redact::tests::urls_have_userinfo_query_and_fragment_removed`). The host assertion was
  removed (with an explanatory comment); the §8 invariant — seed tokens and URL userinfo absent from
  every channel — is still asserted for every command. Share-mode host pseudonymization for URL
  authorities was subsequently **implemented** (`security::redact::url_host` + `pseudonymize_url_host`):
  local mode still shows the host for diagnosis, and share mode replaces it with a deterministic
  `host_<n>` pseudonym. That local-vs-share behaviour is now pinned by
  `tests/security_leaks.rs::share_mode_pseudonymizes_url_hosts_but_local_mode_keeps_them_for_diagnosis`,
  and the previously documented gap is closed — it is no longer listed as open in
  `docs/implementation-status.md` §10 (see §7 of that file, `CHANGELOG.md` and `SECURITY.md`).

## 8. Release-readiness pass (2026-10-11)

Everything in this section was run in this checkout on 2026-10-11.

### 8.1 MSRV

```
$ cargo +1.85.0 check --locked --all-targets --all-features
    Checking configlens v0.1.0 ($REPO)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.15s
$ echo $?
0
```

The declared `rust-version = "1.85"` is real, and `.github/workflows/ci.yml` has a `msrv` job that
runs the same command on every push, pull request and manual prerelease (the prerelease workflow
reuses `ci.yml`).

### 8.2 Dependency policy (cargo-deny 0.20.2)

```
$ cargo deny list          # licences actually present in the graph
Apache-2.0 (28 crates), MIT (31), Unicode-3.0 (1), Unlicense (1)
$ cargo deny check
advisories ok, bans ok, licenses ok, sources ok
$ echo $?
0
```

`deny.toml` denies (not warns) on: a RustSec advisory or a yanked release, any licence outside the
four listed above, a wildcard or git dependency, and any registry other than crates.io. The CI job
`dependencies` installs the pinned tool and runs the same check.

### 8.3 Fuzzing (cargo-fuzz 0.13.2, nightly 1.101.0)

Targets: `parse_documents`, `resolve_precedence`, `hostile_inputs` (`fuzz/`), seeded by
`fuzz/corpus/`, built with `cargo +nightly fuzz build`.

```
Session 1 (90 s per target, fuzz/results/session-1.log)
  parse_documents    exit 1   crash in the harness itself (&text[..32] split a UTF-8 char)
  resolve_precedence exit 0   1,869 execs, 231 new units, no crash, no invariant violation
  hostile_inputs     exit 1   REAL defect: an unsanitized control character in a report pointer

Session 2 (120 s per target, after the pointer fix, fuzz/results/session-2.log)
  parse_documents    exit 0
  resolve_precedence exit 0
  hostile_inputs     exit 1   still failing: a second assertion is violated (unresolved)

Session 4 (120 s per target, corrected oracle, fuzz/results/session-4.log)
  parse_documents    exit 0   408,543 execs, 1,295 new units
  resolve_precedence exit 0   1,948 execs, 49 new units
  hostile_inputs     exit 0   1,544 execs, 60 new units
```

The session-2 `hostile_inputs` failure was a real URL-userinfo leak plus an unsound oracle; both are
resolved. The saved crash artifact is now replayed end to end through the real CLI by
`tests/security_leaks.rs::the_saved_hostile_inputs_crash_artifact_is_neutralised`, and all three
targets run clean under the corrected oracle. Full root cause and evidence:
`docs/research/fuzz-recovery.md`.

The CI job `fuzz` runs 60 s per target and fails the build on a crash or a violated invariant. That
job, like `dependencies`, has not yet run in CI (see the open items at the end of this section).

### 8.4 Security fix — control characters in report pointers

Before: `configlens scan --client claude --format json` on a project whose settings contain an
object key holding an ESC sequence printed `"pointer": "/evil\u001b[31mkey\u202e\u200b"` in the
local JSON report (spec §7 requires pointer displays to be sanitized). After the fix the same
document reports `"pointer": "/evilKey"`, and the raw byte count of ESC in the JSON is `0`.

Fix: `security::display_pointer` applied in `model/report.rs` (both path modes), `render/text.rs`
(explain and summary) and `commands.rs` (diff text and JSON). Regression test:
`tests/security_leaks.rs::control_characters_in_key_names_never_reach_a_channel` (scan, explain,
doctor, snapshot and diff, both clients, both path modes). Proof the test is meaningful: with the
fix reverted (`git stash`) the test fails with the leaked pointer; with the fix it passes.

### 8.5 SBOM (CycloneDX 1.5, cargo-cyclonedx 0.5.9)

```
$ python3 scripts/generate_sbom.py --tag v0.1.0-beta.1 --target <triple> --output <dir>
```

Run for `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin` and `x86_64-pc-windows-msvc`; each
document validates (non-empty, every component carries a licence, no builder paths). The script pins
`SOURCE_DATE_EPOCH` and normalises the document (sorted components and top-level dependency edges),
and `scripts/test_generate_sbom.py` proves that **normalise/serialise path** is byte-stable offline,
with the tool simulated by a `FakeRunner`. Whether the real `cargo-cyclonedx` emits byte-identical
raw output across two runs is **not** yet verified — only the normalised form is guaranteed — so a
real two-run transcript remains pending (see the open items below). The release workflow's `sbom` job
runs the same script and attaches the three SBOMs to the prerelease.
`python3 -m unittest discover -s scripts -p 'test_*.py'` covers the script offline.

### 8.6 OpenCode `$OPENCODE_CONFIG_DIR` — adapter defect found by real Windows testing

Triggered by a Windows beta report (OpenCode 1.18.35): a **directory**-valued
`$OPENCODE_CONFIG_DIR` was reported `not_a_regular_file`, and a **file**-valued one was accepted and
made the winner. Upstream was read at the pinned commit
`53d1eabb61e21162157817bf677da0a4ad3332e3`: `config/paths.ts` appends the variable to the list of
scanned directories, and `config/config.ts` loads `opencode.json` then `opencode.jsonc` from inside
every directory equal to it. The value is a directory, never a file, and the placement is fixed
(after the project files, before `$OPENCODE_CONFIG_CONTENT`). Full transcript:
`docs/research/evidence/opencode-config-dir-20261011.txt`.

Fixed in `src/adapters/opencode.rs` (`oc.custom_dir` now points at the two files *inside* the
directory, and the placement gap is gone because the placement is source-verified). Verified end to
end on a replica of the reported layout: the directory value now wins as
`/model = "custom-dir-model"` with the project and global layers listed as losers, and the file value
now contributes nothing (`/model = "project-model"`). Eight integration tests plus two adapter unit
tests cover directory values, file values, missing files, an invalid/unapproved path, conflicting
values, `.jsonc`-over-`.json` inside the directory, the inline layer above it, a Windows-shaped path
with a trailing separator, and the unset variable.

### 8.7 OpenCode project-config discovery — a wrong winner fixed (finding A3)

Before: `oc.project` used `--project-root` (or the cwd) as the *search start* and read only the first
directory containing an `opencode.json(c)`, so from `repo/sub` the resolution named
`repo/opencode.json` as the winner and dropped the working directory's own file, with no coverage gap.
Upstream collects a match at **every** level from the working directory up to the worktree and merges
them outermost-first (`.toReversed()`), so the nearest file wins and every ancestor contributes
(`config/paths.ts::files` → `packages/core/src/fs-util.ts::up`, pinned
`53d1eabb61e21162157817bf677da0a4ad3332e3`; evidence `OC-PROJECT-SOURCE-20261011`).

Fix: `src/adapters/opencode.rs` now walks `ctx.cwd` up to the boundary inclusive, collects every
`opencode.json`/`opencode.jsonc`, and emits them as `oc.project` candidates **outermost-first**
(boundary first, cwd last), so the engine's last-definition-wins tie-break selects the nearest file.
The boundary is `--project-root` when supplied, else the nearest git root; with no boundary only the
cwd is read and the `project_boundary_uncertain` signal is kept. Layer id/rank (`oc.project` = 30) and
the per-directory `jsonc_coexistence_unverified` note are unchanged.

Before/after on the finding's layout (`repo/.git` + `repo/opencode.json {small_model: repo-small}` +
`repo/sub/opencode.json {small_model: sub-small}`, run from `repo/sub`, release binaries):

```
BEFORE (HEAD 35ad132): /small_model = "repo-small" (observed)   Sources (1): oc.project $PROJECT/opencode.json
AFTER  (this fix):     /small_model = "sub-small" (observed)     Sources (2): oc.project $PROJECT/opencode.json
                                                                              oc.project $PROJECT/sub/opencode.json
```

In the after run the ancestor `$PROJECT/opencode.json` is listed as a loser with reason
`replaced_by_later_layer`. Full transcript:
`docs/research/project-config-discovery-finding-20261011.md` §Fix. Six regression tests added in
`tests/rules_opencode.rs`: `oc_project_nested_configs_nearest_wins_and_every_level_is_a_source`,
`oc_project_ancestor_only_key_resolves_from_the_ancestor`,
`oc_project_cwd_equal_to_the_boundary_reads_a_single_source`,
`oc_project_explicit_boundary_never_reads_above_it`,
`oc_project_without_a_boundary_reads_the_cwd_only`,
`oc_project_jsonc_beats_json_at_an_ancestor_level`. One existing test
(`share_mode_pseudonymizes_unknown_key_names`) was adapted: it had used `ctx.project_root` as the
search start, which the fix removes, so it now places the project file in the working directory and
asserts the file is discovered and its path aliased. Ceiling stays `fixture_verified` (no runtime
conformance claim).

### 8.8 Checks run in this pass

```
$ cargo fmt --all -- --check          -> clean
$ cargo clippy --locked --all-targets --all-features -- -D warnings   -> clean
$ cargo test --locked --no-fail-fast  -> 315 passed, 0 failed (see §3)
$ python -m unittest discover -s scripts -p "test_*.py"              -> OK (13 tests)
$ cargo +stable build --locked --release --bin configlens            -> ok
$ actionlint                          -> clean (both workflows)
```

Still open and **not** verified:

- a real GitHub Actions run of the newer `dependencies` / `fuzz` / `sbom` jobs, and any run at all of
  `prerelease.yml` — it is manual (`workflow_dispatch`) and has **never executed in CI**, so the
  seven-asset release layout is source-level only;
- the release-artefact re-verification against that seven-asset layout (no release has been published);
- real-`cargo-cyclonedx` byte-determinism across two runs (only the script's normalised output is
  proven reproducible — see §8.5).

Resolved since this section was first written: the `hostile_inputs` fuzz target now runs clean
(`fuzz/results/session-4.log`; its crash is fixed and replayed by
`tests/security_leaks.rs::the_saved_hostile_inputs_crash_artifact_is_neutralised` — see
`docs/research/fuzz-recovery.md`), and the 5,000-key benchmark profile has been run (medians in
`docs/implementation-status.md` §8; indicative, not statistical).
