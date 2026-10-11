# ConfigLens engineering decisions

Each decision records the concrete tradeoff. Rejected alternatives are listed so a reviewer can
disagree with the reasoning rather than guess at it.

> **Status note:** a few decisions below reference artifacts that are planned but **not present** in
> this checkout (for example `docs/security.md` and `tests/security/no_raw_dto.rs`). They record
> rationale, not a file index. See `docs/implementation-status.md` for what actually exists.

## D1 — One crate, library + binary (spec §6 offers this as the default)

**Decision:** single package `configlens` with `src/lib.rs` + `src/main.rs`.
**Why:** in-process unit tests need library access to internals (parser, redaction, reducers);
no boundary justifies a workspace yet.
**Rejected:** workspace with `configlens-core` + `configlens-cli` — extra plumbing for zero benefit at 0.1.

## D2 — Hand-written span-preserving JSON/JSONC parser (no `jsonc-parser`) (spec §6)

**Decision:** implement `parse::json` (a JSON/JSONC scanner + depth-aware duplicate-key detection).
**Why:**
- The spec requires *no silent duplicate-key loss*, byte spans for every definition, and *no regex
  comment stripping* for JSONC. A hand-written scanner gives all three by construction: comment and
  string handling happen in one pass over bytes (`jsonc::strip_to_json_with_map` produces both the
  comment-free text and a byte-offset map back to the original source, so spans stay truthful).
- `serde_json` (already a dependency) has no span or duplicate-key evidence, so we would still need
  a second pass — the "write a detection pass if necessary" escape hatch in §6.
- Avoids pinning an extra parser whose maintenance/licence had to be re-verified.
**Cost:** we own the parser and its fuzz surface. Mitigated by a bounded, iterative-free recursive
descent with explicit depth/byte limits and a mutation smoke test.
**Rejected:** `jsonc-parser` (span support, but duplicate-key and limit semantics would still need
our own pass); regex comment stripping (explicitly forbidden).

## D3 — `serde_json` `Map` stays `BTreeMap` (no `preserve_order`)

**Decision:** do not enable `serde_json/preserve_order`.
**Why:** the report schema requires *sorted maps and stable deterministic ordering*; object key order
from a file is meaningless for precedence and would make output non-deterministic across sources.
Object *merge* semantics are modelled explicitly by our own operator, not by map insertion order.

## D4 — Redaction is a projection, not a serializer option

**Decision:** raw values live in `model::definition::SensitiveValue`, which has **no**
`Serialize`, no `Display`, and a `Debug` that prints `<sensitive>`. Only `security::redact`
produces `model::report::PublicValue`, the only value type that implements `Serialize`.
**Why:** makes "renderers cannot serialize raw input" a type-system property, not a code-review
property. A compile-fail test (`tests/security/no_raw_dto.rs` documents the intent; the actual
enforcement is the absence of the impls) plus a grep guard keep it honest.

## D5 — Redaction defaults to "redact unless allowlisted"

**Decision:** a value is public only if (a) the final pointer segment is not sensitive, (b) the key
is in the adapter's verified allowlist, and (c) the value shape passes an allowlist predicate
(boolean, integer, known enum, model-name shape, known URL key).
**Why:** section 7 says arbitrary environment values and unknown free-text fields are redacted by
default. Denylists leak the moment a new client key appears.
**Consequence:** a genuinely public but unrecognised string (for example a new `theme` value) is
reported as `redacted:true, reason:"unclassified_value"`. That is a deliberate usability cost paid
for safety; `docs/security.md` tells the user how to read it.

## D6 — `env` capture is an explicit allowlist, disabled by `--no-env`

**Decision:** the only process variables ConfigLens ever reads are
`HOME`, `XDG_CONFIG_HOME`, `PROGRAMDATA`, `USERPROFILE`,
`OPENCODE_CONFIG`, `OPENCODE_CONFIG_DIR`, `OPENCODE_CONFIG_CONTENT`, `OPENCODE_TUI_CONFIG`,
`CLAUDE_CONFIG_DIR`, `ANTHROPIC_MODEL`, and only as *configuration pointers/values*.
**Why:** §4 requires a finite adapter allowlist; reading the whole environment would put secrets in
reach of the redactor for no user benefit.
**Note:** `OPENCODE_CONFIG_CONTENT` is config *content*, so it is parsed as an `Inline` source and
its values go through the same redaction projection. It is never echoed.

