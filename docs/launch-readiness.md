# ConfigLens — public-launch readiness plan

Prepared 2026-10-11 against `main` @ `e746957` (`v0.1.0-beta.3`, private). This is the owner-facing
plan that sits on top of the three evidence reports in this directory:

- `launch-audit-readme-dx.md` — README front door and developer experience
- `launch-audit-technical.md` — technical credibility, reviewed as a prospective contributor
- `launch-audit-hygiene.md` — public hygiene and sensitive-information classification

**CONFIRMED** = I re-verified it against the repository during this audit. **RECOMMENDATION** = judgement.
Nothing here has been applied except where the "Applied in this branch" section says so.

> **Owner decisions recorded:** history is preserved (no rewrite, no force-push, no tag moves, no
> deleted prereleases); the licence stays **MIT OR Apache-2.0** with the `ob4cl` attribution; the
> initial public distribution channel is **GitHub Releases** (not crates.io/Homebrew/Scoop/winget);
> and the audit PR was merged by fast-forward. The actionable follow-through now lives in
> [`launch/publication-checklist.md`](launch/publication-checklist.md), which holds the exact GitHub
> settings, the public/internal documentation separation proposal and the publication sequence.

## 0. Where the project actually stands

CONFIRMED: the engineering is finished and CI-verified far beyond the usual bar for a 0.1 beta.

- 324 tests pass (194 lib + 130 integration across 9 targets); `fmt`, `clippy --all-features -D
  warnings`, MSRV 1.85, cargo-deny and `actionlint` all clean; three fuzz targets clean.
- All five `ci.yml` jobs green on `main` (run 38157888805); `prerelease.yml` has completed end to end
  twice, producing the seven-asset layout (3 archives + 3 SBOMs + `SHA256SUMS`).
- Every Actions `uses:` is pinned to a 40-hex commit SHA; the repository's default `GITHUB_TOKEN`
  permission is already `read`; workflows declare their own least-privilege `permissions:`.
- No credential-shaped content in any tracked file; no real usernames or personal paths — every
  `/home/...` in tracked files is a synthetic placeholder (`/home/synthetic`, `/home/fuzz`, `/home/dev`).
- Redaction is deny-by-default and share mode is always applied by `support-report`; snapshots are
  `0600` on Unix. Known limits (Windows snapshot ACLs, `.opencode` directories not parsed) are
  disclosed rather than hidden.
- Both adapter registries declare `conformance_status = "fixture_verified_max"` and a test enforces
  that no rule claims more.

The remaining work is **presentation, governance and a distribution decision** — not engineering.

## 1. Scorecard

| Area | State | Notes |
|---|---|---|
| Correctness and evidence discipline | **Strong** | Two real bugs found and fixed against pinned upstream; evidence level never overstated. |
| Test quality | **Strong** | A few near-tautological assertions (see technical report §2) — worth tidying, not blocking. |
| CI reliability | **Strong** | Deterministic, pinned, least-privilege, bounded fuzz budgets. |
| Security boundaries | **Good** | Fail-closed path policy; deny-by-default redaction. One low-severity `..`-normalisation gap. |
| Dependency discipline | **Needs a fix** | `thiserror` is declared but never used. |
| Documentation accuracy | **Needs work** | Several stale counts/claims — corrected in this branch (see §6). |
| Public README | **Needs work** | Leads with beta jargon before the tool is explained. |
| Distribution / install | **Blocking** | No public install path for any platform. |
| Repository governance | **Blocking (at launch)** | `main` unprotected; no tag protection; rulesets unavailable while private on the free plan. |
| Release presentation | **Good, needs wording** | Assets, checksums and SBOMs are right; the notes still say "private beta". |
| Discoverability | **Needs a 10-minute pass** | Description, topics and homepage are unset or stale. |
| Licensing | **Consistent, needs one decision** | `MIT OR Apache-2.0` declared and both texts present. |

## 2. Critical blockers (must be resolved before a public launch)

