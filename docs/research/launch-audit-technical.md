# ConfigLens public-launch technical credibility review

Stance: an experienced Rust maintainer deciding whether to contribute. Read-only inspection of
`main`/`e746957`, plus `cargo test --locked` and `cargo clippy --locked --all-targets
--all-features -- -D warnings` on this checkout.
Verified locally: **324 tests, all passing** (194 lib + 8 acceptance + 33 cli_contract + 10
conformance + 25 rules_claude + 40 rules_opencode + 14 security_leaks); clippy clean.
CONFIRMED = checked against the code/tests; RECOMMENDATION = judgement. Every claim cites `file:line`.

## 1. Structure and module layout — sound, one naming wart

CONFIRMED: single crate, library + binary (`Cargo.toml:13-19`). Layering follows the spec:
`parse` → `filesystem` → `adapters` (discovery + registry) → `resolve` (engine) → `model`/`render`
→ `cli`. Client policy stays in the adapters, client-neutral operators in `resolve` (D15) — right
for two adapters, scales to a third. `src/lib.rs:11-23` lists every module; each has a doc comment
naming its spec section; registries are compiled in (`src/adapters/mod.rs:346-347`), so no runtime
data-loading surprise.

CONFIRMED wart: `src/commands.rs` (snapshot/diff) is a peer of `src/cli.rs` — inviting to look in
the wrong one. The six flat test targets deviate from the spec's `tests/{cli,security,...}/` layout;
`docs/implementation-status.md:62` says so honestly (deviation, not misrepresentation).

## 2. Test quality — strong, with a few weak assertions

CONFIRMED: the 324 tests assert real behaviour. The rules tests check event *kinds*, locations,
reason codes and observed values and cover positive/boundary/negative cases
(`tests/rules_claude.rs:36-46,139-160,195-202,264-289`). `tests/security_leaks.rs` (14 CLI-level,
every command × channel × mode) and `tests/cli_contract.rs` (33) are the strongest suites.

Weak / near-tautological (genuine):

- `tests/cli_contract.rs:44,117,472` assert `!text.contains('\u{1b}')`; this can never fail because
  text output never emits ANSI at all (see §3). Same for `no_color_is_honoured_and_json_never_colours`
  (`:462-473`) — it "proves" NO_COLOR wins on a renderer that never colours.
- `tests/security_leaks.rs:660-698` `the_source_tree_never_spawns_processes_or_opens_sockets` is a
  five-needle substring scan of `src/**/*.rs`; bypassable (`use std::net as n;`), not the capability
  check its name implies.
- `tests/conformance.rs` calls its tests "property" tests, but only the two `fuzz_smoke_*` tests
  (`:189-242`) use the LCG; the rest are fixed examples. `property_redaction_is_idempotent`
  (`:139-161`) calls a pure function twice — verifies determinism, not correctness.

Thin coverage: text rendering of `scan`/`doctor` (only `contains`), `commands::doctor_summary`
(`src/commands.rs:531`), and `--color`/`should_color` (unit-only, `src/cli.rs:807-820`).

## 3. Error handling — centralised and honest, two real gaps

CONFIRMED consistent: `Outcome`/`CliError` are the single boundary (`src/error.rs:21-161`); exit
codes arbitrate in one total function with priority 2→4→3→1→0 (`src/error.rs:54-60`); messages and
locations are sanitized at construction (`:89-95`); `run` prints to stderr and returns the code
(`src/cli.rs:341-350`).

CONFIRMED lint policy holds in `src/`: clippy `-D warnings` is clean, and with
`unwrap_used/expect_used/panic = "warn"` (`Cargo.toml:44-47`) that proves every `unwrap`/`expect`/
`panic!` in `src/` is inside a `#[cfg(test)]` module (e.g. `src/adapters/mod.rs:684`,
`src/model/report.rs:543`); `unsafe` is forbidden twice (`src/lib.rs:6`, `src/main.rs:6`).

CONFIRMED gaps:

