# Security policy

ConfigLens reads other tools' configuration files and prints reports that people may attach to
issues. Its safety properties are the product, so security reports are taken seriously.

> **This repository is private (private beta).** That changes how you report, not whether you should:
> everything below assumes the report stays inside this repository until it is fixed. There is **no
> public vulnerability intake channel yet**, and there is no bug bounty. GitHub private
> vulnerability reporting can only be enabled on public repositories, so it will be switched on when
> the beta is published; until then, reporting means a private issue in this repository.

## How to report

Because the repository is private, anyone with access is already in a restricted channel. Choose the
first option that is available to you:

1. **GitHub private vulnerability reporting.** Open the repository's **Security** tab and use
   **Report a vulnerability** *if that option is present*. It is a per-repository setting and is not
   guaranteed to be enabled; if you do not see it, use option 2.
2. **A normal issue in this repository.** Since the repository is private, issues are visible only to
   maintainers and invited beta testers. Open an issue with a short, factual title (for example
   `redaction: seeded token appears in --paths share URL host`) and keep the body minimal:
   the command, the ConfigLens version, a **minimal synthetic** input that reproduces it, and
   expected versus observed output.

If neither option is available to you — you are outside the beta and have no repository access —
then this project has no way to receive your report yet. Please **do not disclose it publicly**;
an intake channel will be documented when the beta opens.

Do **not** put real credentials, real configuration dumps or third-party secrets in a report.
Reduce every example to obviously fake, synthetic values. If a value in your reproduction is
sensitive, replace it before pasting it — ConfigLens is about *structure*, so a fake value reproduces
the same behaviour.

## What counts as a security issue

In scope:

- **Redaction bypass** — a secret value, secret field name, URL userinfo/query/fragment or unknown
  object key reaching any output channel (stdout, stderr, JSON report, snapshot or diff).
- **Read-policy escape** — reading a file outside the authorised roots, following a symlink out of
  a root, or reading a device/socket/FIFO.
- **Resource-limit bypass** — unbounded read, unbounded nesting, or a limit that silently truncates
  instead of reporting.
- **Terminal-injection** — a crafted path, key or value producing control sequences, OSC hyperlinks,
  bidi overrides or carriage returns in output.
- **Unexpected capability** — any code path that spawns a process, opens a socket, or otherwise
  breaks the offline/read-only contract.
- **Panic or denial of service** on crafted input (including in the parser or JSON Pointer handling).

Out of scope (by design, not a vulnerability):

- A `partial` result, an *observed* winner, or a withheld effective value. That is the documented
  behaviour: ConfigLens never assumes an unreadable layer is absent. See the "bounded semantics"
  section of the [`README`](README.md).
- Permission lists being reported as a provenance merge rather than an authorization decision —
  ConfigLens explicitly does not evaluate authorization.
- Clients or keys that are unsupported; those are labelled, not claimed.
- The documented pseudonymization itself: in `--paths share`, URL **hosts** are replaced with opaque
  ids (`https://host_1/mcp?[redacted]`), user info, query and fragment are stripped, and paths become
  `$HOME`/`$PROJECT` aliases. That is intended output, not a leak. Any *other* identifying value
  reaching any channel is in scope.

## Threat model, briefly

ConfigLens treats a malicious repository configuration as untrusted input: hostile JSON/JSONC, huge
files, symlink loops, deceptive Unicode, and keys crafted to look like secrets. It is a parser, not
an agent executor — it never runs your configuration, your hooks or your plugins. The full model is
in the technical specification
([`docs/spec/ConfigLens_Technical_Specification.md`](docs/spec/ConfigLens_Technical_Specification.md),
section 7).

## What to expect

- We will confirm receipt, reproduce, and agree a fix and a disclosure time with you.
- This is a small private-beta project run by volunteers: there is no guaranteed response time and
  no monetary reward.
- Because the repository is private and there is no release yet, fixes ship to the checkout and are
  recorded in [`CHANGELOG.md`](CHANGELOG.md); a public advisory process will be defined when the
  project is published. Credit is given on request.
