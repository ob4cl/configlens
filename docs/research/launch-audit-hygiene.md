# ConfigLens — public-hygiene / sensitive-information audit (pre-publication)

- Auditor: `researcher` agent. Date: 2026-10-11 (BST).
- Target: repo `/home/user/configlens`, branch `main`, `HEAD e746957`, clean tree, currently PRIVATE.
- Method: read-only. Worked from git-tracked content only (`git ls-files`, `git grep`) so the audit
  covers exactly what a `git clone` would publish, plus a bounded history scan
  (`git rev-list --all` + `git grep`/`git cat-file`) because a public launch also exposes history.
- No file other than this one was created or modified; no commit, push or publish was performed.
  No real secret value is reproduced here. The real `~/.claude` and `~/.config` were never read.
- Placeholders (`$REPO`, `$HOME`, `$PROJECT`, `$XDG_CONFIG`, `$env:USERPROFILE`, `$SCRATCH`) and the
  synthetic identities `/home/dev`, `/home/synthetic`, `/home/fuzz`, `/home/x`, `/home/u` are treated
  as intentional and public-safe.

Legend for the decision column:
PUBLIC-SAFE · REDACT · RELOCATE · OWNER-APPROVAL · STALE (fix before publish) · UPDATE-ON-PUBLIC.

---

## 0. Headline

- **No real secret or credential exists anywhere in tracked content or history.** Everything that
  looks key-like is a marked synthetic (`demo-not-a-real-token`, `fixture-not-a-real-token`,
  `SEEDED`, `ZUINFO`, `jira.example`, `.invalid`).
- The real exposure is **identity**, not secrets: a machine **hostname** and the **maintainer's name**
  in tracked docs, a **local agent path**, machine-fingerprint benchmark text, and a **git-history
  trail of `/home/user`** and a **personal gmail committer address**.
- The second exposure is **audience**: `docs/beta/*`, `docs/research/*`, `.github/AUTOMATION.md` and
  parts of the spec are internal plans/audits with launch, outreach and business content that were
  never written for a public reader, and several carry **stale** or **mutually contradictory**
  statements (chiefly "has `prerelease.yml` ever run?").

---

## 1. Local usernames and absolute personal paths

| # | Location | What | Decision | Reason |
|---|---|---|---|---|
| 1.1 | `docs/verification-log.md:10` | Real WSL hostname `DESKTOP-T5SB5DG` (`\| Host \| Linux \`DESKTOP-T5SB5DG\` …`) | **REDACT** | Machine identifier; uniquely names the author's box. Replace with `Linux (WSL2), x86_64`. |
| 1.2 | `docs/implementation-status.md:198` | `Lenovo ThinkPad T440s, Intel i5-4300U 2c/4t (3 vCPU), 4 GB RAM, Linux 6.18.x-microsoft-standard-WSL2` | **REDACT / OWNER-APPROVAL** | Machine fingerprint (exact laptop model + specs). A benchmark needs "hardware class", not the exact device. Keep the repo-pointer risk low: relabel as `WSL2 Ubuntu, x86_64, 3 vCPU, 4 GB`. |
| 1.3 | `docs/research/fuzz-recovery.md:251` | `SC=~/.hermes/profiles/coder/cache/scratch/prefix-head` | **REDACT** | Absolute local scratch path that leaks the author's agent tooling/profile layout. Replace with `$SCRATCH`. |
| 1.4 | `docs/beta/windows-regression-plan.md:12-13,38,117` | `$env:USERPROFILE\Desktop\configlens-test…` | PUBLIC-SAFE | Intentional placeholder; no real path. |
| 1.5 | `docs/research/run-fixtures.sh:10-12,16,20` | `$ROOT`, `$FIX`, `$HOME`, `$XDG_CONFIG_HOME` derived at runtime | PUBLIC-SAFE | Self-locating script; no hard-coded path. |
| 1.6 | `benches/resolution.rs:22-30`; `fuzz/fuzz_targets/*.rs:41-133`; `src/**` tests; `tests/**`; `scripts/test_generate_sbom.py:17,172,188,203` | `/home/dev`, `/home/synthetic`, `/home/fuzz`, `/home/x`, `/home/y`, `/home/u`, `C:\Users\dev` | PUBLIC-SAFE | Synthetic test fixtures; clearly not a real machine. |
| 1.7 | `docs/beta/README.md:4` | `Owner: the maintainer (user).` | **OWNER-APPROVAL** | Names the real maintainer. Publish under a chosen public handle or drop the name. |
| 1.8 | `docs/beta/user-actions.md:1` | Heading `# What user needs to do` | **OWNER-APPROVAL / RELOCATE** | Real first name in a heading; whole file is an internal action list. |
| 1.9 | `docs/beta/user-actions.md:27` | `No manual Rust work is required from user at this stage.` | OWNER-APPROVAL | Same person named again. |
| 1.10 | `docs/engineering-verification.md:5,10,298,330` | `owned by the coder agent` / `the integration agent` / `the research agent` | **RELOCATE** | Internal multi-agent workstream note; names the authoring agents. |
| 1.11 | `docs/research/beta2-independent-audit.md:3` | `Auditor: researcher agent (independent…)` | RELOCATE | Internal audit provenance. |
| 1.12 | `docs/research/project-config-discovery-finding-20261011.md:3` | `fixed 2026-10-11 (option 1 below, owner-approved)` | OWNER-APPROVAL | Reveals internal approval workflow. |

