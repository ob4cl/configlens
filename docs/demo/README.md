# The recorded demo

This directory holds `configlens-demo.cast`, the recorded terminal demonstration referenced from the
README, and is the destination `demo/record.sh` writes to.

## What is here

- `configlens-demo.cast` — an asciinema v2 recording of ConfigLens resolving configuration
  precedence across three layers (global, project, and a project-local file), showing the observed
  winner, the losers with the reason each lost, the rule, coverage and the support level.

## How it was produced

`demo/record.sh` drives `demo/demo.sh` under a pseudo-terminal via `demo/record_pty.py` and writes
the cast. Both scripts are in `demo/`.

```sh
# from the repository root, with a release binary already built
cargo build --locked --release
./demo/record.sh          # writes docs/demo/configlens-demo.cast
```

`demo/demo.sh` builds its own synthetic sandbox under a temporary directory, then deletes it on exit.
It reads **no** real configuration, needs **no** network, starts no client, and never touches
`$HOME/.config` or `$HOME/.claude` — every value in the recording is synthetic.

## Playing it back

Any asciinema-compatible player, for example:

```sh
asciinema play docs/demo/configlens-demo.cast
```

The recording is documentation, not evidence: the fixture-level evidence for the behaviour it shows
lives in `docs/verification-log.md` and `compatibility/`.
