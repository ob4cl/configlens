# ConfigLens implementation status

Living checklist mapping the technical specification (`docs/spec/ConfigLens_Technical_Specification.md`,
40439 bytes) to what is **actually** implemented, verified and **not** implemented in this repository.

Every `[x]` below is backed by a command recorded in [`docs/verification-log.md`](verification-log.md)
or by a named test that passes in `cargo test --locked`. Anything not proven that way is marked `[~]`
or `[ ]`. This file was previously written as if deliverables existed that do not; it has been
corrected against the actual file tree and the actual test run.

Status vocabulary (spec §3): `inventory_only` < `documented` < `fixture_verified` <
`upstream_conformance_verified`. `unsupported` means ConfigLens makes no claim.

Legend: `[x]` done and verified · `[~]` partial or bounded · `[ ]` not present / not implemented.

Toolchain used for the verification run: `cargo`/`rustc` 1.99.0 (see `docs/verification-log.md`).
Test totals for the recorded run: **270 passed, 0 failed, 0 ignored** (177 lib unit + 93 integration).

---

## 0. Repository contents (spec §6 layout, §12 first steps)

| Item | Status | Evidence / reality |
|---|---|---|
| Spec copy in repo | [x] | `docs/spec/ConfigLens_Technical_Specification.md`, 40439 bytes (no external copy was available to byte-compare) |
| `docs/implementation-status.md` | [x] | this file (rewritten to match the tree) |
| `docs/decisions.md` | [x] | present, decisions D1–D20. **Caveat:** a few entries name artifacts that do not exist yet (e.g. `docs/security.md`, `tests/security/no_raw_dto.rs`); read them as rationale, not as a file index |
| `docs/verification-log.md` | [x] | present (new in this run) |
| `compatibility/{claude,opencode}/registry.json` | [x] | bundled registries load, asserted by `adapters::tests::both_bundled_registries_load_and_are_pinned_to_a_revision` |
| `compatibility/{claude,opencode}/registry.md` | [x] | evidence snapshots with URLs, retrieval date and pinned-commit status |
| Library + single binary crate | [x] | `src/lib.rs` + `src/main.rs` |
| Bundled report schema | [x] | `src/schema/report-1.0.json`; `schema --kind report`; validated by `model::report::tests::the_bundled_schema_is_valid_json_and_matches_the_schema_version` |
| `rust-toolchain.toml` | [x] | `channel = "stable"` + `rustfmt`/`clippy`; MSRV declared in `Cargo.toml` |
| `Cargo.lock` | [~] | present and used (`--locked`); the worktree has **no commits**, so nothing is "committed" |
| Integration tests | [~] | six flat targets (`tests/acceptance.rs`, `cli_contract.rs`, `conformance.rs`, `rules_claude.rs`, `rules_opencode.rs`, `security_leaks.rs`) + `tests/common/mod.rs`; **not** the spec's `tests/{cli,security,discovery,conformance}/` layout |
| `README.md` | [x] | present (new, minimal) |
| `demo/demo.sh` (runnable isolated demo) | [x] | present (new); runs in a throwaway sandbox, exits 0 |
| `fixtures/{claude,opencode}/` | [ ] | directories exist but are **empty**; all fixtures are inline in the test files |
| `examples/` | [ ] | directory exists but is **empty** (the runnable demo lives in `demo/`) |
| `benches/resolution.rs` | [ ] | `benches/` exists but is **empty**; no benchmark harness and no numbers |
| `.github/workflows/{ci,release}.yml` | [ ] | `.github/workflows/` exists but is **empty**; no CI is configured or was ever executed |
| `docs/{semantics,compatibility,report-schema,security}.md` | [ ] | absent |
| `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md` | [ ] | absent |
| `LICENSE-MIT`, `LICENSE-APACHE` | [ ] | absent (only the SPDX expression `MIT OR Apache-2.0` in `Cargo.toml`) |

## 1. Product decisions (§1)

| Requirement | Status | Evidence |
|---|---|---|
| Local, read-only Rust CLI | [x] | the only write path is `snapshot --output` (`commands::write_snapshot`) |
| Deterministic resolution with an ordered event trace | [x] | `resolve::resolve_pointer`; `conformance::property_identical_context_produces_byte_identical_reports` |
| Ignored sources preserved in the report | [x] | `Ignored` events; `resolve::collect_contributions` never filters contributions out |
| Preserve client-native namespaces (RFC 6901 pointers) | [x] | pointers are built from native field names; no renaming |
| Separate defaults / instructions / enforced policy | [~] | policy phase exists (`Constrained` events, managed tier, `availableModels` lock). Instruction bodies are never read; instruction/unknown keys are `inventory_only` |
| Offline, no credentials/telemetry/subprocess/network | [x] | no network/subprocess dependency compiled in; `security_leaks::the_source_tree_never_spawns_processes_or_opens_sockets` scans `src/` |
| Safe terminal output + stable machine-readable reports | [x] | `security::sanitize` + fixed JSON envelope |

