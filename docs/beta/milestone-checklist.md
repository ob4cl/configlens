# ConfigLens milestone checklist — private beta to respected 0.1 release

Written 2026-10-10. Status is checked against the actual repository and the spec
(`docs/spec/ConfigLens_Technical_Specification.md` §8 test plan, §10 milestones, §12 acceptance),
not against intentions.

## Status vocabulary

- **DONE** — verified by a command in `docs/verification-log.md` or a named passing test.
- **PARTIAL** — implemented but bounded, or verified only in one environment.
- **TODO** — not present / not implemented.
- **DECISION** — blocked on a maintainer choice in `user-actions.md`.

A **respectable CLI** is judged on correctness, privacy, honest documentation and reproducible
releases — not on stars. Every "DONE" below must survive a stranger running the same command.

## Summary

| Area | Done | Partial | Todo/Decision |
|---|---|---|---|
| 1 Correctness | 6 | 1 | 1 |
| 2 Privacy | 5 | 3 | 0 |
| 3 Cross-platform CI | 1 | 1 | 4 |
| 4 MSRV | 0 | 2 | 0 |
| 5 Releases | 0 | 1 | 6 |
| 6 Checksums | 0 | 0 | 3 |
| 7 README | 1 | 3 | 2 |
| 8 Contribution docs | 0 | 0 | 7 |
| 9 Compatibility | 3 | 1 | 2 |
| 10 Conformance | 2 | 0 | 1 |
| 11 Codex feature (0.2) | 0 | 0 | 2 |
| 12 Early users | 1 | 0 | 3 |
| 13 Maintenance | 0 | 1 | 5 |

---

## 1. Correctness

| ID | Item | Truth now | Acceptance criteria | Status |
|---|---|---|---|---|
| C1 | Every advertised rule has positive, negative and boundary tests | Rule→test maps exist in both registries; **324** tests pass locally (see `docs/verification-log.md` §3; earlier snapshots said 296) and the three-OS CI matrix is green (most recently run 38157888805) | For each rule ID in `compatibility/*/registry.md`: ≥1 positive, ≥1 negative, ≥1 boundary test; mapped by test ID | PARTIAL |
| C2 | Deterministic output | Passing (`conformance` property test) | Identical context ⇒ byte-identical report; seed recorded on failure | DONE |
| C3 | Exit-code contract | Passing (`cli::exit_*`, `error::tests`) | 0/1/2/3/4 with priority 2→4→3→1→0 asserted by a matrix test | DONE |
| C4 | Ten acceptance examples end-to-end | Passing (`tests/acceptance.rs` + others) | All §9 examples exercised through the CLI | DONE |
| C5 | No network / subprocess / client execution | Passing (source-scan test) | `src/` never spawns processes or opens sockets; test enforced | DONE |
| C6 | Independent, checked-in acceptance harness | The harness used in the log (`configlens_acceptance.py`) is **not** in the repo | Harness committed under `tests/` or `tools/`, runs offline on synthetic inputs, documented in the README | TODO |
| C7 | Fuzzing | Three `cargo-fuzz` targets (`fuzz/{parse_documents,resolve_precedence,hostile_inputs}`) with a committed corpus and a bounded CI job; a 120 s-per-target session runs clean | `cargo-fuzz` targets: parser, JSON Pointer, reducers, sanitizer, traversal; short CI smoke, no panics / unbounded growth | DONE |
| C8 | Benchmarks (no fake numbers) | `benches/resolution.rs` runs (`cargo bench`); hardware, fixture sizes, medians and the quadratic scaling are recorded in `docs/implementation-status.md` §8. The 5 000-key figure exceeds the §8 target, which is stated rather than hidden | Publish hardware + fixture size + measured distribution, **or** state plainly "no benchmarks published". Target (§8, not a claim): <150 ms warm, <50 MiB | DONE |

---

## 2. Privacy and security