B1. **No install path.** CONFIRMED: no crates.io package, no Homebrew/Scoop/winget, no installer, and
    the README's only install route is "build from source". The release archives exist but are private.
    RECOMMENDATION: decide the distribution channel (GitHub Releases alone, or Releases + crates.io),
    then add a per-platform install section with `SHA256SUMS` verification. The archives, checksums and
    SBOMs already exist and need no rework.

B2. **`main` is unprotected and tags are unprotected.** CONFIRMED: `branches/main` reports
    `protected: false`, and the rulesets endpoint returns 403 —
    *"Upgrade to GitHub Pro or make this repository public to enable this feature."* So neither
    rulesets nor classic branch protection can be configured while the repo is private on the free
    plan. This is a **sequencing** constraint, not a dead end: enable protection immediately after the
    visibility change (see the plan in §3). Until then the release guard (tag/parents checks in
    `prerelease.yml`) is the only control, and a direct push to `main` can bypass it.

B3. **README leads with unexplained beta mechanics.** CONFIRMED: the first prose after the title is a
    private-beta block that uses `tool_version`, `SBOM root component`, `schema_version` and
    `adapter_revision` before anything is defined. RECOMMENDATION: a four-line
    what/why/supported/install strip under the title, with the beta mechanics demoted to a `## Status`
    section.

B4. **Licensing decision.** CONFIRMED: `MIT OR Apache-2.0` in `Cargo.toml`, both texts present,
    `LICENSE-MIT` carries `Copyright (c) 2026 ob4cl`, and the Apache text keeps the verbatim
    `[yyyy] [name of copyright owner]` appendix placeholder. The README carries the standard
    dual-licence notice and the Apache contribution clause. This is self-consistent. OWNER DECISION
    ONLY: confirm the licence, decide whether the copyright line should name a legal entity, and decide
    on a contributor policy (DCO, CLA, or the implicit Apache-2.0 clause that is there today).

## 3. Repository security and governance plan (for a solo maintainer)

CONFIRMED available today: Actions SHA pinning (in use), least-privilege `GITHUB_TOKEN` (already
`read`), a `SECURITY.md`, and dependency policy via `deny.toml` + a cargo-deny CI job.

CONFIRMED not available while private on the free plan: repository rulesets (403), classic branch
protection (403), secret scanning and Dependabot alert APIs (404/absent).

Recommended sequence — smallest secure configuration, nothing that can lock you out:

1. **Immediately after the visibility change** (not before — the APIs are unavailable while private):
   add a **repository ruleset** on the default branch:
   - Target `main`. Rules: *Require a pull request before merging* with **0 required approvals**
     (a solo maintainer cannot approve their own PR, so requiring 1 would lock you out),
     *Require status checks to pass* → `Test (ubuntu-24.04)`, `Test (macos-15)`,
     `Test (windows-2022)`, `Format and Clippy`, `MSRV (1.85.0)`, `Dependency policy (cargo-deny)`,
     *Block force pushes*, *Restrict deletions*. Add a **bypass for the repository admin** so you can
     always recover.
2. Add a **second ruleset targeting tags** `v*`: *Restrict creations*, *Restrict updates*,
   *Restrict deletions* — so a published tag cannot be moved or deleted. Keep the admin bypass.
3. Turn on **private vulnerability reporting** (Settings → Code security) so `SECURITY.md` can point
   at a real intake channel, and enable **Dependabot alerts** + **Dependabot security updates**.
   `deny.toml` + the cargo-deny job already cover the build-time advisory surface; Dependabot adds
   PR-level updates. A `dependabot.yml` for `cargo` and `github-actions` is a small, optional extra.
4. Keep Actions restricted: the current `allowed_actions: all` is acceptable **because** every action
   is SHA-pinned and reviewed; tightening to "GitHub-owned + allow-list" is optional hardening.
5. Do **not** enable repository rules requiring signed commits unless you set up signing first.

Workflow/token posture is already correct and needs no change: `permissions: contents: read`
everywhere, with `contents: write` only in the `publish` job, which never checks out or executes
repository code.

## 4. Public documentation changes to apply at launch

