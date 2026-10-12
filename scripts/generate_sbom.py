"""Generate a CycloneDX SBOM for one release target.

The SBOM is generated from the pinned dependency graph (`Cargo.lock` + `Cargo.toml`) with
`cargo-cyclonedx`, then normalised and validated here, so a release cannot ship an SBOM that is
empty, licence-less, non-reproducible, or that leaks the builder's filesystem paths.

Determinism: this script pins `SOURCE_DATE_EPOCH` and normalises the document (sorted components and
top-level dependency edges), so **its own** normalise/serialise path is reproducible; the offline
unit tests prove that path is byte-stable. Whether the real `cargo-cyclonedx` emits byte-identical
raw input across runs is **not** proven here — only the normalised output is guaranteed — so a real
two-run transcript is still pending (see `docs/verification-log.md` §8.5). Re-running for the same
target into the same output directory is refused rather than silently overwritten.

Never reads user configuration, never contacts the network itself (the tool resolves the already
pinned local graph), and never writes outside `--output`.

Usage:
    python scripts/generate_sbom.py --tag v0.1.0-beta.1 \
        --target x86_64-unknown-linux-gnu --output dist
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from package_release import TARGETS, package_version  # noqa: E402

# The pinned SBOM tool. `cargo install --locked --version 0.5.9 cargo-cyclonedx` reproduces it.
TOOL_VERSION = "0.5.9"
SPEC_VERSION = "1.5"
TOOL_TIMEOUT_SECONDS = 600
# Fixed epoch: makes the SBOM reproducible and keeps build dates out of the artifact.
SOURCE_DATE_EPOCH = "1"

# Absolute paths that must never appear in a published SBOM: they are builder-environment data, not
# dependency data, and they would make the document non-reproducible across machines.
HOST_PATH_PATTERNS = (
    r"/home/",
    r"/Users/",
    r"/root/",
    r"/private/var/",
    r"/var/",
    r"/opt/",
    r"/srv/",
    r"/tmp/",
    r"\.cargo/registry",
    # Windows drive paths, backslash- or forward-slash-separated. The bounded prefix keeps this from
    # matching the "s:/" inside an ordinary URL such as "https://...".
    r"(?:^|[^A-Za-z0-9])[A-Za-z]:[\\/]",
    r"file://[A-Za-z]:/",
    r"\\\\\?\\",
)


def crate_version(root):
    """Use the same package-scoped manifest version as release archives."""
    return package_version(root)


def tool_arguments(root, target):
    """The exact `cargo cyclonedx` invocation used for a release target."""
    return [
        "cargo",
        "cyclonedx",
        "--format",
        "json",
        "--describe",
        "binaries",
        "--spec-version",
        SPEC_VERSION,
        "--manifest-path",
        str(Path(root) / "Cargo.toml"),
        "--target",
        target,
        "--target-in-filename",
    ]


def tool_environment(base=None):
    environment = dict(os.environ if base is None else base)
    environment["SOURCE_DATE_EPOCH"] = SOURCE_DATE_EPOCH
    environment["CARGO_TERM_COLOR"] = "never"
    return environment


def run_tool(arguments, cwd, environment):
    """Run the SBOM tool, refusing to continue on any non-zero exit."""
    result = subprocess.run(
        arguments,
        cwd=str(cwd),
        env=environment,
        capture_output=True,
        text=True,
        timeout=TOOL_TIMEOUT_SECONDS,
        check=False,
    )
    if result.returncode != 0:
        raise ValueError(
            f"cargo-cyclonedx failed with exit {result.returncode}: {result.stderr.strip()[:400]}"
        )
    return result


def check_tool_version(runner, cwd, environment):
    """Fail closed when the installed tool is not the pinned version."""
    result = runner(["cargo", "cyclonedx", "--version"], cwd, environment)
    reported = f"{getattr(result, 'stdout', '')}{getattr(result, 'stderr', '')}".strip()
    if reported != f"cargo-cyclonedx-cyclonedx {TOOL_VERSION}":
        raise ValueError(f"expected cargo-cyclonedx {TOOL_VERSION}, got {reported!r}")
    return reported


def normalise(document, version):
    """Make the document reproducible and path-free, in place, and return it."""
    root_ref = document.get("metadata", {}).get("component", {}).get("bom-ref")
    stable_ref = f"pkg:cargo/configlens@{version}"
    if root_ref and root_ref != stable_ref:
        document["metadata"]["component"]["bom-ref"] = stable_ref
        for entry in document.get("dependencies", []):
            if entry.get("ref") == root_ref:
                entry["ref"] = stable_ref
            if "dependsOn" in entry:
                entry["dependsOn"] = [
                    stable_ref if dependency == root_ref else dependency
                    for dependency in entry["dependsOn"]
                ]
    components = document.get("components", [])
    components.sort(key=lambda component: (component.get("name", ""), component.get("version", "")))
    document["components"] = components
    document.setdefault("dependencies", [])
    document["dependencies"].sort(key=lambda entry: entry.get("ref", ""))
    return document


def validate(document, target, version):
    """Assert the properties a release SBOM must have. Raises ValueError on any failure."""
    if document.get("bomFormat") != "CycloneDX":
        raise ValueError("the document is not a CycloneDX BOM")
    if document.get("specVersion") != SPEC_VERSION:
        raise ValueError(f"expected CycloneDX {SPEC_VERSION}, got {document.get('specVersion')!r}")

    component = document.get("metadata", {}).get("component", {})
    if component.get("name") != "configlens":
        raise ValueError(f"root component is {component.get('name')!r}, expected 'configlens'")
    if component.get("version") != version:
        raise ValueError(
            f"root component version {component.get('version')!r} does not match Cargo.toml {version!r}"
        )
    if component.get("bom-ref") != f"pkg:cargo/configlens@{version}":
        raise ValueError("the root component identifier was not normalised to a purl")

    components = document.get("components") or []
    if not components:
        raise ValueError("the SBOM lists no components")
    for entry in components:
        name = entry.get("name", "<unnamed>")
        if not entry.get("version"):
            raise ValueError(f"component {name!r} has no version")
        if not entry.get("purl"):
            raise ValueError(f"component {name!r} has no package URL")
        if not entry.get("licenses"):
            raise ValueError(f"component {name!r} carries no licence information")

    references = {entry["bom-ref"] for entry in components} | {f"pkg:cargo/configlens@{version}"}
    for entry in document.get("dependencies", []):
        if entry.get("ref") not in references:
            raise ValueError(f"dependency graph names unknown component {entry.get('ref')!r}")
        for dependency in entry.get("dependsOn", []):
            if dependency not in references:
                raise ValueError(f"dependency graph names unknown component {dependency!r}")

    serialised = json.dumps(document)
    for pattern in HOST_PATH_PATTERNS:
        if re.search(pattern, serialised):
            raise ValueError(f"the SBOM leaks a builder path matching {pattern!r}")
    return document


def generate(root, output, tag, target, runner=run_tool):
    """Generate, normalise, validate and write one target's SBOM; return the written path."""
    if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+-beta\.[0-9]+", tag):
        raise ValueError("expected vMAJOR.MINOR.PATCH-beta.N")
    if target not in TARGETS:
        raise ValueError("unsupported release target")
    root, output = Path(root).resolve(), Path(output)
    version = crate_version(root)
    if version != tag[1:]:
        raise ValueError(
            f"tag {tag} requires package version {tag[1:]}, found {version}"
        )

    check_tool_version(runner, root, tool_environment())
    runner(tool_arguments(root, target), root, tool_environment())

    produced = root / f"configlens_bin_{target}.cdx.json"
    if not produced.is_file():
        raise ValueError(f"the SBOM tool did not write {produced}")
    document = json.loads(produced.read_text(encoding="utf-8"))
    produced.unlink()

    normalise(document, version)
    validate(document, target, version)

    output.mkdir(parents=True, exist_ok=True)
    destination = output / f"configlens-{tag}-{target}.cdx.json"
    with destination.open("x", encoding="utf-8", newline="\n") as handle:
        json.dump(document, handle, indent=2, sort_keys=True)
        handle.write("\n")
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--target", choices=sorted(TARGETS), required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print(generate(Path(__file__).resolve().parent.parent, args.output, args.tag, args.target))


if __name__ == "__main__":
    main()
