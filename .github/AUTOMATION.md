# Private-beta automation

## CI

`ci.yml` tests locked dependencies on Ubuntu 24.04 (x64), macOS 15
(Apple Silicon), and Windows Server 2022 (x64). Separate required-to-pass jobs
check formatting (`fmt --check`), Clippy (`--all-targets --all-features -D
warnings`), the declared Rust 1.85.0 MSRV (`check --locked --all-targets
--all-features`), and the dependency policy (`cargo deny check`: advisories,
licences, bans, sources). A `fuzz` job builds the three `cargo-fuzz` targets and
runs each for 60 seconds against the committed seed corpus, so a crash or a
broken invariant fails fast. No job hides failures with `continue-on-error`.
Stable Rust moves over time; Cargo.lock is not rewritten. Synthetic tests
demonstrate the implemented contract, not upstream-client conformance.

`fuzz-scheduled.yml` is the longer pass and is **not** part of PR latency: it runs
weekly (and on manual dispatch) on Ubuntu 24.04, one matrix job per target, each
fuzzing for a bounded wall-clock budget (`-max_total_time`, default 600 s,
adjustable on dispatch) under an explicit memory cap (`-rss_limit_mb=4096`) and a
per-input hang timeout (`-timeout=25`), with a 90-minute job timeout. A crash
fails the job and uploads the reproducer plus the evolved corpus as a
short-retention artifact. Nothing in it comments on, opens or publishes anything.

The committed corpus under `fuzz/corpus/` is the seed for both jobs; it is kept as
the libFuzzer coverage-minimised set (every retained input adds coverage), not the
raw accumulated corpus.

Python standard-library packaging tests run on all three platforms. No caching,
third-party toolchain action, installer service, secrets, or user config is needed.
Only official actions are used, pinned to full commit IDs. The hosted images supply
rustup and Python. Test results must be checked on GitHub after integration; local
Linux results cannot establish Windows/macOS support.

## Manual private prerelease

Nothing publishes on a push, including a tag push. `prerelease.yml` accepts only
`workflow_dispatch`, from `main` in private `ob4cl/configlens`, with an explicit
existing `vMAJOR.MINOR.PATCH-beta.N` tag. It resolves the tag to a commit, requires
that commit to be an ancestor of the dispatch commit, then runs the complete CI
workflow against that exact SHA. All checks must pass before builds start.

The preflight also requires the tag's version to equal the `[package] version` in
`Cargo.toml` at that tag (read straight out of the tag with `git show`, so nothing
is checked out or executed and a mismatch fails before any build starts). That is
what keeps the git tag, the archive names, `configlens --version`, the report
`tool_version` and the SBOM root component pointing at the same release. The check
applies from `v0.1.0-beta.3` onward: the earlier `v0.1.0-beta.1`/`v0.1.0-beta.2` tags
carry package version `0.1.0` and would now be rejected, which is intended — they are
already published and must not be re-cut.

Maintainer procedure (not performed by this implementation):

1. Integrate and review code, workflows, README, and both license files. Push and
   independently inspect all CI jobs. Protect main and beta tags from unauthorized
   updates; a tag must include these scripts and current license notices.
2. Create/push a reviewed beta tag using the intended package base version. This
   workflow does not change the Cargo version, but it does reject a tag whose version
   does not equal the package version at that tag, so bump `Cargo.toml` `version` (and
   `Cargo.lock`) in the commit that gets tagged.
3. In Actions, choose **Private beta prerelease**, **Run workflow**, branch `main`,
   and enter the exact existing tag. No token secret configuration is required.
4. Inspect the private prerelease and independently download/verify its assets.
   Checksums detect corruption, not publisher identity; binaries are unsigned.

Archives contain only the binary, README.md, LICENSE-MIT, and LICENSE-APACHE:

- `x86_64-unknown-linux-gnu`: `.tar.gz`, built on Ubuntu 24.04; not a musl or
  broad older-glibc compatibility promise.
- `aarch64-apple-darwin`: `.tar.gz`; no Intel macOS build, signing, or notarization.
- `x86_64-pc-windows-msvc`: `.zip`; no Authenticode signing.

Packaging refuses missing inputs, symlinks, invalid tags/targets, and archive
replacement. Archive metadata is normalized; this does **not** claim reproducible
Rust builds. Every platform uploads an archive plus SHA-256 sidecar with seven-day
Actions retention. The final job checks all sidecars and publishes three archives
plus `SHA256SUMS`, explicitly prerelease and not latest. It verifies the resulting
release metadata. Repository visibility is rechecked immediately before publishing;
the workflow never changes visibility or publishes to a package registry.

Only the publishing job has `contents: write`; other jobs have `contents: read`.
The write-enabled job does not check out or execute repository code. It rechecks
the tag SHA before release creation. Existing releases are not overwritten; a
partial publication requires maintainer inspection rather than blind reruns.
Artifacts/releases in a private repository still require appropriate repository
access and may consume private Actions billing/storage. Do not make the repository
public without separately reviewing existing assets and release policy.

## Local validation

```sh
python3 -m unittest discover -s scripts -p 'test_*.py' -v
actionlint .github/workflows/ci.yml .github/workflows/prerelease.yml .github/workflows/fuzz-scheduled.yml
cargo +1.85.0 check --locked --all-targets --all-features
cargo +stable fmt --all -- --check
cargo +stable clippy --locked --all-targets --all-features -- -D warnings
```

For an actual archive, first build the exact target with `cargo +stable build
--locked --release --target x86_64-unknown-linux-gnu --bin configlens`, then run
`python3 scripts/package_release.py --tag v0.1.0-beta.1 --target
x86_64-unknown-linux-gnu --output /path/to/fresh/output`. Packaging never collects
configuration files or recursively sweeps a workspace.

## Official references and pin verification

Pins were verified using `git ls-remote` against the official action repositories:

| Action tag | Verified commit |
| --- | --- |
| actions/checkout v4.3.1 | 34e114876b0b11c390a56381ad16ebd13914f8d5 |
| actions/upload-artifact v4.6.2 | ea165f8d65b6e75b540449e92b4886f43607fa02 |
| actions/download-artifact v4.3.0 | d3f86a106a0bac45b974a628896c90dbdf5c8093 |

Reverify explicit upstream tags before changing a pin; never substitute floating
major-version tags. These are verified versions, not a claim to be the newest.

- https://docs.github.com/en/actions/security-for-github-actions/security-guides/security-hardening-for-github-actions
- https://docs.github.com/en/actions/using-workflows/events-that-trigger-workflows
- https://cli.github.com/manual/gh_release_create
- https://github.com/actions/runner-images/blob/main/README.md
- https://github.com/actions/upload-artifact
- https://github.com/actions/download-artifact
