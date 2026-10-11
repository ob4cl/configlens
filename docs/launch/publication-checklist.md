# Publication checklist — preparing ConfigLens for a public repository

Status: **partly executed.** A2 and A3 are done (internal planning material relocated to a private
archive, and the specification's commercial roadmap and coding-agent brief removed) and A1 and A7 are
approved. **No GitHub setting below has been applied** and the repository is still private, so this
document remains the single source of truth for the switch from PRIVATE to PUBLIC.

Owner decisions this checklist implements:

1. Merge the launch-audit PR by fast-forward — **done** (`main` is `af7b0dc`).
2. **Preserve Git history.** No rewrite, no force-push, no tag moves, no deleted prereleases.
3. Keep **MIT OR Apache-2.0** with the `ob4cl` attribution.
4. **GitHub Releases** are the initial public distribution channel; no crates.io/Homebrew/Scoop/winget.
5. README carries prominent installation, supported platforms and checksum verification — **done**.

---

## 1. Public versus internal documentation (proposal)

Nothing here has been executed. The repository currently mixes user/contributor documentation with
maintainer planning notes and research transcripts. Three options:

- **Option A — recommended: keep everything, add an index.** No file moves, so no evidence is lost
  and no history is disturbed. A new `docs/README.md` separates "For users and contributors" from
  "Maintainer and planning notes" so a visitor is not dropped into outreach drafts. Public-safe: the
  material is honest, and its candour (including the audits that found real bugs) is a credibility
  asset rather than a liability.
- **Option B — leaner public surface.** Relocate the internal planning material out of the public
  tree, keeping a copy elsewhere. Use this if you would rather the public repo contain only developer
  documentation. It removes those files from future checkouts, but the content stays in history from
  before the move.
- **Option C — delete.** Rejected: it destroys evidence the audit trail depends on.

### Classification

| Path | What it is | Class | Proposed action |
|---|---|---|---|
| `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `CHANGELOG.md`, `LICENSE-*` | Project front door and policy | Public | Keep |
| `docs/getting-started.md`, `docs/decisions.md`, `docs/implementation-status.md`, `docs/verification-log.md`, `docs/maintaining.md`, `docs/demo/README.md` | Developer/maintainer documentation | Public | Keep |
| `compatibility/*/registry.md`, `docs/assets`, `demo/`, `benches/`, `fuzz/README`-equivalent comments | Product evidence and assets | Public | Keep |
| `docs/research/*` (compatibility audit, fuzz recovery, discovery finding, three launch audits) | Research transcripts and audits | Public-safe | Keep — cite as evidence |
| `docs/spec/ConfigLens_Technical_Specification.md` | Design spec | **Done (A3)** | The commercial roadmap (old §10) and the coding-agent brief (old §12) are removed; the document is renumbered §1–§11 and remains the design record |
| `docs/beta/*` (1,397 lines: pilot guide, outreach messages, launch drafts, success metrics, issue triage, tester instructions, release runbook, milestone checklist, user-actions, windows plan) | Internal planning, recruitment and business intent | **Relocated (A2)** | Moved to the private archive `~/configlens-internal-archive/docs-beta-20261011/`; removed from the public tree |
| `.github/AUTOMATION.md` | Maintainer release mechanics | **Relocated (A2)** | Moved to `~/configlens-internal-archive/AUTOMATION.md`; the workflows themselves remain public and are self-documenting |

### Exact commands for Option B (not run)

```sh
# Keep a copy outside the repository first.
mkdir -p ../configlens-internal && cp -r docs/beta .github/AUTOMATION.md ../configlens-internal/
git rm -r --cached docs/beta .github/AUTOMATION.md
git commit -m "docs: move internal planning material out of the public tree"
```

## 2. GitHub settings to apply (exact)

**Constraint (verified):** this is a personal free account. While the repository is private, *both*
repository rulesets and classic branch protection return
`403 Upgrade to GitHub Pro or make this repository public to enable this feature`, and the legacy tag
protection endpoint is gone (`404`). So the rulesets can only be applied **after** the repository is
public — or before it, if you upgrade to Pro for a month. Nothing in this section has been applied.

### 2.1 Branch ruleset for `main`

Rules: require a pull request before merging with **0 required approvals** (a solo maintainer cannot
approve their own PR — requiring 1 would lock you out); require the seven status checks in §3;
block force pushes; restrict deletions; add a bypass for the repository admin (so you can always
recover).

UI: Settings → Rules → Rulesets → New branch ruleset → Target `main`.

API equivalent (run after the flip):

```sh
gh api --method POST repos/ob4cl/configlens/rulesets --input - <<'JSON'
{
  "name": "protect-main",
  "target": "branch",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["refs/heads/main"], "exclude": [] } },
  "bypass_actors": [ { "actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always" } ],
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    { "type": "pull_request", "parameters": {
        "required_approving_review_count": 0,
        "dismiss_stale_reviews_on_push": false,
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": false } },
    { "type": "required_status_checks", "parameters": {
        "strict_required_status_checks_policy": true,
        "required_status_checks": [
          { "context": "Test (ubuntu-24.04)" },
          { "context": "Test (windows-2022)" },
          { "context": "Test (macos-15)" },
          { "context": "Format and Clippy" },
          { "context": "MSRV (1.85.0)" },
          { "context": "Dependency policy (cargo-deny)" },
          { "context": "Fuzz smoke (bounded)" } ] } }
  ]
}
JSON
```

### 2.2 Tag ruleset for `v*`

```sh
gh api --method POST repos/ob4cl/configlens/rulesets --input - <<'JSON'
{
  "name": "protect-release-tags",
  "target": "tag",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["refs/tags/v*"], "exclude": [] } },
  "bypass_actors": [ { "actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always" } ],
  "rules": [ { "type": "creation" }, { "type": "update" }, { "type": "deletion" } ]
}
JSON
```

### 2.3 Code security

| Setting | Where | Note |
|---|---|---|
| Private vulnerability reporting | Settings → Code security | Makes the primary channel in `SECURITY.md` real |
| Dependabot alerts | Settings → Code security | Currently absent (API 404) |
| Dependabot security updates | Settings → Code security | Currently disabled |
| Secret scanning / push protection | Settings → Code security | Available on public repositories |

### 2.4 Actions

Already correct: every `uses:` is pinned to a 40-hex commit SHA, the repository default
`GITHUB_TOKEN` permission is `read`, and only the `publish` job takes `contents: write` (it never
checks out or runs repository code). Optional hardening: restrict `allowed_actions` to GitHub-owned
plus an explicit allow-list. `.github/dependabot.yml` is included in this branch and starts version
update PRs once merged — it does not enable the security settings above.

### 2.5 Repository metadata

Description: *Trace which Claude Code or OpenCode configuration value actually wins — offline,
read-only, and with every redaction and coverage gap stated.*

Topics: `rust`, `cli`, `configuration`, `configuration-management`, `developer-tools`, `opencode`,
`claude-code`, `ai-coding-agents`, `provenance`, `static-analysis`, `devtools`,
`command-line-tool`.

Homepage: optionally the demo cast or leave blank.

## 3. Required status checks

All seven come from the `CI` workflow (`.github/workflows/ci.yml`) and must be required on `main`:

`Test (ubuntu-24.04)` · `Test (windows-2022)` · `Test (macos-15)` · `Format and Clippy` ·
`MSRV (1.85.0)` · `Dependency policy (cargo-deny)` · `Fuzz smoke (bounded)`

`Beta prerelease` is deliberately **not** required (it is manual and tag-driven).

## 4. Rollback and recovery

- **Take a mirror backup before the flip:** `git clone --mirror https://github.com/ob4cl/configlens.git ../configlens-mirror.git`
  (cheap, complete, includes every tag and ref).
- **Going public is effectively one-way.** You can flip back to private, but anyone who cloned or
  forked during the public window keeps their copy, and cached views can persist. Decide before
  flipping.
- **Rulesets are reversible:** delete or disable either ruleset at any time; the admin bypass keeps
  you unblocked even while they are active.
- **If a bad commit lands:** once `main` is protected, never force-push — revert with a new commit.
  Before protection is applied, a mistake can still be force-pushed away.
- **Tags and releases are untouched by all of this.** Per your decision, history is preserved, so the
  three existing prereleases and their checksums remain valid and their provenance stays intact.
- **If the `Beta prerelease` workflow misbehaves after the flip:** it is manual and reversible; a
  failed dispatch creates no release (the `gh release create` step is last, and the 7-asset assertion
  runs immediately after it, so a partial publication fails the job loudly).

## 5. Publication sequence

| Phase | Where | Actions |
|---|---|---|
| 0 | private | Merge this checklist PR. Take the mirror backup. Decide §1 option A or B. |
| 1 | private | Optionally strip `docs/spec` §10/§12, add `docs/README.md`, set description/topics. |
| 2 | private | Confirm the release workflow change (`Beta prerelease`) by dispatching it for an existing tag, or accept the code review of the change. |
| 3 | **the flip** | Set visibility to public, then immediately apply §2.1, §2.2 and §2.3, in one sitting. |
| 4 | public | Verify the seven checks are required and firing on a test PR; verify the Security tab offers private vulnerability reporting. |
| 5 | public | Publish or re-verify a release with `SHA256SUMS` + SBOMs; confirm the README install path works from outside. |
| 6 | separate approval | Announcement, tester recruitment, promotion — none of these are prepared as actions. |

## 6. Actions awaiting your approval

- **A1** ✔ Approved and merged (fast-forward).
- **A2** ✔ Executed — internal planning material relocated to the private archive.
- **A2** ✔ Executed — `docs/beta/*` and `.github/AUTOMATION.md` relocated (see §1).
- **A3** ✔ Executed — the specification's commercial roadmap and coding-agent brief are removed and the
document renumbered.
- **A4** Approve the exact §2 settings and say when to apply them (they require the repo to be public,
  or a temporary Pro upgrade).
- **A5** Confirm the release-workflow change is acceptable (the visibility gate is removed; every
  provenance check is retained).
- **A6** Approve the visibility flip itself, with the §5 sequence.
- **A7** Approve `dependabot.yml` landing (it starts opening monthly dependency PRs).
