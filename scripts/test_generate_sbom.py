"""Offline SBOM contract tests; the SBOM tool is simulated, nothing is downloaded or built."""
import json
from pathlib import Path
import tempfile
import unittest

from generate_sbom import (
    SPEC_VERSION,
    TOOL_VERSION,
    crate_version,
    generate,
    normalise,
    tool_arguments,
    validate,
)

LOCAL_REF = "path+file:///home/dev/configlens#0.1.0"


class FakeResult:
    def __init__(self, stdout="", stderr="", returncode=0):
        self.stdout = stdout
        self.stderr = stderr
        self.returncode = returncode


def document(version="0.1.0", *, root_ref=LOCAL_REF, licences=True, extra_component=None):
    components = [
        {
            "type": "library",
            "bom-ref": "pkg:cargo/serde@1.0.229",
            "name": "serde",
            "version": "1.0.229",
            "purl": "pkg:cargo/serde@1.0.229",
            "licenses": [{"expression": "MIT OR Apache-2.0"}],
        },
        {
            "type": "library",
            "bom-ref": "pkg:cargo/clap@4.6.7",
            "name": "clap",
            "version": "4.6.7",
            "purl": "pkg:cargo/clap@4.6.7",
            "licenses": [{"expression": "MIT OR Apache-2.0"}],
        },
    ]
    if not licences:
        del components[0]["licenses"]
    if extra_component is not None:
        components.append(extra_component)
    return {
        "bomFormat": "CycloneDX",
        "specVersion": SPEC_VERSION,
        "metadata": {
            "timestamp": "1970-01-01T00:00:01Z",
            "component": {
                "bom-ref": root_ref,
                "type": "application",
                "name": "configlens",
                "version": version,
                "licenses": [{"expression": "MIT OR Apache-2.0"}],
            },
        },
        "components": components,
        "dependencies": [
            {"ref": root_ref, "dependsOn": ["pkg:cargo/serde@1.0.229"]},
            {"ref": "pkg:cargo/serde@1.0.229", "dependsOn": []},
            {"ref": "pkg:cargo/clap@4.6.7", "dependsOn": []},
        ],
    }


class FakeRunner:
    """A stand-in for subprocess that reports the pinned version and writes a synthetic SBOM."""

    def __init__(self, payload=None, *, tool_version=TOOL_VERSION, returncode=0):
        self.payload = payload
        self.tool_version = tool_version
        self.returncode = returncode
        self.calls = []

    def __call__(self, arguments, cwd, environment):
        self.calls.append(
            {"arguments": list(arguments), "cwd": str(cwd), "environment": dict(environment)}
        )
        if arguments[-1] == "--version":
            return FakeResult(stdout=f"cargo-cyclonedx-cyclonedx {self.tool_version}\n")
        if self.returncode != 0:
            return FakeResult(returncode=self.returncode, stderr="synthetic failure")
        target = arguments[arguments.index("--target") + 1]
        body = self.payload if self.payload is not None else document()
        (Path(cwd) / f"configlens_bin_{target}.cdx.json").write_text(
            json.dumps(body), encoding="utf-8"
        )
        return FakeResult()

    def generation_calls(self):
        return [call for call in self.calls if call["arguments"][-1] != "--version"]


def fake_runner(payload=None, *, tool_version=TOOL_VERSION, returncode=0):
    return FakeRunner(payload, tool_version=tool_version, returncode=returncode)


class SbomTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "Cargo.toml").write_text(
            '[package]\nname = "configlens"\nversion = "0.1.0"\n', encoding="utf-8"
        )

    def test_generates_valid_normalised_reproducible_sbom(self):
        runner = fake_runner()
        first = generate(self.root, self.root / "first", "v0.1.0-beta.1", "x86_64-unknown-linux-gnu", runner)
        second = generate(self.root, self.root / "second", "v0.1.0-beta.1", "x86_64-unknown-linux-gnu", runner)
        self.assertEqual(first.name, "configlens-v0.1.0-beta.1-x86_64-unknown-linux-gnu.cdx.json")
        self.assertEqual(first.read_bytes(), second.read_bytes(), "the SBOM must be reproducible")
        written = json.loads(first.read_text(encoding="utf-8"))
        root_component = written["metadata"]["component"]
        self.assertEqual(root_component["bom-ref"], "pkg:cargo/configlens@0.1.0")
        self.assertNotIn("/home/", first.read_text(encoding="utf-8"))
        # Components and dependency edges are sorted, so the bytes do not depend on graph order.
        self.assertEqual([c["name"] for c in written["components"]], ["clap", "serde"])
        self.assertEqual(
            [entry["ref"] for entry in written["dependencies"]],
            ["pkg:cargo/clap@4.6.7", "pkg:cargo/configlens@0.1.0", "pkg:cargo/serde@1.0.229"],
        )
        self.assertIn("pkg:cargo/serde@1.0.229", written["dependencies"][1]["dependsOn"])
        # The tool was invoked exactly as the release workflow documents, with a pinned epoch.
        generate_call = runner.generation_calls()
        self.assertEqual(
            generate_call[0]["arguments"], tool_arguments(self.root, "x86_64-unknown-linux-gnu")
        )
        self.assertEqual(generate_call[0]["environment"]["SOURCE_DATE_EPOCH"], "1")
        # The tool's own output file is not left behind in the crate root.
        self.assertFalse((self.root / "configlens_bin_x86_64-unknown-linux-gnu.cdx.json").exists())

    def test_validate_rejects_incomplete_documents(self):
        cases = {
            "no components": {"components" : []},
            "no licences": {"components": document(licences=False)["components"]},
            "missing version": {
                "components": [
                    {"name": "x", "bom-ref": "pkg:cargo/x@1", "purl": "pkg:cargo/x@1", "licenses": [{"id": "MIT"}]}
                ]
            },
            "missing purl": {
                "components": [
                    {"name": "x", "version": "1", "bom-ref": "pkg:cargo/x@1", "licenses": [{"id": "MIT"}]}
                ]
            },
            "unknown dependency": {"dependencies": [{"ref": "pkg:cargo/ghost@1", "dependsOn": []}]},
            "unknown edge target": {
                "dependencies": [
                    {"ref": "pkg:cargo/configlens@0.1.0", "dependsOn": ["pkg:cargo/ghost@1"]}
                ]
            },
            "host path in a component": {
                "components": document()["components"]
                + [
                    {
                        "name": "leaky",
                        "version": "1",
                        "bom-ref": "pkg:cargo/leaky@1",
                        "purl": "pkg:cargo/leaky@1",
                        "licenses": [{"id": "MIT"}],
                        "properties": [{"name": "cargo:manifest", "value": "/home/dev/configlens/Cargo.toml"}],
                    }
                ]
            },
            "wrong spec version": {"specVersion": "1.4"},
            "wrong root version": {"metadata_component_version": "9.9.9"},
            "forward-slash windows path in a component": {
                "components": document()["components"]
                + [
                    {
                        "name": "leaky-win",
                        "version": "1",
                        "bom-ref": "pkg:cargo/leaky-win@1",
                        "purl": "pkg:cargo/leaky-win@1",
                        "licenses": [{"id": "MIT"}],
                        "properties": [
                            {"name": "cargo:manifest", "value": "C:/Users/dev/configlens/Cargo.toml"}
                        ],
                    }
                ]
            },
            "backslash windows path in a component": {
                "components": document()["components"]
                + [
                    {
                        "name": "leaky-win-bs",
                        "version": "1",
                        "bom-ref": "pkg:cargo/leaky-win-bs@1",
                        "purl": "pkg:cargo/leaky-win-bs@1",
                        "licenses": [{"id": "MIT"}],
                        "properties": [
                            {"name": "cargo:manifest", "value": "C:\\Users\\dev\\configlens\\Cargo.toml"}
                        ],
                    }
                ]
            },
            "posix build directory in a component": {
                "components": document()["components"]
                + [
                    {
                        "name": "leaky-srv",
                        "version": "1",
                        "bom-ref": "pkg:cargo/leaky-srv@1",
                        "purl": "pkg:cargo/leaky-srv@1",
                        "licenses": [{"id": "MIT"}],
                        "properties": [{"name": "cargo:manifest", "value": "/srv/build/Cargo.toml"}],
                    }
                ]
            },
        }
        for name, patch in cases.items():
            with self.subTest(case=name):
                body = document()
                if "metadata_component_version" in patch:
                    body["metadata"]["component"]["version"] = patch.pop("metadata_component_version")
                body.update(patch)
                normalise(body, "0.1.0")
                with self.assertRaises(ValueError):
                    validate(body, "x86_64-unknown-linux-gnu", "0.1.0")

    def test_validate_accepts_ordinary_urls(self):
        # The Windows-drive path guard is bounded so that an ordinary URL is not mistaken for a drive
        # path: a naive pattern like r"[A-Za-z]:[\\/]" would match the "s:/" inside "https://".
        body = document()
        body["externalReferences"] = [
            {"type": "website", "url": "https://github.com/ob4cl/configlens"}
        ]
        normalise(body, "0.1.0")
        validate(body, "x86_64-unknown-linux-gnu", "0.1.0")

    def test_normalise_is_idempotent_and_path_free(self):
        body = document()
        first = normalise(body, "0.1.0")
        second = normalise(json.loads(json.dumps(first)), "0.1.0")
        self.assertEqual(first, second)
        self.assertEqual(json.dumps(first).count("pkg:cargo/configlens@0.1.0"), 2)

    def test_bad_tool_version_fails_closed(self):
        for version in ("0.4.0", "unknown"):
            with self.subTest(version=version):
                with self.assertRaises(ValueError):
                    generate(
                        self.root,
                        self.root / "out",
                        "v0.1.0-beta.1",
                        "x86_64-unknown-linux-gnu",
                        fake_runner(tool_version=version),
                    )

    def test_tool_failure_is_reported(self):
        with self.assertRaises(ValueError):
            generate(
                self.root,
                self.root / "out",
                "v0.1.0-beta.1",
                "x86_64-unknown-linux-gnu",
                fake_runner(returncode=1),
            )
        self.assertFalse((self.root / "out").exists())

    def test_rejects_unsafe_tags_and_unknown_target(self):
        for tag in ("../bad", "v1.0.0", "v1.0.0-beta.1\n", "$(touch bad)"):
            with self.assertRaises(ValueError):
                generate(self.root, self.root / "out", tag, "x86_64-unknown-linux-gnu", fake_runner())
        with self.assertRaises(ValueError):
            generate(self.root, self.root / "out", "v1.0.0-beta.1", "unknown-target", fake_runner())

    def test_refuses_output_overwrite(self):
        args = (self.root, self.root / "out", "v0.1.0-beta.1", "x86_64-unknown-linux-gnu")
        generate(*args, fake_runner())
        with self.assertRaises(FileExistsError):
            generate(*args, fake_runner())

    def test_crate_version_comes_from_the_manifest(self):
        self.assertEqual(crate_version(self.root), "0.1.0")
        (self.root / "Cargo.toml").write_text('[package]\nname = "configlens"\n', encoding="utf-8")
        with self.assertRaises(ValueError):
            crate_version(self.root)


if __name__ == "__main__":
    unittest.main()
