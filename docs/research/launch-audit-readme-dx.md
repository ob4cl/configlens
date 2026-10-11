# ConfigLens — public-launch README & developer-experience audit

- Repo: <repo path redacted> · branch `main` · HEAD `e746957` · clean tree · private, three private prereleases.
- Executed: Sun 11 Oct 2026, WSL/Linux x86_64, cargo/rustc 1.99.0 (default `stable`). All test runs used isolated temp `HOME` and temp `--cwd`; the real `~/.config` and `~/.claude` were never touched.
- Evidence tags: **[C]** = CONFIRMED (a command I ran or a file:line I read). **[R]** = RECOMMENDATION.

---

## 0. What the tool is and what actually runs today

**[C]** `cargo build --locked --release` exits `0`. A clean build in an isolated `CARGO_TARGET_DIR` compiled 14 crates and finished in `1m 29s`; the repo's own `target/` was left untouched (timestamp unchanged at 18:02).

**[C]** `./target/release/configlens --version` -> `configlens 0.1.0-beta.3` (matches `Cargo.toml`, the tag, and the report `tool_version`).

**[C]** `./demo/demo.sh` exits `0`; runs fully isolated (synthetic sandbox under the temp dir).

---

## 1. README as a public front door

The README answers most required questions, but the ordering is wrong for a first-time visitor. What is present:

| Question | Where | Verdict |
|---|---|---|
| What it does | `README.md:5` `**Find out why your AI coding tool is ignoring your configuration.**` then `README.md:7-9` | [C] Clear, strong one-liner. |
| Why a developer uses it | `README.md:23-36` "The question this answers" | [C] Good, concrete. |
| Supported clients | `README.md:8` Claude Code + OpenCode; matrix `README.md:162-165` | [C] Present. |
| Explicitly unsupported | `README.md:171-178` (Codex, Gemini, Aider, Cursor, Copilot CLI, Cline, Roo, Hermes) | [C] Present; `configlens clients` confirms the same list verbatim. |
| Install | `README.md:38-52` "Quick start (from source)" | [C] Source only. |
| First useful command | `README.md:45` `./target/release/configlens --help` | [C] Only `--help`; no runnable example in Quick start. |
| What output means | `README.md:66-89` "A real terminal example" + line-by-line reading | [C] Strong. |
| Limited/experimental features | `README.md:14-21`, `:138-153`, `:194-198` | [C] Present, but scattered/buried. |

**[C] Jargon before explanation (the main front-door problem).** The first prose after the title is a dense private-beta block, `README.md:14-21`, which opens with terms never yet defined: "working prototype", `tool_version`, "SBOM root component", `schema_version`, `adapter_revision`. The reader has not run anything and cannot attach meaning to these. Similarly `observed`, `fixture_verified`, `coverage gaps` and `partial` are first *used* at `README.md:78-89` and only *defined* at `README.md:138-153` and `:157-160`.

**[C] Limitations are below the fold.** The only platform-specific limitation (Windows snapshot ACLs not implemented) sits at `README.md:194-198`, inside "Privacy and caveats", ~190 lines down. The Windows guidance (`Get-FileHash`, `%USERPROFILE%`) appears only in `docs/beta/windows-regression-plan.md:44-48`, not in the README or getting-started.

**[R]** Add a 4-line "what / why / supported / install" strip immediately after the title, and demote the beta mechanics to a short `## Status` section further down.
**[R]** Add a 3-line glossary (`observed`, `fixture_verified`, `coverage gap`) before the first use at `README.md:78`.
**[R]** Put the first *runnable* command with expected output in Quick start, not just `--help`.

---

## 2. Installation & DX — Windows, Linux, macOS

**[C] Distribution reality (confirmed by search, not assumed).** No `Formula/`, no Scoop manifest, no `install.sh`, no `.nsi`/`.wxs`/winget/Dockerfile exists anywhere in the tree. No crates.io package: `docs/beta/milestone-checklist.md:99` records "not published". Getting-started confirms it: `docs/getting-started.md:26` "There is no published crate or prebuilt binary yet. Build from the checkout."