## D7 — Trust is modelled as a tri-state affecting applicability, not a filter

**Decision:** `Trust::{Unknown,Trusted,Untrusted}`. Claude `permissions.allow` and most `env` values
from project files are `Conditional` when trust is `Unknown`, and produce a `trust_unknown`
coverage gap. `deny`/`ask` apply unconditionally (documented).
**Why:** acceptance example 5 — "record skipped project layer and explain why; unknown trust remains
conditional" — forbids inventing a decision either way.

## D8 — Unobservable layers always produce a coverage gap

**Decision:** OpenCode remote defaults and macOS MDM preference domains; Claude server-managed
settings, MDM/OS policy, HKCU registry, and a missing/unreadable managed file — all emit
`CoverageGap { kind: ExpectedUnobservable }`.
**Why:** §7 "Remote, MDM and cloud state are not assumed absent." Consequence: `effective_value` is
withheld and status is `partial` unless a rule proves the missing layer cannot affect that key
(no such per-key completeness proof is implemented at 0.1, so `partial` is the honest answer).

## D9 — Exit-code arbitration is centralised and total

**Decision:** every command returns `Result<Report, CliError>`; `cli::exit::resolve` maps the
collected findings to the code with the documented priority `2 → 4 → 3 → 1 → 0`.
**Why:** per-command `exit(1)` calls are how exit-code contracts rot. One function, one test matrix.

## D10 — No `proptest` / no `insta` / no `criterion` dependency

**Decision:** property tests use a small deterministic LCG-based generator
(`tests/conformance/properties.rs`), snapshots use literal `assert_eq!` on stable strings, and the
benchmark is a plain `std::time::Instant` harness behind `#[bench]`-free `cargo bench`-shaped `main`.
**Why:** §6 says to justify every dependency; none of the three earns its weight here, and the
deterministic generator is *stronger* for our main property (identical context ⇒ byte-identical
output), because the seed is recorded in the failure message.
**Rejected:** `proptest` (rand + shrinker churn, non-deterministic failures), `insta` (writes files
during test runs, which conflicts with "no automatic caches/log files"), `criterion` (heavy build
and no numbers we are willing to publish).

## D11 — `unsafe_code = "forbid"`, no `unsafe` for the symlink race

**Decision:** accept the documented TOCTOU limitation instead of using `openat2`/`O_NOFOLLOW`
platform code; document it in `docs/security.md` and the spec's own allowance ("Do not claim
complete TOCTOU immunity across all OSes").
**Why:** `unsafe` is forbidden crate-wide; the mitigation we do implement is metadata validation
(`is_file`, not symlink-to-outside-root, size limit *before* a bounded read).

## D12 — JSONC is handled by pre-pass + offset map, not by a second parser

**Decision:** `parse::jsonc::prepare(source)` walks the bytes tracking string state and returns the
comment-free JSON text plus a monotonic `Vec<usize>` mapping comment-free byte offsets to original
offsets (comments are replaced by spaces so line/column arithmetic still works).
**Why:** one parser, one set of span semantics, and JSONC-specific tests can assert that a `//`
inside a string is *not* a comment (the classic regex-stripping bug).
**Cost:** span offsets need mapping; a bug here would misreport columns, so the mapping is tested
directly (`parse::jsonc` unit tests + discovery tests asserting line/column).

## D13 — The managed tier is a policy phase, not a higher merge layer

**Decision:** Claude managed files (and OpenCode managed files) are resolved as constraints
(`ResolutionEvent::Constrained`) and additionally participate in the merge for the keys they define
with the documented semantics (whole-value replace for scalars, ordered replace for `fallbackModel`,
stable concat + dedupe for lists).
**Why:** §5 step 7 requires a distinct policy phase. `availableModels` demonstrates the difference:
it does not *set* `/model`, it *locks* it, so it must appear as a constraint on the resolution of
`/model` rather than as a definition of it.