- **Broken pipe unhandled.** `docs/implementation-status.md:124` claims a "`BrokenPipe` → exit 0
  mapping in `error`". No `BrokenPipe`/`SIGPIPE` handling exists in `src/`. `print!` to a closed pipe
  panics; the hook (`src/main.rs:13-15`) hides the message but the process exits 101
  (`panic = unwind`; `[profile.release]` does not set `panic = "abort"`). The documented behaviour is
  unsubstantiated.
- **`--color` is a no-op.** `src/cli.rs:530-535` computes `should_color(...)` then discards it
  (`let _ = color;`). No renderer emits ANSI, yet `README.md:117` and the help advertise
  `--color auto|always|never`.

Minor: `list_dir` swallows a `read_dir` failure and returns empty (`src/filesystem.rs:187-189`), so
an unreadable `managed-settings.d/` looks empty — no diagnostic.

## 4. Documentation accuracy — weakest area; several confirmed drift items

CONFIRMED stale test counts (actual 324, above): `CONTRIBUTING.md:44-47` says "270 tests (177 lib +
93 integration)"; `docs/implementation-status.md:27-29,253` says "315 (194 lib + 121 integration)"
and `:193` lists `cli_contract (24 tests)` (actual 33); `docs/verification-log.md` §3 (`:51-62`) says
**315**/`cli_contract 24` while §9 (`:339-341`) in the same file says **324**/`cli_contract 33` — its
"where they disagree, this table is correct" note (`:64-68`) is itself stale.

CONFIRMED contradictory CI claims: `docs/implementation-status.md:68,199` and
`docs/verification-log.md:316-323` say `dependencies`/`fuzz`/`sbom` have never run in CI and
`prerelease.yml` has never run; `docs/maintaining.md:88-93` (updated at HEAD), `CHANGELOG.md`
("0.1.0-beta.2 — released 2026-10-11") and the HEAD commit message say they have run green. Both
cannot be true; neither is verifiable from the checkout.

CONFIRMED wrong names in `docs/decisions.md`: D2 (`:17-31`) names `jsonc::strip_to_json_with_map`
and a byte-offset map — the function is `strip_jsonc` (`src/parse/jsonc.rs:9`) and the design is
length-preserving spaces with *no* offset map (`:1-6`); D12 (`:114-122`) names
`parse::jsonc::prepare` (also `strip_jsonc`); D9 (`:88-92`) names `cli::exit::resolve` (there is no
`exit` module — it is `cli::exit_code_for_known`, `src/cli.rs:672`, + `error::arbitrate`,
`src/error.rs:54`); D10 (`:94-104`) cites `tests/conformance/properties.rs` (the file is the flat
`tests/conformance.rs`); D14 (`:134-141`) lists alias `$SESSION` and id `mcp_1`, but the code emits
only `$HOME/$XDG_CONFIG/$PROJECT/$MANAGED/$EXTERNAL` (`src/security/path_policy.rs:41-62,105-111`) and
`field_N`/`host_N`/`rule_N` (`src/security/redact.rs:467,475`, `src/resolve/mod.rs:349`).

CONFIRMED wrong attribution: `docs/implementation-status.md:177` puts the panic hook "in `error`";
it is in `src/main.rs:13`. `decisions.md:6-8` is candid that some entries name files that do not
exist — honest, but still a drift list.

Positive: the evidence-level claim checks out. Both registries declare
`conformance_status = "fixture_verified_max"` (`compatibility/claude/registry.json:30`,
`compatibility/opencode/registry.json:45`), and a test enforces no rule exceeds `FixtureVerified`
(`src/adapters/mod.rs:901-921`).

## 5. Dependency discipline — one unused runtime dependency (confirmed)

CONFIRMED: `thiserror` is a declared runtime dependency (`Cargo.toml:25`, in `Cargo.lock`) but is
**never referenced** in `src/`, `tests/` or `benches/` (grep: only the manifest); `CliError`
implements `Display`/`Error` by hand (`src/error.rs:71-77`). For a crate whose policy is "justify
every dependency" (`CONTRIBUTING.md:96-101`, `deny.toml:6-9`), shipping a dead one is a credibility
problem — a reviewer's first grep finds it. The other three (`clap` `src/cli.rs:6`, `serde`
`src/adapters/mod.rs:10`, `serde_json`) are all used and all appropriate.

