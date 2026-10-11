![ConfigLens](docs/assets/configlens-title.svg)

# ConfigLens

**Find out why your AI coding tool is ignoring your configuration.**

ConfigLens is a small, local, read-only Rust CLI that traces a supported configuration key for
**Claude Code** and **OpenCode** across supported configuration sources, and shows the observed
winner — with the file, line and reason for each contribution, including the ones that lost.

It never executes your configuration, never runs a client binary, opens no socket and reads no
credential. Values are redacted unless an adapter explicitly allowlists them.

> **Private beta — version 0.1.0.** This checkout is a working prototype, not a finished product.
> There is **no published crate or binary, and no benchmarks yet**, and nothing here claims upstream
> conformance. Exactly what is implemented and verified is tracked item by item in
> [`docs/implementation-status.md`](docs/implementation-status.md).

## The question this answers

You set a model, a permission, an MCP flag. The tool ignores it. You now have five plausible files,
an environment variable, a session flag and a managed policy that might all be involved — and the
tool just says "using the other one". ConfigLens answers the four questions that follow:

- **Which value won?** The observed winner among everything it could read.
- **Where did each value come from?** File, line and column for every contribution, in precedence
  order, including the definitions that were replaced or skipped.
- **Which file should I edit?** The winning source is named, so you edit the right one.
- **Can I paste this into an issue?** Reports redact protected values, and `--paths share`
  reduces identifying details. Review the output and the privacy limitations below before sharing;
  a redacted report is not a guarantee of anonymity.

## Quick start (from source)

Until a package is published, build from this checkout. No client needs to be installed, and no
network access is needed at run time.

```sh
cargo build --locked --release
./target/release/configlens --help
```

Prefer to install the binary into your `PATH` from the checkout:

```sh
cargo install --locked --path .
```

## Run the isolated demo

`demo/demo.sh` reads **no real configuration**. It creates a throwaway home and project under a
temporary directory, writes only obviously-fake values, clears the environment, and runs the CLI
against that sandbox. Nothing outside the temporary directory is written, and no network or client
subprocess is used.

```sh
cargo build --locked --release
./demo/demo.sh
```

## A real terminal example

Captured from the isolated demo above (synthetic values only). The long coverage line is elided —
the tool prints every gap.

```console
$ configlens explain /model --client claude --client-version 2.1.296
/model = "local-model" (observed)
Observed winner: $PROJECT/.claude/settings.local.json:1:12
  "project-model"  $PROJECT/.claude/settings.json:2:12  replaced_by_later_layer
  "local-model"  $PROJECT/.claude/settings.local.json:1:12  observed_winner
Rule: claude.scalar.layer-override
Coverage: partial — cc.managed.mdm — platform_policy_store_not_read (expected_unobservable); … (3 more gaps)
Support: fixture_verified; confidence: evidence_class:fixture_verified
Conclusion: "local-model" wins among the observed applicable sources; effective value withheld.
```

