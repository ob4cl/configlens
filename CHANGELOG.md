# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Until `0.1.0` is published, entries describe the in-repo prototype. Nothing here is a claim about a
published crate, a downloadable binary or verified upstream conformance — for the honest, item-by-item
account of what is implemented and verified, see
[`docs/implementation-status.md`](docs/implementation-status.md) and
[`docs/verification-log.md`](docs/verification-log.md).

## Unreleased — public-launch preparation (documentation and workflow only)

No code behaviour changes. These prepare the repository for a public launch; the repository is still
private and no release has been cut from this state.

### Changed

- **Release workflow works when the repository is public.** `prerelease.yml` no longer requires a
  private repository, so publishing the project cannot silently disable releases. Every provenance
  safeguard is retained: tag/parentage shape, tag-version equals package-version, tag→commit
  resolution, ancestor-of-dispatch, CI re-run on that SHA, `EXPECTED_SHA` equality at publish,
  per-archive checksum verification, the 3-archive/3-checksum/3-SBOM assertion and the 7-asset
  postcondition. Renamed to **Beta prerelease**.
- **README is now a public front door**: a prominent install section with a platform table and
  checksum verification for Linux, macOS and PowerShell, a first-command example, a glossary for
  `observed winner` / `fixture_verified` / `coverage gap`, and a Status section.
- **SECURITY.md, CONTRIBUTING.md and CODE_OF_CONDUCT.md** reworded for a public audience; the
  security policy now names GitHub private vulnerability reporting as the preferred channel.
- **Specification trimmed for publication.** The commercial roadmap and the coding-agent
  implementation brief were removed from `docs/spec/ConfigLens_Technical_Specification.md`; the
  document is renumbered §1–§11 and the technical design is unchanged.

### Removed from the public tree

- `docs/beta/*` and `.github/AUTOMATION.md` (internal planning and maintainer release mechanics).
  Preserved privately outside the repository, not deleted.

### Added

- `docs/README.md` documentation index, `docs/launch/publication-checklist.md` and
  `.github/dependabot.yml`.

## 0.1.0-beta.3 — pre-release, published 2026-10-11

Released as the private `v0.1.0-beta.3` pre-release (commit `e746957`, tag `v0.1.0-beta.3`).
Version identification changed and one CLI flag was added.

### Added

- **`--quiet` global flag.** Suppresses **informational** stderr messages only — the snapshot
  `wrote N bytes` line, and the support-report write and `--preview` notes. Errors are always printed
  to stderr, stdout stays machine-readable (JSON), and exit codes are unchanged. It is accepted on
  every subcommand (`configlens <cmd> --quiet`).

### Changed

- **Unified prerelease version identification.** The crate version is now `0.1.0-beta.3`
  (`Cargo.toml`), and `configlens --version`, the report `tool_version` field, the SBOM root
  component and the `v0.1.0-beta.3` git tag all agree on it. The prerelease workflow now **rejects a
  tag whose version does not match the package version**, so a mis-tagged release fails instead of
  publishing inconsistent metadata. `schema_version` (the report schema) stays `1.0` and the adapter
  `adapter_revision` stays `0.1.0` — both independent of the crate version.
- **Windows snapshot-ACL disclosure clarified (documentation only; runtime behaviour unchanged).**
  Windows has no `0600` equivalent, and the snapshot is written as a same-directory temporary file
  and then renamed, so it **inherits the ACL of the destination folder**; `redaction.snapshot_acl`
  remains `not_implemented`. The documents now state the inheritance consequence and recommend saving
  snapshots in a private, user-controlled directory. A restrictive DACL remains out of scope for 0.1
  (no `unsafe`, no new dependency).

## 0.1.0-beta.2 — private beta, released 2026-10-11

Published as the private `v0.1.0-beta.2` prerelease (commit `51a2ca7`). Behaviour changed for one
input class and new checks were added:

### Added

- **Dependency policy** — `deny.toml` plus a CI job (`dependencies`) that fails on a RustSec
  advisory, a yanked release, a licence outside the four present in the graph, a wildcard or git
  dependency, or a registry other than crates.io.
- **Fuzz targets** — `fuzz/parse_documents`, `fuzz/resolve_precedence` and `fuzz/hostile_inputs`
  with a committed, coverage-minimised seed corpus, a short CI smoke job (`fuzz`) that fails on a
  crash or a violated invariant, and a separate longer scheduled pass (`fuzz-scheduled.yml`) that
  runs weekly (and on dispatch) per target under bounded wall-clock and memory limits.
