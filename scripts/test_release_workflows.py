"""Execute prerelease preflight against isolated, synthetic Git repositories."""
import os
from pathlib import Path
import subprocess
import tempfile
import textwrap
import unittest


class PreflightTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.env = dict(os.environ, HOME=str(self.root), GIT_CONFIG_NOSYSTEM="1",
                        GIT_CONFIG_GLOBAL=os.devnull, TAG="v0.1.0-beta.1",
                        GITHUB_OUTPUT=str(self.root / "output"))
        self.git("init", "-q")
        self.git("config", "user.name", "Synthetic Tester")
        self.git("config", "user.email", "synthetic@example.invalid")
        self.workflow = (Path(__file__).resolve().parent.parent /
                         ".github/workflows/prerelease.yml").read_text(encoding="utf-8")
        block = self.workflow.split("      - name: Resolve existing reviewed beta tag\n", 1)[1]
        self.script = textwrap.dedent(block.split("        run: |\n", 1)[1].split("\n\n  checks:", 1)[0])

    def git(self, *args):
        return subprocess.run(["git", "-c", "core.hooksPath=" + os.devnull, *args],
                              cwd=self.root, env=self.env, check=True,
                              capture_output=True, text=True).stdout.strip()

    def commit(self, version):
        (self.root / "Cargo.toml").write_text(
            f'[package]\nname = "configlens"\nversion = "{version}"\n', encoding="utf-8")
        self.git("add", "Cargo.toml")
        self.git("commit", "-qm", "synthetic version")
        self.env["GITHUB_SHA"] = self.git("rev-parse", "HEAD")

    def run_preflight(self):
        return subprocess.run(["bash", "-c", self.script], cwd=self.root,
                              env=self.env, capture_output=True, text=True)

    def test_accepts_lightweight_and_annotated_matching_tags(self):
        self.commit("0.1.0-beta.1")
        for annotated in (False, True):
            with self.subTest(annotated=annotated):
                args = ["tag"]
                if annotated:
                    args += ["-a", "-m", "synthetic tag"]
                self.git(*args, self.env["TAG"])
                result = self.run_preflight()
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("sha=" + self.env["GITHUB_SHA"],
                              (self.root / "output").read_text(encoding="utf-8"))
                self.git("tag", "-d", self.env["TAG"])

    def test_rejects_mismatch_even_when_same_named_branch_matches(self):
        self.commit("0.1.0-beta.2")
        self.git("tag", self.env["TAG"])
        self.git("checkout", "-qb", self.env["TAG"])
        self.commit("0.1.0-beta.1")
        result = self.run_preflight()
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Tag/package version mismatch", result.stderr)
        self.assertFalse((self.root / "output").exists())

    def test_accepts_matching_tag_even_when_same_named_branch_mismatches(self):
        self.commit("0.1.0-beta.1")
        self.git("tag", self.env["TAG"])
        tag_sha = self.env["GITHUB_SHA"]
        self.git("checkout", "-qb", self.env["TAG"])
        self.commit("0.1.0-beta.2")
        result = self.run_preflight()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.root / "output").read_text(encoding="utf-8"),
                         "sha=" + tag_sha + "\n")


if __name__ == "__main__":
    unittest.main()