RECOMMENDATION (minor, `deny.toml:46-50`): the "No git dependencies…" comment sits on
`[bans] allow = []`, but `[bans].allow` is the bans-check crate allow-list; git/registry provenance is
enforced by `[sources]` (`deny.toml:52-56`). The policy is correct; the comment is on the wrong key.

## 6. Configuration resolution correctness

CONFIRMED deterministic and side-effect-free: contributions sort by
`(rank, source_id, definition_id)` (`src/resolve/mod.rs:1057-1063`), the scalar winner is the last
non-ignored/non-unresolved contribution (`:1258-1260`), output is a `BTreeMap` (`:597-601`);
byte-identical determinism is asserted (`tests/conformance.rs:62-73`,
`fuzz/fuzz_targets/resolve_precedence.rs:286-297`).

Risks a maintainer would question:

1. **Precedence by insertion order.** The multi-file `oc.project` collection emits ancestors
   outermost-first (`src/adapters/opencode.rs:147-171`) and leans on the `source_id` tie-break to make
   the nearest file win; all `oc.project` candidates share rank 30. Correct today, but a future push
   reorder (or a same-rank second adapter) would silently flip a winner.
2. **Env-pair precedence is a magic constant outside the registry.** `rank: 35`
   (`src/resolve/mod.rs:1041`) places documented env pairs between file layers and session args, yet
   the registry is "the single source of truth for layers, per-key rules…" (`src/adapters/mod.rs:3-5`)
   and `env_pairs` carry no rank (`compatibility/claude/registry.json:299-312`). A client precedence
   decision in shared engine code contradicts D15.
3. **`availableModels` lock reads an arbitrary definition.** `/model` uses
   `definitions.iter().find(|d| d.pointer == "/availableModels")` (`src/resolve/mod.rs:1416-1420`) —
   the lowest-`source_id` one; `managed_sources_behavior` is documentation-only
   (`src/adapters/mod.rs:302-310`). Latent (only one managed source is observable offline).

Evidence level: `/model` is labelled `fixture_verified`
(`compatibility/claude/registry.json:90`, `…/opencode/registry.json:113`) but its cited tests are
ConfigLens' own synthetic fixtures (`tests/rules_claude.rs:26-46`), not upstream. Consistent with
`fixture_verified_max`, though the JSON label reads stronger than "verified against our fixtures".

## 7. Security boundaries

CONFIRMED fail-closed structure: `check_read` returns `Err` unless within an approved root
(`src/security/path_policy.rs:116-122`). Redaction is deny-by-default — subtree → inventory-only →
sensitive-field → shape-allowlist, everything unmatched redacted (`src/security/redact.rs:313-403`).
URL parsing refuses ambiguous authorities (`:185-245`, regression pinned `:696-718`); share-mode
pseudonyms are content-independent and sorted (`:455-478`).

CONFIRMED low-severity gap: `is_within` compares components lexically and does not normalise `.`/`..`
(`src/security/path_policy.rs:26-30`). A candidate that lexically sits under a root but contains `..`
(reachable via an env-derived value such as `$OPENCODE_CONFIG_DIR`) passes `check_read`, and
`File::open` then resolves the `..` (`src/filesystem.rs:161`); `classify` canonicalizes only
symlinks, not `..` (`:200-219`). Low impact — the actor sets their own env/args and the tool is
read-only — but the module's stated invariant (`path_policy.rs:2-4`) is not closed.

CONFIRMED non-issue: `--input-context` cannot cause an arbitrary read — supplied sources parse only
their inline `content`; `path` is a sanitized display location (`src/resolve/mod.rs:622-624,655-659`).

## 8. Performance — not a release blocker

CONFIRMED quadratic and correctly diagnosed: every pointer is resolved independently
(`src/resolve/mod.rs:597-600`) and `collect_contributions` rescans all definitions+sources per
pointer (`:933-943`), O(keys²). Medians are recorded (`docs/implementation-status.md:198`:
1.620 ms / 0 keys → 1 224 ms / 5 000 keys). Real client configs have tens of top-level keys, so the
engine is single-digit milliseconds; 5 000 keys is artificial. **Not a release blocker; I do not
recommend optimising it now** — the docs already say "investigated and documented, not optimised",
which is the right posture, and the documented one-line fix (index definitions by pointer once)
suffices if ever needed. The bench asserts no timing bound, so it cannot be flaky.