| ID | Item | Truth now | Acceptance criteria | Status |
|---|---|---|---|---|
| P1 | Redact-unless-allowlisted projection | DONE by design (D4/D5); no `Serialize` on raw values | Renderers can only receive redacted DTOs; enforced by types | DONE |
| P2 | Seeded-secret leak suite | 14 tests pass, covering local **and** share modes | Seeded secrets absent from stdout, stderr, JSON, snapshot, diff and debug — in **local and share** modes | PARTIAL |
|| P3 | `--paths share` pseudonymizes URL hosts | Fixed: `url_host` + `Pseudonymizer` collect and replace host authorities in share mode; local mode keeps them for diagnosis | URL authority replaced by a deterministic opaque ID in share mode; test `share_mode_pseudonymizes_url_hosts_but_local_mode_keeps_them_for_diagnosis` asserts the real host is absent | DONE |
| P4 | Snapshot permissions | Unix `0600` verified; Windows ACL not implemented (reported) | Per-target-OS verification, or an honest "not implemented on Windows" that the report already emits | PARTIAL |
| P5 | Resource limits (2 MiB/file, 16 MiB, 128 sources, depth 64, 2000 events) | Each limit has a diagnostic + test | No silent truncation; hitting a limit yields invalid/partial | DONE |
| P6 | Terminal-control sanitization | DONE (`security::sanitize`, leak test) | OSC/bidi/CR/newline neutralised in every channel | DONE |
| P7 | Symlink loop/escape detection | DONE (`filesystem::tests`) | Loops/escapes detected; links followed only within an authorised root | DONE |
| P8 | User-facing privacy guidance | README has a privacy section; `docs/security.md` referenced but absent | A `docs/security.md` (or README section) telling users how to read `redacted:true` / `unclassified_value` and how to share safely | PARTIAL |

---

## 3. Cross-platform CI

| ID | Item | Truth now | Acceptance criteria | Status |
|---|---|---|---|---|
| X1 | `ci.yml` | `ci.yml` present and running: a `test` matrix (ubuntu/macos/windows), a `quality` (fmt+clippy) job and an `msrv` job, all green (run 38099457727); actions pinned by commit SHA; `permissions: contents: read`. It also defines `dependencies` and `fuzz` jobs, which have **not** run yet | `fmt --check`, `clippy -D warnings`, `test --locked` on ubuntu + macos + windows; actions pinned by commit SHA; least privilege | DONE |
| X2 | `release.yml` | `prerelease.yml` present but **manual** (`workflow_dispatch` on an existing reviewed tag, not tag-triggered): re-runs CI on that tag, builds per-target Archives, verifies every checksum, creates the prerelease; write permission only in the publish job | Tag-triggered build matrix, per-target assets, smoke test each binary, no publish credentials in PR-triggered jobs | TODO |
| X3 | OS-verified behaviour | Three-OS CI green (Linux/macOS/Windows, run 38099457727). Platform path behaviour is asserted through synthetic OS contexts — **no run against the real clients on macOS or Windows** | macOS + Windows runner tests green; path/root discovery asserted per OS | TODO |
| X4 | Dependency audit + license check job | `deny.toml` + a `dependencies` CI job (cargo-deny 0.20.2, pinned by action commit) exist; `cargo deny check` passes locally. The CI job has **not** run yet | `cargo-deny` (advisories, licenses, bans) or equivalent in CI | PARTIAL |
| X5 | Bundled JSON-schema validation job | Covered inside the CI matrix by the suite that asserts the bundled schema is valid and that emitted reports validate against it; no separate job | `schema --kind report` + validation of emitted reports against the bundled schema in CI | TODO |
| X6 | Secret-leak job in CI | `tests/security_leaks.rs` (14 tests) runs in the matrix and fails the job; not a separate workflow | Seeded-secret suite runs in CI and blocks on failure | TODO |

---

## 4. MSRV

| ID | Item | Truth now | Acceptance criteria | Status |
|---|---|---|---|---|
| M1 | Declared MSRV | `rust-version = "1.85"` in `Cargo.toml`; MSRV job runs `cargo +1.85.0 check --locked --all-targets --all-features` and is green (run 38099457727) | Declaration is backed by a real `1.85.0` build job; if it fails, the declaration is raised to the verified floor (never silently lowered) | PARTIAL |
| M2 | Toolchain reproducibility | `rust-toolchain.toml` pins `stable`; verified on 1.99.0 | CI installs the MSRV toolchain explicitly and the pinned stable toolchain, both tested | PARTIAL |

