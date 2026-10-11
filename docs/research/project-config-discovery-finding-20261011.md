# Finding — project-config discovery reads one file; upstream merges all ancestors

Recorded 2026-10-11. Status: **confirmed divergence; fixed 2026-10-11** (option 1 below, owner-approved).
The wrong-winner path is closed in `src/adapters/opencode.rs`, regression-tested in
`tests/rules_opencode.rs`, and cited in `compatibility/opencode/registry.{json,md}` (evidence
`OC-PROJECT-SOURCE-20261011`). See ["Fix"](#fix-2026-10-11) at the end for the before/after repro.

Raised as item A3 ("source-inferred, not proven") in
[`beta2-independent-audit.md`](beta2-independent-audit.md). This file records the confirmation:
the claim is now **proven** against the pinned upstream source and against this tree's release
binary.

## What upstream does

Pinned commit `53d1eabb61e21162157817bf677da0a4ad3332e3` (`v1.18.35`).

`packages/opencode/src/config/paths.ts`:

```ts
export const files = Effect.fn("ConfigPaths.projectFiles")(function* (name, directory, worktree?) {
  const afs = yield* FSUtil.Service
  return (yield* afs.up({
    targets: [`${name}.jsonc`, `${name}.json`],
    start: directory,          // the working directory
    stop: worktree,            // the repository root
  })).toReversed()
})
```

`packages/core/src/fs-util.ts` — `up()` pushes a match at **every** level from `start` up to `stop`:

```ts
const up = Effect.fn("FileSystem.up")(function* (options) {
  const result: string[] = []
  let current = options.start
  while (true) {
    for (const target of options.targets) {
      const search = join(current, target)
      if (yield* fs.exists(search)) result.push(search)
    }
    if (options.stop === current) break
    const parent = dirname(current)
    if (parent === current) break
    current = parent
  }
  return result
})
```

So `files()` returns **every** ancestor `opencode.json` and `opencode.jsonc` between the working
directory and the repository root, in walk-up order, then `.toReversed()` → outermost first,
working-directory last. `config/config.ts:421` merges them in that order, so **the nearest file
wins** and every intermediate ancestor contributes.

## What ConfigLens does

`src/cli.rs:276-279` auto-detects the repository root and stores it in `context.project_root`. The
OpenCode adapter then uses that as the *search start*, not as a boundary
(`src/adapters/opencode.rs`):

```rust
let search_start = ctx.project_root.clone().unwrap_or_else(|| ctx.cwd.clone());
if let Some(dir) = project_config_dir(ctx, &search_start, access) { /* one directory only */ }
```

and `project_config_dir` (`src/adapters/opencode.rs:216-245`) returns the **first** directory that
contains a config file and stops:

```rust
while let Some(dir) = current {
    if CONFIG_FILES.iter().any(|name| access.exists_regular(&dir.join(name))) {
        return Some(dir);            // <- one directory, then stop
    }
    if boundary.as_deref() == Some(dir.as_path()) { return None; }
    current = dir.parent().map(|p| p.to_path_buf());
}
```

Two consequences, one root cause:

1. The search starts at `project_root` (the repository root), so the **working directory's own**
   `opencode.json` is never read when it differs from the root.
2. Only **one** file is read; ancestors between the working directory and the root are dropped.

## Reproduction — before the fix (release binary at HEAD `35ad132`, 2026-10-11)

```
repo/.git/                                        (repository root)
repo/opencode.json          {"model":"repo-model","small_model":"repo-small"}
repo/sub/opencode.json      {"model":"sub-model","small_model":"sub-small"}
```

Upstream, launched in `repo/sub`, merges `[repo/opencode.json, repo/sub/opencode.json]` with the
nearest last, so `/small_model = "sub-small"`.

ConfigLens, `cwd = repo/sub` (root auto-detected), and equally with `--project-root repo`:

```
$ configlens explain /small_model --client opencode --client-version 1.18.35
/small_model = "repo-small" (observed)
Observed winner: $PROJECT/opencode.json:1:37  "repo-small"  observed_winner
```

`$PROJECT/opencode.json` is the repository-root file. ConfigLens names the value upstream would
**override** as the winner, and `scan` lists a single `[30] oc.project` source. The result is
presented as a confident `observed_winner` with no coverage gap for the unread file.

## Why this matters

- It is a **wrong winner**, not merely an incomplete inventory — the outcome the project exists to
  prevent.
- The critical correctness principle requires the limitation to be reported when coverage is
  incomplete; here the missing ancestor file produces no gap at all.
- It is **pre-existing**: it is not introduced by the `OPENCODE_CONFIG_DIR` work and is present in
  `v0.1.0-beta.1`.

## Options

1. **Fix in beta.2.** Change project discovery to walk from `ctx.cwd` up to the boundary
   (`project_root` else nearest git root), collecting every `opencode.json`/`opencode.jsonc`, and
   emit them as `oc.project` candidates outermost-first so the nearest wins. Add an evidence entry,
   a fixture, and regression tests for: nested configs (nearest wins), ancestor-only keys, the
   `--project-root` boundary, and no-boundary behaviour.
2. **Disclose in beta.2, fix in beta.3.** Add a coverage gap (and record it in the compatibility
   registry) whenever the boundary is an ancestor of the working directory, so the under-report is
   visible instead of silent. Smaller change, but the winner stays wrong.
3. **Defer entirely.** Not recommended: it leaves a known wrong-winner path undisclosed in a
   private beta.

Recommendation: option 1. No change is made until the owner chooses.

## Fix (2026-10-11)

Owner chose **option 1**. `project_config_dir` (which returned the *first* directory containing a
config) is replaced by a private helper that returns the **ordered file list**: every
`opencode.json`/`opencode.jsonc` from `ctx.cwd` up to the boundary, inclusive, boundary first and
`ctx.cwd` last, with `opencode.json` before `opencode.jsonc` at each level. The adapter emits those as
`oc.project` candidates in that order, so the nearest file is last in definition order and the
engine's last-definition-wins tie-break (`src/resolve/mod.rs`) selects it. `ctx.project_root` is now
only the **boundary**, never the search start.

- Boundary = `--project-root` when supplied, else the nearest git root (`find_git_root`).
- No boundary ⇒ `ctx.cwd` only (unchanged), and the run keeps the `project_boundary_uncertain`
  signal.
- The walk stops at the boundary inclusive and at the filesystem root regardless.
- Kept: layer id/rank `oc.project` = 30; the per-directory `jsonc_coexistence_unverified` note when
  one directory holds both dialects.

Evidence `OC-PROJECT-SOURCE-20261011` (`compatibility/opencode/registry.json`), mirrored in
`registry.md`. Regression tests (`tests/rules_opencode.rs`):
`oc_project_nested_configs_nearest_wins_and_every_level_is_a_source`,
`oc_project_ancestor_only_key_resolves_from_the_ancestor`,
`oc_project_cwd_equal_to_the_boundary_reads_a_single_source`,
`oc_project_explicit_boundary_never_reads_above_it`,
`oc_project_without_a_boundary_reads_the_cwd_only`,
`oc_project_jsonc_beats_json_at_an_ancestor_level`.

### Before / after (release binaries, same tree, 2026-10-11)

Layout (git repository root `repo/`, working directory `repo/sub`):

```
repo/.git/
repo/opencode.json          {"model":"repo-model","small_model":"repo-small"}
repo/sub/opencode.json      {"model":"sub-model","small_model":"sub-small"}
```

Run from `repo/sub`, with `--project-root` naming the boundary and `--allow-read-root` for each level:

```
# BEFORE — release binary built from HEAD 35ad132
$ configlens explain /small_model --client opencode --client-version 1.18.35 \
      --project-root .. --allow-read-root .. --allow-read-root .
/small_model = "repo-small" (observed)
Observed winner: $PROJECT/opencode.json:1:37  "repo-small"  observed_winner
...
$ configlens scan --client opencode --client-version 1.18.35 \
      --project-root .. --allow-read-root .. --allow-read-root .
Sources (1):
  [30] oc.project  $PROJECT/opencode.json  observed  applicable

# AFTER — release binary built from this fix
$ configlens explain /small_model --client opencode --client-version 1.18.35 \
      --project-root .. --allow-read-root .. --allow-read-root .
/small_model = "sub-small" (observed)
Observed winner: $PROJECT/sub/opencode.json:1:36  "sub-small"  observed_winner
Losers:
  "repo-small"  $PROJECT/opencode.json:1:37  replaced_by_later_layer
...
$ configlens scan --client opencode --client-version 1.18.35 \
      --project-root .. --allow-read-root .. --allow-read-root .
Sources (2):
  [30] oc.project  $PROJECT/opencode.json  observed  applicable
  [30] oc.project  $PROJECT/sub/opencode.json  observed  applicable
```

`/small_model` flips from `repo-small` (the ancestor, wrongly selected as the sole source) to
`sub-small` (the nearest file), and the ancestor is now reported as a loser instead of the winner. The
same result holds on the auto-detected boundary (no `--project-root`, `.git` at `repo/`) and with
`--project-root repo` run from the parent directory.

Verification (same pass): `cargo fmt --all -- --check` clean; `cargo clippy --locked --all-targets
--all-features -- -D warnings` clean; `cargo test --locked --no-fail-fast` 315 passed / 0 failed;
`python -m unittest discover -s scripts -p "test_*.py"` OK (13 tests). No runtime-conformance claim is
made: the ceiling stays `fixture_verified`.