**[C] No release artifacts exist yet.** The only release workflow is `prerelease.yml` (manual `workflow_dispatch`). `docs/beta/release-runbook.md:65` states it "has **never run**"; `:62` "There is **no smoke-test step**". `docs/beta/milestone-checklist.md:94-95` lists "R2 Tagged source release | none" and "R3 Binary targets | none built for release". So **today every platform is build-from-source only.**

**[C] What the (future) release archives will be** — `.github/workflows/prerelease.yml:70-76`:
- Linux `x86_64-unknown-linux-gnu` (glibc)
- macOS `aarch64-apple-darwin` (Apple Silicon only)
- Windows `x86_64-pc-windows-msvc`

Archive formats/names from `scripts/package_release.py:11-13,59,66`: `configlens-<tag>-<target>.tar.gz` (Linux/macOS), `.zip` (Windows), binary `configlens.exe` on Windows, plus `SHA256SUMS` and a CycloneDX SBOM (`prerelease.yml:182-194`).

### Per platform, what a user can do today
| Platform | Today | Missing |
|---|---|---|
| Linux x86_64 (glibc) | Build from source: `cargo build --locked --release` (verified [C]). | No downloadable archive, no aarch64/musl targets, no package. |
| Linux arm64 / musl | Build from source only (untested upstream). | No release target. |
| macOS Apple Silicon | Build from source only. | Archive defined but never produced; no Homebrew formula. |
| macOS Intel (x86_64) | Build from source only. | **No release target at all** (`prerelease.yml:74` builds only `aarch64-apple-darwin`). |
| Windows x86_64 | Build from source only. | No archive (the plan assumes one: `docs/beta/windows-regression-plan.md:37` downloads `configlens-v0.1.0-beta.3-x86_64-pc-windows-msvc.zip`, which does not exist), no Scoop/winget, no installer. |

**[C] Copy-paste stumble.** `docs/getting-started.md:15` says `git clone <this repository>` — a literal placeholder that fails if pasted. The real URL is only in `docs/beta/tester-instructions.md:32` (`https://github.com/ob4cl/configlens.git`).

**[R]** State explicitly in the README: "No binary is published yet; all platforms build from source with Rust >= 1.85." Add the intended-distribution table above so Intel-Mac and Windows users are not surprised.
**[R]** Replace the `<this repository>` placeholder in getting-started with the real clone command.

---

## 3. Behaviour actually tested (real output excerpts)

### Help surface (all exit `0`)
`configlens --help` prints a one-line description, the command list, and **8 worked examples**; every subcommand `--help` exits `0`. `scan`/`explain`/`support-report --help` list all shared options with descriptions.

### Completions (`configlens completions --shell ...`)
- `bash` -> 1680 bytes, ends `complete -F _configlens configlens`, exit `0`.
- `zsh` -> 1073 bytes, starts `#compdef configlens`, exit `0`.
- `fish` -> 1801 bytes, `complete -c configlens ...` lines, exit `0`.
- `--shell powershell` -> `error: invalid value 'powershell' ... [possible values: bash, zsh, fish]`, exit `2`; missing `--shell` -> exit `2`. All plausible.

### Exit-code contract (README `README.md:126-136`) — all five reachable
| Code | Trigger I ran | Observed |
|---|---|---|
| `0` | `scan`/`explain`/`clients`/`schema`/`snapshot`/`support-report`/`diff` success | [C] exits `0`. |
| `1` | `doctor ... --fail-on warning` (a warning present, no error) | [C] `ConfigLens doctor — 0 error(s), 1 warning(s) ...` exit `1`. With default `--fail-on error` the same run exits `0`. |
| `2` | `--client bogus`; missing `--client`; `--shell powershell`; `--kind bogus`; snapshot to an existing file without `--overwrite`; `support-report` with neither flag; malformed `--input-context` | [C] exit `2`. |
| `3` | `--client codex` (known-unimplemented); `--strict` on an incomplete context | [C] exit `3`. |
| `4` | malformed `settings.json`; missing/unreadable required file; unreadable `--input-context` | [C] exit `4`. |

**[C] Documented priority `2 -> 4 -> 3 -> 1 -> 0` holds:** a malformed file combined with `--strict` returns `4`, not `3`.