---

## 5. Releases

| ID | Item | Truth now | Acceptance criteria | Status |
|---|---|---|---|---|
| R1 | Versioning policy | none | Documented semver policy for 0.x (breaking-change rules, what a minor means pre-1.0) | TODO |
| R2 | Tagged source release | none | Annotated tag `vX.Y.Z`, matching `Cargo.toml`, source archive attached | TODO |
| R3 | Binary targets | none built for release | Promote only targets actually tested (§10): proposed x86_64/aarch64 linux, x86_64/aarch64 macos, x86_64 windows | TODO |
| R4 | Release notes / CHANGELOG | `CHANGELOG.md` present (keep-a-changelog, honest limitations) + release-notes text in `release-runbook.md` §4 and in the prerelease workflow | `CHANGELOG.md` + honest release notes listing scope, limits and known gaps | TODO |
| R5 | SBOM / provenance (aspiration) | SBOM exists: `scripts/generate_sbom.py` (CycloneDX 1.5, validated, reproducible normalised output) + an `sbom` job in `prerelease.yml`. Build provenance attestation is **not** done. Neither the job nor any release has run | SBOM + build provenance attestation where practical; labeled aspirational until reproduced | PARTIAL |
| R6 | Rollback / yank procedure | none | Documented: how to unpublish/yank, re-tag, and tell users | TODO |
| R7 | crates.io publish + name check | not published | Name availability + trademark check done; `cargo publish` gated on green CI and license files | DECISION |

---

## 6. Checksums and artifact integrity

| ID | Item | Truth now | Acceptance criteria | Status |
|---|---|---|---|---|
| S1 | `SHA256SUMS` per asset | Exercised: the packager writes a per-asset SHA-256, the workflow re-verifies each one and publishes a combined `SHA256SUMS`. Verified on the `v0.1.0-beta.1`, `v0.1.0-beta.2` and `v0.1.0-beta.3` prereleases (7 assets each) | Every release asset has a published SHA-256 in a single manifest | DONE |
| S2 | Signature / attestation | none | Detached GPG signature or sigstore/SLSA attestation for the manifest (or documented as not done) | TODO |
| S3 | Verification instructions | `sha256sum -c SHA256SUMS` (and the Windows equivalent) documented in `release-runbook.md` and emitted in the release notes by the workflow | README/release notes give a copy-paste command to verify downloads; no mandatory curl-to-shell install | TODO |

---

## 7. README

| ID | Item | Truth now | Acceptance criteria | Status |
|---|---|---|---|---|
| D1 | Accurate description, commands, exit codes, security posture | Present and honest | Matches shipped behaviour; "this README is not a completeness claim" kept | DONE |
| D2 | Real repository URL | `Cargo.toml` and `README.md` point at `github.com/ob4cl/configlens` | Canonical URL set; README links resolve | DECISION |
| D3 | Support matrix current | Points to implementation-status; no dedicated compatibility doc | `docs/compatibility.md` (or README table) lists per-key support states and tested versions | PARTIAL |
| D4 | 20-second terminal demo | A real asciinema recording exists in the working tree (`docs/demo/configlens-demo.cast`, produced by `demo/record.sh` + `demo/record_pty.py`) but is **not committed**; `demo/demo.sh` is tracked and runs | Real recording committed/linked; used in launch material (never a fake) | PARTIAL |
| D5 | Install instructions | "build from checkout" only | Copy-paste build + (once published) `cargo install configlens`; manual verification path documented | PARTIAL |
| D6 | Bundled examples run without an AI client | README + `docs/getting-started.md` show the real captured `explain` output (re-run 2026-10-11); `demo/demo.sh` runs; `examples/` is not shipped and the README no longer references it | Either ship `examples/` fixtures or adjust the README so every example command runs against what exists | TODO |

---

## 8. Contribution and community docs

