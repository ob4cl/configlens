# Things to say to testers (copy-paste, approved wording)

Five messages, in the order they are used. Send them yourself — nothing here is sent by the tooling.
They are deliberately short, contain no hype, and state the real limits, because that is what gets
replies and honest feedback.

Replace `[name]` and never send more than one follow-up.

---

## A. First contact — someone you already know (WhatsApp, LinkedIn, X DM, email)

> Hi [name] — quick favour if you've got half an hour spare at some point.
>
> I've built a small command-line tool that explains why a config setting in Claude Code / OpenCode
> gets ignored or changes — it traces which file the value actually came from. It's private and
> unfinished, and before I show it to anyone I want a few developers to poke at it and tell me where
> it's confusing.
>
> It's read-only, runs offline, and doesn't touch anything else on your machine. About 30 minutes
> total, nothing to pay.
>
> If you're up for it I'll add you to the repo — I only need your GitHub username. If not, no worries
> at all. And if you know someone who spends their life in these config files, point them at me.

## B. When they say yes

> Nice one — thanks. Send me your GitHub username and I'll add you. You'll get an email from GitHub
> inviting you to a private repo; click Accept and you're in.
>
> Everything you need is in the repo at `docs/beta/tester-instructions.md`. It's short: build it, run
> the demo, then bring one real puzzle of your own — a setting that was ignored or changed on you —
> and see whether the tool explains it.
>
> Two honest things up front: it only supports Claude Code and OpenCode right now (no Codex, no
> Cursor), and it's read-only and offline, so it never sends anything anywhere. Anything you send back
> is voluntary — run reports with `--paths share` first and read them yourself before sending. You can
> stop at any time and ask me to delete whatever you've sent.

## C. Once they have access (and as the one reminder, about a week later)

> You should have the GitHub invite now — the repo is github.com/ob4cl/configlens. Start with
> `docs/beta/tester-instructions.md`. The half hour is: build it, run `./demo/demo.sh`, then one real
> puzzle using `scan`, `explain` and `doctor`, and fill in the template at the bottom of that page.
>
> If any step doesn't work on your machine, that's useful too — tell me what it printed.

## D. If they go quiet (send this once, then drop it)

> No rush at all — just checking the GitHub invite didn't get lost. If the full 30 minutes isn't
> happening, even the demo plus one sentence on whether the output made sense is genuinely useful. If
> it's not for you, that's completely fine — say the word and I'll remove your access.

## E. Community reply — ONLY after someone has engaged in a thread

Never post this cold; answer a real question first and offer it afterwards, in the same thread.

> That's exactly the kind of thing I got tired of chasing, so I built a read-only CLI that traces the
> key across your config layers and shows which source won and what it couldn't see — managed and
> remote state is usually unobservable and it says so rather than guessing. It's a private beta:
> Claude Code and OpenCode only, fixture-verified rather than verified against the real clients.
>
> I'm after honest "this was confusing" feedback, not praise. If you want in, tell me your GitHub
> handle and I'll add you.

---

## Never say

- "It's finished" / "production-ready" — it is a beta, and the honest limits are in the pitches above.
- "Verified against Claude Code / OpenCode" — nothing has been runtime-verified yet; the wording above
  says `fixture-verified`, keep it that way.
- Any request for stars, follows, shares or upvotes, and never offer anything in exchange for one.
- "Just try it and let me know if it's good" — ask for one specific thing: whether it showed the
  winner, the reason, and what it could not see.
- Anything about the testers, their setups or their feedback in public without their written OK.