These are deliberately **not** applied yet: while the repository is private and the releases are
private, the current wording is accurate and the replacements would be false. Draft them now, apply
them in the same change that opens the repository.

| File | Change |
|---|---|
| `README.md` | Add the what/why/supported/install strip at the top; move the beta mechanics to a `## Status` section; add a 3-line glossary (`observed`, `fixture_verified`, `coverage gap`) before their first use; add a real install path and `Get-FileHash` / `sha256sum -c` verification; add a short Windows note; add a per-platform today/planned table. |
| `SECURITY.md` | The reporting section currently says GitHub private vulnerability reporting is "not guaranteed to be enabled" and that anyone outside the beta has no way to report. Replace with the enabled channel and a monitored contact. |
| `docs/getting-started.md` | Already fixed: the literal `git clone <this repository>` placeholder now shows the real URL. |
| `.github/ISSUE_TEMPLATE/*.yml` | Drop "private-beta" from the descriptions; the warnings about synthetic inputs are good and should stay. |
| `prerelease.yml` note | The preflight requires `github.ref == 'refs/heads/main'`, so a `release/*` branch cannot drive it — that is intentional, but the runbook should say so plainly. |
| `docs/demo/README.md` | Already fixed: the file `demo/record.sh` links to now exists. |

## 5. Licensing decisions requiring owner approval

- Confirm **MIT OR Apache-2.0** as the shipped licence (dual-licensed, "at your option" — already
  stated in the README).
- Decide the **copyright holder** line. `LICENSE-MIT` currently reads `Copyright (c) 2026 ob4cl`;
  the Apache appendix keeps the unmodified `[yyyy] [name of copyright owner]` placeholder, which is
  the verbatim text and normal for Rust projects.
- Decide the **contributor policy**: today the README's Apache-2.0 clause makes contributions
  implicitly dual-licensed inbound. DCO and CLA are the alternatives; both add friction.
- Decide whether the crate is ever **published to crates.io** and confirm name availability
  (`docs/maintaining.md` still lists this as open).

No licensing text has been changed in this branch.

## 6. Sensitive information in tracked files (classification)

| Item | Where | Class | Note |
|---|---|---|---|
| WSL hostname (Windows machine name) | `docs/verification-log.md` §1, plus quotations in the audit reports | **Redacted in HEAD; history pending your decision** | The single current-tree occurrence and the audit quotations are now `<hostname redacted>`. It still appears in **all 21** commits, so a public clone exposes it unless history is rewritten (destructive — needs explicit approval). |
| Exact machine model (Lenovo ThinkPad T440s, i5-4300U, 4 GB) | `docs/implementation-status.md` | **Owner approval** | Same reasoning; benchmark context has some value, so softening it is optional. |
| Local agent path `~/.hermes/profiles/coder/...` | `docs/research/fuzz-recovery.md` | **Redacted in this branch** | Revealed local tooling layout, not the project. Now `<scratch>/prefix-head`. |
| Personal email address | 1 commit author (`84c1c77`), plus quotations in the audit reports | **Redacted in HEAD; commit metadata pending your decision** | The audit quotations are now `<personal email redacted>`. The address itself remains in that one commit's author field, which only a history rewrite can change. It is PII (spam/phishing surface), not a credential. |
| `docs/beta/*` (1,397 lines: pilot guide, outreach messages, launch drafts, success metrics, issue triage, user-actions, tester instructions, release runbook, milestone checklist, windows plan) | tracked | **Needs relocation or acceptance** | Internal planning and business intent. Recommend moving to a private location before publication, or accepting them as maintainer notes. |
| `docs/spec/ConfigLens_Technical_Specification.md` (458 lines, incl. a business roadmap §10 and an agent brief §12) | tracked | **Needs redaction or relocation** | Strip the commercial/roadmap sections and the coding-agent brief if it is published. |
| `.github/AUTOMATION.md` | tracked | **Needs relocation or acceptance** | Maintainer release mechanics; harmless but internal in tone. |
| `docs/research/*` (audits, fuzz-recovery narrative, compatibility audit, findings) | tracked | **Public-safe** | Unusually candid, and that is a strength for credibility. Keep. |
| Fuzz corpus (2,461 files, 9.8 MB) | tracked | **Public-safe, consider LFS** | Intentional and documented; note the clone-size cost. |
| Historical evidence transcripts (old versions, old run IDs) | `docs/verification-log.md`, `docs/engineering-verification.md` | **Public-safe** | Dated records; left as written rather than retconned. |