## 2. Secrets and tokens

| # | Location | Kind | Decision | Reason |
|---|---|---|---|---|
| 2.1 | — | — | **NONE FOUND** | `git grep` for `api[_-]?key / secret / token / bearer / password / private key / sk- / ghp_ / github_pat_ / AKIA / xox[baprs]-`, and a full history-blob scan, found **no** real secret. |
| 2.2 | `demo/demo.sh:41-57`; `docs/research/fixtures/**` | synthetic tokens (`demo-not-a-real-token`, `fixture-not-a-real-key`) | PUBLIC-SAFE | Explicitly fake; used by the redaction tests. |
| 2.3 | `tests/security_leaks.rs`, `src/security/redact.rs`, `fuzz/fuzz_targets/hostile_inputs.rs` | `SEEDED`, `ZUINFO` sentinels; `user:pass@…example` | PUBLIC-SAFE | Test sentinels in synthetic URLs on reserved `example`/`.invalid` domains. |
| 2.4 | `compatibility/{claude,opencode}/registry.json:314-332 / 535-553` | lists `password`, `token`, `private_key`, … | PUBLIC-SAFE | These are the **sensitive-key denylist** the tool matches on — field *names*, not values. |
| 2.5 | No tracked `.env`, `.pem`, `.key`, `id_rsa`, `credentials`, `.npmrc`, `.netrc` | — | PUBLIC-SAFE | `git ls-files` finds none; `.gitignore:24-26` also excludes `pilot-log.md` tester data. |

## 3. Private / internal URLs and hosts