### Empty / unreadable client config (`--cwd` = empty temp dir)
**[C] No crash; the gap is explained.** `scan` exits `0` and lists **7 coverage gaps** (`client_version_not_tested_or_documented`, `project_trust_unknown`, `cc.managed.hkcu/mdm/server`, `project_root_uncertain`, `cc.managed.file`). `explain /model` exits `0`:
```
/model = <redacted:absent> (observed)
Coverage: complete
Conclusion: no observed source defines this key.
Next inspection:
  - no observed source defines this key; confirm the file you expect to edit is under $PROJECT or $HOME
```
**[C] Naming a non-existent `--cwd` does not error** — it behaves as an empty project (exit `0`, same 7 gaps). Reasonable, though silent.

### Snapshot / support-report round-trip
**[C]** `snapshot` writes `0600` (verified `stat` = `600`) and prints `wrote N bytes ... (permissions: owner_only_0600)`. Second run without `--overwrite` -> `usage: the output file already exists; pass --overwrite to replace it` exit `2`. `diff` of two identical snapshots reports no differences; protected values compare as `unknown_redacted`. `support-report --output` -> `0600` + "Review it before attaching it anywhere."

---

## 4. CLI help & error-message quality

**[C] Help is actionable and leaks nothing.** `configlens --help` and every subcommand help contain no absolute paths, no hostnames, and no usernames.

**[C] DEFECT — some fatal errors print the code twice and drop the detail.** `src/error.rs:73` renders `"{code}: {message}"`, and `file_problem()` sets `message = code` (`src/error.rs:119-121`); `run()` prints it at `src/cli.rs:346`. Observed, verbatim:
```
configlens: context_unreadable: context_unreadable        (exit 4)
configlens: snapshot_write_failed: snapshot_write_failed  (exit 4)
configlens: report_unreadable: report_unreadable          (exit 4)
```
The useful detail exists but is never shown: `commands.rs:52-53` builds `"{temp_path} ({os_error})"` and `cli.rs:300` passes the real path — both land in the `location` field, which the text renderer ignores and which no JSON envelope prints (see next). A user who points `--input-context` at a directory learns nothing about which path failed or why.

**[C] Two error styles are inconsistent.** Some are good and actionable:
```
configlens: incompatible_input: context file is not valid JSON: expected ident at line 1 column 2   (exit 2)
configlens: usage: unknown client; supported values are opencode and claude                          (exit 2)
```
Others are the code-duplicated stubs above.

**[C] Documented claim contradicted by reality.** `docs/beta/windows-regression-plan.md:83-84` says a genuine failure "for `--format json`, prints the JSON error envelope on stdout". Verified false: for every fatal error (`usage`, `incompatible_input`, `file_problem`) **stdout is empty even with `--format json`** — only the stderr text line is emitted. (`--strict` exit `3` is different: that *does* print a report on stdout, but its `"errors": []` array is empty, so a JSON consumer sees a normal report with no machine-readable reason for the non-zero code.)

**[C]** `--client <CLIENT>` in help has no `[possible values: ...]` (`scan --help`), so the two valid clients are not discoverable from help; you must run `clients` or trip the exit-2 error.

**[R]** Print `location` in the stderr line (or add it to a one-line human message); give `context_unreadable`/`snapshot_write_failed`/`report_unreadable` a real sentence. **[R]** Fix or delete the `windows-regression-plan.md:83-84` JSON-envelope claim. **[R]** Add `[possible values: opencode, claude]` to `--client`.

---

## 5. Safe handling of sensitive configurations

**[C] Nothing documented encourages pasting a real config; several places warn against it.**
- `README.md:199` — "Reports still describe your project structure. Never paste raw credential files into an issue."
- `docs/beta/tester-instructions.md:69` — "**Open the file and read it before sending anything** — it can still describe the shape of your project."
- `docs/beta/tester-instructions.md:88,93` — "Seeded secrets are redacted by default" / "**Never your raw configuration.**"
- `SECURITY.md:30-33` — "Do **not** put real credentials... Reduce every example to obviously fake, synthetic values."
- `SECURITY.md:59-62` documents the intended pseudonymization and defines *other* leaks as in scope.

