"""Run the actual std-only build script with a private, fake .NET executable."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


@unittest.skipUnless(os.name == "posix", "fake executable uses a POSIX shebang")
class BuildHelperTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        build = tempfile.TemporaryDirectory(prefix="scanner build script ")
        cls.addClassCleanup(build.cleanup)
        cls.binary = Path(build.name) / "build-script"
        source = Path(__file__).resolve().parents[1] / "build.rs"
        subprocess.run(
            [shutil.which("rustc") or "rustc", "--edition=2024", source, "-o", cls.binary],
            check=True, capture_output=True, text=True,
        )

    def setUp(self):
        fixture = tempfile.TemporaryDirectory(prefix="scanner build fixture ")
        self.addCleanup(fixture.cleanup)
        self.root = Path(fixture.name)
        self.manifest = self.root / "crates/radio-scanner"
        self.manifest.mkdir(parents=True)
        self.output = self.root / "cargo output"
        self.bin_dir = self.root / "bin"
        self.bin_dir.mkdir()
        self.log = self.root / "dotnet.json"
        self.env = {
            "PATH": str(self.bin_dir),
            "CARGO_MANIFEST_DIR": str(self.manifest),
            "OUT_DIR": str(self.output),
            "FIXTURE_LOG": str(self.log),
        }

    def fake_dotnet(self):
        executable = self.bin_dir / "dotnet"
        executable.write_text(
            f"#!{sys.executable}\n"
            "import json, os, pathlib, sys\n"
            "pathlib.Path(os.environ['FIXTURE_LOG']).write_text(json.dumps({\n"
            " 'args': sys.argv[1:], 'home': os.environ['DOTNET_CLI_HOME'],\n"
            " 'nologo': os.environ['DOTNET_NOLOGO'],\n"
            " 'first_run': os.environ['DOTNET_SKIP_FIRST_TIME_EXPERIENCE']}))\n"
            "mode = os.environ.get('FIXTURE_MODE', 'success')\n"
            "if mode == 'fail': sys.exit(23)\n"
            "if mode == 'success':\n"
            " output = pathlib.Path(sys.argv[sys.argv.index('--output') + 1])\n"
            " output.mkdir(parents=True)\n"
            " (output / 'osu-lazer-realm-parser').write_bytes(b'fake helper')\n"
        )
        executable.chmod(0o755)

    def run_build(self):
        return subprocess.run(
            [self.binary], env=self.env, cwd=self.root,
            capture_output=True, text=True, timeout=10,
        )

    def test_explicit_helper_bypasses_dotnet_and_cargo_directories(self):
        helper = self.root / "prebuilt helper"
        # Overrides are forwarded verbatim; existence is checked when importing.
        self.env = {"PATH": str(self.bin_dir), "OSU_LAZER_REALM_PARSER_PATH": str(helper)}
        result = self.run_build()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), [
            "cargo:rerun-if-env-changed=OSU_LAZER_REALM_PARSER_PATH",
            f"cargo:rustc-env=OSU_LAZER_REALM_PARSER_BUILT_PATH={helper}",
        ])
        self.assertFalse(self.output.exists())

    def test_build_arguments_environment_and_emitted_helper_path(self):
        self.fake_dotnet()
        for custom_home in [None, self.root / "private dotnet home"]:
            with self.subTest(custom_home=custom_home):
                shutil.rmtree(self.output, ignore_errors=True)
                if custom_home is not None:
                    self.env["DOTNET_CLI_HOME"] = str(custom_home)
                result = self.run_build()
                self.assertEqual(result.returncode, 0, result.stderr)
                project_dir = self.manifest / "../../tools/osu-lazer-realm-parser"
                project = project_dir / "osu-lazer-realm-parser.csproj"
                helper = self.output / "dotnet-helper/osu-lazer-realm-parser"
                self.assertEqual(json.loads(self.log.read_text()), {
                    "args": ["build", "--configuration", "Release", "--nologo",
                             "--output", str(helper.parent), str(project)],
                    "home": str(custom_home or self.manifest / "../../target/dotnet-home"),
                    "nologo": "1", "first_run": "1",
                })
                self.assertEqual(result.stdout.splitlines(), [
                    "cargo:rerun-if-env-changed=OSU_LAZER_REALM_PARSER_PATH",
                    f"cargo:rerun-if-changed={project}",
                    f"cargo:rerun-if-changed={project_dir / 'Program.cs'}",
                    f"cargo:rustc-env=OSU_LAZER_REALM_PARSER_BUILT_PATH={helper}",
                ])
                self.assertEqual(helper.read_bytes(), b"fake helper")

    def test_missing_dotnet_reports_install_or_override_remedy(self):
        result = self.run_build()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("failed to start dotnet", result.stderr)
        self.assertIn(".NET 10 SDK or set OSU_LAZER_REALM_PARSER_PATH", result.stderr)
        self.assertNotIn("cargo:rustc-env=", result.stdout)

    def test_failed_dotnet_does_not_advertise_a_helper(self):
        self.fake_dotnet()
        self.env["FIXTURE_MODE"] = "fail"
        result = self.run_build()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("dotnet failed to build", result.stderr)
        self.assertIn("23", result.stderr)
        self.assertNotIn("cargo:rustc-env=", result.stdout)

    def test_dotnet_success_without_an_executable_is_an_error(self):
        self.fake_dotnet()
        self.env["FIXTURE_MODE"] = "no-helper"
        result = self.run_build()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("dotnet reported success but did not produce", result.stderr)
        self.assertIn(str(self.output / "dotnet-helper/osu-lazer-realm-parser"), result.stderr)
        self.assertNotIn("cargo:rustc-env=", result.stdout)


if __name__ == "__main__":
    unittest.main()
