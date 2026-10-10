# ConfigLens

Explain which AI coding-tool configuration value wins, and why — offline, read-only, redacted.

ConfigLens is a local Rust CLI (one library + one binary) that traces supported configuration keys
for **Claude Code** and **OpenCode** across their configuration files and *supplied* session inputs.
It never executes your configuration, never makes a network request, never runs a client binary and
never reads credentials. Values are redacted unless an adapter explicitly allowlists them.

> **Status: 0.1, work in progress.** Exactly what is implemented, verified and **not** implemented
> is listed item by item in [`docs/implementation-status.md`](docs/implementation-status.md). This
> README is not a completeness claim. In particular there are **no** CI workflows, benchmarks,
> `fixtures/` tree, license files or extra design documents in this checkout yet.

## Build

Requires Rust (edition 2021). The declared minimum is 1.85 (`Cargo.toml`); the build has only been
verified with the toolchain recorded in [`docs/verification-log.md`](docs/verification-log.md).

```sh
cargo build --locked --release
./target/release/configlens --help
```

## Run the isolated synthetic demo

`demo/demo.sh` reads no real configuration and needs no network. It creates a throwaway home and
project under a temporary directory, writes only obviously-fake values, clears the environment, and
runs the CLI against that sandbox:

```sh
cargo build --locked --release
./demo/demo.sh
```

## Commands

```sh
configlens <command> --client <opencode|claude> [options]
```

| Command | What it does |
|---|---|
| `clients` | Report implemented adapters and label unsupported clients. |
| `scan` | Inventory candidate sources, applicability and readability. |
| `explain KEY` | Show the observed winner (or unresolved result) for one RFC 6901 pointer, with coverage gaps. |
| `doctor` | Parse/schema/applicability/coverage diagnostics. Never certifies a configuration as safe. |
| `snapshot` | Write the redacted report to an explicit destination (`--output`, `--overwrite`). |
| `diff` | Compare two reports from the same client and schema; protected values compare as `unknown_redacted`. |
| `schema --kind report` | Emit the bundled report JSON Schema (no network). |

Useful options: `--cwd`, `--client-version` (a claimed version, never read from a binary),
`--format text|json`, `--strict`, `--no-env`, `--input-context PATH`, `--paths local|share`,
`--color auto|always|never`, `--allow-read-root PATH`, `--project-root PATH`.

## Exit codes

`0` report ok (non-fatal findings allowed) · `1` `doctor` findings at `--fail-on` · `2` usage or
incompatible input · `3` unsupported key/client semantics, or `--strict` detected an incomplete
context · `4` required file inaccessible, malformed or over a resource limit. When several apply the
priority is `2 → 4 → 3 → 1 → 0`.

## Security posture

- Offline and read-only: no network, no subprocess, no client binary execution (a test scans `src/`
  for the corresponding APIs).
- Redact unless allowlisted: only the redaction projection produces reportable values.
- `--paths local` (default) shows locations and key names for diagnosis. `--paths share`
  pseudonymizes paths, replaces unknown key names with opaque field IDs, and strips URL
  userinfo/query/fragment.
- Snapshots are written atomically, create-new by default, `0600` on Unix (Windows ACLs are not
  implemented and the report says so).

## Development checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --no-fail-fast
```
