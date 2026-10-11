"""Offline packaging contract tests; all payloads are synthetic, not release binaries."""
import hashlib
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile

from package_release import NOTICES, TARGETS, expected_version, package, package_version

# The fixture manifest version; every tag used below must match it (see the mismatch test).
FIXTURE_VERSION = "0.1.0-beta.1"


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "Cargo.toml").write_text(
            '[package]\nname = "configlens"\nversion = "'
            + FIXTURE_VERSION
            + '"\nedition = "2021"\n\n[profile.release]\nversion = "not-the-package-version"\n',
            encoding="utf-8",
        )
        for name in NOTICES:
            (self.root / name).write_text("Synthetic notice\n", encoding="utf-8")
        for target in TARGETS:
            binary = "configlens.exe" if target.endswith("windows-msvc") else "configlens"
            path = self.root / "target" / target / "release" / binary
            path.parent.mkdir(parents=True)
            path.write_bytes(b"synthetic test payload, not executable\n")
        (self.root / "private-config.json").write_text("MUST-NOT-BE-PACKAGED")

    def test_all_archives_checksums_members_and_reproducibility(self):
        for target in TARGETS:
            with self.subTest(target=target):
                archive = package(self.root, self.root / "first", "v0.1.0-beta.1", target)
                second = package(self.root, self.root / "second", "v0.1.0-beta.1", target)
                self.assertEqual(archive.read_bytes(), second.read_bytes())
                checksum = archive.with_name(archive.name + ".sha256").read_text()
                self.assertEqual(checksum, f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n")
                if archive.suffix == ".zip":
                    with zipfile.ZipFile(archive) as bundle:
                        names = bundle.namelist()
                        self.assertIsNone(bundle.testzip())
                else:
                    with tarfile.open(archive) as bundle:
                        names = bundle.getnames()
                        self.assertEqual(bundle.getmembers()[0].mode, 0o755)
                        self.assertTrue(all(item.isfile() for item in bundle.getmembers()))
                binary = "configlens.exe" if target.endswith("windows-msvc") else "configlens"
                self.assertEqual({Path(name).name for name in names}, {binary, *NOTICES})
                self.assertEqual(len(names), 4)

    def test_rejects_unsafe_tags_and_unknown_target(self):
        for tag in ("../bad", "v1.0.0", "v1.0.0-beta.1\n", "$(touch bad)"):
            with self.assertRaises(ValueError):
                package(self.root, self.root / "out", tag, next(iter(TARGETS)))
        with self.assertRaises(ValueError):
            package(self.root, self.root / "out", "v1.0.0-beta.1", "unknown")

    def test_missing_notice_fails_closed(self):
        (self.root / NOTICES[0]).unlink()
        with self.assertRaises(ValueError):
            package(self.root, self.root / "out", "v0.1.0-beta.1", next(iter(TARGETS)))
        self.assertFalse((self.root / "out").exists())

    def test_refuses_archive_overwrite(self):
        args = (self.root, self.root / "out", "v0.1.0-beta.1", next(iter(TARGETS)))
        package(*args)
        with self.assertRaises(FileExistsError):
            package(*args)

    def test_expected_version_strips_the_tag_prefix(self):
        self.assertEqual(expected_version("v0.1.0-beta.3"), "0.1.0-beta.3")
        self.assertEqual(expected_version("0.1.0-beta.3"), "0.1.0-beta.3")

    def test_package_version_reads_only_the_package_block(self):
        # A later section also carries a `version` key; it must not be picked up.
        self.assertEqual(package_version(self.root), FIXTURE_VERSION)

    def test_rejects_a_tag_that_does_not_match_the_package_version(self):
        for tag in ("v0.1.0-beta.9", "v0.1.0", "v0.1.0-beta.2"):
            with self.subTest(tag=tag):
                with self.assertRaises(ValueError):
                    package(self.root, self.root / "mismatch", tag, next(iter(TARGETS)))
        # Nothing may be written when the identity does not add up.
        self.assertFalse((self.root / "mismatch").exists())

    def test_rejects_a_manifest_without_a_package_version(self):
        (self.root / "Cargo.toml").write_text('[package]\nname = "configlens"\n', encoding="utf-8")
        with self.assertRaises(ValueError):
            package(self.root, self.root / "noversion", "v0.1.0-beta.1", next(iter(TARGETS)))
        (self.root / "Cargo.toml").unlink()
        with self.assertRaises((ValueError, OSError)):
            package(self.root, self.root / "nomanifest", "v0.1.0-beta.1", next(iter(TARGETS)))
        self.assertFalse((self.root / "noversion").exists())
        self.assertFalse((self.root / "nomanifest").exists())


if __name__ == "__main__":
    unittest.main()