## 2. Scope boundaries (§1 non-goals, §3)

| Item | Status |
|---|---|
| OpenCode adapter (bounded local scope) | [x] `fixture_verified` for the local merge order; remote/MDM/dirs are gaps |
| Claude Code adapter (bounded settings keys) | [x] `fixture_verified` for supported scalars/lists, `documented` elsewhere |
| Codex / Gemini / Aider / Cursor / Copilot CLI / Cline / Roo / Hermes | [ ] **not implemented**; listed as `unsupported` by `clients` |
| Config rewriting / auto-fix / launcher / TUI / LLM explanation | [ ] out of scope by design |
| Permission *authorization* evaluation | [ ] not implemented and never claimed; list merge is provenance only (`Caveat:` printed) |
| Instructions discovery/assembly (§10) | [ ] instruction keys return `inventory_only` |
| MCP connection / plugin loading / hooks / subprocess | [ ] forbidden by design and by test |

## 3. Adapter discipline (§3)

1. No single global precedence list — [x] each adapter owns its ordered layers.
2. Permission-list concatenation ≠ authorization — [x] `claude.permissions.list-concat` prints the caveat.
3. A file containing `env` ≠ process environment — [x] `env` blocks are ordinary (redacted) file keys.
4. Missing managed file ≠ enterprise policy absent — [x] managed tier always emits a gap.
5. Instructions file is not a settings map — [x] instruction keys are `inventory_only`.
6. Unknown settings preserved as opaque inventory — [x] `inventory_only` resolutions; opaque field IDs in share mode.
7. Client binaries never executed — [x] no `Command`/network usage in `src/` (scan test).
8. Unknown defaults stay unknown — [x] no defaults are invented; `absent` status with an `absent` reason.

## 4. CLI contract (§4)

| Command | Status | Evidence |
|---|---|---|
| `clients` | [x] | `cli_contract::clients_lists_implemented_and_unsupported_clients` |
| `scan` | [x] | `rules_opencode::scan_lists_every_source_with_status_and_applicability` |
| `explain KEY` | [x] | `cli_contract`, `tests/acceptance.rs` |
| `doctor` | [x] | `cli_contract::doctor_never_claims_the_configuration_is_safe` |
| `snapshot` | [x] | `cli_contract::snapshot_*`, `commands::tests::snapshot_*` |
| `diff` | [x] | `cli_contract::diff_*`, `commands::tests::*` |
| `schema --kind report` | [x] | `cli_contract::schema_emits_the_bundled_report_schema` |
| Shared options | [x] | `--client --cwd --client-version --format --strict --no-env --input-context --color --paths --allow-read-root --project-root`; `--output`/`--overwrite` snapshot-only; `--fail-on` doctor-only |
| `NO_COLOR` + non-TTY defaults | [x] | `cli::tests::colour_is_disabled_for_json_and_non_tty_and_no_color` |
| Exit codes 0/1/2/3/4, priority 2→4→3→1→0 | [x] | `cli::exit_code_*`, `error::tests::exit_codes_match_the_specification` |
| JSON envelope on stdout, incl. fatal errors | [x] | `error`, `render::json` |
| No ANSI in JSON; sanitized stderr | [x] | `render::json::tests::output_contains_no_ansi_escape_sequences` |
| Broken pipe exits without backtrace | [x] | `BrokenPipe` → exit 0 mapping in `error` |

## 5. Resolution model (§5)

| Requirement | Status | Evidence |
|---|---|---|
| Explicit resolution context | [x] | `context::ResolutionContext` |
| Discover adapter-known sources; record expected-but-unobservable | [x] | `adapters::discover` + `CoverageGap { kind: expected_unobservable }` |
| Bounded regular-file reads, spans preserved | [x] | `filesystem`, `parse::json` span tests |
| Duplicate keys / unknown keys preserved | [x] | `duplicate_key` diagnostic; unknown keys become `inventory_only` |
| Applicability per source; ignored sources retained | [x] | `Applicability::{Applicable,Skipped,Conditional}` |
| Verified rule per key + version; ordered definitions | [x] | `adapters::Registry::rule_for` + layer ordering |
| Typed reduction with provenance per contribution | [x] | `resolve::push_event`, `model::event::Event` |
| Policy phase distinct from merge | [x] | `Constrained` events, `availableModels` lock |
| Per-key completeness / support / confidence | [x] | `ResolutionStatus`, `SupportLevel`, `Confidence` |
| Sanitize/redact into a public report; render only that | [x] | `security::redact` → `model::report::PublicValue` |
| Operators: scalar replace, recursive object merge, ordered replace, stable concat + dedupe | [~] | implemented; other adapter reducers are `inventory_only` |
| Never assign an operator to an unknown key | [x] | `RuleSupport::InventoryOnly` default |
| `null` / empty list / missing modelled distinctly | [x] | `ValueKind::Null/EmptyArray/Missing` + tests |

