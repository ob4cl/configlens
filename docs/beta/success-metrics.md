# Success metrics (modest and honest)

The spec (§2) says the validation signal is *"several users solving an actual incident faster
with a trace, not stars on an announcement."* These metrics follow that. No star, download or
follower targets are set anywhere.

## Primary metrics

### 1. Task success (does it actually help?)

Definition: a tester **completes the pilot task and reaches a useful diagnostic without the
maintainer doing the reasoning for them.**

- Measure: testers who answer "yes" to *"did you solve the puzzle without my help?"* in the
  feedback template.
- Floor to proceed to public launch: **≥3 of 5–10 testers** (≤ the spec's "at least three actual
  incidents"). Stretch: ≥70% of testers.
- Record: count of `yes` / testers who completed the script.

### 2. Return usage (does it earn a second run?)

Definition: a tester uses ConfigLens again **unprompted** within 2 weeks of the pilot, on a new
question, without being asked to.

- Measure: unprompted re-use reported by the tester or seen in follow-up feedback.
- Floor: **≥2 of 5–10** testers use it again on a fresh problem.
- Why modest: a first beta rarely earns habitual use; 2 independent re-uses is a real signal.

### 3. Resolved bugs (does it survive scrutiny?)

Definition: correctness/privacy/security findings from the pilot **closed** — fixed with a
regression test, or explicitly documented as an honest limitation.

- Measure: count of serious (S1/S2) findings closed vs. total raised.
- Floor before public launch: **100% of S1 (security/privacy or silently-wrong) findings closed
  or documented loudly; ≥80% of S2 closed.** No S1 may remain open when the repo goes public.

## Secondary (context, not goals)

- Time to first useful result during the pilot (report the distribution, not an average claim).
- Number of distinct puzzles diagnosed across different clients/versions.
- Number of concrete issues opened from the pilot and their disposition.
- Whether any tester reports a *real* precedence surprise in the client itself (a good sign the
  tool surfaced something non-obvious).

## What is explicitly NOT a metric

- GitHub stars, forks, watchers, downloads — recorded at most as background, never a target.
- Number of posts, mentions or "impressions".
- Any forecast of "first 100/1000 users". The spec says those are not guaranteeable; do not
  claim them.

## Where results live

- Per-tester rows (handle, date, client/version, task success, return usage, findings) live only
  with the maintainer — redacted templates, no raw config.
- Aggregate the three primary metrics into the release notes / a short "what we learned" section,
  using only consented, anonymizable quotes.

## Go / no-go reading

- **Go to public launch if:** task-success floor met, return-usage floor met, all S1 closed and
  ≥80% S2 closed, and the release-readiness gate in `milestone-checklist.md` is satisfied.
- **Hold if:** the tool only works with maintainer explanation, or any S1 privacy/correctness
  finding is unresolved. Honesty beats a hasty launch.
