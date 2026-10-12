"""Build the release binary with best-effort Rust source-path remapping.

Home (including the default dependency cache) maps to /build-home; the repository
maps to /configlens. Repo mappings come last because rustc uses the last match.
Encoded flags preserve spaces without shell quoting. Existing encoded flags take
precedence over RUSTFLAGS, whose whitespace splitting matches Cargo's contract.

This is not a general artifact scrubber: linker paths (including MSVC PDBs and
Apple OSO entries), arbitrary strings from build scripts/external tools, and
paths outside these prefixes may remain. Caller remap scopes can also restrict
coverage. Inspect final artifacts independently before publishing. Cargo output
is inherited; this helper never prints host prefixes or its constructed flags.
"""
import argparse
import os
from pathlib import Path, PureWindowsPath
import subprocess
import sys


DEFAULT_ROOT = Path(__file__).resolve().parent.parent
SEPARATOR = "\x1f"


def build_release(*, root=None, home=None, environment=None, target=None,
                  toolchain=None, run=None):
    """Invoke Cargo from the script's repository, returning its exit status."""
    root = DEFAULT_ROOT if root is None else Path(root)
    home = Path.home() if home is None else home
    env = dict(os.environ if environment is None else environment)
    if "CARGO_ENCODED_RUSTFLAGS" in env:
        encoded = env["CARGO_ENCODED_RUSTFLAGS"]
        flags = encoded.split(SEPARATOR) if encoded else []
    else:
        flags = env.get("RUSTFLAGS", "").split()
    env.pop("RUSTFLAGS", None)
    for prefix, replacement in ((str(home), "/build-home"), (str(root), "/configlens")):
        spellings = [prefix]
        if PureWindowsPath(prefix).is_absolute():
            spellings += [prefix.replace("\\", "/"), prefix.replace("/", "\\")]
        for spelling in dict.fromkeys(spellings):
            flags.append(f"--remap-path-prefix={spelling}={replacement}")
    env["CARGO_ENCODED_RUSTFLAGS"] = SEPARATOR.join(flags)
    arguments = ["cargo"]
    if toolchain:
        arguments.append("+" + toolchain)
    arguments += ["build", "--locked", "--release", "--bin", "configlens"]
    if target:
        arguments += ["--target", target]
    return (subprocess.run if run is None else run)(arguments, cwd=root, env=env).returncode


def main(argv=None, *, run=None):
    parser = argparse.ArgumentParser(description="Build a source-path-remapped release binary")
    parser.add_argument("--target", help="optional Rust target triple")
    parser.add_argument("--toolchain", help="optional installed rustup toolchain")
    args = parser.parse_args(argv)
    try:
        return build_release(target=args.target, toolchain=args.toolchain, run=run)
    except OSError:
        print("Release build could not start.", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