**[C] support-report/snapshot guidance is correct and verified.** `support-report` is always share mode (`README.md:106`; confirmed: `"paths": "share"` regardless of `--paths`). I seeded `{"model":"mymodel","apiKey":"sk-SUPERSECRET123"}` and ran `support-report --preview`:
- No absolute path, username, or hostname appeared anywhere in the output (grepped for the temp home dir, `whoami`, `hostname`). Paths render as `$PROJECT`/`$HOME`; the model value is shown only because it is on the adapter allowlist (`verified_model_name`); the API key is redacted `{"redacted": true, "reason": "sensitive_field"}`.
- The report is honest about what *is* disclosed: `"snapshot_acl": "owner_only_0600"`, and the run context records `"os": "wsl"` and `"environment_keys"` (key names only, not values).

**[C] Even `--paths local` does not leak the absolute home path** — a config in `$HOME` rendered as `$HOME/.claude/settings.json` (not `/home/<user>/...`). README `:186-188` describes local vs share slightly imprecisely: local shows *real key names and aliased file locations*; it does not expose absolute paths.

**[R]** (Minor) With **both** `--preview` and `--output` given, the tool silently ignores `--output`, prints the preview, and says "preview only — nothing was written. Add --output <path>". `README.md:123` implies these are alternatives but does not say they are mutually exclusive; make both-given a usage error or document the precedence.

---

## Prioritised list for public launch

**BLOCKER**
1. **[C]** No install path exists for anyone: no crate, no Homebrew/Scoop, no installer, and the only release workflow has **never run** (`release-runbook.md:65`), so no archive exists for any platform (`milestone-checklist.md:94-95`). The README already says this (`README.md:15,40`), but a public launch cannot ship "clone the private repo and build". Produce at least one signed release with `SHA256SUMS`, or move the launch date. [R] Ship the archives + `sha256sum -c SHA256SUMS` block the runbook already drafts (`release-runbook.md:80-83`).
2. **[C]** README leads with an unexplained beta block (`README.md:14-21`) full of undefined jargon before the reader knows what the tool is or does. This is the front door; fix the ordering. [R] see §1.

**IMPORTANT**
3. **[C]** Fatal-error messages print the code twice and drop the path/cause — `configlens: context_unreadable: context_unreadable` (`src/error.rs:73,119-121`; `src/cli.rs:346`). [R] surface `location`.
4. **[C]** Docs claim `--format json` emits a JSON error envelope on stdout (`windows-regression-plan.md:83-84`); verified false for all fatal errors. [R] fix the doc (and consider emitting the envelope).
5. **[C]** `docs/getting-started.md:15` has a literal `git clone <this repository>` placeholder; copy-paste fails. [R] use the real URL.
6. **[C]** README/gSG list no macOS-Intel or arm64-Linux guidance while the release matrix is Apple-Silicon-only and x86_64-Linux-only (`prerelease.yml:70-76`). [R] add a per-platform "today / planned" table.
7. **[C]** `--client` help omits the valid values; discoverability relies on an error or `clients`.

**POLISH**
8. **[C]** `explain` on a source-less context prints `Coverage: complete` yet `--strict` exits `3` on the same run (run-level gaps exist in the JSON `coverage` array). [R] show the run-level gaps or say "per-key coverage complete; context incomplete".
9. **[C]** Both-given `--preview --output` silently ignores `--output` (§5). [R] error or document.
10. **[C]** `README.md:123` parenthetical "(neither is a usage error)" about `support-report` reads ambiguously; the tool *does* error when neither flag is given (exit `2`). [R] reword.
11. **[C]** Windows limitation + `Get-FileHash` verify steps live only under "Privacy and caveats" (`README.md:194-198`) and in `windows-regression-plan.md:44-48`. [R] surface a short Windows note near install.
12. **[C]** A non-existent `--cwd` is silently treated as an empty project (exit `0`). [R] warn when the cwd does not exist.

**Verified-good (no action):** exit-code contract incl. the `2->4->3->1->0` priority; offline/read-only + redaction behaviour; support-report/snapshot 0600 perms and "review before attaching" reminder; completions for bash/zsh/fish; no path/host/user leaks in help, errors, local or share output.
