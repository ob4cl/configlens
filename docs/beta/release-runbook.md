# Maintainer release runbook

How to cut a ConfigLens release and how to undo one. Written for a solo maintainer; every step is
a real command you run, not something automated for you. Nothing here publishes anything until you
approve it (`user-actions.md`).

## 0. Preconditions (all must be true)

- [ ] CI green on Linux, macOS and Windows: `fmt`, `clippy -D warnings`, `test --locked`.
- [ ] MSRV build green (1.85.0 or the corrected floor).
- [ ] Secret-leak suite green in **local and share** modes.
- [ ] `docs/implementation-status.md` and `compatibility/*/registry.md` re-checked against reality;
      support labels not overstated.
- [ ] Release-readiness gate in `milestone-checklist.md` satisfied for anything you will claim.
- [ ] Known limitations listed for the release note (see §4). Do not hide them.

## 1. Prepare the tree

```sh
cd configlens
git switch main && git pull --ff-only
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --no-fail-fast
python -m unittest discover -s scripts -p "test_*.py"
cargo build --locked --release
./demo/demo.sh          # must exit 0
./target/release/configlens --version
```

- [ ] Bump `version` in `Cargo.toml` (`0.1.0` → next).
- [ ] Run `cargo build --locked` once so `Cargo.lock` matches; commit the lockfile.
- [ ] Write the `CHANGELOG.md` entry (Added / Changed / Fixed / Known limitations).
- [ ] Update `README.md` support matrix and any "not verified" statements to match this release.

## 2. Tag the source and run the manual prerelease workflow

There is **no tag-triggered release workflow** — `release.yml` does not exist (an earlier draft of
this runbook described one). The only release workflow is `.github/workflows/prerelease.yml`, and it
is **manual**: `workflow_dispatch` against an **existing reviewed tag** on `main`.

```sh
# 1. Create and push the annotated tag yourself.
git tag -a vX.Y.Z-beta.N -m "ConfigLens vX.Y.Z-beta.N"
git push origin vX.Y.Z-beta.N

# 2. Dispatch the workflow on main, naming that tag.
gh workflow run prerelease.yml -f tag=vX.Y.Z-beta.N
```

- [ ] Tag matches the `Cargo.toml` version and the `vMAJOR.MINOR.PATCH-beta.N` shape the workflow
      enforces.
- [ ] `prerelease.yml` resolves the tag's commit, re-runs the reused `ci.yml` on exactly that commit,
      builds the supported targets (`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`,
      `x86_64-pc-windows-msvc`), verifies every checksum and creates the prerelease. Write permission
      lives only in the `publish` job.
- [ ] **There is no smoke-test step.** The build job builds and packages only; it never runs the binary.
      If you want a smoke test (`--version`, `clients`, `schema --kind report`) it must be added to the
      workflow first — do not claim one in the release notes until it exists.
- [ ] This workflow has **never run**. The first dispatch is an untested procedure: watch it and be
      ready to fix it forward.

## 3. Artifacts, checksums and integrity

```sh
# in the release job: scripts/package_release.py writes a per-asset "*.sha256" next to each archive;
# the publish job then re-verifies each and concatenates them:
cat -- "${sums[@]}" > SHA256SUMS      # 3 archives + 3 SBOMs + SHA256SUMS = 7 assets
```

- [ ] Publish, alongside the binaries: source archive, `SHA256SUMS`, and release notes.
- [ ] Sign or attest where practical: detached GPG signature on `SHA256SUMS`, or a
      sigstore/SLSA provenance attestation. If not done, say so — do not imply it.
- [ ] Add SBOM if you produced one; otherwise omit rather than fake.
- [ ] README/release notes get a copy-paste **verify** block:
      ```sh
      sha256sum -c SHA256SUMS
      ```
- [ ] No mandatory `curl | sh` install. Manual download or `cargo install` only.

## 4. Release notes template

```
ConfigLens vX.Y.Z

What it does: <one sentence, unchanged>
Works with: Claude Code <v>, OpenCode <v>  (supported keys in compatibility/opencode/registry.md and
                                           compatibility/claude/registry.md; summary in README.md)
Does not support yet: Codex, Gemini, ... (see `configlens clients`)

Highlights
- <only DONE, tested features>

Fixes
- <issue # — one line>

Known limitations (read before filing a bug)
- Windows snapshot ACLs are not implemented (0.1).
- Managed/enterprise/remote/MDM state is often unobservable; the tool says so and withholds
  an effective value instead of guessing.
- Verified on: <list OSes actually tested>. Others are untested.
- MSRV: 1.85 (by then verified or corrected).

Verify downloads: sha256sum -c SHA256SUMS
Install / build: <commands>
```

## 5. crates.io (only if name cleared)

- [ ] Name available + trademark checked (`user-actions.md` A.4).
- [ ] `LICENSE-MIT` / `LICENSE-APACHE` present; `Cargo.toml` repository URL real.
- [ ] `cargo publish --dry-run` clean, then `cargo publish`.
- [ ] Publishing credentials live **only** in the release job, never in PR-triggered jobs.

## 6. Announce

- [ ] Only posts you approved in `launch-drafts.md`, only after repo visibility is approved.
- [ ] Lead with a reproducible behaviour, not hype. Include the honest limits.
- [ ] No star/follower/visibility solicitation of any kind.

## 7. Post-release checks

- [ ] Fresh-machine quick start works from the published source/binaries.
- [ ] `sha256sum -c SHA256SUMS` passes on a download.
- [ ] Watch issues per `issue-triage.md` for 48h.

## 8. Rollback / yank

If a release is wrong:

- [ ] **Do not silently re-tag.** Delete the GitHub release and tag only if nothing depends on it,
      then re-tag with a fix in a **new** patch version (`vX.Y.Z+1`) — prefer a forward fix.
- [ ] If published to crates.io: `cargo yank --version X.Y.Z` (yank ≠ delete) and say why in
      `CHANGELOG.md` and the README.
- [ ] Edit the release notes to point at the fixed version; never leave a broken asset linked.
- [ ] If a secret ever leaked into an artifact: rotate, remove the asset, and state it plainly.
