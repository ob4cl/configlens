# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Until `0.1.0` is published, entries describe the in-repo prototype. Nothing here is a claim about a
published crate, a downloadable binary or verified upstream conformance — for the honest, item-by-item
account of what is implemented and verified, see
[`docs/implementation-status.md`](docs/implementation-status.md) and
[`docs/verification-log.md`](docs/verification-log.md).

## Unreleased

Nothing yet.

## 0.1.0 — 2026-10-10 (private beta prototype, not published)

The initial CLI prototype: a local, read-only, offline Rust tool that traces supported configuration
keys for two clients.

### Added

- **Commands** — `clients`, `scan`, `explain <KEY>`, `doctor`, `snapshot`, `diff <BEFORE> <AFTER>`
  and `schema --kind report`, with the shared options `--client`, `--cwd`, `--client-version`,
  `--format text|json`, `--strict`, `--no-env`, `--input-context`, `--color`, `--paths local|share`,
  `--allow-read-root` and `--project-root`; `--output`/`--overwrite` for `snapshot`;
  `--fail-on warning|error` for `doctor`.
- **Exit codes** — `0`/`1`/`2`/`3`/`4` with the documented priority `2 → 4 → 3 → 1 → 0`, and a
  single valid JSON envelope on stdout including structured fatal errors.
- **Clients** — a bounded OpenCode adapter (tested against documented behaviour for 1.18.35) and a
  bounded Claude Code adapter (2.1.296), both at most `fixture_verified`. Unsupported clients
  (Codex, Gemini, Aider, Cursor, Copilot CLI, Cline, Roo, Hermes) are listed with reasons and make
  no compatibility claim.
- **Resolution** — a provenance engine with typed events (selected / replaced / contributed /
  ignored / constrained / unresolved), per-key support levels and coverage gaps; a policy phase
  distinct from the merge; and `null` / empty-list / missing modelled distinctly.
- **Parser** — a hand-written, span-preserving JSON/JSONC scanner with duplicate-key detection and
  byte spans back to the original source.
- **Redaction** — a redaction projection as the only producer of reportable values; raw values in a
  type with no `Serialize`/`Display`; URL userinfo/query/fragment stripping; terminal-control and
  bidi sanitization; and a `--paths share` mode that pseudonymizes paths and opaque-ifies unknown key
  names. Enforced by `tests/security_leaks.rs`.
- **Filesystem safety** — bounded regular-file reads under explicit roots, `--allow-read-root`, symlink
  loop and escape detection, rejection of devices/sockets/FIFOs/directories, and explicit resource
  limits (2 MiB/file, 16 MiB total, 128 sources, depth 64, 2000 events/key).
- **Reports** — a bundled report JSON Schema 1.0, an atomic create-new snapshot writer with
  owner-only (`0600`) permissions on Unix, and a `diff` that reports protected values as
  `unknown_redacted`.
- **Evidence** — per-adapter compatibility registries under `compatibility/` with source URLs,
  retrieval dates, pinned-commit status and rule → test IDs.
- **Demo** — an isolated synthetic end-to-end demo, `demo/demo.sh`, that reads no real configuration
  and needs no network.
- **Licensing and docs** — dual `MIT OR Apache-2.0` license texts (`LICENSE-MIT`, `LICENSE-APACHE`),
  this changelog, `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`,
  `docs/getting-started.md`, `docs/maintaining.md` and a title graphic.
- **Share-mode privacy** — `--paths share` pseudonymizes URL hosts as well as paths, user info, query
  strings and fragments (`https://host_1/mcp?[redacted]`), in every output channel.
- **Tests** — 296 passing (191 library unit tests plus 105 integration tests) per the recorded run.

### Known limitations

- **No upstream conformance run.** No pinned loader harness exists, so every advertised rule is at
  most `fixture_verified`; none claims `upstream_conformance_verified`.
- **Windows ACLs on snapshot output are not implemented.** The report states this rather than
  pretending.
- **MSRV verified in CI.** `rust-version = "1.85"` is declared and the MSRV job builds the crate with
  1.85.0; local development used 1.99.0.
- **No benchmarks and no published artifacts.** `fixtures/`, `examples/` and `benches/` are empty
  directories; fixtures are inline in the test files.
