# Launch drafts — DRAFTS ONLY, nothing is posted

Every item below is a draft for the maintainer to review, edit and — only after explicit approval
(`user-actions.md` E.19/E.20) — post themselves. No posting, tagging, DMing, contacting moderators
or publishing is done by the tooling that produced this file.

Every command transcript below was re-run against a synthetic home before being written here; the
earlier drafts contained an invented `Rule:` name and example text, which is exactly what a reviewer
would catch.

## Rules for all drafts (from spec §10 "Release and distribution")

- Lead with **one reproducible behaviour**, not AI hype.
- State the **honest limitations** in the post itself.
- **No star/follower/download solicitation**, no "please upvote", no manufactured proof.
- Several communities require **permission before posting**. Ask first; do not assume.
- Use the **real** demo recording (`user-actions.md` E.21). Do not post with a placeholder.
- Attribute and thank Ruler/Rulesync as *complementary* tools without implying endorsement.

Before posting anywhere: confirm the repo is public (approval A.1) and the release-readiness gate
is met.

---

## 1. GitHub release notes (v0.1.0)

See the template in `release-runbook.md` §4. Audience: people who found the repo. Tone:
factual, limits up front. This is the canonical text other posts link to.

---

## 2. r/rust

- **Audience:** Rust developers; values correctness, MSRV honesty, no-hype posts.
- **Check first:** the subreddit's current self-promotion rules and whether a "project" flare is
  required. Ask a mod if unsure.
- **Draft:**

  > **ConfigLens — an offline, read-only Rust CLI that explains which AI-coding-tool config value wins**
  >
  > I kept losing time to *"why did my model/permission/MCP setting change?"* in Claude Code and
  > OpenCode, so I built a small CLI that traces a supported key across the config files and prints
  > an ordered trace: which file the value came from, what replaced it, and what it *couldn't* see.
  >
  > It's deliberately boring on safety: no network, no subprocess, no client execution, redacted by
  > default, and `--paths share` to pseudonymize paths before you attach a report to an issue.
  >
  > Example, real output from a synthetic three-file home (reproduce with `./demo/demo.sh`):
  > ```
  > $ configlens explain /model --client claude --client-version 2.1.296 --paths share
  > /model = "model-b" (observed)
  > Observed winner: $PROJECT/.claude/settings.local.json:1:11  "model-b"  observed_winner
  > Losers:
  >   "model-user"  $HOME/.claude/settings.json:1:11  replaced_by_later_layer
  >   "model-a"  $PROJECT/.claude/settings.json:1:11  replaced_by_later_layer
  > Rule: claude.scalar.layer-override
  > Coverage: partial — cc.managed.hkcu — registry_not_read (expected_unobservable); cc.managed.mdm — ...
  > Support: fixture_verified; confidence: evidence_class:fixture_verified
  > Conclusion: "model-b" wins among the observed applicable sources; effective value withheld.
  > ```
  >
  > Honest limits: only Claude Code and OpenCode adapters exist right now; Codex is on the roadmap
  > but not implemented. Every rule is at most fixture-verified (neither client exposes a pinnable
  > loader; Claude is closed source), and managed/enterprise/remote state is often unobservable, so
  > it shows an *observed* winner rather than pretending to know the effective value.
  >
  > Build: `cargo build --locked --release`, then `./demo/demo.sh` runs a fully synthetic sandbox
  > demo. Rust 1.85 MSRV. MIT/Apache-2.0.
  >
  > [link]. Feedback — especially where the explanation is confusing — is welcome.

---

## 3. users.rust-lang.org (community / announcements)

- **Audience:** long-form; a "Show and tell" style thread if the forum has one.
- **Check first:** the forum's self-promotion norms.
- **Draft:** reuse the r/rust text, expanded with one paragraph on the design decision that
  mattered most (the span-preserving parser that never silently loses duplicate keys, and
  redaction as a *type-system* property — raw values have no `Serialize` impl). Link the spec.

---

## 4. Hacker News — "Show HN"

- **Audience:** technical, allergic to hype and to fake signals.
- **Check first:** HN's rules on Show HN; do not solicit votes anywhere.
- **Draft title:** `Show HN: ConfigLens – explain which AI coding-tool setting wins, offline`
- **Draft body:**

  > ConfigLens is a local, read-only Rust CLI that traces a supported configuration key for Claude
  > Code and OpenCode: which file supplied the winning value, what overrode it, and what layers it
  > could not observe (managed/enterprise settings, remote defaults) so it won't claim a value is
  > *effective* when its context is incomplete.
  >
  > It never makes a network request, never runs your config, and redacts secrets by default (a
  > `--paths share` mode pseudonymizes paths and unknown key names for attaching to issues).
  >
  > The interesting engineering bit: a hand-written span-preserving JSON/JSONC parser, because
  > off-the-shelf parsers silently drop duplicate keys, and duplicate-key evidence matters for
  > precedence. Redaction is enforced by types, not code review.
  >
  > Limitations are listed in the README: two adapters (Claude Code, OpenCode), every rule at most
  > fixture-verified because neither client exposes a pinnable loader, managed/remote layers reported
  > as unobservable rather than guessed, no macOS/Windows runtime comparison yet.
  > [link]