## D14 — `--paths share` pseudonymizes deterministically from sorted order

**Decision:** share mode emits `$PROJECT`, `$HOME`, `$XDG_CONFIG`, `$MANAGED`, `$SESSION` aliases,
host hashed to `host_<n>` … plus `mcp_1`, `field_1` opaque IDs assigned by sorted order of the
*definition*, never by content.
**Why:** §5 "machine-specific path bytes never become stable IDs", §7 "Source-level public IDs
identify location roles, not content fingerprints". Sorting first keeps output deterministic, which
the determinism property test asserts.

## D15 — Adapter rules live in the adapter, operators stay client-neutral

**Decision:** `resolve::operators` knows nothing about clients. Adapters supply
`RuleSupport { rule_id, operator, evidence, support, notes }` per pointer and the ordered layer
list. Client-specific quirks (Claude's `permissions.defaultMode` scope exclusion) are expressed as
predicate hooks the adapter owns (`fn is_scope_excluded_definition`).
**Why:** §6 "Generic operators must not encode client policy."

## D16 — Version handling: `--client-version` is a claim, `--strict` enforces it

**Decision:** `VersionContext` = `Unknown | Claimed(String)`. Rules carry a `tested_versions`
string; an unknown or untested claim marks supported values *provisional* and `--strict` exits 3.
**Why:** §4 "Unknown client version: mark output provisional; `--strict` fails supported-value
assertions."
**Rejected:** guessing a "supported range" from a semver string. None is published until tested.

## D17 — Snapshot write path

**Decision:** `tempfile`-style create-new in the destination directory → write → `chmod 0600` →
`rename`. `--overwrite` is required to replace an existing file; a symlink destination is rejected
outright. On Windows the ACL is not restricted and the tool says so in the JSON report
(`redaction.snapshot_acl: "not_implemented"`).
**Why:** §7 atomic-write + owner-only requirement, and refusing to overclaim on Windows.
**Windows consequence (documented, behaviour unchanged):** Windows has no `0600` equivalent. The
snapshot is created as a same-directory temporary file and then renamed into place, so it
**inherits the ACL of the destination folder** — ConfigLens neither tightens nor loosens it. The
report states this (`redaction.snapshot_acl = "not_implemented"`) rather than claiming a protection
that does not exist. Guidance: save snapshots in a **private, user-controlled directory** (for
example `%USERPROFILE%`) rather than a shared or world-readable folder, so the inherited ACL is the
one you intend.
**Rejected for 0.1:** a restrictive DACL. Implementing one would need Win32 security APIs through
`unsafe` FFI or a new Windows-security dependency; this crate forbids `unsafe` crate-wide
(`unsafe_code = "forbid"`) and keeps a deliberate minimal-dependency policy. A DACL is therefore out
of scope for 0.1 — a stated scope boundary, not a silent gap.

## D18 — Candid downgrades rather than invented parity

**Decision:** where the documentation (not the loader) is the only evidence, rules are labelled
`documented` rather than `fixture_verified`; where even documentation is thin, `inventory_only`.
**Why:** §3 "Documentation and fixture tests alone do not establish runtime equivalence" and §12
"If a rule is blocked, explicitly downgrade its support and continue independent tasks."

## D19 — Test isolation

**Decision:** every test that touches the filesystem builds its own synthetic home with `tempfile`
and a `MemoryEnv` map; the real `$HOME` is never read by tests. `tests/cli/*` pass `--cwd` and
`HOME`/`XDG_CONFIG_HOME`/`CLAUDE_CONFIG_DIR` explicitly via `Command::env`.
**Why:** §12 "Tests must use isolated synthetic homes and captured env; never read the developer's
real credentials."

## D20 — MSRV

**Decision:** declare `rust-version = "1.85"` and verify it by building with a 1.85.0 toolchain.
**Why:** §8 CI requires a "declared MSRV build"; declaring a number we have not executed would be a
fabrication. If the 1.85 build had failed we would have raised the declaration to the verified
toolchain instead of lowering it silently.