## 6. Report schema 1.0 (§5)

| Field | Status |
|---|---|
| `schema_version`, `tool_version`, `client`, `adapter_revision`, `compatibility`, `context`, `sources`, `resolutions`, `diagnostics`, `coverage`, `redaction`, `errors` | [x] all present and required, validated against the bundled schema |
| Sorted maps + deterministic ordering | [x] `BTreeMap` throughout; stable sort |
| No timestamps in normal reports | [x] none emitted |
| Display aliases in `--paths share` | [x] `$PROJECT`, `$HOME`, `$XDG_CONFIG`, `$MANAGED`, `$EXTERNAL` |
| Secret → `{"redacted":true,"reason":...}` | [x] |
| Diff of two redacted values → `unknown_redacted` | [x] `commands::tests::redacted_values_compare_as_unknown_and_never_as_equal` |
| No secret hashes | [x] no hashing of values anywhere |
| Source public IDs are location roles, not fingerprints | [x] ids derive from role + ordinal only |
| Unknown key names opaque in share reports | [x] `model::report::tests::share_mode_makes_unknown_key_names_opaque` (new in this run) |

## 7. Security (§7)

| Rule | Status | Evidence |
|---|---|---|
| No network / subprocess / shell / hooks / plugins / MCP / keychain | [x] | `security_leaks::the_source_tree_never_spawns_processes_or_opens_sockets` |
| `$schema` never fetched | [x] | treated as an opaque key |
| Only known config paths under explicit roots | [x] | `security::path_policy` |
| `--allow-read-root`; blocked references shown unread | [x] | `path_policy` tests; `security_leaks::files_outside_authorised_roots_are_never_read` |
| Interpolation only when adapter-verified; taint propagates | [x] | opencode `{env:}`/`{file:}` tests |
| Secrets redacted (tokens/keys/auth/cookies/mcp env/env values/unknown free text) | [x] | `security_leaks`, `security::redact::tests` |
| Public values allowlisted per adapter | [x] | booleans, numbers, enums, verified (lower-case) model names, URLs |
| URL userinfo / query / fragment redaction | [x] | `security::redact::tests::urls_have_userinfo_query_and_fragment_removed` |
| Share mode pseudonymizes paths | [x] | `path_policy` + `model::report::tests::share_mode_pseudonymizes_paths_and_keeps_output_deterministic` |
| Share mode pseudonymizes unknown key names / instances | [x] | `model::report::tests::share_mode_makes_unknown_key_names_opaque` |
|| Share mode pseudonymizes **URL hosts** | [x] | `security_leaks::share_mode_pseudonymizes_url_hosts_but_local_mode_keeps_them_for_diagnosis` |
| Tool-rule lists local / pseudonymized in share | [x] | `security::redact::tests::tool_rule_lists_are_shown_locally_and_pseudonymized_in_share_mode` |
| No global `--show-secrets` | [x] | no such flag exists |
| Terminal control/OSC/bidi/CR/newline sanitization | [x] | `security::sanitize::tests::*`, `security_leaks::terminal_control_payloads_are_neutralised_in_every_channel` |
| No parser error excerpts / raw clap echoes | [x] | diagnostics carry code + role + line/column only |
| Panic hook suppresses input content | [x] | custom hook in `error` |
| Limits: 2 MiB/file, 16 MiB total, 128 sources, depth 64, include depth 8, 2000 events/key | [x] | each with its own diagnostic and test |
| Reject devices/sockets/FIFOs/directories | [x] | `filesystem::tests::{directories_are_refused,sockets_are_refused}` |
| Symlink loops + escape detected | [x] | `filesystem::tests::symlink_*` |
| Snapshot: explicit path, create-new, atomic same-dir temp, 0600, reject symlink target, `--overwrite` | [~] | Unix verified; **Windows ACL not implemented** (`redaction.snapshot_acl = "not_implemented"`) |
| No automatic snapshots/caches/log files | [x] | none created |