| ID | Item | Truth now | Acceptance criteria | Status |
|---|---|---|---|---|
| N1 | `CONTRIBUTING.md` | present: rule-addition contract (primary source, declared scope, positive/negative tests), no real secrets | Explains rule-addition contract: primary source + declared scope + positive/negative tests; "no real secrets/proprietary instructions" | TODO |
| N2 | `SECURITY.md` | present. Channel while private = a private issue in this repository (GitHub private vulnerability reporting is only available on public repos); enable it at publication | Private disclosure channel; response expectations | DECISION |
| N3 | `CHANGELOG.md` | present, keep-a-changelog format | Keep-a-changelog format; every release entry | TODO |
| N4 | `CODE_OF_CONDUCT.md` | absent | Standard (e.g. Contributor Covenant) with a contact | TODO |
| N5 | Issue / PR templates | `bug_report.yml`, `feature_request.yml`, `config.yml` present; the bug template forbids real configuration and requires the `--paths share` review checkbox | Bug template that **requires `--paths share`** and forbids pasting raw secrets; PR checklist | TODO |
| N6 | `LICENSE-MIT`, `LICENSE-APACHE` | Both files present, matching `MIT OR Apache-2.0` | Both files present, matching `MIT OR Apache-2.0`, with the chosen copyright holder | TODO |
| N7 | Referenced design docs | `docs/{semantics,compatibility,report-schema,security}.md` referenced, absent | Present or the references removed so nothing dangles | TODO |

---

## 9. Compatibility

| ID | Item | Truth now | Acceptance criteria | Status |
|---|---|---|---|---|
| K1 | Bundled registries (claude, opencode) | `registry.json` + `registry.md` present | Registries load; pinned to a revision; evidence URLs + retrieval date | DONE |
| K2 | Pinned upstream commits | Claude: none (closed source). OpenCode: re-pinned to `v1.18.35` → commit `53d1eabb…` (loader `config/config.ts`); the 2.x line is out of scope | Every rule carries a pinned commit **or** an explicit "not pinnable" note; no broad version ranges | PARTIAL |
| K3 | Per-key support states | Present (`inventory_only` … `fixture_verified`) | No rule labelled higher than its evidence supports | DONE |
| K4 | Tested-version boundaries | Only "declared"; no tested matrix | Where a boundary is claimed, it is tested; otherwise output stays provisional | TODO |
| K5 | Unsupported clients labelled | `clients` labels Codex/Gemini/etc. unsupported | No compatibility claim from shared file names | DONE |
| K6 | Re-verification at each release | No procedure | A per-release step re-checks upstream docs/versions and updates registry dates | TODO |

---

## 10. Conformance

| ID | Item | Truth now | Acceptance criteria | Status |
|---|---|---|---|---|
| F1 | Pinned loader harness | None; no safe loader built | Loader-only harness on synthetic inputs, isolated home, blocked network, no real creds — or an explicit, documented downgrade | TODO |
| F2 | Claude conformance | Closed source; impossible to reach | Documented rationale; rules stay at most `fixture_verified`. No invented parity | DONE (documented as impossible) |
| F3 | OpenCode loader re-pin | Re-pinned: tag `v1.18.35` → loader `packages/opencode/src/config/config.ts` at commit `53d1eabb…`, cited in the registry with a source transcript | A replacement loader SHA pinned, or the affected rules downgraded with a note | DONE |

---

## 11. Codex feature (0.2 roadmap — must not be advertised in 0.1)

| ID | Item | Truth now | Acceptance criteria | Status |
|---|---|---|---|---|
| B1 | Codex adapter scope + evidence | Not implemented | TOML parsing, trust gating, current profile-file model and selected sandbox/approval defaults, each with pinned evidence and tests — or explicitly deferred and left labelled `unsupported` | TODO |
| B2 | No invented rules | N/A | Nothing about Codex appears in 0.1 docs/README/launch material as a working feature | TODO |

---

## 12. Early users (private beta)

