# ConfigLens v0.1.0-beta.3 — Windows regression plan (PowerShell)

Copy-paste plan for a Windows machine. Reproduces the beta.1/beta.2 findings, confirms the
`OPENCODE_CONFIG_DIR` correction and the ancestor-merge fix, and exercises the beta.3 `--quiet` flag
and exit-code handling. Written before the beta.3 candidate exists; run it once the candidate binary
is approved and built.

Paths are expressed with `$env:USERPROFILE` so nothing here is tied to one machine — substitute your
own folder names if you like, but the two test folders should keep the names used below.

```powershell
$test  = "$env:USERPROFILE\Desktop\configlens-test"          # project
$cfgd  = "$env:USERPROFILE\Desktop\configlens-test-global"   # custom configuration directory
```

## 0. Prerequisites (unchanged from beta.1)

- OpenCode `1.18.35`, Node `v24.20.0`, npm `11.19.0`.
- The two folders above, unchanged from the earlier test.

Project file `$test\opencode.json`:

```json
{"$schema":"https://opencode.ai/config.json","model":"opencode/test-model"}
```

Custom-directory file `$cfgd\opencode.json`:

```json
{"model":"opencode/global-model"}
```

## 1. Unpack, verify and start

```powershell
$ErrorActionPreference = 'Stop'
$zip = "$env:USERPROFILE\Downloads\configlens-v0.1.0-beta.3-x86_64-pc-windows-msvc.zip"
$dst = "$env:USERPROFILE\Desktop\configlens-beta3"
New-Item -ItemType Directory -Force -Path $dst | Out-Null
Expand-Archive -Path $zip -DestinationPath $dst -Force
$exe = (Get-ChildItem -Path $dst -Recurse -Filter configlens.exe | Select-Object -First 1).FullName
$exe

# Verify the download against the release's SHA256SUMS (expected digest printed there):
Get-FileHash -Algorithm SHA256 $zip | Format-List

# 1. The executable starts.
& $exe --version
```

Expect: `configlens 0.1.0-beta.3`.

## 1b. Exit codes, output capture and `--quiet` (new in beta.3)

