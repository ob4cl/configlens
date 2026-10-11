# Getting started

ConfigLens is a local, read-only CLI that explains which configuration value wins for a supported
client, and why. This guide takes you from a fresh checkout to reading a real trace.

If you only want to see it work, jump to [Run the demo](#2-run-the-demo).

## 1. Build from source

You need a Rust toolchain. `Cargo.toml` declares `rust-version = "1.85"`, but the project has only
been built and tested with `1.99.0` so far — see [`verification-log.md`](verification-log.md).

```sh
git clone <this repository>       # private beta: use your existing access
cd configlens
cargo build --locked --release
```

To install the binary into your `PATH` instead:

```sh
cargo install --locked --path .
```

There is no published crate or prebuilt binary yet. Build from the checkout.

Check that it runs:

```sh
./target/release/configlens --help
./target/release/configlens clients
```

`clients` prints the two implemented adapters (OpenCode, Claude Code) with their tested versions and
the layers that are always incomplete, followed by the clients that are explicitly unsupported.

## 2. Run the demo

The demo is fully isolated and synthetic: it creates a throwaway home and project under a temporary
directory, writes only obviously-fake values, clears the environment, runs the CLI against that
sandbox and deletes it on exit. It reads none of your real configuration and needs no network.

```sh
./demo/demo.sh
```

It exits `0`. Read its output top to bottom — it is a tour of the core commands.

## 3. A first real run

Run ConfigLens against a project of your own. `--client` is required for anything that resolves, and
`--client-version` is a *claim* you supply (ConfigLens never runs a client binary to find out).

```sh
cd /path/to/your/project
configlens scan --client claude --client-version 2.1.296
```

`scan` inventories the candidate sources it knows about for that client, and reports for each one
whether it is applicable and readable. It also lists **coverage gaps**: layers the adapter expects
but cannot observe offline (server-managed settings, an MDM policy store, the Windows registry).
Those gaps are the reason a result is often `partial` rather than absolute.

Then ask about a specific key:

```sh
configlens explain /model --client claude --client-version 2.1.296
```

OpenCode works the same way with `--client opencode` and its tested version (`1.18.35` at the time of
writing). Run `configlens clients` for the current matrix.

## 4. Reading the output

A real `explain` looks like this (synthetic values, from the demo):

```console
$ configlens explain /model --client claude --client-version 2.1.296
/model = "local-model" (observed)
Observed winner: $PROJECT/.claude/settings.local.json:1:12  "local-model"  observed_winner
Losers:
  "project-model"  $PROJECT/.claude/settings.json:2:12  replaced_by_later_layer
Rule: claude.scalar.layer-override
Coverage: partial — cc.managed.hkcu — registry_not_read (expected_unobservable); … (3 more gaps)
Support: fixture_verified; confidence: evidence_class:fixture_verified
Conclusion: "local-model" wins among the observed applicable sources; effective value withheld.
Next inspection:
  - supply the managed settings file with --input-context to include it
  - remote/MDM layers are not observable here; re-run where the managed source is readable
```

Line by line:

- **`/model = "local-model" (observed)`** — the observed value for the pointer. `(observed)` means it
  was not proven complete; a runtime value would be labelled differently.
- **`Observed winner:`** — where the winning definition lives, as `path:line:column`.
- **`Losers:` and the lines under it** are the contribution trace, in precedence order.
  `replaced_by_later_layer` and `observed_winner` are reason codes; both the winner *and* the losers
  are shown.
- **`Rule:`** the rule ID that produced the result, so you can look it up in the adapter's registry.
- **`Coverage: partial`** — the gaps that stop ConfigLens from calling this the runtime value.
- **`Support:`** the evidence class: `fixture_verified`, `documented` or `inventory_only`.
- **`Conclusion:`** one sentence in plain language.

For a Claude permission list you will also see a caveat that the output is a **provenance merge and
not an authorization decision**.

### Other output modes

```sh
configlens explain /model --client claude --format json     # stable, redacted JSON
configlens doctor   --client opencode --format json --strict
configlens scan     --client opencode --paths share         # pseudonymized paths/key names
```

## 5. Snapshots and diffs

Capture a redacted report and compare two of them across a change:

```sh
configlens snapshot --client claude --output before.json
# ... change something ...
configlens snapshot --client claude --output after.json
configlens diff before.json after.json
```

Snapshots are written create-new and atomically, with owner-only permissions on Unix. Pass
`--overwrite` to replace an existing file. Protected values compare as `unknown_redacted` — two
redacted values are never reported as equal, because equality cannot be proven from a mask.

## 6. What "wins" actually means

ConfigLens only reports what it can read, and it never assumes an unreadable layer is absent. When a
higher-priority layer is unobservable (a server-managed policy, an MDM/registry setting, OpenCode
remote defaults) or the project trust state is unknown, the resolution is `partial`, the winner is
labelled *observed*, and the effective value is withheld. Exit code `0` therefore means "a report was
produced", not "this is the value your tool will use". Use `--strict` to make incomplete context
fail with exit code `3`.

See the [README](../README.md#what-wins-actually-means) for the full explanation.

## 7. Exit codes

| Code | Meaning |
|---|---|
| `0` | Report produced. Non-fatal findings allowed. |
| `1` | `doctor` findings at or above `--fail-on` (default `error`). |
| `2` | Usage error, or incompatible input/report. |
| `3` | Unsupported key/client semantics, or `--strict` found incomplete context. |
| `4` | A required file was inaccessible, malformed or over a resource limit. |

Priority when several apply: `2 → 4 → 3 → 1 → 0`.

## 8. Troubleshooting

- **"`--client` is required for resolution"** — add `--client opencode` or `--client claude`.
- **Exit `3` with `--strict`** — the context is incomplete (an unobservable layer or unknown trust).
  Drop `--strict` for an ordinary `partial` report, or supply context with `--input-context`.
- **Exit `2` for a known client** — a known-but-unimplemented client (for example `codex`) exits `3`;
  an unknown name exits `2`. Run `configlens clients`.
- **A key shows `inventory_only`** — configlens preserves it but makes no merge claim for it. That is
  deliberate, not a bug.
- **The winner surprises you** — read the contribution trace and the coverage gaps before the
  conclusion. A `partial` result can legitimately differ from what the tool actually uses.

## 9. Next steps

- [`../README.md`](../README.md) — the full command and option reference.
- [`maintaining.md`](maintaining.md) — how the project is verified and maintained.
- [`implementation-status.md`](implementation-status.md) — exactly what is and is not implemented.
- [`decisions.md`](decisions.md) — why the tool behaves the way it does.
