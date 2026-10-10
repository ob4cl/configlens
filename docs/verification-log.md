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
| MSRV | declared `1.85` in `Cargo.toml`; **not** verified (no 1.85 toolchain build was run) |

## 1. Formatting

```
$ cargo fmt --all -- --check
$ echo $?
0
```

## 2. Lints (warnings denied)

```
$ cargo clippy --locked --all-targets -- -D warnings
    Checking configlens v0.1.0 (/home/user/configlens)
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
| `unittests src/lib.rs` | ok. 177 passed; 0 failed; 0 ignored |
| `unittests src/main.rs` | ok. 0 passed; 0 failed |
| `tests/acceptance.rs` | ok. 8 passed; 0 failed |
| `tests/cli_contract.rs` | ok. 21 passed; 0 failed |
| `tests/conformance.rs` | ok. 10 passed; 0 failed |
| `tests/rules_claude.rs` | ok. 22 passed; 0 failed |
| `tests/rules_opencode.rs` | ok. 26 passed; 0 failed |
| `tests/security_leaks.rs` | ok. 6 passed; 0 failed |
| Doc-tests | ok. 0 passed; 0 failed |
| **Total** | **270 passed, 0 failed, 0 ignored** |

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
  authorities remains a **documented gap** (see `docs/implementation-status.md`).
