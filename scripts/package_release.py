"""Package an already-built native binary; never inspect user configuration."""
import argparse
import hashlib
import io
from pathlib import Path
import re
import tarfile
import zipfile

TARGETS = {
    "x86_64-unknown-linux-gnu": "tar.gz",
    "aarch64-apple-darwin": "tar.gz",
    "x86_64-pc-windows-msvc": "zip",
}
NOTICES = ("README.md", "LICENSE-MIT", "LICENSE-APACHE")


def expected_version(tag):
    """The package version a release tag must carry: `v0.1.0-beta.3` -> `0.1.0-beta.3`."""
    return tag[1:] if tag.startswith("v") else tag


def package_version(root):
    """Read the `[package] version` from the manifest without a TOML dependency.

    Only the `[package]` block is read, so a `version` key in any later section cannot be
    mistaken for the crate version.
    """
    manifest = Path(root) / "Cargo.toml"
    match = re.search(
        r'^\[package\]\s*$(?P<body>.*?)(?=^\[|\Z)',
        manifest.read_text(encoding="utf-8"),
        re.MULTILINE | re.DOTALL,
    )
    version = None
    if match is not None:
        found = re.search(r'^version\s*=\s*"([^"]+)"\s*$', match.group("body"), re.MULTILINE)
        if found is not None:
            version = found.group(1)
    if version is None:
        raise ValueError(f"no [package] version found in {manifest}")
    return version


def package(root, output, tag, target):
    if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+-beta\.[0-9]+", tag):
        raise ValueError("expected vMAJOR.MINOR.PATCH-beta.N")
    if target not in TARGETS:
        raise ValueError("unsupported release target")
    root, output = Path(root), Path(output)
    # Fail closed before writing anything: the tag and the package version must agree, so that the
    # archive name, the git tag, `configlens --version`, the report `tool_version` and the SBOM root
    # component all identify the same release.
    version = package_version(root)
    if version != expected_version(tag):
        raise ValueError(
            f"tag {tag} requires package version {expected_version(tag)}, found {version}"
        )
    binary = "configlens.exe" if target.endswith("windows-msvc") else "configlens"
    members = [(binary, root / "target" / target / "release" / binary)]
    members += [(name, root / name) for name in NOTICES]
    for _, path in members:
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"missing regular release input: {path}")
    output.mkdir(parents=True, exist_ok=True)
    stem = f"configlens-{tag}-{target}"
    archive = output / f"{stem}.{TARGETS[target]}"
    # Refuse accidental overwrite; reruns should use a fresh output directory.
    with archive.open("xb") as destination:
        if TARGETS[target] == "zip":
            with zipfile.ZipFile(destination, "w", zipfile.ZIP_DEFLATED) as bundle:
                for name, path in members:
                    info = zipfile.ZipInfo(f"{stem}/{name}", (1980, 1, 1, 0, 0, 0))
                    info.external_attr = (0o755 if name == binary else 0o644) << 16
                    info.compress_type = zipfile.ZIP_DEFLATED
                    bundle.writestr(info, path.read_bytes())
        else:
            import gzip
            with gzip.GzipFile(fileobj=destination, mode="wb", filename="", mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w") as bundle:
                    for name, path in members:
                        data = path.read_bytes()
                        info = tarfile.TarInfo(f"{stem}/{name}")
                        info.size = len(data)
                        info.mode = 0o755 if name == binary else 0o644
                        info.mtime = 0
                        bundle.addfile(info, io.BytesIO(data))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    checksum = archive.with_name(archive.name + ".sha256")
    with checksum.open("x", encoding="ascii", newline="\n") as handle:
        handle.write(f"{digest}  {archive.name}\n")
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print(package(Path(__file__).resolve().parent.parent, args.output, args.tag, args.target))


if __name__ == "__main__":
    main()