| # | Location | What | Decision | Reason |
|---|---|---|---|---|
| 3.1 | `Cargo.toml:8`, `README.md`, `docs/maintaining.md:85` | `https://github.com/ob4cl/configlens` | PUBLIC-SAFE / OWNER-APPROVAL | The project's own (currently private) repo; fine once public, but confirm it is the intended public org/name (checklist D2). |
| 3.2 | `.github/workflows/prerelease.yml:20` | `if: github.repository == 'ob4cl/configlens' && github.event.repository.private == true` | **OWNER-APPROVAL / UPDATE-ON-PUBLIC** | Preflight hard-requires a **private** repo — the workflow becomes inert the moment the repo goes public. Decide: retarget to the public repo or leave it private-only. |
| 3.3 | `.github/AUTOMATION.md:37-49,86` | "private `ob4cl/configlens`", "Do not make the repository public without …" | RELOCATE / OWNER-APPROVAL | Internal maintainer note about the private-release process in a file that would be published under `.github/`. |
| 3.4 | `docs/beta/README.md:61`; `docs/implementation-status.md:68,195` | `https://github.com/ob4cl/configlens/actions/runs/38099457727` | **REDACT / OWNER-APPROVAL** | Direct link to a run in the **private** repo — 404s for the public and confirms private CI. Remove or replace with a neutral "CI green" statement. |
| 3.5 | `docs/research/evidence/*.txt`, `docs/research/compatibility-audit.md` | `raw.githubusercontent.com/.../53d1eabb…`, `registry.npmjs.org`, `code.claude.com/docs/...`, `opencode.ai/docs/config` | PUBLIC-SAFE | Public upstream evidence URLs. |
| 3.6 | `compatibility/opencode/registry.{json,md}` | `api.github.com/repos/sst/opencode/...` (org now `anomalyco/opencode`) | PUBLIC-SAFE | Already carries the rename note; public. |
| 3.7 | `demo/demo.sh:54`; `docs/research/fixtures/**` | `demo.internal.example`, `jira.example.invalid`, `host.example` | PUBLIC-SAFE | Reserved synthetic hosts. |

## 4. Internal development notes — per-file classification

All files below are tracked and would publish. None contains a secret; the issue is audience.

| File | Verdict | Why |
|---|---|---|
| `docs/beta/README.md` | **RELOCATE** | Index of private-beta plans; names maintainer; "stays private until …". |
| `docs/beta/milestone-checklist.md` | **RELOCATE** | Internal status tracker with TODO/DECISION states and stale run links. |
| `docs/beta/pilot-guide.md` | **RELOCATE** | Tester recruitment + consent workflow; explicitly says keep the log outside the repo. |
| `docs/beta/issue-triage.md` | **OWNER-APPROVAL** | Internal triage policy; usable publicly after light edit, but written for maintainers. |
| `docs/beta/release-runbook.md` | **OWNER-APPROVAL** | Maintainer runbook; several claims already drift (see §6). |
| `docs/beta/success-metrics.md` | **RELOCATE** | Defines pilot "go/no-go" floors and user-count targets — business/validation plan. |
| `docs/beta/launch-drafts.md` | **RELOCATE** | Marketing drafts for r/rust, HN, Lobsters, etc.; unposted commercial intent. |
| `docs/beta/outreach-messages.md` | **RELOCATE** | Copy-paste DM/email scripts (WhatsApp/LinkedIn/X) for recruiting testers. |
| `docs/beta/user-actions.md` | **RELOCATE** | Personal action list; names user; approval/pricing/legal decisions. |
| `docs/beta/tester-instructions.md` | **OWNER-APPROVAL** | Tester-facing; references "private repo / private issue"; becomes stale publicly. |
| `docs/beta/windows-regression-plan.md` | **OWNER-APPROVAL** | QA plan tied to the private beta; no secrets, but internal. |
| `docs/spec/ConfigLens_Technical_Specification.md` | **OWNER-APPROVAL** | Design deliverable, but §10 carries the 30-day roadmap, adoption/launch strategy and "first 100/1,000 users" business intent, and §12 is a literal prompt for a coding agent. Decide per section: design §1-9/§13 can stay; strategy §10 and the agent prompt ruin an audience fit. |
| `docs/research/beta2-independent-audit.md` | **RELOCATE** | Internal audit; names agents, cites a private commit/history, contains a stale privacy claim (S-list). |
| `docs/research/compatibility-audit.md` | **OWNER-APPROVAL** | Large evidence audit; valuable and public-safe in content, but it is an internal work product with historical commit refs. |
| `docs/research/codex-adapter-contract.md` | **OWNER-APPROVAL** | 0.2 roadmap design note ("draft for ConfigLens 0.2") — forward-looking internal plan. |
| `docs/research/fuzz-recovery.md` | **REDACT then OWNER-APPROVAL** | Useful evidence, but §"Method" leaks the local scratch path (1.3) and describes the internal parent/agent workflow. |
| `docs/research/project-config-discovery-finding-20261011.md` | **OWNER-APPROVAL** | Internal finding record; "owner-approved". |
| `docs/research/run-fixtures.sh` | **OWNER-APPROVAL** | Helper; safe content, but pairs with the fixtures tree below. |
| `docs/research/fixtures/**` (+ evidence `*.txt`) | **PUBLIC-SAFE** | Synthetic fixtures on reserved hosts; evidence transcripts use `$REPO`/`$HOME`. Keep if the research docs stay. |
| `.github/AUTOMATION.md` | **RELOCATE** | Private-beta CI/prerelease automation notes; references the private repo. |

