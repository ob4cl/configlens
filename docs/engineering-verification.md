# ConfigLens engineering verification

This document records the **engineering work done in this session** for the private beta, the
**commands actually executed**, and the **measured results**. Nothing here is a claim about a
command that was not run. This file is owned by the coder agent; design docs
(`docs/spec/…`, `docs/decisions.md`, `docs/implementation-status.md`, `docs/research/…`) are owned by
other agents and were **not** modified.

Base commit for this work: `b2684c7` ("Initial ConfigLens CLI prototype"), previously 270 tests
green. **No git commit or push was made.** No server, real configuration, credential, profile change
or delegation was used. The shipped tool still makes no network request and spawns no process (this
is enforced by `tests/security_leaks.rs::the_source_tree_never_spawns_processes_or_opens_sockets`).

## 1. Scope of the change (files owned and touched)

| Area | Files |
|---|---|
| Share-mode privacy | `src/security/redact.rs`, `src/security/path_policy.rs`, `src/model/report.rs`, `src/cli.rs`, `src/resolve/mod.rs` |
| `explain` quality | `src/render/text.rs` |
| `diff` source/status/coverage | `src/commands.rs` |
| Support-report workflow | `src/cli.rs` |
| Shell completions / help examples | `src/completions.rs` (new), `src/cli.rs`, `src/lib.rs` |
| Windows portability of tests | `src/filesystem.rs`, `src/commands.rs`, `tests/cli_contract.rs` |
| Demo | `demo/demo.sh` |
| New tests | `src/completions.rs`, `src/security/redact.rs`, `src/model/report.rs`, `src/commands.rs`, `tests/cli_contract.rs`, `tests/security_leaks.rs` |

`Cargo.toml` / `Cargo.lock` were **not** changed: completions are generated without adding
`clap_complete`, so the build stays `--locked` and offline.

## 2. What was implemented

### 2.1 Share-mode privacy (comprehensive)

The known gap was "share mode does not pseudonymize URL hosts". The fix covers four leak paths and
projects them in **every** output channel (JSON envelope, text `explain`, `scan`, snapshot and
support report):

1. **URL hosts.** `redact::url_host` extracts the authority (userinfo already stripped by
   `sanitize_url`); `model::report` collects every revealed URL host, assigns deterministic
   `host_1, host_2, …` pseudonyms by sorted order, and rewrites the value. Unknown hosts fall back
   to `host_opaque`, so no private host can survive.
2. **Embedded absolute paths.** `PathPolicy::alias_embedded` replaces known roots with their alias
   (`$HOME`, `$PROJECT`, `$XDG_CONFIG`, `$MANAGED`) inside an already-revealed string, so a value
   such as a shell path cannot leak the machine prefix. Longest root wins when roots are nested.
3. **Unknown-key leakage across outputs.** Previously only the JSON `resolutions` were projected;
   the text `explain` renderer used the *unprojected* run and therefore echoed unknown key names
   from the queried pointer. `cli::print_report` now renders the **projected** resolution (and its
   child pointers) from the report.
4. **Command-argument leakage.** The `unrecognised_session_flag` diagnostic echoed the raw flag
   (`--api-key=…` would leak a credential). It now reports the argument position only
   (`src/resolve/mod.rs`). This is a spec §7 requirement ("Do not expose … command arguments").

Local mode is unchanged by design: it is the diagnostic view and still shows the private host
(spec §7 "In local mode show config locations for diagnosis"), while userinfo/query/fragment remain
stripped in both modes.

### 2.2 `explain`: winner / losers / rule / gaps / next inspection

`render_explain` now emits a concise shape (all derived from the redacted resolution):

```
<pointer> = <value> (observed)
Observed winner: <location>  <value>  <reason>
Losers:
  <value>  <location>  <reason>
  ignored: <location>  <reason>
Rule: <rule id>
Coverage: <status> — <gap>; <gap>
Support: <support>; confidence: <class>
Conclusion: …
Next inspection:
  - <actionable hint>
```

The hints are actionable and never invent a value: supply the managed file via `--input-context`,
declare project trust, pass the exact tested `--client-version`, note unobservable remote/MDM
layers, or check the pointer spelling / `configlens clients` for unsupported keys. Existing
contract substrings (`Observed winner:`, `Rule:`, `Coverage: partial`, `effective value withheld`,
`no merge result is claimed`) are preserved, so all previous assertions still pass.

### 2.3 `diff`: source, status and coverage changes

`diff_reports` now also compares the report-level **source inventory** (added / removed / status or
applicability change, new `DiffKind::Source`) and the **run-wide coverage gaps** (`DiffKind::Coverage`
against `run`). Per-pointer value / provenance / status / coverage diffs and the
`unknown_redacted` rule for protected values are unchanged.

