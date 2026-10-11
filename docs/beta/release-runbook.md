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
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --no-fail-fast
cargo build --locked --release
./demo/demo.sh          # must exit 0
./target/release/configlens --version
```

- [ ] Bump `version` in `Cargo.toml` (`0.1.0` → next).
- [ ] Run `cargo build --locked` once so `Cargo.lock` matches; commit the lockfile.
- [ ] Write the `CHANGELOG.md` entry (Added / Changed / Fixed / Known limitations).
- [ ] Update `README.md` support matrix and any "not verified" statements to match this release.

## 2. Tag the source

```sh
git tag -a vX.Y.Z -m "ConfigLens vX.Y.Z"
git push origin vX.Y.Z
```

- [ ] Tag matches `Cargo.toml`.
- [ ] The tag-triggered `release.yml` builds the matrix. Promote **only tested** targets:
      x86_64/aarch64 linux, x86_64/aarch64 macos, x86_64 windows.
- [ ] Each built binary is smoke-tested (`--version`, `clients`, `schema --kind report`) inside
      the workflow before upload.

## 3. Artifacts, checksums and integrity

```sh
# in the release job, per artifact:
sha256sum configlens-<target>.tar.gz > SHA256SUMS   # append per target
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
Works with: Claude Code <v>, OpenCode <v>  (supported keys in docs/compatibility.md)
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