## 7. Discoverability

Current (CONFIRMED): description is *"Offline configuration provenance CLI for Claude Code and
OpenCode — working Rust prototype"*; `repositoryTopics` is **empty**; `homepageUrl` is **empty**.

Recommended description:
> Trace which Claude Code or OpenCode configuration value actually wins — offline, read-only, and
> with every redaction and coverage gap stated.

Recommended topics: `rust`, `cli`, `configuration`, `configuration-management`, `developer-tools`,
`opencode`, `claude-code`, `ai-coding-agents`, `provenance`, `static-analysis`, `devtools`,
`command-line-tool`.

Suggested README shape: title + one-line pitch → four-line what/why/supported/install → a real
terminal example with the output explained → commands and exit codes → "what wins actually means"
(observed vs effective) → compatibility matrix → install per platform → privacy and caveats →
contributing. That ordering fixes the "jargon before explanation" problem without adding hype.

## 8. Applied in this branch

Documentation accuracy only; no legal, licence, workflow, security-setting or code change.

- Reconciled the stale test census (315 → 324, `cli_contract` 24 → 33) in `CONTRIBUTING.md`,
  `docs/implementation-status.md`, `docs/verification-log.md` and `docs/engineering-verification.md`,
  removing the internal contradiction between `verification-log.md` §3 and §9.
- Replaced the stale "`dependencies`/`fuzz` have never run", "`prerelease.yml` has never run" and
  "no release has been published" claims with the verified run IDs, in `docs/implementation-status.md`,
  `docs/beta/README.md`, `docs/beta/release-runbook.md` and `docs/beta/milestone-checklist.md`.
- Corrected two false capability claims in `docs/implementation-status.md`: the "BrokenPipe → exit 0"
  mapping (no such code exists; a closed pipe exits 101) and the panic-hook location (`src/main.rs`,
  not `error`).
- Corrected `docs/decisions.md` where it named symbols and files that do not exist (D2, D9, D10, D12, D14).
- Fixed the literal `git clone <this repository>` placeholder in `docs/getting-started.md`.
- Corrected the false claim that `--format json` prints a JSON error envelope on stdout
  (`docs/beta/windows-regression-plan.md`).
- Added the missing `docs/demo/README.md` that `demo/record.sh` links to.
- Redacted one local-tooling path (`~/.hermes/profiles/coder/...` → `<scratch>/prefix-head`) in
  `docs/research/fuzz-recovery.md`. This is the only machine-specific string changed; the other
  candidate identifiers are listed in §6 for your decision because they also live in git history.
- Redacted the machine hostname from its single current-tree occurrence (`docs/verification-log.md`
  §1, the benchmark host line) and removed the hostname, local home path and personal email address
  that the audit reports themselves quoted, so HEAD now contains none of them. Note this reduces
  casual exposure only — all three remain in the commit history, which only a rewrite would change.
- Added the three audit reports and this plan.

## 9. Go / no-go

**NO-GO for a public launch today.** Two blockers are outside engineering: there is no public install
path (B1), and repository protection cannot even be configured until the repository is public (B2).

**GO for the following, in this order, once you approve:**

1. Apply the documentation changes in §4 and update the release-notes wording — one PR.
2. Make the licensing and sensitive-information decisions in §5 and §6 (especially `docs/beta/*`,
   `docs/spec` and the git-history email).
3. Decide the distribution channel and publish at least one public release with `SHA256SUMS`.
4. Change visibility, then immediately apply the §3 protection plan.
5. Update the description/topics/homepage (§7).

The engineering is ready. The remaining risk is presentational and procedural, and every item is a
decision you own rather than outstanding development work.