### 2.4 Support-report workflow (explicit preview, explicit output, no upload)

New subcommand `configlens support-report`:

- always renders in **share mode** regardless of `--paths` (safe sanitization), recorded as
  `redaction.mode = "share"` and rules entry `share_mode_path_and_host_pseudonymization`;
- `--preview` prints the sanitized JSON to **stdout** and writes **nothing** (notice on stderr);
- `--output <path>` writes atomically via the existing snapshot path (0600 on Unix) — an **explicit**
  destination, never a default;
- neither flag → usage error (exit 2);
- there is **no automatic upload** and no network code path at all.

### 2.5 Shell completions and help examples

`configlens completions --shell bash|zsh|fish` prints a completion script generated by walking the
live `clap` command tree (no `clap_complete` dependency, nothing written to disk). Top-level help
and the `explain` / `completions` / `support-report` subcommands now carry `Examples:` blocks.

### 2.6 Windows portability (tests and code)

Unix-only test fixtures that used `std::os::unix::*` without a gate would not compile on Windows.
They are now `#[cfg(unix)]` (`src/filesystem.rs`: symlink follow / escape / loop and socket refusal;
`src/commands.rs` and `tests/cli_contract.rs`: symlink snapshot destination). Non-test code already
gated the only Unix-specific branch (`#[cfg(unix)]` permission bit). Remaining, documented
limitations: `is_within` compares path components without Windows case folding; `std::fs::canonicalize`
on Windows returns `\\?\` verbatim paths; the Windows snapshot ACL is still `not_implemented`.

## 3. Commands actually run and their real results

All commands were run from `/home/user/configlens` with the absolute toolchain
`~/.cargo/bin/cargo` (`cargo 1.99.0 (5f94df478 2026-08-27)`, `rustc 1.99.0`), host
Linux/WSL2 x86_64, `rust-toolchain.toml` → `stable`.

```
$ cargo check --locked --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.41s          # exit 0

$ cargo fmt --all -- --check
FMT_CLEAN                                                                        # exit 0

$ cargo clippy --locked --all-targets -- -D warnings
    Checking configlens v0.1.0 (/home/user/configlens)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.93s          # exit 0, no warnings

$ cargo test --locked --no-fail-fast
    Running unittests src/lib.rs ...            ok. 185 passed; 0 failed; 0 ignored
    Running unittests src/main.rs ...           ok. 0 passed; 0 failed
    Running tests/acceptance.rs ...             ok. 8 passed; 0 failed
    Running tests/cli_contract.rs ...           ok. 24 passed; 0 failed
    Running tests/conformance.rs ...            ok. 10 passed; 0 failed
    Running tests/rules_claude.rs ...           ok. 22 passed; 0 failed
    Running tests/rules_opencode.rs ...         ok. 26 passed; 0 failed
    Running tests/security_leaks.rs ...         ok. 11 passed; 0 failed
    Doc-tests configlens ...                    ok. 0 passed; 0 failed
    TOTAL: 286 passed, 0 failed, 0 ignored                                        # exit 0

$ cargo build --locked --release
    Finished `release` profile [optimized] target(s) in 2m 09s                    # exit 0
$ ./target/release/configlens --version
configlens 0.1.0

$ ./demo/demo.sh
demo complete.                                                                   # exit 0

