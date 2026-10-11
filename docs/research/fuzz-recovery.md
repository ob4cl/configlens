# Fuzz recovery: `hostile_inputs` URL-userinfo leak + oracle audit

Scope of this task (as handed off): own `fuzz/` and `tests/security_leaks.rs`, plus this report.
Production `src/` edits were permitted only if a **new** genuine bug was reproduced; see
[Production code](#production-code-no-new-bug-the-existing-fix-is-sufficient).

Repo: `$REPO` · HEAD `35ad132fc1190124f30d609ff1d1afaa659691db` (dirty tree preserved).
Toolchain: `rustc 1.99.0` (stable) for the crate, `cargo 1.101.0-nightly` for `cargo-fuzz 0.13.2`.

## Summary

1. The saved crash `fuzz/artifacts/hostile_inputs/crash-6ac24e86f9105163905201b031b3bc1ae4ddb070`
   (`user:passtool_version.ex{/b`) is genuinely reproduced by the pre-fix sanitizer and is **fixed**
   by the URL change already present in the working tree. I did **not** touch `src/security/redact.rs`
   — the existing fix is sufficient and no new production bug was found.
2. A pipeline regression was added to `tests/security_leaks.rs` that replays the exact artifact
   through the real CLI (both clients, both path modes, both formats). It is RED pre-fix and GREEN
   post-fix.
3. The `hostile_inputs` **oracle had two unsound assumptions**, both demonstrated empirically and
   both corrected:
   - arbitrary input text can itself spell a sentinel marker and reach an intentionally Local-visible
     key name / permission rule, producing a **false positive**;
   - the JSON control-character check searched serialized text, which cannot tell a decoded ESC from
     an escaped literal (`\u001b`), and never matches bidi / zero-width characters because
     `serde_json` writes those as raw UTF-8 — a **false positive and a false negative** at once.
4. With the oracle corrected, all three targets run clean for 120 s each (`fuzz/results/session-4.log`).
   The corrected oracle is strictly stronger: built against the pre-fix source it still catches the
   real leak (and additionally catches a real pre-fix pointer-display control leak).

## 1. The bug

### Evidence (saved)

`fuzz/results/session-2.log` (11:44, 120 s run):

```
thread '<unnamed>' panicked at fuzz_targets/hostile_inputs.rs:184:9:
URL userinfo/query/fragment leaked into the Local JSON report: the "ZUINFO" form of the injected text is present
...
MS: 1 PersAutoDict- DE: "tool_version"-; base unit: 0ced13fe586d555d9d34b7a35a2256ee16515e44
0x75,0x73,0x65,0x72,0x3a,...
user:passtool_version.ex{/b
artifact_prefix=...; Test unit written to fuzz/artifacts/hostile_inputs/crash-6ac24e86f9105163905201b031b3bc1ae4ddb070
Base64: dXNlcjpwYXNzdG9vbF92ZXJzaW9uLmV4ey9i
stat::number_of_executed_units: 82
stat::peak_rss_mb: 406
exit_status=1
```

Artifact: 27 bytes, `user:passtool_version.ex{/b` (`fuzz/results/hostile_inputs-crash-6ac24e86f9105163905201b031b3bc1ae4ddb070`,
recorded here because `/fuzz/artifacts` is gitignored).

### Root cause

`hostile_inputs` builds, for both clients, a document whose MCP `url` is
`https://ZUINFO{<text>}:pw@host.example/mcp?token=ZUINFO{<text>}#ZUINFO{<text>}`. The pre-fix
`sanitize_url` split the authority at the **first** `/`, `?` or `#` and then stripped userinfo with
`rsplit_once('@')`:

```rust
let (authority, tail) = match rest.find(['/', '?', '#']) { Some(i) => (&rest[..i], &rest[i..]), None => (rest, "") };
let host = match authority.rsplit_once('@') { Some((_, host)) => host, None => authority };
```

The `/` inside the value's userinfo (`…ex{/b…`) ends the authority **before** the `@`, so the
credentials are no longer recognised as userinfo and the truncated authority is published as the
host — and the `:pw@host` tail is published as the path. The parse is therefore steerable by the
value it is parsing, which is exactly what a sanitizer must not allow.

For the artifact the pre-fix sanitizer returns
`Some("https://ZUINFOuser:passtool_version.ex{/b:pw@host.example/mcp?[redacted]")` — userinfo,
marker and `:pw` all republished.

### Minimal repro (unit level)

```rust
configlens::security::sanitize_url("https://user:pw/x@host.example/mcp")
```

| | pre-fix (HEAD) | fixed (working tree) |
|---|---|---|
| `sanitize_url` | `Some("https://user:pw/x@host.example/mcp")` | `None` |
| `url_host` | `Some("user:pw")` | `None` |

A single `/` inside the userinfo (`https://user:pw/x@host.example/mcp`) is enough to republish the
credential as the host. Even `https://a/U@b.example/mcp` leaks under the pre-fix code. (These were
produced by an isolated probe in scratch copies; see [Method](#method-exact-commands).)

### RED evidence

Running the new pipeline regression against an isolated **pre-fix** source copy (HEAD archive),
real tree untouched:

```
thread 'the_saved_hostile_inputs_crash_artifact_is_neutralised' panicked at tests/security_leaks.rs:591:21:
the saved crash artifact leaked "ZUINFO" for opencode [] ["--format", "json"]
test result: FAILED. 0 passed; 1 failed; ...
```

### GREEN evidence

Same test, fixed source (and the real tree):

```
test the_saved_hostile_inputs_crash_artifact_is_neutralised ... ok
test result: ok. 14 passed; 0 failed; ...
```

The fuzz target binary run directly on the artifact also exits 0 on the fixed tree.

## 2. Pipeline regression added

`tests/security_leaks.rs::the_saved_hostile_inputs_crash_artifact_is_neutralised`

- reads the committed artifact from `fuzz/results/hostile_inputs-crash-6ac24e86f9105163905201b031b3bc1ae4ddb070`
  (deliberately **not** `fuzz/artifacts/`, which `.gitignore` excludes);
- rebuilds the exact `hostile_inputs` URL template around the artifact bytes and writes a Claude and
  an OpenCode config whose MCP `url` is that value;
- runs `scan --client {claude,opencode} [--paths share] [--format json]` and asserts none of
  `ZUINFO`, `user:pass`, `passtool_version` reaches stdout/stderr.

This complements the pre-existing `a_url_value_cannot_smuggle_credentials_past_the_sanitizer`
(which pins a hand-written value for OpenCode only) by tying the regression to the saved artifact and
extending coverage to Claude and to share mode.

## 3. Fuzz oracle audit

The oracle in `fuzz/fuzz_targets/hostile_inputs.rs` asserts a report never contains four value
markers (`ZSENTINELV`, `ZUINFO`, `ZKEY`, `ZRULE`) or four control characters. Two of its assumptions
are invalid, and were only visible once the input space started mutating real bytes.

### 3a. A sentinel can arrive inside the arbitrary input text (false positive)

`text` (derived from the fuzz bytes) is placed verbatim into contexts the report is **meant** to show
in Local mode: unknown key names and Claude permission rules. If the input happens to spell a marker,
the "marker must be absent" check fires on a report that is perfectly safe.

Demonstrated on the **current, safe** code (scratch probe, key `ZKEY<CSI>ZUINFO`):

```
raw ESC survived into local JSON: false
old oracle would FIRE on local JSON (false positive): true
share JSON contains marker (should be false): false
```

### 3b. The JSON control check cannot distinguish decoded controls from escaped literals

The old check lower-cased the serialized JSON and searched for the six-character sequences
`\u001b`, `\u202e`, `\u200b`, `\ufeff`. Probed against `serde_json 1.0.152`:

| input string | serialized JSON | old needle `\uXXXX` matched | raw control present |
|---|---|---|---|
| actual ESC | `{"v":"\u001b"}` | yes | no (escaped) |
| actual U+202E | `{"v":"\u202e"}` (raw UTF-8 `e2 80 ae`) | **no** | yes |
| actual U+200B | raw UTF-8 `e2 80 8b` | **no** | yes |
| actual U+FEFF | raw UTF-8 `ef bb bf` | **no** | yes |
| literal text `\u001b` | `{"v":"\\u001b"}` | **yes** | no |

So: a benign input that literally contains `\u001b`/`\u202e` trips the check (**false positive**,
the escape-substring overlaps the escaped backslash), while a genuine bidi / zero-width / BOM leak in
a JSON value is **never** seen by it (**false negative**) because `serde_json` emits those as raw
UTF-8 rather than escapes.

### Corrections

- `strip_sentinels`: the arbitrary input text has any occurrence of the four sentinel tokens removed
  before documents are built, so a marker can only appear in a channel by being published by the code
  under test. The security invariant is retained, not weakened.
- `assert_no_control_chars`: the JSON report is now **parsed** and every decoded string value *and
  object key* is asserted to contain no ESC / U+202E / U+200B / U+FEFF. This distinguishes a decoded
  control (a real leak) from an escaped literal (harmless input text), and it catches raw UTF-8
  control characters that an escaped-form search cannot. The text-report check already inspected
  decoded characters and is unchanged.
- Module doc item 4 updated to describe the decoded inspection.

### Testing the corrected oracle

| check | command / input | result |
|---|---|---|
| no false positive on tricky text | target binary on `ZKEYZUINFO\u001bZUINFO\u202eZSENTINELV` (literal backslashes) | exit 0 |
| fix holds for the artifact | target binary on the saved artifact | exit 0 |
| oracle still has teeth | **corrected oracle built against pre-fix source**, saved artifact | panic, exit 77 |

The last row is the important one. On the pre-fix source the corrected oracle panics with:

```
thread '<unnamed>' panicked at fuzz_targets/hostile_inputs.rs:230:21:
an unsanitized bidi override reached the Local JSON report
```

The artifact's hostile character is `HOSTILE_CHARS[27 % 8]` = U+202E, which lands in the attacker-
chosen key name; the pre-fix code did not sanitize pointer displays, so a raw bidi override reached
the Local JSON report. The old escape-substring check would have **missed** this entirely (see 3b).
That leak is already covered by the working-tree `display_pointer` change in
`src/security/sanitize.rs`, and the current tree passes. The URL marker assertion still fires for
inputs whose hostile character is not a control, so both legs of the invariant remain enforced.

## 4. Fuzz runs

`fuzz/results/session-4.log`, sequential, `-max_total_time=120 -rss_limit_mb=2048 -print_final_stats=1`,
current (fixed) tree with the corrected oracle:

| target | wall | executed units | exec/s | new units | peak RSS | exit |
|---|---|---|---|---|---|---|
| `parse_documents` | 121 s | 408 543 | 3376 | 1295 | 555 MB | 0 |
| `resolve_precedence` | 121 s | 1 948 | 16 | 49 | 520 MB | 0 |
| `hostile_inputs` | 121 s | 1 544 | 12 | 60 | 464 MB | 0 |

No crash artifacts were written (only the pre-fix one is retained) and no target exited non-zero.

## Production code: no new bug, the existing fix is sufficient

- The leak is fixed entirely by the `authority_host` + `tail.contains('@')` refusal already present in
  the working tree's `src/security/redact.rs`; when the authority is ambiguous `sanitize_url` returns
  `None` and the value is redacted. `url_host` applies the same refusal, so share-mode pseudonymization
  cannot be steered either.
- The second real leak the corrected oracle exposes (a control character in a pointer display) is
  likewise already fixed by the working-tree `display_pointer` in `src/security/sanitize.rs`.
- Therefore **no edit to `src/security/redact.rs` was made** — reporting a fresh production bug would
  be fabrication. `mtime` on `src/security/redact.rs` is unchanged (13:28, pre-session).

## Limitations and residual risk

- Fuzzing for 121 s per target is a bounded, single-seed run: it is evidence of no *found* failure, not
  an absence proof. `resolve_precedence` and `hostile_inputs` run at ~12–16 exec/s because each unit
  builds several documents and renders several reports, so their wall-clock coverage is shallow.
- `-rss_limit_mb=2048` is below the host's ~4 GB; a memory-hungry path could be truncated as an OOM
  rather than explored. Observed peak was 555 MB, so there was headroom this session.
- The oracle is a necessary-condition check (markers and four control characters absent), not a proof
  of correct redaction.
- Corpus growth is committed by design (`.gitignore` comment: "the corpus is committed"): after this
  run `fuzz/corpus` was ~22 MB (`parse_documents` 4525 files, `resolve_precedence` 466,
  `hostile_inputs` 331). **Resolved before commit:** the parent ran `cargo +nightly fuzz cmin` on
  each target, so the committed corpus is the coverage-minimised set — 2410 files / 9.6 MB
  (`parse_documents` 1866, `resolve_precedence` 297, `hostile_inputs` 247) — and a 30-second
  re-smoke of each target against it exits 0.
- The full test suite, benches and workflows were not run here (parent's scope). `tests/security_leaks.rs`
  was run in full and passes (14/14).
- All pre-fix RED work was done in isolated scratch copies under
  `$SCRATCH/`; the real tree was never reverted.

## Method (exact commands)

```sh
export PATH="$HOME/.cargo/bin:$PATH"

# GREEN on the real tree
cargo test --test security_leaks the_saved_hostile_inputs_crash_artifact_is_neutralised -- --exact

# Isolated pre-fix copy (real tree untouched)
SC=~/.hermes/profiles/coder/cache/scratch/prefix-head
mkdir -p "$SC"; git -C $REPO archive HEAD | tar -x -C "$SC"
cp $REPO/tests/security_leaks.rs "$SC/tests/security_leaks.rs"
# ... artifact copied to "$SC/fuzz/results/..." -> RED:
cd "$SC" && cargo test --test security_leaks the_saved_hostile_inputs_crash_artifact_is_neutralised -- --exact

# Oracle has teeth: build the corrected fuzz target against the pre-fix source, run the artifact
cd "$SC/fuzz" && cargo +nightly fuzz build hostile_inputs
"$SC/fuzz/target/x86_64-unknown-linux-gnu/release/hostile_inputs" \
  "$SC/fuzz/artifacts/hostile_inputs/crash-6ac24e86f9105163905201b031b3bc1ae4ddb070"   # exit 77

# Full session (all three targets, 120 s each, rss 2048)
cd $REPO/fuzz
for t in parse_documents resolve_precedence hostile_inputs; do
  cargo +nightly fuzz run "$t" -- -max_total_time=120 -rss_limit_mb=2048 -print_final_stats=1 "corpus/$t"
done
```

Note: `CARGO_TARGET_DIR` must **not** be shared between two source trees of the same crate — a stale
test binary is reused silently and reports the wrong library's behaviour.