---

## 5. OpenCode community (Discord / GitHub Discussions)

- **Audience:** OpenCode users; a support-oriented crowd.
- **Check first:** ask maintainers in the community whether a tool announcement is welcome. Post
  only with permission; otherwise offer it in a "show your tools" channel if one exists.
- **Angle:** a config-precedence debugger for OpenCode's own layering (remote → global → custom
  path → project → `.opencode` → inline → managed). Emphasize complementary to OpenCode's docs,
  and that remote/MDM layers are reported as gaps rather than guessed.

---

## 6. Claude Code community (GitHub Discussions / issues)

- **Audience:** Claude Code users; the spec cites real precedence-confusion issues (#19369, #39109).
- **Check first:** the project's norms about third-party tools; do **not** post an unsolicited
  advert on an issue tracker. If referencing an issue, do so helpfully and with permission.
- **Angle:** the tool surfaces the documented precedence stack (managed → session args →
  settings.local → settings → user) and shows the exact source locations; it explicitly does not
  treat permission-list merging as an authorization decision.

---

## 7. Lobsters

- **Audience:** engineering-focused; strict on content quality.
- **Check first:** the site's self-promotion rules.
- **Draft:** a condensed version of the HN post, leading with the parser/redaction-by-types
  design and the honest-limits section.

---

## Timing and sequencing

1. Release notes live first (so every link lands somewhere real).
2. One primary community post (choose per your judgment), then see whether people actually engage
   before posting elsewhere. Do not blast every community at once.
3. Only post where a maintainer has given permission or the channel explicitly welcomes tools.
4. Answer questions in the thread honestly; turn real bug reports into issues via
   `issue-triage.md`.

## Never

- Never invent stars, downloads, users or testimonials.
- Never claim Codex/Gemini support, conformance, or cross-platform verification that isn't DONE.
- Never post without (a) approval and (b) the release gate met.

---

## 8. r/ClaudeCode and r/ClaudeAI (the users themselves)

- **Audience:** exactly the people who hit precedence confusion; tone is plain and practical.
- **Check first:** the current self-promotion rule and whether a "Tools"/"Project" flair is required.
  Several of these communities restrict self-promotion to a weekly thread or a flair; ask a mod if
  unsure rather than guessing.
- **Draft (r/ClaudeCode version):**

  > **Which of your Claude Code settings files actually won? I built a read-only CLI that tells you — and says what it can't see**
  >
  > The thing that kept costing me time: a setting looks like it should apply, and it doesn't. So I
  > wrote ConfigLens. Point it at a project and it prints the trace for one key: which file supplied
  > the winning value, what overrode it, and what it could not observe.
  >
  > ```
  > $ configlens explain /model --client claude --client-version 2.1.296 --paths share
  > /model = "model-b" (observed)
  > Observed winner: $PROJECT/.claude/settings.local.json:1:11  "model-b"  observed_winner
  > Losers:
  >   "model-user"  $HOME/.claude/settings.json:1:11  replaced_by_later_layer
  >   "model-a"  $PROJECT/.claude/settings.json:1:11  replaced_by_later_layer
  > Rule: claude.scalar.layer-override
  > Coverage: partial — cc.managed.hkcu — registry_not_read (expected_unobservable); cc.managed.mdm — ...
  > Support: fixture_verified; confidence: evidence_class:fixture_verified
  > Conclusion: "model-b" wins among the observed applicable sources; effective value withheld.
  > ```
  >
  > Safety was the design constraint, not a feature: no network, no subprocess, never runs your
  > config, redacted by default, and a `--paths share` mode that pseudonymizes paths and URL hosts so
  > you can attach a report to an issue. Rust, MIT/Apache-2.0, builds with `cargo build --locked
  > --release`.
  >
  > Honest limits, up front: Claude Code and OpenCode only (no Codex or Cursor); managed/enterprise,
  > remote and MDM state is usually unobservable, so it shows an *observed* winner and withholds an
  > "effective" value instead of guessing; and nothing has been verified against the real clients at
  > runtime yet — the rules are fixture-verified. [link]
  >
  > If the trace is confusing, that's the feedback I want.

## 9. Private phase: what you can post before the repo is public

Nothing public links anywhere until the repo is public and the release notes exist — a link to a
private repository 404s for everyone and reads as broken. So while the repo is private, the only
public activity is **answering**: find a thread where someone is confused about why a setting is
being ignored, answer it genuinely, and mention the tool as a reply (message E in
`outreach-messages.md`).

If someone asks for the link while the repo is private, do **not** post the private URL. Reply:
"It's in a private beta right now — send me your GitHub handle and I'll add you." That is also the
only way to turn a passing reader into a tester.

## 10. Before posting anywhere (checklist)

1. Repo is public, the release-readiness gate is met, and the release notes are live (so every link
   lands somewhere real).
2. You have read that community's self-promotion rules and applied the required flair/thread.
3. The post states its own limitations — a launch post without them reads as hype.
4. No request for stars, upvotes, follows or shares, and no invented users or testimonials.
5. One community at a time; see whether people engage before posting to the next.
6. No claim of runtime verification against Claude Code or OpenCode on any platform.