## 9. CI reliability — trustworthy, minor notes

CONFIRMED `ci.yml` is deterministic and well-built: pinned action SHAs, `--locked`,
`fail-fast: false`, per-job timeouts, a 3-OS test matrix (`:23-44`), `quality`, `msrv`, and a bounded
`fuzz` smoke that fails on a crash (`:108-114`). `fuzz-scheduled.yml` runs 600 s/target weekly with
`-timeout=25`/`-rss_limit_mb=4096` and uploads reproducers on failure (`:29-75`). The committed corpus
seeds every run (1898/305/258 files), so these are regression runs, not cold starts. Budgets are
meaningful for regression; the 60 s smoke is deliberately too short to find new bugs.

Notes: `cargo install cargo-fuzz --locked --version 0.13.2` runs every invocation (`ci.yml:102`,
`fuzz-scheduled.yml:47`) — a network/availability dependency where actions are SHA-pinned; not flaky,
not fully hermetic. `msrv` runs `cargo check` only (`ci.yml:70`), so the 1.85 floor is compile-proven,
not test-proven (acceptable, documented). `prerelease.yml:20` requires
`github.ref == 'refs/heads/main'`, yet the tree of record for the beta is a `release/*` branch
(`docs/verification-log.md:382`) — a release branch cannot drive this workflow; the runbook should
say so.

## 10. Maintainability and onboarding

CONFIRMED: `CONTRIBUTING.md` + `docs/maintaining.md` give a usable first-PR path — build/test
commands, the regression loop, a concrete add-a-rule procedure (primary source → support state →
rule id → positive/boundary/negative tests), and a redaction-invariant section. A newcomer can land a
fixture or rule PR from these two documents alone.

Frictions (all confirmed above): the stale "270 tests" baseline (`CONTRIBUTING.md:44-47`); the note
that it has "only been built and tested with 1.99.0" (`:28-29`) clashing with the verified-MSRV
claims; and the CI/prerelease status contradiction — the newcomer cannot tell which document is
authoritative, and `maintaining.md:32` points them at `verification-log.md`, which is a stale one.

## Genuine weaknesses only (consolidated)

1. Unused runtime dependency `thiserror` (`Cargo.toml:25`).
2. Stale/incorrect test counts across `CONTRIBUTING.md`, `implementation-status.md`,
   `verification-log.md`; the last is internally contradictory (§3 vs §9).
3. Broken-pipe → exit 0 claimed (`implementation-status.md:124`) but not implemented.
4. `--color` documented but unimplemented (`README.md:117`, `src/cli.rs:530-535`).
5. `decisions.md` names symbols/files that do not exist (D2, D9, D10, D12, D14); panic hook
   mis-attributed (`implementation-status.md:177`).
6. CI/prerelease run-status contradiction across three documents.
7. Path policy does not normalise `..` (`path_policy.rs:26-30`) — stated invariant not closed.
8. Env-pair rank (35) and `availableModels` selection live in engine code, not the per-adapter
   registry.
9. Dead public API (`cli::render_report` `src/cli.rs:644` returns `render_scan(&RunOutput::default())`;
   `cli::exit_code_for` `:652`).
10. `list_dir` failures are silent (`src/filesystem.rs:187-189`).

No other weaknesses found; I did not invent work where the code is genuinely complete.

## Fix before public launch vs. fine to leave

Fix before launch: remove `thiserror` (1); correct the test counts and the §3/§9 contradiction (2);
fix or delete the broken-pipe claim and the wrong hook location (3, 5); reconcile the CI/prerelease
status so all four documents agree (6); fix the `decisions.md` names (5); wire or drop `--color` (4).

Fine to leave for 0.1 (documented or low-impact — do not gold-plate): quadratic scaling at 5 000 keys;
the Windows snapshot-ACL limitation; and `..` normalisation, moving env-pair rank into the registry,
making `oc.project` depth explicit, deleting the dead `render_report`/`exit_code_for`, and giving
`list_dir` a diagnostic (7-10) — follow-up issues, none blocks a credible launch.
