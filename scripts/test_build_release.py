"""Release build invocation and path-remapping regression tests (no Cargo needed)."""
import contextlib
import io
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock

import build_release


class BuildReleaseTests(unittest.TestCase):
    def test_spaces_and_encoded_caller_flags_survive_target_build(self):
        self.assertIsNotNone(build_release, "release build helper is missing")
        with tempfile.TemporaryDirectory(prefix="release build ") as directory:
            home = Path(directory) / "Synthetic User"
            root = home / "project with spaces"
            root.mkdir(parents=True)
            caller = "--cfg\x1ffeature=\"synthetic flag\"\x1f--remap-path-prefix=/old=/caller"
            environment = {"CARGO_ENCODED_RUSTFLAGS": caller, "RUSTFLAGS": "ignored", "KEEP": "yes"}
            target = "x86_64-unknown-linux-gnu"

            def cargo(arguments, *, cwd, env):
                self.assertEqual(arguments, ["cargo", "+stable", "build", "--locked", "--release",
                                             "--bin", "configlens", "--target", target])
                self.assertEqual(cwd, root)
                flags = env["CARGO_ENCODED_RUSTFLAGS"].split("\x1f")
                self.assertEqual(flags[:3], caller.split("\x1f"))
                self.assertEqual(flags[3:], [f"--remap-path-prefix={home}=/build-home",
                                             f"--remap-path-prefix={root}=/configlens"])
                self.assertEqual(env["KEEP"], "yes")
                self.assertNotIn("RUSTFLAGS", env)
                output = cwd / "target" / target / "release" / "configlens"
                output.parent.mkdir(parents=True)
                output.write_bytes(b"synthetic target artifact")
                return subprocess.CompletedProcess(arguments, 0)

            self.assertEqual(build_release.build_release(root=root, home=home, environment=environment,
                                                        target=target, toolchain="stable", run=cargo), 0)
            self.assertEqual((root / "target" / target / "release" / "configlens").read_bytes(),
                             b"synthetic target artifact")
            self.assertEqual(environment["RUSTFLAGS"], "ignored")

    def test_windows_separator_spellings_are_remapped_home_before_repo(self):
        self.assertIsNotNone(build_release)
        cargo = Mock(return_value=subprocess.CompletedProcess([], 0))
        build_release.build_release(root=r"C:\Users\Synthetic User\my repo",
                                    home=r"C:\Users\Synthetic User", environment={}, run=cargo)
        self.assertEqual(cargo.call_args.kwargs["env"]["CARGO_ENCODED_RUSTFLAGS"].split("\x1f"), [
            r"--remap-path-prefix=C:\Users\Synthetic User=/build-home",
            "--remap-path-prefix=C:/Users/Synthetic User=/build-home",
            r"--remap-path-prefix=C:\Users\Synthetic User\my repo=/configlens",
            "--remap-path-prefix=C:/Users/Synthetic User/my repo=/configlens",
        ])

    def test_standard_flags_use_cargo_whitespace_splitting(self):
        cargo = Mock(return_value=subprocess.CompletedProcess([], 0))
        build_release.build_release(root=Path("/synthetic/repo"), home="/synthetic",
                                    environment={"RUSTFLAGS": "  -C\topt-level=2\n--cfg synthetic  "},
                                    run=cargo)
        flags = cargo.call_args.kwargs["env"]["CARGO_ENCODED_RUSTFLAGS"].split("\x1f")
        self.assertEqual(flags[:4], ["-C", "opt-level=2", "--cfg", "synthetic"])

    def test_empty_encoded_flags_override_standard_flags(self):
        cargo = Mock(return_value=subprocess.CompletedProcess([], 0))
        build_release.build_release(root=Path("/synthetic/repo"), home="/synthetic",
                                    environment={"CARGO_ENCODED_RUSTFLAGS": "", "RUSTFLAGS": "ignored"},
                                    run=cargo)
        flags = cargo.call_args.kwargs["env"]["CARGO_ENCODED_RUSTFLAGS"].split("\x1f")
        self.assertEqual(len(flags), 2)
        self.assertTrue(all(flag.startswith("--remap-path-prefix=") for flag in flags))

    def test_main_defaults_to_script_repo_not_callers_working_directory(self):
        cargo = Mock(return_value=subprocess.CompletedProcess([], 0))
        previous = Path.cwd()
        with tempfile.TemporaryDirectory() as directory:
            try:
                os.chdir(directory)
                self.assertEqual(build_release.main([], run=cargo), 0)
            finally:
                os.chdir(previous)
        self.assertEqual(cargo.call_args.kwargs["cwd"], Path(__file__).resolve().parent.parent)
        self.assertEqual(cargo.call_args.args[0],
                         ["cargo", "build", "--locked", "--release", "--bin", "configlens"])

    def test_main_propagates_cargo_failure_without_logging_identities(self):
        cargo = Mock(return_value=subprocess.CompletedProcess([], 37))
        output, errors = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
            result = build_release.main(["--target", "synthetic-target"], run=cargo)
        self.assertEqual(result, 37)
        self.assertEqual(output.getvalue() + errors.getvalue(), "")
        self.assertEqual(cargo.call_args.args[0][-2:], ["--target", "synthetic-target"])

    def test_launch_error_does_not_echo_exception_paths(self):
        cargo = Mock(side_effect=OSError("/synthetic/private/home cannot launch"))
        errors = io.StringIO()
        with contextlib.redirect_stderr(errors):
            self.assertEqual(build_release.main([], run=cargo), 1)
        self.assertEqual(errors.getvalue(), "Release build could not start.\n")


if __name__ == "__main__":
    unittest.main()
