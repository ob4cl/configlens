# Launch drafts — DRAFTS ONLY, nothing is posted

Every item below is a draft for the maintainer to review, edit and — only after explicit approval
(`user-actions.md` E.19/E.20) — post themselves. No posting, tagging, DMing, contacting moderators
or publishing is done by the tooling that produced this file.

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
  > Example (synthetic fixture), `explain /model --client claude`:
  > ```
  > /model = model-b
  > Observed winner: $PROJECT/.claude/settings.local.json:2:12
  >   model-a  $PROJECT/.claude/settings.json:2:12  replaced by local setting
  > Rule: claude.model.local-over-project
  > Coverage: partial — runtime arguments and managed sources not supplied
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
  > Limitations are listed in the README — only two adapters exist, no runner has reached
  > `upstream_conformance_verified`, and there's at least one known redaction gap in share mode.
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
