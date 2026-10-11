# ConfigLens private-beta and launch preparation

Working documents for taking ConfigLens 0.1 from a private prototype toward a first
honest public release. Written 2026-10-10. Owner: the maintainer (user).

These files are **plans, checklists and drafts**. None of them is a claim that the work is
done. Every status is checked against the actual repository, the technical specification
(`docs/spec/ConfigLens_Technical_Specification.md`), `docs/implementation-status.md`,
`docs/verification-log.md`, `docs/decisions.md` and `README.md`.

## Governing constraints (read first)

- **The repository stays private** until the release-readiness gate in
  `milestone-checklist.md` is met and the maintainer explicitly approves public visibility.
- **No recruitment, outreach, invitations, posting, publishing, paid services, bots or
  manufactured social proof** is performed by these documents. They are drafts and checklists.
  The maintainer decides and does any real-world step, after their own approval.
- **`docs/beta/` is the only directory these documents own.** Code, CI workflows, license
  files, `Cargo.toml`, repository settings and git history are out of scope here; anything
  that needs to change there is listed as a decision/action in `user-actions.md`.
- **No fake signals.** No star forecasts, no invented benchmark numbers, no "verified"
  labels that were not produced by a command recorded in `docs/verification-log.md`.

## Files

| File | Purpose |
|---|---|
| `README.md` | This index and the governing rules. |
| `milestone-checklist.md` | Item-by-item tracker (correctness, privacy, CI, MSRV, releases, checksums, README, contribution docs, compatibility, conformance, Codex, early users, maintenance) with acceptance criteria and truthful status. |
| `pilot-guide.md` | Guide to running a 5–10 developer private beta: task script, consent template, redacted feedback template. |
| `issue-triage.md` | Checklist for triaging beta issues without leaking secrets. |
| `release-runbook.md` | Maintainer runbook for cutting and rolling back a release. |
| `success-metrics.md` | Modest, honest success metrics (task success, return usage, resolved bugs). |
| `launch-drafts.md` | Draft posts for relevant communities. **Drafts only — nothing is posted.** |
| `user-actions.md` | Terse list of the exact decisions and actions only the maintainer can make. |

## Truth baseline at 2026-10-10 (what actually exists)

- One Cargo package: library + single binary `configlens` 0.1.0, edition 2021.
- Local verification (WSL2/Linux, `cargo`/`rustc` 1.99.0): `cargo fmt --check` clean,
  `cargo clippy --locked --all-targets -- -D warnings` clean, `cargo test --locked`
  **270 passed / 0 failed**, `cargo build --locked --release` OK, `demo/demo.sh` exits 0.
- Only two adapters: `claude` and `opencode`, both bounded (`fixture_verified` /
  `documented` / `inventory_only`). Codex is **not implemented** (0.2 roadmap).
- **Absent**: CI workflows (`.github/workflows/` empty), MSRV build, release assets,
  checksums, `LICENSE-*`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`,
  `CODE_OF_CONDUCT.md`, issue/PR templates, benchmarks (`benches/` empty), a fuzz
  campaign, checked-in fixtures (`fixtures/` empty), runnable examples (`examples/` empty),
  `docs/{semantics,compatibility,report-schema,security}.md`, and the independent acceptance
  harness (`configlens_acceptance.py`) referenced by the verification log but not committed.
- Known honest gaps (from `docs/implementation-status.md` §10): no upstream conformance run;
  OpenCode loader commit not re-pinned; Windows snapshot ACLs not implemented; MSRV 1.85 declared but never built.
- Git: one local commit (`b2684c7`); remote `github.com/ob4cl/configlens.git`. Visibility and
  remote parity were not verified from this checkout.