PowerShell treats a native command's **stderr** output as error-stream content. With
`$ErrorActionPreference = 'Stop'` (set in §1), the *informational* note ConfigLens writes on
stderr — for example `configlens: wrote 1234 bytes ...` — can surface as a `NativeCommandError` and
abort a script even though ConfigLens succeeded. That note is **informational only**; the process
**exit code** is the contract (`0`/`1`/`2`/`3`/`4`, see the README's exit-code table). Capture stdout
and branch on `$LASTEXITCODE`:

```powershell
# Capture stdout; discard the informational stderr note; fail on a non-zero exit code.
$out = & $exe scan --client opencode --client-version 1.18.35 2>$null
if ($LASTEXITCODE -ne 0) { Write-Error "configlens scan failed: exit $LASTEXITCODE" }
$out

# Machine-readable output: JSON on stdout, and --quiet so no informational note is written at all.
$json = & $exe doctor --client opencode --client-version 1.18.35 --format json --quiet
if ($LASTEXITCODE -ne 0) { Write-Error "configlens doctor failed: exit $LASTEXITCODE" }
$json
```

Two safe patterns — use either:

- `2>$null` (or `2>&1` to keep the note) **plus** a `$LASTEXITCODE` check: the note is
  discarded/kept and the exit code is what you branch on.
- `--quiet`: the informational note is never written, so there is nothing for PowerShell to render as
  an error. Errors are **always** printed to stderr regardless, and the exit code still changes, so
  `--quiet` does not hide failures.

Branch on `$LASTEXITCODE`, never on the mere presence of stderr text: `2>$null` hides only the
informational note, a genuine failure still writes to stderr *and* still sets `$LASTEXITCODE` (and,
for `--format json`, prints the JSON error envelope on stdout).

## 2. Supported-client metadata

```powershell
& $exe clients
```

Expect: `opencode revision 0.1.0 tested 1.18.35 ... conformance fixture_verified_max` and
`claude ... conformance fixture_verified_max`, plus a list of explicitly unsupported clients. No
supported client may claim more than `fixture_verified`.

## 3. Scan the test project (no custom directory)

```powershell
Set-Location $test
Remove-Item Env:OPENCODE_CONFIG_DIR -ErrorAction SilentlyContinue
& $exe scan --client opencode --client-version 1.18.35
& $exe explain /model --client opencode --client-version 1.18.35
```

Expect: `/model = "opencode/test-model"`, winner from the project `opencode.json`; `--paths` local;
coverage gaps listed. Three are always incomplete (`oc.remote.wellknown`, `oc.opencode_dirs`,
`oc.managed_macos_prefs`). Because the Desktop test folder has no `.git`, expect a **fourth** gap,
`run — project_root_uncertain (project_boundary_uncertain)`: with no repository boundary ConfigLens
deliberately reads the working directory only and now says so instead of staying silent.

## 3b. New in beta.2 — every ancestor project config is merged

ConfigLens now reads **every** `opencode.json`/`opencode.jsonc` from the working directory up to the
repository root, merging outermost-first so the **nearest** file wins — matching OpenCode. To see it:

```powershell
$r = "$env:USERPROFILE\Desktop\configlens-ancestor"
New-Item -ItemType Directory -Force -Path "$r\repo\sub" | Out-Null
New-Item -ItemType Directory -Force -Path "$r\repo\.git" | Out-Null
@'{"model":"repo-model","small_model":"repo-small"}'@ | Set-Content -Encoding utf8 "$r\repo\opencode.json"
@'{"model":"sub-model","small_model":"sub-small"}'@  | Set-Content -Encoding utf8 "$r\repo\sub\opencode.json"
Set-Location "$r\repo\sub"
& $exe scan    --client opencode --client-version 1.18.35 --allow-read-root "$r\repo"
& $exe explain /small_model --client opencode --client-version 1.18.35 --allow-read-root "$r\repo"
```

Expect: `scan` lists **two** `[30] oc.project` sources, and `/small_model = "sub-small"` with
`"repo-small"` shown as a loser (`replaced_by_later_layer`). Both must be correct — in beta.1 this
command answered `"repo-small"`, the opposite of OpenCode.

## 4–5. Correct usage: `OPENCODE_CONFIG_DIR` is a *directory*

```powershell
$env:OPENCODE_CONFIG_DIR = $cfgd
& $exe scan    --client opencode --client-version 1.18.35 --allow-read-root $cfgd
& $exe explain /model --client opencode --client-version 1.18.35 --allow-read-root $cfgd
```

Expect:

- `scan` lists an `oc.custom_dir` source at rank **45** pointing at `…\configlens-test-global\opencode.json`,
  marked `observed` / `applicable`.
- `explain /model` → `/model = "opencode/global-model"` (the custom directory outranks the project).
- Losers list `"opencode/test-model"` from the project file as `replaced_by_later_layer`.
- **No** `not_a_regular_file` diagnostic (the beta.1 bug). No `custom_dir_placement_unverified` gap.

## 6. Incorrect usage: `OPENCODE_CONFIG_DIR` points at a *file*

```powershell
$env:OPENCODE_CONFIG_DIR = "$cfgd\opencode.json"
& $exe scan    --client opencode --client-version 1.18.35 --allow-read-root $cfgd
& $exe explain /model --client opencode --client-version 1.18.35 --allow-read-root $cfgd
```

Expect: **no** `oc.custom_dir` source; `/model = "opencode/test-model"`. The file is not read as a
custom-directory configuration (upstream would look for `<file>\opencode.json`, which does not
exist). This is the second half of the beta.1 bug.

## 7. JSONC precedence inside the custom directory

```powershell
'{"model":"opencode/global-model-jsonc"}' | Set-Content -Encoding utf8 "$cfgd\opencode.jsonc"
& $exe explain /model --client opencode --client-version 1.18.35 --allow-read-root $cfgd
```

(Re-set `$env:OPENCODE_CONFIG_DIR = $cfgd` first if step 6 left it pointing at the file.)

Expect: `/model = "opencode/global-model-jsonc"` — `.jsonc` wins because upstream merges it second,
and both files inside the directory are listed by `scan`.

```powershell
Remove-Item "$cfgd\opencode.jsonc"
```

## 8–9. Diagnostics, privacy and redaction

```powershell
& $exe doctor --client opencode --client-version 1.18.35 --format json --strict | Out-File -Encoding utf8 doctor.json
& $exe support-report --client opencode --client-version 1.18.35 --preview
```

Expect, in the support report: no absolute user-profile paths (they are projected), no
credential-looking values, a `review before attaching` reminder, and no file written by `--preview`.
Confirm `--paths local` is ignored by `support-report` (it is always share mode).

## 10. Snapshot and diff

```powershell
& $exe snapshot --client opencode --client-version 1.18.35 --output before.json
& $exe snapshot --client opencode --client-version 1.18.35 --output after.json
& $exe diff before.json after.json
```

Expect: `before.json` created (create-new; a second run without `--overwrite` must fail), and
`diff` reporting no changes for two identical snapshots.

## 11. Incomplete coverage stays disclosed

```powershell
& $exe scan --client opencode --client-version 1.18.35
& $exe explain /model --client opencode --client-version 1.18.35
```

Expect: coverage gaps for `oc.remote.wellknown`, `oc.opencode_dirs` and `oc.managed_macos_prefs`;
the conclusion says the winner is *observed* and the **effective value is withheld**.

## Cleanup

```powershell
Remove-Item Env:OPENCODE_CONFIG_DIR -ErrorAction SilentlyContinue
```

## Fixed in beta.2

Subdirectory discovery was wrong in beta.1: if you pointed ConfigLens at a **subdirectory** of a
repository whose *root* also had an `opencode.json`, it read only one project file and could name the
repository root's value where OpenCode prefers the nearer file. It now merges every ancestor
`opencode.json`/`opencode.jsonc` from the working directory up to the repository boundary with the
nearest winning (see section 3b above, and
`docs/research/project-config-discovery-finding-20261011.md` for the evidence).
