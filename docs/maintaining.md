# Maintaining ConfigLens

This is the maintainer-side companion to [`CONTRIBUTING.md`](../CONTRIBUTING.md). It covers how the
project is verified, how claims are kept honest, and what still has to happen before a release.

## Principles

1. **Every claim is backed by evidence.** A capability is only "done" if a recorded command or a
   named passing test proves it. Anything else is `[~]` or `[ ]` in
   [`implementation-status.md`](implementation-status.md).
2. **Downgrade rather than invent.** If a rule cannot be verified, lower its support level
   (`fixture_verified` → `documented` → `inventory_only`) instead of guessing. Never claim
   `upstream_conformance_verified` without a pinned loader harness.
3. **Docs track reality.** If behaviour changes, the affected doc changes in the same commit. A doc
   that describes an aspiration is a bug.

## Verification routine

Run this routine on every change. The repository's workflow files under `.github/workflows/` repeat
much of it on Linux, macOS and Windows plus a declared-MSRV check; treat their results as an
additional gate, never a substitute for running the checks yourself:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --no-fail-fast
cargo build --locked --release
./demo/demo.sh
```

Record the real results in [`verification-log.md`](verification-log.md) — commands actually run,
with their actual output. The log is the source of truth for "is this verified?"; the earlier
`docs/spec/` and `docs/implementation-status.md` documents defer to it.

When a run fixes a bug, add a row to the log's "fixes made in this run" table: symptom → root cause →
fix → the regression test that now guards it. The failing test *is* the regression test.

## Keeping the support matrix honest

Each adapter carries a registry under `compatibility/<client>/`:

- `registry.json` — the machine-readable metadata (adapter revision, tested and documented versions,
  pinned commit, evidence).
- `registry.md` — the human-readable evidence snapshot: source URLs, retrieval dates, pinned-commit
  status, the documented precedence order, per-key rules, and a rule → test-ID table.

When you add or promote a rule, update both, and keep the support-state summary at the bottom of the
`.md` consistent with the per-key table. The `clients` command prints the tested versions and the
always-incomplete layers straight from these registries, so an out-of-date registry shows up in the
CLI output.

When upstream documentation moves (paths, versions, precedence language), treat it as a **new**
evidence snapshot with a new retrieval date. Do not silently overwrite history; note the change.

## Documentation ownership

These files are maintained together and should stay consistent with each other:

- `README.md` — positioning, quick start, commands, compatibility, privacy.
- `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `CHANGELOG.md`.
- `LICENSE-MIT`, `LICENSE-APACHE` (dual `MIT OR Apache-2.0`, owner `ob4cl`).
- `docs/getting-started.md`, `docs/maintaining.md`, `docs/assets/`.
- `docs/implementation-status.md`, `docs/decisions.md`, `docs/verification-log.md`.

Do not introduce links to files that do not exist. The specification names several documents that
this checkout does not have (`docs/semantics.md`, `docs/compatibility.md`, `docs/report-schema.md`,
`docs/security.md`); until they exist, link to the registries and the status document instead.

## Redaction invariants (do not regress these)

- Raw values stay in `SensitiveValue`, which has no `Serialize`/`Display`. Only the redaction
  projection produces reportable values.
- `tests/security_leaks.rs` must stay green: seeded secrets in fields, keys, paths, URLs, arguments,
  malformed input and errors appear in **no** output channel.
- New public value shapes need a shape predicate *and* a leak test.
- Share mode pseudonymizes paths and unknown key names, and strips URL userinfo/query/fragment/hosts.

## Release checklist (not yet satisfied)

The `0.1.x` line is a private-beta prototype (currently `0.1.0-beta.3`). Before any published
release, these must be true — they are the "launch dependencies". Open items are marked `[ ]`,
satisfied ones `[x]`:

- [x] **Publish target and URL.** `Cargo.toml` carries the real repository URL
      (`repository = "https://github.com/ob4cl/configlens"`) plus licence, readme, description,
      keywords and categories. Still open before publishing to a registry: confirm crate/crate-name
      availability, and that the metadata is exactly what you want shipped.
- [x] **CI.** Every `ci.yml` job is green — the Linux/macOS/Windows test matrix, `quality`,
      `msrv`, `dependencies` (cargo-deny) and the bounded `fuzz` smoke — most recently on `main` at
      `51a2ca7`. `prerelease.yml` has now run end to end successfully for `v0.1.0-beta.1` and
      `v0.1.0-beta.2`. The weekly `fuzz-scheduled.yml` has **not** fired yet (its first run lands on
      its schedule). Continue to confirm every job is green on each release commit, and keep Actions
      pinned to verified commits.
- [x] **MSRV actually built.** `cargo +1.85.0 check --locked --all-targets --all-features` exits `0`
      (§8.1 of `verification-log.md`) and the CI `msrv` job runs the same check; local development used
      1.99.0.
- [ ] **Conformance, or an honest label.** Either build a pinned loader harness, or keep every rule at
      `fixture_verified`/`documented` and say so.
- [x] **Benchmarks.** `benches/resolution.rs` runs (`cargo bench`); its medians and quadratic scaling
      are recorded in `docs/implementation-status.md`. Publish no speed claim beyond those numbers.
- [ ] **Fixtures tree.** `fixtures/` and `examples/` are empty; fixtures are inline in tests.
- [ ] **Known gaps closed or accepted.** Windows snapshot ACLs. (Share-mode URL-host pseudonymization
      is now implemented and tested.)
- [ ] **Artifacts.** Tagged source, binaries and checksums; no mandatory curl-to-shell installer.

Until all of that is true, keep the honest status wording in the README and changelog.

## What not to do

- Do not push commits, publish packages, create public posts, or post to other communities as part of
  routine maintenance without explicit authorisation.
- Do not paste real configuration, credentials or third-party secrets into the repository, issues or
  the verification log — use synthetic values.
- Do not overstate compatibility to make a milestone. Cutting a feature is always preferable to
  inventing a claim.
