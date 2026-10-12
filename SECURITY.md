# Security policy

ConfigLens reads other tools' configuration files and prints reports that people may attach to
issues. Its safety properties are the product, so security reports are taken seriously.

> **ConfigLens is pre-release software.** Reports are welcome and taken seriously; there is no bug
> bounty. Use GitHub's private vulnerability reporting where it is offered, open an ordinary issue
> otherwise, and never put real credentials or real configuration into either.

## How to report

Use the first option available to you:

1. **GitHub private vulnerability reporting** — the preferred channel. Open the repository's
   **Security** tab and choose **Report a vulnerability**. The maintainer enables this at
   publication; if you do not see it, use option 2.
2. **An ordinary issue**, with a short factual title (for example
   `redaction: seeded token appears in --paths share URL host`) and a minimal body: the command, the
   ConfigLens version, a **minimal synthetic** input that reproduces it, and expected versus observed
   output.

If you believe something is exploitable, prefer a **minimal description of the class of problem**
over a public proof of concept, so other users are not exposed before a fix exists.

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

Filesystem authorization is path-based. Canonicalization includes parent symlinks, but classification,
opening and directory enumeration are separate operations. ConfigLens does not provide a
descriptor-relative sandbox against concurrent hostile path replacement. Use trusted, stable roots
and a private, user-controlled snapshot destination directory. Snapshot temporary files are created
exclusively, with owner-only mode at creation on Unix. Create-new publication uses a non-replacing
hard link; filesystems without hard-link support fail closed. Explicit overwrite uses rename.
Windows snapshots inherit directory ACLs; restrictive Windows ACLs are not implemented. Atomic
publication is not a guarantee of durability across power loss.

## What to expect

- We will confirm receipt, reproduce, and agree a fix and a disclosure time with you.
- This is a small project run by one maintainer in their own time: there is no guaranteed response
  time and no monetary reward.
- Fixes ship in the next release and are recorded in [`CHANGELOG.md`](CHANGELOG.md). Credit is given
  on request.
