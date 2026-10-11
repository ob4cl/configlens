# Issue triage checklist

For every issue that arrives during the private beta and after launch. First rule: **privacy
before diagnosis.** Never ask a reporter to paste a raw config file, secret or credential.

## 1. Protect the reporter first

- [ ] If a raw config or unredacted report was attached: stop, ask the reporter to delete it from
      the thread, and re-share using `configlens snapshot … --paths share`.
- [ ] Remind them that reports can still describe project structure; ask them to review before
      sending.
- [ ] If a real secret may have leaked into a public thread, advise rotating it immediately.
- [ ] For suspected secrets in an issue, edit/hide the content and note that you did.

## 2. Classify

Pick exactly one primary type:

| Type | Meaning |
|---|---|
| Security | Network/subprocess/traversal/redaction failure, or a secret reaching any output channel. |
| Privacy-leak | A value that should be redacted appearing in output (incl. the known URL-host gap). |
| Correctness | Wrong winner, wrong provenance, wrong status/exit code, wrong rule. |
| Coverage | Tool claims more certainty than the available sources justify. |
| UX / docs | Confusing output, missing guidance, README error. |
| Feature request | New client/key/rule or capability. |
| Unsupported | Out of scope for 0.1 (Codex, instructions assembly, permission evaluation…). |
| Question | Support request. |

## 3. Severity

- **S1 — security/privacy or silent wrong answer.** Blocks release. Fix or document loudly.
- **S2 — correctness with an honest status.** No false "effective" value but a wrong detail.
- **S3 — UX/docs/coverage wording.**
- **S4 — feature request / nice-to-have.**

## 4. Reproduce with synthetic data

- [ ] Build a **synthetic** fixture reproducing the report; never use the reporter's real files.
- [ ] Capture the exact command, `--client`, `--client-version`, pointer and observed output.
- [ ] Map the pointer to a rule ID in `compatibility/{claude,opencode}/registry.md`.
- [ ] Decide: does the rule's stated support level match what it did? If not, this is an S1/S2.

## 5. Security issues

- [ ] Route **S1 security/privacy** issues to the private disclosure channel in `SECURITY.md`
      (once it exists — a `user-actions.md` decision). Do not discuss exploit details publicly.
- [ ] Never "fix" a leak by loosening redaction; only stricter.

## 6. Label and record

- [ ] Labels: type + severity + area (`adapter/claude`, `adapter/opencode`, `security`, `docs`).
- [ ] Note the decision: fix now / fix before release / document as a limitation / decline.
- [ ] If declined, state the reason and the support label the tool should honestly carry.

## 7. Response (short, honest)

Templates:

- **Needs a redacted repro:** "Thanks — to debug this I only need a redacted report, not your
  real config. Could you run `configlens snapshot --client <c> --client-version <v> --paths share
  --output report.json`, review it, and attach it?"
- **Unsupported:** "Codex isn't implemented yet (0.2 roadmap). `configlens clients` lists it as
  unsupported on purpose — I'd rather show a gap than invent a rule."
- **Declined:** "I'm not modelling this in 0.1 because the precedence isn't provable from primary
  evidence. It stays `inventory_only` rather than guessing."

## 8. Close

- [ ] Fix has a regression test (positive + negative) or the limitation is documented in
      `docs/implementation-status.md`.
- [ ] CHANGELOG entry if user-visible.
- [ ] Reporter told what changed and what didn't.

## 9. Anti-patterns

- Never reproduce against the reporter's real `$HOME`.
- Never request screenshots of raw settings instead of a redacted report.
- Never triage "it should probably be X" into a rule without primary evidence and tests.
- Never close an S1 without either a fix or a bold, visible limitation note.