**Names real people / testers / contacts / prices / business intent:** `docs/beta/README.md:4` and
`docs/beta/user-actions.md:1,27` name the maintainer; `docs/beta/*` describe unpaid testers,
consent logs and "who to contact"; `docs/beta/launch-drafts.md`, `outreach-messages.md`,
`success-metrics.md`, `pilot-guide.md` and `docs/spec/…:305-333` carry the full launch/pricing-free
but commercial-intent plan. **No customer names, contact lists or prices are present.**

## 5. Scratch / generated files that are tracked

| Path | Verdict | Why |
|---|---|---|
| `fuzz/results/session-{1..4}.log`, `fuzz/results/fuzz-session.log` | PUBLIC-SAFE, **consider RELOCATE** | Deliberately committed evidence (referenced by `verification-log.md` §8.3); content is synthetic and uses `$REPO`. Bulky (session-2 786 KB); publish only if you want the evidence trail. |
| `fuzz/results/hostile_inputs-crash-6ac24e86…`, `fuzz/results/found-before-fix/**` | PUBLIC-SAFE | Crash reproducers (27 B / 20 B / 0 B), synthetic. `.gitignore:15` ignores the *live* `fuzz/artifacts` dir, so these are intentional keeps. |
| `fuzz/corpus/**` (2461 files, ~9.8 MB) | PUBLIC-SAFE, note size | Committed coverage-minimised seed corpus (`.gitignore:10-12` explains it). No secrets (grep for `SEEDED`/`ZUINFO` → 0 hits). |
| `docs/demo/configlens-demo.cast` | PUBLIC-SAFE, but see §6/§7 | Real asciinema recording; uses `$SCRATCH`/`$HOME`/`$PROJECT` aliases, no real value. `docs/demo/` has no README though `record.sh:24` links `docs/demo/README.md` (dangling). |
| `target/`, `dist/`, `*.cdx.json`, `*.rs.bk`, `__pycache__/`, `.idea/`, `.vscode/`, editor backups (`*~`, `*.swp`, `*.orig`) | — | **None tracked** (`.gitignore` covers them). Clean. |

## 6. Misleading / stale version references and docs

Cross-checked against `Cargo.toml` (`version = "0.1.0-beta.3"`) and the three tags
`v0.1.0-beta.1`, `v0.1.0-beta.2`, `v0.1.0-beta.3` (beta.3 == `e746957`).