## 8. Tests (§8)

| Layer | Status | Evidence |
|---|---|---|
| Parsers (comments, strings, trailing comma, duplicates, malformed, spans, Unicode, limits) | [x] | `parse::json`/`parse::jsonc` unit tests |
| Discovery (cwd depth, home, XDG, worktree `.git`, missing, blocked symlink) | [x] | `adapters::tests`, `filesystem::tests` |
| Reducers (scalar/object/list/null/missing, chains, scope exclusion, unknown keys) | [x] | `resolve` + `rules_{claude,opencode}` |
| Coverage (missing runtime args, MDM, remote defaults, unknown trust/version) | [x] | `acceptance`, `rules_*` |
| Security (seeded secrets in fields, keys, paths, URLs, args, malformed JSON, env, escapes) | [x] | `security_leaks` (6 tests) |
| CLI (exit codes, strict, JSON, NO_COLOR, non-TTY, unknown pointer, incompatible snapshots) | [x] | `cli_contract` (21 tests) |
| Conformance (pinned upstream loader) | [~] **not run** | no safe loader harness built; rules stay `documented`/`fixture_verified` |
| Portability (Linux/macOS/Windows runners) | [~] **Linux only verified** | WSL2 Linux executed; macOS/Windows paths are unit-tested logic, not OS-verified |
| Property tests (determinism, idempotent redaction, irrelevant-definition independence, no false complete) | [x] | `tests/conformance.rs` (deterministic LCG, no `proptest`) |
| Fuzz targets | [~] | bounded deterministic byte-mutation smoke in `tests/conformance.rs`; no long `cargo-fuzz` campaign |
| Benchmarks | [ ] | **none** — no harness, no numbers |
| CI | [ ] | **none** — no workflow files exist; every check above was run locally only |

## 9. Acceptance examples (§9)

All ten are exercised end to end through the CLI in `tests/acceptance.rs` (8 tests: examples 1–5,
5b, 9, 10; examples 6–8 are covered by `cli_contract`/`security_leaks`). Status: [x] passing under
`cargo test --locked`.

## 10. Known gaps and unsupported behaviour (candid)

1. **No upstream conformance run.** No pinned loader harness exists for either client; every
   advertised rule is at most `fixture_verified`.
2. **OpenCode loader commit not pinned for the current layout.** The historical path
   `packages/opencode/src/config/config.ts` was removed upstream (commit `44b6938b…`, 2026-07-21) and
   the repository was restructured; no replacement loader SHA was re-pinned.
3. **Managed/runtime state is never assumed absent.** OpenCode remote defaults and macOS MDM
   preference domains, and Claude server-managed/MDM/HKCU sources, are `expected_unobservable`.
4. **macOS `.plist` and Windows registry managed sources are not parsed** (only their locations are
   recorded).
5. **`auto`/`bypassPermissions` scope exclusion** is implemented from documentation; the
   `v2.1.257` boundary behaviour is not modelled per version.
6. **List merge ordering across Claude scopes** carries an explicit coverage note because the
   documentation does not state the cross-scope order.
7. **Windows ACLs on snapshot output** are not implemented; the report says so rather than pretending.
8. **`managed-settings.d/`** is merged alphabetically by filename, verified by fixture only.
9. **Codex, Gemini, Aider, Cursor, Copilot CLI, Cline, Roo, Hermes** adapters do not exist.
10. **Instructions assembly, permission-policy evaluation, remote/MDM fetching, MCP connections,
    plugin loading, config rewriting, TUI, LLM explanation** are out of scope for 0.1.
11. **Cross-platform verification:** only WSL2/Linux was executed. macOS/Windows behaviour is
    unit-tested logic, not OS-verified.
12. **`v2.1.296` / `1.18.35`** are the newest versions at the retrieval date; neither runtime was
    executed or tested against ConfigLens output.
13. **MSRV 1.85 is declared but not built.** The verified toolchain is 1.99.0.
15. **No CI, no benchmarks, no license files, no `fixtures/` files, and no extra design documents**
    (`docs/semantics.md`, `docs/compatibility.md`, `docs/report-schema.md`, `docs/security.md`) are
    present, despite earlier claims in this file.

## 11. Verification log

Commands actually run, with their real results, are recorded in
[`docs/verification-log.md`](verification-log.md). The recorded run: `cargo fmt --check` clean,
`cargo clippy --locked --all-targets -- -D warnings` clean, `cargo test --locked --no-fail-fast`
270/270 passing, `cargo build --locked --release` successful, `demo/demo.sh` exits 0, and the
independent acceptance harness reports 7/7 checks ok.
