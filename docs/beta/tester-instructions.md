# ConfigLens — what a beta tester is asked to do

This page is written to be forwarded to a tester as-is (or pasted into a message). It is the
tester-facing copy of `pilot-guide.md`, which is the maintainer's version.

## What this is

ConfigLens is a local, offline, read-only command-line tool that explains **which configuration value
wins** for Claude Code and OpenCode — which file supplied it, what overrode it, and what it could not
see. It never makes a network request, never runs your configuration, and never reads credentials.

## What you get, and what it costs you

- Read-only access to a private repository. You can clone and build it; you cannot change it.
- **30–40 minutes** of your time, once. No payment (in either direction), no obligation, no
  testimonial request.
- You do **not** need a Claude Code subscription. If you don't use either client, you can still run
  the synthetic demo and say so in the feedback — that is useful on its own.

## Before you start

1. **Git** and **Rust 1.85 or newer**. If `rustup --version` prints nothing, install Rust from
   https://rustup.rs (one command, no admin rights needed).
2. Which client you use, and its version — `claude --version` or `opencode --version`.
   If you don't use either, tell us and skip step 3.

## The task

**1. Get it running (5 min)**

```sh
git clone https://github.com/ob4cl/configlens.git
cd configlens
cargo build --locked --release
./target/release/configlens --help
```

**2. Watch the isolated demo (2 min)** — this reads none of your files; it runs in a synthetic
sandbox it deletes on exit.

```sh
./demo/demo.sh
```

**3. Bring one real puzzle (10 min)** — the point of this exercise. Think of a moment where one of
your settings was ignored, or changed without you changing it. Then, in that project:

```sh
/path/to/configlens scan   --client <claude|opencode> --client-version <your version>
/path/to/configlens explain <pointer, e.g. /model> --client <claude|opencode> --client-version <your version>
/path/to/configlens doctor --client <claude|opencode> --client-version <your version>
```

**4. Answer three questions (5 min)** — no need to write them up yet, the template catches them:

- Did it show you which file won, and why?
- Did it clearly say what it could **not** see?
- Did that match what you expected from the client — and if not, is that a ConfigLens bug, an
  unsupported key, or a real surprise in the client's precedence?

**5. Optional: attach a redacted report (2 min)** — only if you want to:

```sh
/path/to/configlens snapshot --client <claude|opencode> --client-version <version> \
  --paths share --output ./configlens-report.json
```

`--paths share` replaces your home/project paths, URL hosts, and unknown key names with opaque ids
(`$HOME`, `https://host_1/...`). **Open the file and read it before sending anything** — it can still
describe the shape of your project.

**6. Fill in the feedback template** at the bottom of this file and send it back.

## Things to know so you interpret the output correctly

- Only **Claude Code** and **OpenCode** are supported. Anything else is labelled `unsupported`.
- Permission lists are reported as **provenance, not an authorization decision** — it is not telling
  you what you are allowed to do.
- Managed/enterprise, remote and MDM settings are usually unobservable from your machine, so the tool
  says so and withholds an "effective value" instead of guessing. An `observed` winner plus a
  coverage gap is intended behaviour, not a bug.
- Every rule is currently **fixture-verified**, not verified against the real clients at runtime.
- On Windows, snapshot file ACLs are not implemented; the report states this.

## Privacy and security

- The tool is read-only and offline. Nothing leaves your machine unless you send it.
- Seeded secrets are redacted by default, and `--paths share` pseudonymizes as described above.
- Found an actual security problem (a value that should have been redacted appearing in output, a
  read outside the allowed roots, a panic on crafted input)? Open a **private issue** in the repo and
  mark it `security`. Do not post it publicly.
- We record only: your handle, the date, the client and version, whether you gave consent, and what
  you agreed to share. **Never your raw configuration.**

## What happens next

Your feedback is classified as a bug, a documentation gap, or an unsupported-key gap, and anything
serious is fixed before the project is published. Credit is given on request. You can stop at any
time and ask for anything you sent to be deleted.

---

## Redacted feedback template

```
Tester handle:
Date:
OS + version:            (e.g. macOS 15.2 / Windows 11 / Ubuntu 24.04 / WSL2)
Client + version:        (claude X.Y.Z  or  opencode X.Y.Z)
Pilot consent given:     yes / no

--- The real puzzle ---
What was confusing (1-3 sentences):
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