| # | Location | Statement | Decision | Reality |
|---|---|---|---|---|
| 6.1 | `docs/verification-log.md:76` | `configlens 0.1.0` | **STALE** | Binary now prints `0.1.0-beta.3` (same file §9:346). |
| 6.2 | `docs/verification-log.md:29,160` | `Checking configlens v0.1.0 ($REPO)` | STALE | Should read `v0.1.0-beta.3`. |
| 6.3 | `docs/verification-log.md:232` | `generate_sbom.py --tag v0.1.0-beta.1` | STALE | §9 uses `0.1.0-beta.3`; example drifts from the release line. |
| 6.4 | `docs/verification-log.md` §3 (`315 passed`, :62) vs §9 (`324 passed`, :341) | two different totals in one file | **STALE / CONTRADICTION** | Same file gives 315 and 324; §8.8 also says 315. Reconcile from one fresh run. |
| 6.5 | `.github/AUTOMATION.md:100` | `--tag v0.1.0-beta.1` | STALE | Example predates the beta.3 line. |
| 6.6 | `docs/maintaining.md:88-93` (rewritten at `e746957`) | "`prerelease.yml` has now run end to end successfully for `v0.1.0-beta.1` and `v0.1.0-beta.2`"; `dependencies`/`fuzz` jobs green | **OWNER-APPROVAL / CONTRADICTION** | Directly contradicted by `implementation-status.md:68,199`, `docs/beta/README.md:63`, `.github/AUTOMATION.md:36-49`, `docs/beta/milestone-checklist.md:72,74`, `docs/verification-log.md:318-321` and `CHANGELOG.md:151`, which all say the workflow/jobs have **never run**. Cannot be resolved offline (repo is private) — confirm against GitHub before publishing. |
| 6.7 | `CHANGELOG.md:42-44` vs `:151-153` | beta.2 "Published as the private `v0.1.0-beta.2` prerelease" vs "no release has been published" | **CONTRADICTION** | Self-contradictory within the changelog. |
| 6.8 | `docs/beta/README.md:60-67` | "`HEAD` is `35ad132`", "315 passed" | STALE | `HEAD` is now `e746957`. |
| 6.9 | `README.md:14-18,164` / `implementation-status.md:31` | `0.1.0-beta.3`, `adapter_revision 0.1.0`, `schema_version 1.0` | PUBLIC-SAFE | Consistent and current. |

## 7. Outdated screenshots/examples and unsupported claims

| # | Location | Claim | Verdict |
|---|---|---|---|
| 7.1 | `docs/beta/milestone-checklist.md:120,207` (D4) | the recording "exists in the working tree … but is **not committed**" | **STALE** — `docs/demo/configlens-demo.cast` **is** tracked. |
| 7.2 | `docs/beta/release-runbook.md:43,62` | "the tag-triggered `release.yml` builds the matrix" and "each built binary is smoke-tested" | **STALE/UNSUPPORTED** — no `release.yml` exists; `prerelease.yml` is manual and has no smoke test (already flagged in `beta2-independent-audit.md` B1). |
| 7.3 | `docs/beta/README.md:45-46,55-56` | 270-test census; MSRV "declared but never built"; Windows ACLs missing | STALE (labelled a dated snapshot, but easy to misread). |
| 7.4 | `README.md:74-76`, `docs/getting-started.md:79-90`, `demo/demo.sh` output | `explain /model` shows `local-model`/`project-model` | OK — matches the current demo's synthetic values; verify once more after any output change. |
| 7.5 | `README.md:19,155-156`; `implementation-status.md:198` | benchmarks exist, quadratic scaling, medians | SUPPORTED by `benches/resolution.rs`; but the *hardware* text is the fingerprint in 1.2. |
| 7.6 | `SECURITY.md:6-11`, `CONTRIBUTING.md:3`, `docs/getting-started.md:15` | "This repository is private (private beta)" | UPDATE-ON-PUBLIC — not harmful, but wrong the day the repo is public; enable private vulnerability reporting then. |
| 7.7 | `docs/verification-log.md:104-111,318-323`; `docs/beta/milestone-checklist.md:46,156-157,163` | independent harness `configlens_acceptance.py` "not in the repo"; C6/F1 TODO | SUPPORTED as *documented gaps* — no unsupported claim; just confirm you publish the gap honestly. |
| 7.8 | `compatibility/claude/registry.md` / `opencode/registry.md` | all rules ≤ `fixture_verified`; no `upstream_conformance_verified` | SUPPORTED — consistent with README and tests. |

## 8. Git history (also published on push)

