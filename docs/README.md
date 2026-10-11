# ConfigLens documentation

Two audiences live in this directory. Start with the first list.

## For users

- [`getting-started.md`](getting-started.md) — build or install, first run, how to read the output.
- [`../README.md`](../README.md) — installation, commands, exit codes, compatibility and privacy.

## For contributors and maintainers

- [`decisions.md`](decisions.md) — engineering tradeoffs and rejected alternatives.
- [`implementation-status.md`](implementation-status.md) — what is implemented, verified and **not**.
- [`verification-log.md`](verification-log.md) — the commands that were actually run, with results.
- [`maintaining.md`](maintaining.md) — verification, adding adapter rules, release checklist.
- [`demo/README.md`](demo/README.md) — the recorded terminal demo.
- [`spec/ConfigLens_Technical_Specification.md`](spec/ConfigLens_Technical_Specification.md) — the
  design the project is built to.

## Evidence

- [`research/`](research/) — compatibility audits, the fuzz-recovery record, the project-discovery
  finding, and independent launch audits. These are working documents, not marketing.
- [`../compatibility/`](../compatibility/) — per-client compatibility registries with source URLs,
  retrieval dates, pinned-commit status and rule→test IDs.

## Maintainer and planning notes

- [`beta/`](beta/) — private-beta planning: pilot guide, tester instructions, launch drafts, success
  metrics, release runbook and the milestone checklist. Historical context for how the beta was run.
- [`launch/`](launch/) — the public-launch audit and the publication checklist.
- [`../.github/AUTOMATION.md`](../.github/AUTOMATION.md) — CI, release mechanics and action pinning.