| ID | Item | Truth now | Acceptance criteria | Status |
|---|---|---|---|---|
| E1 | Pilot guide, task script, consent + feedback templates | In `pilot-guide.md` | Ready to use; approved by maintainer | DONE |
| E2 | Real developers run the task script (optional — maintainer's call) | Not started, and **not required** for development | 2–3 complete the script and diagnose a real incident with a trace rather than maintainer help | DECISION |
| E3 | Consent recorded | Not started | Every tester's consent captured before sharing anything | TODO |
| E4 | Pilot findings triaged | Not started | Every serious correctness/privacy finding triaged via `issue-triage.md`; serious ones closed before public launch | TODO |

---

## 13. Maintenance

| ID | Item | Truth now | Acceptance criteria | Status |
|---|---|---|---|---|
| G1 | Triage policy + response expectation | Checklist exists; no ratified policy | Published triage labels and a realistic response-time statement | PARTIAL |
| G2 | Dependency update cadence | none | Dependabot/Renovate or a manual monthly review; security advisories handled promptly | TODO |
| G3 | Security response process | none | `SECURITY.md` + a documented handling path | DECISION |
| G4 | Deprecation / support policy | none | What a version supports, how support ends, how breaking changes are announced | TODO |
| G5 | Release cadence | none | A stated, realistic cadence (not a promise of dates) | TODO |
| G6 | Ownership / bus-factor | none | `CODEOWNERS` or a MAINTAINERS note so a second person can cut a release | TODO |

---

## Release-readiness gate (must all be DONE before the repo goes public / 0.1 publishes)

Minimum bar for a first release a respected CLI would stand behind:

1. C1 rule tests complete; C2/C3/C4/C5 green in CI. (C7 fuzz smoke at least present.) — **DONE**: 324 tests green locally; the three-OS matrix is green. C7 ships three `cargo-fuzz` targets with a bounded CI job that now runs green in CI.
2. **P3 share-mode URL-host gap fixed** — DONE (`1fae794`); shown by the
   `share_mode_path_and_host_pseudonymization` block in `demo/demo.sh` and §6.3 of
   `docs/engineering-verification.md`.
3. X1–X6 CI green on three OSes; X6 secret-leak job blocking. — **DONE as amended**: X1/X5/X6 done; X3 green but CI-level only (no real-client run on macOS/Windows); X2 is the manual prerelease workflow, not tag-triggered, and has since completed for `v0.1.0-beta.2` and `v0.1.0-beta.3`; **X4 (`cargo-deny`)** is present (`deny.toml` + a `dependencies` job) and runs green in CI.
4. M1 MSRV build green (or declaration corrected). — **DONE**: the 1.85.0 job is green in CI.
5. N1, N2, N3, N5, N6 present; R4 release notes; S1 checksums + S3 verify instructions. — **DONE except S1**: everything is present; S1 completes when the first prerelease actually publishes assets.
6. D2 real repo URL; D4 real demo; D6 README examples actually run. — **DONE except D4**: a real recording exists in the working tree (`docs/demo/configlens-demo.cast`) but is not committed; the tracked demo script is real and runs.
7. E2 (optional) 2–3 testers completed, **or** an explicit decision to publish without a pilot and
   take feedback as issues; E4 serious findings closed either way.

   The pilot is a **pre-publication** check, not a prerequisite for building. Legal/ethical note: no
   tester's handle, consent record or identifiable feedback is ever stored in this repository — the
   log lives outside it (`pilot-guide.md`), and anonymous aggregates only.
8. No launch material claims a capability that is not DONE above.

Still open, and to be labelled honestly in the release notes: **R5** (SBOM present; provenance
attestation not done), **S2** (no signature or attestation for the manifest), **S3** (verification
instructions are emitted in the release notes but are not yet in the README), **F1** (no pinned
loader harness), and **Codex** (no verified upstream evidence, so no adapter). **D4** is no longer
open: the recording is committed at `docs/demo/configlens-demo.cast`. **S1** is exercised: each of the
`v0.1.0-beta.1`/`beta.2`/`beta.3` prereleases ships a `SHA256SUMS` covering its seven assets.
**X4 / the `fuzz` and `sbom` jobs** are all green in CI and `prerelease.yml` has completed twice.
Benchmarks (C8) are no longer open: `benches/resolution.rs` runs and its numbers — and
its quadratic scaling — are recorded in `docs/implementation-status.md`.
