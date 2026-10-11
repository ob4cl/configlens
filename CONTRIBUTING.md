# Contributing to ConfigLens

Thanks for helping. This is a **private beta**: the repository is visible to maintainers and invited
beta testers only, and there is no public release yet. Contributions that improve correctness,
diagnostics and evidence are the most valuable kind.

Before anything else, read the two things that keep this project honest:

- [`docs/implementation-status.md`](docs/implementation-status.md) — what is actually implemented
  and verified, item by item.
- [`docs/decisions.md`](docs/decisions.md) — the tradeoffs behind the design.

## Ground rules

1. **Correctness beats features.** A narrower, verified capability is better than a broad, guessed
   one.
2. **No invented semantics.** Do not assign a merge rule to a key you have not verified, and do not
   copy one client's precedence onto another. Unknown keys stay `inventory_only`.
3. **Never commit real secrets or third-party configuration.** All fixtures and tests use synthetic
   values in isolated temporary homes.
4. **Keep the report contract stable.** The public report shape is the bundled JSON Schema
   (`src/schema/report-1.0.json`); changes to it are a breaking change and need discussion first.
5. **Redaction is a type-system property.** Raw values live in `SensitiveValue`, which has no
   `Serialize`/`Display`. Do not add one. See "Redaction invariants" below.

## Prerequisites

- A Rust toolchain. `Cargo.toml` declares `rust-version = "1.85"`, but the project has only been
  built and tested with `1.99.0` so far — see [`docs/verification-log.md`](docs/verification-log.md).
- No AI client needs to be installed, and no network access is needed at run time.

## Build and test

Run every check below locally before you open a pull request.

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --no-fail-fast
cargo build --locked --release
./demo/demo.sh          # isolated synthetic end-to-end smoke test; exits 0
```

The recorded run in [`docs/verification-log.md`](docs/verification-log.md) covered 324 tests (194
library unit tests plus 130 integration tests across `tests/acceptance.rs`, `tests/cli_contract.rs`,
`tests/conformance.rs`, `tests/rules_claude.rs`, `tests/rules_opencode.rs` and
`tests/security_leaks.rs`). Keep them all green.

## The regression workflow

When you change behaviour, the test *is* the deliverable. Follow this loop:

1. **Reproduce the behaviour in a test first.** Add a focused test that fails for the current code
   and states the expected behaviour in its name. Tests that touch the filesystem must build their
   own synthetic home with `tempfile` and pass `HOME` / `XDG_CONFIG_HOME` / `--cwd` explicitly —
   never read the developer's real configuration (decision D19).
2. **Make the smallest change** that turns the test green. Prefer fixing the root cause over
   special-casing the input.
3. **Add the boundary and negative cases**, not just the happy path. A single new rule needs at
   least: one positive case, one boundary case (e.g. `null` vs missing vs empty list) and one
   negative case (the rule must *not* apply).
4. **Re-run the full suite**, plus the demo.
5. **Record what you verified** in [`docs/verification-log.md`](docs/verification-log.md) if you ran
   commands the log does not already cover, and update
   [`docs/implementation-status.md`](docs/implementation-status.md) if a checkbox changed.

### Adding a client rule

An advertised rule is only as good as its evidence. To add or promote one:

1. Find the **primary source** (upstream documentation or, where available, the pinned loader
   commit). Record the URL, the retrieval date and the pinned commit in the adapter's registry under
   `compatibility/`.
2. Decide the **support state** honestly: `inventory_only`, `documented`, `fixture_verified` or
   `upstream_conformance_verified`. Documentation and fixtures alone never justify
   `upstream_conformance_verified`.
3. Give the rule an **ID** (`<client>.<area>.<rule>`) and implement it in the adapter
   (`src/adapters/`). Client-neutral operators live in `src/resolve/`; client policy stays in the
   adapter (decision D15).
4. Add **positive / boundary / negative** tests and list them in the registry's rule → test table.
5. If the rule defines new *public* values, add a seeded-secret leak case showing they cannot leak
   (see below).

## Redaction invariants

These have their own test target, `tests/security_leaks.rs`, and must stay green:

- Secrets in fields, object keys, paths, URLs, arguments, malformed input, environment and error
  messages must appear in **no** channel: stdout, stderr, JSON, snapshots or diffs.
- Public values are allowlisted per adapter (booleans, integers, known enums, verified lower-case
  model names, allowlisted URL keys). A new publishable value shape needs a shape predicate *and* a
  leak test — the model-name predicate that once accepted upper-case tokens is the cautionary tale
  (see the verification log).
- Do not export hashes of secrets for comparison. Two redacted values compare as `unknown_redacted`.

## Dependencies

Justify every new dependency: the current set is deliberately small (`clap`, `serde`, `serde_json`,
`thiserror`, plus `assert_cmd` / `predicates` / `tempfile` for tests). Property tests use a
deterministic generator rather than `proptest`, and snapshots are literal strings rather than
`insta` (decision D10). If in doubt, open an issue before adding one.

## Style

- `rustfmt` and `clippy -D warnings` are the gate. Production code keeps `unwrap_used`,
  `expect_used` and `panic` lints active; test code may allow them.
- `unsafe` is forbidden crate-wide.
- Keep raw values out of `Debug`, `Display`, tracing and serializable structs.

## Pull requests

- Keep changes small and reviewable; one concern per pull request.
- Describe **what you verified** and how, and call out anything you could not verify.
- Update the relevant docs in the same change. If a claim in a doc is no longer true, fix the doc —
  do not leave it aspirational.
- All contributors are expected to follow the [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

## Reporting bugs and security issues

- Ordinary bugs and unclear diagnostics: open an issue in this repository.
- Security issues: follow [`SECURITY.md`](SECURITY.md). Do not open a public issue with exploit
  details or real configuration.