- **SBOM** — `scripts/generate_sbom.py` produces a validated, reproducible CycloneDX 1.5 document
  per release target; the `sbom` job in the prerelease workflow attaches all three to the release.
- **Recorded demo** — `demo/record.sh` (plus `demo/record_pty.py`) produces
  `docs/demo/configlens-demo.cast`, a real asciinema recording of precedence resolution across three
  layers.

### Fixed

- **Security: terminal control characters in report pointers.** A configuration file could put an
  ESC sequence (or a bidi/zero-width character) in an object key name, and the local JSON report
  printed it verbatim as part of the resolution `pointer`, so the report could repaint the terminal
  of whoever read it. Pointers are now sanitized at every rendering boundary
  (`security::display_pointer`), with a regression test covering scan, explain, doctor, snapshot and
  diff in both path modes.
- **OpenCode `$OPENCODE_CONFIG_DIR` was read as a file.** Upstream treats the value as a *directory*
  and loads `opencode.json`/`opencode.jsonc` from inside it (pinned loader: `config/paths.ts`,
  `config/config.ts`). The adapter previously refused a directory value with `not_a_regular_file`
  and accepted a file value as a source — the opposite of upstream in both cases. The layer now
  reads the two files inside the directory, `.jsonc` overriding `.json`, and its placement
  (rank 45, above the project layer and below `$OPENCODE_CONFIG_CONTENT`) is source-verified, so the
  former `custom_dir_placement_unverified` coverage gap is gone. Found through real Windows beta
  testing; evidence in `docs/research/evidence/opencode-config-dir-20261011.txt`.
- **OpenCode project config read only one file — a wrong winner.** Project discovery used
  `--project-root` (or the cwd) as the *search start* and read only the first directory containing an
  `opencode.json`/`opencode.jsonc`, so a working directory nested below the repository root could be
  resolved from the wrong (ancestor) file and every intermediate ancestor was dropped — a wrong
  `observed_winner` with no coverage gap for the unread file. Upstream (`config/paths.ts::files` →
  `packages/core/src/fs-util.ts::up`, pinned `53d1eabb61e21162157817bf677da0a4ad3332e3`) collects a
  match at *every* level from the working directory up to the worktree and merges them outermost-first,
  so the nearest file wins and every ancestor contributes. Discovery now walks `ctx.cwd` up to the
  boundary (`--project-root`, else the nearest Git directory), inclusive, and emits each level's
  `opencode.json` then `opencode.jsonc` as `oc.project` candidates outermost-first, so the engine's
  last-definition-wins tie-break selects the nearest file; with no boundary only the working directory
  is read and the boundary-uncertain signal is kept. Evidence `OC-PROJECT-SOURCE-20261011`; six
  regression tests in `tests/rules_opencode.rs`; independent-audit finding A3 closed.

### Verified (no code change)

- The declared MSRV (`rust-version = "1.85"`) is now genuinely verified:
  `cargo +1.85.0 check --locked --all-targets --all-features` exits `0`.

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
- **Tests** — 315 passing (194 library unit tests plus 121 integration tests) per the current
  recorded run of `cargo test --locked` (see `docs/verification-log.md` §3). Older totals quoted in
  earlier drafts (270, 286, 296) were intermediate snapshots and are superseded.

### Known limitations

- **No upstream conformance run.** No pinned loader harness exists, so every advertised rule is at
  most `fixture_verified`; none claims `upstream_conformance_verified`.
- **Windows ACLs on snapshot output are not implemented.** The report states this rather than
  pretending.
- **MSRV verified in CI.** `rust-version = "1.85"` is declared and the MSRV job builds the crate with
  1.85.0; local development used 1.99.0.
- **Release workflow not yet exercised.** `prerelease.yml` is manual (`workflow_dispatch` on an
  existing reviewed tag) and has **never run in CI**; no release has been published, so its
  seven-asset layout (3 archives + 3 SBOMs + `SHA256SUMS`) is source-level only.
- **No published artifacts.** `fixtures/` and `examples/` are empty directories; fixtures are inline
  in the test files. Benchmarks **are** present: `benches/resolution.rs` runs under `cargo bench` and
  its recorded medians and quadratic scaling are in `docs/implementation-status.md`.
