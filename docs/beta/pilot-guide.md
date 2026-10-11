# Private beta pilot guide (5–10 real developers)

Goal: find out whether ConfigLens actually helps a developer answer *"why did this setting
change / why is it ignored?"* faster than reading files by hand. Stars and downloads are not
the point. A trace that solves a real incident is.

You (the maintainer) run this. This guide does not contact anyone; it prepares the material so
the only real-world step is your own invitation, after you approve it (`user-actions.md` B.5/B.6).

**Forward `tester-instructions.md` to testers — not this file.** It is the tester-facing copy of the
task below plus the consent text and the feedback template, and it can be pasted into a message as-is.

## Scope and rules

- **5–10 testers**, each a real developer who uses **Claude Code or OpenCode** and has hit a
  confusing configuration result.
- **Only Claude Code and OpenCode** are supported. Do not put testers on Codex/Gemini/etc. —
  those adapters do not exist yet.
- Testers run everything **locally**. ConfigLens is offline and read-only.
- Reports are redacted, but a report can still describe project structure: testers must use
  `--paths share` before sharing anything.
- No testers are paid, and no testimonial is requested. Any quote used later must be shown back
  to the tester and approved in writing.

## Consent (read this to each tester before they start)

> ConfigLens is a local, offline, read-only tool. It reads supported Claude Code / OpenCode
> configuration files on your machine to explain which value wins. It does **not** make network
> requests, run your configuration, or read credentials.
>
> For this pilot I'd like you to run the task below and share feedback. Anything you share is
> voluntary. Before sharing any report, run it with `--paths share`, which replaces your paths,
> unknown key names and server names with opaque IDs. Still, review the output yourself and
> remove anything you don't want seen — a report can describe your project layout.
>
> I will not paste your report into a public issue or post without asking you first. You can
> stop at any time and ask me to delete anything you sent.

Record, per tester: handle, date, client+version, consent given (yes/no), and what they agreed
to share. Do **not** record their raw configuration.

## Task script (give verbatim; ~20–30 minutes)

Each step already matches shipped behaviour; nothing here requires a feature that is not
implemented.

1. **Build it (5 min).**
   ```sh
   git clone <private repo URL>   # testers get access from you
   cd configlens
   cargo build --locked --release
   ./target/release/configlens --help
   ```

2. **See the isolated demo (2 min).** No real config is read; safe first run.
   ```sh
   ./demo/demo.sh
   ```
   Ask: *did the `/model` explanation make sense without me explaining it?*

3. **Bring one real puzzle (10 min).** Think of a moment where a setting was ignored or
   unexpectedly changed. Then:
   ```sh
   cd <your real project>
   configlens scan    --client <claude|opencode> --client-version <your version>
   configlens explain <RFC6901 pointer, e.g. /model> --client <claude|opencode> --client-version <your version>
   configlens doctor  --client <claude|opencode> --client-version <your version>
   ```
   Ask: *did it show you which file won, and why? Did it tell you what it could not see?*

4. **Answer the diagnostic questions (5 min).** Did the observed winner match what you expected
   from your client? If not, is it a ConfigLens bug, an unsupported-key gap, or a real
   precedence surprise in the tool?

5. **Share safely (2 min).** Only if you choose to:
   ```sh
   configlens snapshot --client <claude|opencode> --client-version <version> \
     --paths share --output ./configlens-report.json
   ```
   Review it; delete anything you don't want shared; then send it with the feedback template.

6. **Fill the feedback template** below.

### Known limits to warn testers about (so results are interpreted correctly)

- Only Claude Code and OpenCode adapters exist. Anything else is labelled `unsupported`.
- `permissions` list merging is provenance only — **not** an authorization decision.
- Managed/enterprise, remote and MDM state is often unobservable; the tool says so and withholds
  an "effective" value rather than inventing one.
- Windows snapshot ACLs are not implemented; the report says so.

## Redacted feedback template (copy per tester)

```
Tester handle:
Date:
OS + version:            (e.g. macOS 15.2 / Windows 11 / Ubuntu 24.04 / WSL2)
Client + version:        (claude X.Y.Z  or  opencode X.Y.Z)
Pilot consent given:     yes / no

--- The real puzzle ---
What was confusing (1–3 sentences):
Which command(s) you ran:
Pointer(s) investigated:

--- Outcome ---
Did ConfigLens show the winner?            yes / partly / no
Did it explain WHY (locations + reason)?   yes / partly / no
Did it clearly state what it could NOT see? yes / partly / no
Did you solve the puzzle without my help?  yes / no
Time to first useful result:               ___ min

--- Quality ---
Most confusing output (quote, redacted):
Wrong / missing / misleading anything?     (describe; attach redacted report if OK)
Would you use it again next time a setting surprises you?  yes / no / maybe
One thing that would make it obviously useful:

--- Sharing ---
Redacted report attached:  yes / no
OK to quote your feedback (with your handle / anonymized / no)?
```

## How to run the pilot

1. Pick testers (`user-actions.md` B.5), approve invitations (B.6), grant access per your chosen
   model (B.7).
2. Send consent text + task script. Ask each tester to pick a puzzle *before* running anything.
3. Collect only the redacted templates + optional `--paths share` reports.
4. Log every concrete finding (bug vs. doc gap vs. unsupported-key gap) into the triage flow
   (`issue-triage.md`). Never log raw config.
5. Track results against `success-metrics.md`.
6. **Pilot exit criteria:** ≥5 testers complete the script; ≥3 diagnose a real incident
   themselves; every serious correctness/privacy finding either fixed or explicitly documented
   before the repo goes public.
