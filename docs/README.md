# ConfigLens documentation

Two audiences live in this directory. Start with the first list.

## For users

- [`getting-started.md`](getting-started.md) — build or install, first run, how to read the output.
- [`../README.md`](../README.md) — installation, commands, exit codes, compatibility and privacy.

## For contributors and maintainers

- [`decisions.md`](decisions.md) — engineering tradeoffs and rejected alternatives.
- [`implementation-status.md`](implementation-status.md) — what is implemented, verified and **not**.
- [`verification-log.md`](verification-log.md) — the commands that were actually run, with results.
- [`audit-2026-10-12.md`](audit-2026-10-12.md) — reproduced defects, fixes, privacy scope and remaining limits.
- [`maintaining.md`](maintaining.md) — verification, adding adapter rules, release checklist.
- [`demo/README.md`](demo/README.md) — the recorded terminal demo.
- [`spec/ConfigLens_Technical_Specification.md`](spec/ConfigLens_Technical_Specification.md) — the
  design the project is built to.

## Evidence

- [`research/`](research/) — compatibility audits, the fuzz-recovery record, the project-discovery
  finding and the Codex adapter contract. These are working documents, not marketing.
- [`../compatibility/`](../compatibility/) — per-client compatibility registries with source URLs,
  retrieval dates, pinned-commit status and rule→test IDs.

## Release

- [`../.github/workflows/`](../.github/workflows/) — continuous integration and the beta release
  workflow.