| # | Scope | Finding | Decision |
|---|---|---|---|
| 8.1 | 14 of 19 commits (`git rev-list --all`) | Old revisions of `docs/verification-log.md`, `docs/research/run-fixtures.sh`, `docs/research/fixtures/codex/home/.codex/config.toml`, `docs/research/evidence/fixture-run.txt`, `docs/engineering-verification.md` contain `/home/user/configlens`. | **REDACT (rewrite history or accept)** — HEAD is clean, but a clone exposes the username. Rewrite or accept knowingly. |
| 8.2 | All 19 commits | `DESKTOP-T5SB5DG` appears in every commit's `verification-log.md` revision. | REDACT — same machine identifier as 1.1, historically. |
| 8.3 | commit `84c1c77234c994435acf7c54e70364ef9ea2a9aa` ("Add GitHub Actions workflow for Rust project") | Committer/author email `ob4cl@users.noreply.github.com` (git metadata) alongside the `…+ob4cl@users.noreply.github.com` used elsewhere. | **OWNER-APPROVAL** — a personal email becomes public with the push. Confirm/rewrite to the noreply address. |
| 8.4 | all blobs | No secret-like token anywhere in history. | PUBLIC-SAFE. |

---

## MUST be handled before the repository is made public

1. **Redact the hostname** `DESKTOP-T5SB5DG` — `docs/verification-log.md:10` **and all history**
   (§1.1, §8.2).
2. **Redact/replace the machine fingerprint** in `docs/implementation-status.md:198` (§1.2).
3. **Redact the local agent path** `~/.hermes/profiles/coder/…` — `docs/research/fuzz-recovery.md:251` (§1.3).
4. **Remove the private-repo Actions run link** `…/ob4cl/configlens/actions/runs/38099457727`
   (`docs/beta/README.md:61`, `docs/implementation-status.md:68,195`) (§3.4).
5. **Resolve the "has `prerelease.yml`/`dependencies`/`fuzz` ever run?" contradiction** across
   `docs/maintaining.md:88-93` vs `implementation-status.md` / `CHANGELOG.md` / `AUTOMATION.md` /
   `milestone-checklist.md` / `verification-log.md` / `docs/beta/README.md`, and the internal
   `CHANGELOG.md:44` vs `:151` clash — confirm against GitHub, then make every doc agree (§6.6–6.7).
6. **Relocate every `docs/beta/*` file and `.github/AUTOMATION.md`** to a private location (or get
   explicit owner approval to publish), because they are unreleased plans, outreach scripts, tester
   procedures, business metrics and named-person action lists (§4).
7. **Decide the audience for `docs/research/*`** (internal audits/findings that name the authoring
   agents and cite the internal workflow) — relocate or redact before publishing (§1.10, §1.11, §4).
8. **Decide on `docs/spec/…`**: strip or relocate the launch strategy / business roadmap (§10) and
   the coding-agent prompt (§12) before the design doc goes public (§4).
9. **Owner decision on naming the maintainer** — `docs/beta/README.md:4`,
   `docs/beta/user-actions.md:1,27` (§1.7–1.9), and on the **personal gmail committer address** in
   git history (§8.3).
10. **Fix the stale version strings** `0.1.0` → `0.1.0-beta.3` (`verification-log.md:29,76,160`;
    `.github/AUTOMATION.md:100`) and **reconcile the 315-vs-324 test census** in a single file
    (§6.1–6.4), and correct the D4 "not committed" claim (§7.1) and the runbook's non-existent
    `release.yml`/smoke test (§7.2).
11. **Update the "repository is private" wording** in `SECURITY.md`, `CONTRIBUTING.md`,
    `docs/getting-started.md` and re-target/disable `prerelease.yml`'s private-repo preflight on the
    day the repo opens (§3.2, §7.6).

Not required, but decide deliberately: whether to ship the ~10 MB `fuzz/corpus` + `fuzz/results`
evidence and the tracked `docs/demo/configlens-demo.cast`, and whether to add the missing
`docs/demo/README.md` that `demo/record.sh:24` links to (§5).

_Report written to `/home/user/configlens/docs/research/launch-audit-hygiene.md` (the one file this
audit was authorised to create)._