$ python3 /home/user/.hermes/cache/scratch/configlens_acceptance.py target/debug/configlens
# 7/7 checks "ok": true (clients, empty_scan, redacted_scan, strict_partial,
#                     unknown_pointer, malformed_higher_layer, duplicate_keys)     # exit 0
```

Test count moved from 270 → 286 (+16): lib +8 (4 completion tests, the `diff` source/coverage test,
the URL-host/path projection test, the `url_host` unit test, and the split symlink snapshot test),
`cli_contract` +3 (completions, help examples, support-report usage), `security_leaks` +5 (URL host,
embedded path, share-mode text key, session-argument echo, support-report). The 8 acceptance
examples, the 10 conformance/property tests and all rule tests continue to pass.

### 3.1 Observed evidence for the privacy fix (from `demo/demo.sh`)

```
$ configlens explain /mcp/demo/url --client opencode --client-version 1.18.35 --paths share
/mcp/field_1/url = "https://host_1/mcp?[redacted]" (observed)
Observed winner: $XDG_CONFIG/opencode/opencode.json:6:14  "https://host_1/mcp?[redacted]"  observed_winner
```

The synthetic private host `demo.internal.example` (and the synthetic token) appears **0 times** in
the complete demo transcript; `host_` appears 9 times. The share-mode text view of an unknown
pointer (`explain /totally_unknown/nested/deep --paths share`) contains no raw key name, and the
support-report preview contains neither the host nor the seeded secret.

## 4. Limitations and things deliberately **not** done

1. **Codex adapter: explicitly pending research.** The brief requires verified upstream evidence
   before implementation. No pinned Codex loader/version evidence was available to this agent, so
   Codex stays in the `unsupported` list (`configlens clients`) with no compatibility claim. It was
   **not** implemented, and no rule, default or precedence was invented. The evidence researcher
   writes `docs/research/` separately; upstream conformance remains unverified and is not claimed.
2. **No upstream conformance run.** Every advertised rule is at most `fixture_verified`; no pinned
   loader harness was built or run here.
3. **Windows was not executed.** Portability work is source-level gating of Unix-only test fixtures
   plus documentation of remaining limitations. Only WSL2/Linux was verified. macOS/Windows paths,
   drive roots, UNC verbs and registry/plist managed sources remain unverified.
4. **MSRV 1.85 is declared but not built** (verified toolchain is 1.99.0).
5. **Residual share-mode items:** a user-supplied source's `path` string used as a diagnostic
   `location` (from `--input-context`) is not run through the alias projection; managed `.plist` /
   Windows registry sources are still location-only; the Windows snapshot ACL is still
   `not_implemented` (reported honestly as such).
6. **Text `explain` now renders the projected resolution**, so in share mode its header pointer is
   the opaque `/field_n` form. This is intended (it matches the JSON envelope), but is a visible
   change from the previous build.
7. **No benchmarks, no CI, no license files, no `fixtures/` files** were added — out of the owned
   scope or unchanged from the base.

## 5. Reproduce

```sh
cd /home/user/configlens
~/.cargo/bin/cargo fmt --all -- --check
~/.cargo/bin/cargo clippy --locked --all-targets -- -D warnings
~/.cargo/bin/cargo test --locked --no-fail-fast
~/.cargo/bin/cargo build --locked --release
./demo/demo.sh
python3 /home/user/.hermes/cache/scratch/configlens_acceptance.py target/debug/configlens
```

## 6. Compatibility-registry correctness pass (audit findings C1–C5, O1–O4)

Base: the same uncommitted working tree as §1, `/home/user/configlens`. This pass owns
`src/adapters/` and `compatibility/{claude,opencode}/` plus their tests, and fixes the nine findings in
`docs/research/compatibility-audit.md` (audit date 2026-10-10). **No git commit or push**; no server,
real credential, profile change or delegation; the tool still makes no network request and spawns no
process.

### 6.1 Findings fixed

| # | Change | Files |
|---|---|---|
| **C1** | HKCU rank inversion: `cc.managed.hkcu` rank **90 → 50**, i.e. the *lowest* managed layer, below `cc.managed.file` (60). registry.md's managed-tier table gains a rank column and the reconciliation note. | `compatibility/claude/registry.{json,md}`, `src/adapters/mod.rs` (tests) |
| **C2** | `cc.project_shared.location` = `"the session primary working directory/.claude/settings.json"` (was `<project root>/…`); the adapter already used `ctx.cwd`. | `compatibility/claude/registry.json` |
| **C3** | Managed tier is declared **first-wins**: `managed_sources_behavior: "first-wins"` in the registry, a `RegistryFile` field + `Registry::managed_sources_behavior()`, and a doc note that `merge` is opt-in (v2.1.242+), that only **single-source** managed resolution is claimed, and that the cross-source key set + `policyHelper` are a documented gap. | `compatibility/claude/registry.{json,md}`, `src/adapters/mod.rs` |
| **C4** | Trust gating is now **key-granular**: an untrusted/unknown project layer is `Conditional` (never `Skipped`), and only a *trust-gated* key (`permissions.allow`, `env`) quoted from the **shared** project file is ignored; `deny`/`ask` apply immediately and an untracked local `allow` applies without trust. | `src/adapters/mod.rs`, `src/resolve/mod.rs`, `tests/rules_claude.rs` |
| **C5** | `/modelPicker` downgraded to `inventory-only` (`claude.unknown.inventory-only`); registry.md's supported-keys and support-state tables updated to match (no ordered-replace claim). | `compatibility/claude/registry.{json,md}`, `src/adapters/mod.rs` (tests) |
| **O1** | OpenCode pinned to the **v1** line: `pinned_commit = 53d1eabb61e21162157817bf677da0a4ad3332e3` (tag `v1.18.35`), evidence note rewritten; the v2 dev line (`@opencode/cli` 2.0.26, HEAD `7b3d4ce3…`) is declared out of scope. No v1-version/v2-SHA mix remains. | `compatibility/opencode/registry.{json,md}` |
| **O2/O3** | Registry now matches the chosen **v1** line: the v1 documented precedence order and the v1 file/macOS managed tier; registry.md cites the v1 docs and the v1 tag. | `compatibility/opencode/registry.{json,md}` |
| **O4** | Removed the OpenCode trust gate: `oc.project` (and every OpenCode source) is always `Applicable`; new `ClientAdapter::trust_applicable()` returns `false` for OpenCode, so no `trust_unknown`/`workspace_untrusted` gap or diagnostic is emitted for that client. | `src/adapters/{opencode,mod}.rs`, `src/resolve/mod.rs`, `tests/rules_opencode.rs` |

### 6.2 Commands run and measured results

```
$ ~/.cargo/bin/cargo fmt --all -- --check
FMT_CLEAN                                                                        # exit 0