Read it as: the project file set `model` to `"project-model"`, a later layer replaced it with
`"local-model"`, and `"local-model"` is what ConfigLens observed to win. It deliberately does **not**
call that the runtime value, because a higher layer it cannot read might still change it — see
[bounded semantics](#what-wins-actually-means).

## Commands

```
configlens <command> [options]
```

| Command | Example | What it does |
|---|---|---|
| `clients` | `configlens clients` | List implemented adapters with their tested versions, the layers that are always incomplete, and the clients explicitly unsupported. |
| `scan` | `configlens scan --client claude` | Inventory candidate sources with applicability and readability, plus coverage gaps. No recursive crawl. |
| `explain` | `configlens explain /model --client claude --client-version 2.1.296` | Explain one RFC 6901 JSON Pointer: the ordered trace, the observed winner (or an unresolved result) and coverage gaps. |
| `doctor` | `configlens doctor --client opencode --format json --strict` | Parse / schema / applicability / coverage diagnostics. Never certifies a configuration as safe. |
| `snapshot` | `configlens snapshot --client claude --output before.json` | Write the redacted report JSON to an explicit path. Create-new by default; `--overwrite` to replace. |
| `diff` | `configlens diff before.json after.json` | Compare two reports from the same client and schema; protected values compare as `unknown_redacted`. |
| `schema` | `configlens schema --kind report` | Print the bundled report JSON Schema. No network. |

Run `configlens --help` (and `configlens <command> --help`) for the current, authoritative list — the
table above covers the core commands.

### Options

`--client <opencode|claude>` is required for resolution. Shared options:
`--cwd <PATH>` (default `.`), `--client-version <VERSION>` (a *claimed* version — ConfigLens never
reads a client binary), `--format text|json`, `--strict`, `--no-env`, `--input-context <PATH>`,
`--color auto|always|never`, `--paths local|share`, `--allow-read-root <PATH>` (repeatable),
`--project-root <PATH>`. `snapshot` adds `--output`/`--overwrite`; `doctor` adds
`--fail-on warning|error`.

### Exit codes

| Code | Meaning |
|---|---|
| `0` | Report produced. Non-fatal findings are allowed. |
| `1` | `doctor` findings at or above `--fail-on` (default `error`). |
| `2` | Usage error or incompatible input/report. |
| `3` | Requested key/client semantics unsupported, or `--strict` found an incomplete context. |
| `4` | A required file was inaccessible, malformed or over a resource limit. |

When several apply, the priority is `2 → 4 → 3 → 1 → 0`.

## What "wins" actually means

ConfigLens is a static analyser of the inputs it can read. It does not inspect a running session and
it never assumes an unreadable layer is absent. That has three visible consequences:

- **Observed, not effective.** When a higher-priority layer exists but is unobservable (a
  server-managed policy, an MDM/registry setting, OpenCode remote defaults) — or the project trust
  state is unknown — the result is marked `partial`, the winner is labelled an *observed* winner,
  and the effective value is **withheld** alongside the list of coverage gaps.
- **Inventory, not guessing.** An unknown key is preserved as opaque inventory and never given an
  invented merge rule. An unknown or untested client version is reported provisionally; `--strict`
  then exits `3`.
- **Provenance, not authorization.** For Claude permission lists, ConfigLens shows how list entries
  combine and prints an explicit caveat that this is **not** an authorization decision.

So exit code `0` means "a report was produced", not "this is the value your tool will use".

## Compatibility

Two bounded adapters ship today. Both are `fixture_verified` at most — verified against the
project's own fixtures and the current upstream documentation, **not** against the upstream loaders,
so no rule claims `upstream_conformance_verified`. Support states run
`inventory_only < documented < fixture_verified < upstream_conformance_verified`.

| Client | Adapter revision | Tested / documented version | Max support | Always-incomplete layers |
|---|---|---|---|---|
| OpenCode | 0.1.0 | 1.18.35 | `fixture_verified` | remote defaults, `.opencode` directories, macOS managed preferences |
| Claude Code | 0.1.0 | 2.1.296 | `fixture_verified` | MDM policy store, server-managed settings, HKCU registry, managed-file-absent |

Per-key detail, evidence URLs, retrieval dates and rule→test IDs live in the adapters' registries:
[`compatibility/opencode/registry.md`](compatibility/opencode/registry.md) and
[`compatibility/claude/registry.md`](compatibility/claude/registry.md).

These clients are **not implemented** and are labelled `unsupported` — no compatibility claim is
made from shared file names: Codex and Gemini (0.2 backlog), Aider (backlog), and Cursor, Copilot
CLI, Cline, Roo and Hermes (not investigated). Run `configlens clients` for the current matrix.

## Privacy and caveats

- **Offline and read-only.** No network, no subprocess and no client-binary execution; a test scans
  `src/` to keep it that way. `$schema` references are never fetched.
- **Redact unless allowlisted.** Only the redaction projection produces reportable values; secrets
  appear as `{"redacted": true, "reason": "..."}`.
- **Two path modes.** `--paths local` (default) shows config locations and key names so you can
  diagnose; `--paths share` pseudonymizes paths, replaces unknown key names with opaque IDs, and
  strips URL userinfo, query and fragment.
- **Snapshots.** Written create-new, atomically, with `0600` owner-only permissions on Unix.
  Windows ACLs are **not** implemented and the report says so.
- **Known 0.1 gap.** Windows snapshot ACLs are not implemented.
- Reports still describe your project structure. Never paste raw credential files into an issue.

## Requirements

Rust, edition 2021. `rust-version = "1.85"` is declared in `Cargo.toml`, but the only toolchain the
tests and demo have actually been run with is `1.99.0`; the declared MSRV is not yet verified. See
[`docs/verification-log.md`](docs/verification-log.md) for the exact commands and results.

## Contributing

Good first contributions are synthetic fixtures, clearer diagnostics and version-specific rule
evidence. Every adapter change needs a primary source, a declared compatibility scope and
positive/negative tests — and never a real secret or proprietary instruction. The development
checks are:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --no-fail-fast
```

Run these locally before opening a pull request. See
[`CONTRIBUTING.md`](CONTRIBUTING.md) for the regression workflow and
[`docs/maintaining.md`](docs/maintaining.md) for the maintainer-side process. Report security
issues as described in [`SECURITY.md`](SECURITY.md).

## Documentation

- [`docs/getting-started.md`](docs/getting-started.md) — build, first run, and how to read the output.
- [`docs/maintaining.md`](docs/maintaining.md) — verification, adding rules and the release checklist.
- [`docs/implementation-status.md`](docs/implementation-status.md) — what is implemented, verified and not.
- [`docs/verification-log.md`](docs/verification-log.md) — the commands that were actually run.
- [`docs/decisions.md`](docs/decisions.md) — engineering tradeoffs and rejected alternatives.
- [`docs/spec/ConfigLens_Technical_Specification.md`](docs/spec/ConfigLens_Technical_Specification.md) — the design it is built to.

## License

Licensed under either of

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))
- MIT license ([`LICENSE-MIT`](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this project by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without
any additional terms or conditions.