$ ~/.cargo/bin/cargo clippy --locked --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.30s          # exit 0, no warnings

$ ~/.cargo/bin/cargo test --locked --no-fail-fast
    lib 191 / acceptance 8 / cli_contract 24 / conformance 10
    rules_claude 25 / rules_opencode 27 / security_leaks 11
    TOTAL: 296 passed, 0 failed, 0 ignored                                       # exit 0

$ ~/.cargo/bin/cargo build --locked --release
    Finished `release` profile [optimized] target(s) in 39.65s                   # exit 0
$ ./target/release/configlens --version
configlens 0.1.0

$ ./demo/demo.sh
demo complete.                                                                   # exit 0

$ python3 /home/user/.hermes/cache/scratch/configlens_acceptance.py target/debug/configlens
# 7/7 checks "ok": true                                                           # exit 0

$ bash docs/research/run-fixtures.sh        # read-only, isolated, no network      # exit 0
```

Test count moved **286 → 296 (+10)**. New tests, each of which fails if its fix is reverted:

* unit `src/adapters/mod.rs` +6 — `claude_hkcu_is_the_lowest_managed_layer`,
  `claude_project_shared_location_names_the_primary_working_directory`,
  `claude_managed_sources_behavior_is_first_wins`, `claude_model_picker_is_inventory_only`,
  `opencode_registry_matches_its_pinned_v1_line`, `trust_is_a_claude_only_concept`;
* `tests/rules_claude.rs` +3 — `cc_untrusted_shared_project_gates_only_allow_deny_still_applies`,
  `cc_untrusted_local_allow_applies_without_trust`,
  `cc_untrusted_project_non_permission_keys_still_apply`;
* `tests/rules_opencode.rs` +1 — `oc_project_is_never_trust_gated`.

### 6.3 Fixture verification (H, and the two fixtures the audit cited)

Fresh `run-fixtures.sh` output, compared with the pre-fix `docs/research/evidence/fixture-run.txt`:

* **H** (Claude JSON scan, share mode) — still one valid envelope. Coverage now ranks
  `cc.managed.hkcu` **50** *below* `cc.managed.file` 60; redaction is unchanged (`mode: share`,
  34 values redacted, no `fixture-not-a-real-token` / raw tool-rule leak; the deliberately public
  model-name allowlist still shows). **C1 does not disturb the redaction contract.**
* **D** (Claude, `project_trust=untrusted`) — `/model` now resolves from the untrusted project
  (winner `$PROJECT/.claude/settings.local.json`), because `model` is not a trust-gated key; the
  `workspace_untrusted` diagnostics remain for the gated keys. **C4**, key-granular.
* **I** (OpenCode scan) — `oc.project` is now `applicable` (was `not unconditional (trust_unknown)`)
  and the run-level `project_trust_unknown` gap is gone (5 → 4 gaps). **O4**.

The pre-existing `docs/research/evidence/fixture-run.txt` is owned by the research agent and — beyond
my changes — predates the §2.2 `explain` renderer change (its lines have no `Losers:`/`Next
inspection:`), so it was **left untouched**; the run above is the current engine behaviour.

### 6.4 Limitations / deliberately not done

1. Git-tracking of `.claude/settings.local.json` is not observable offline; ConfigLens assumes the
   file is untracked, so a *tracked* local `allow` rule would be under-gated. Disclosed in
   `registry.md`.
2. `managed_sources_behavior` is declared and documented but not enforced by code: only the file tier
   is observable, so no cross-source managed merge can occur in practice. The cross-source key set
   and `policyHelper` remain unmodelled.
3. O1 chose the **v1** line; moving the adapter to the v2 line (`@opencode/cli` 2.0.26, Console policy
   statements) was not done.
4. No upstream-conformance run (audit §5) was performed; `conformance_status` stays
   `fixture_verified_max`.

## 7. Reproduce this pass

```sh
cd /home/user/configlens
~/.cargo/bin/cargo fmt --all -- --check
~/.cargo/bin/cargo clippy --locked --all-targets -- -D warnings
~/.cargo/bin/cargo test --locked --no-fail-fast
~/.cargo/bin/cargo build --locked --release
./demo/demo.sh
python3 /home/user/.hermes/cache/scratch/configlens_acceptance.py target/debug/configlens
bash docs/research/run-fixtures.sh
```
