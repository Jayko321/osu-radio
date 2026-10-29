import importlib.util
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location("windows_build", Path(__file__).with_name("build.py"))
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)


class WindowsPayloadTest(unittest.TestCase):
    def test_missing_and_empty_files_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            stage = Path(directory)
            for name in build.REQUIRED:
                path = stage / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"fixture")
            build.check_payload(stage)
            (stage / "platforms/qwindows.dll").unlink()
            with self.assertRaisesRegex(ValueError, "platforms/qwindows.dll"):
                build.check_payload(stage)
            (stage / "platforms/qwindows.dll").touch()
            with self.assertRaisesRegex(ValueError, "platforms/qwindows.dll"):
                build.check_payload(stage)

    def test_wrong_tool_pin_stops_before_staging(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            qt, redist, materials = root / "qt", root / "redist", root / "materials"
            for file in (qt / "bin/qmake.exe", qt / "bin/windeployqt.exe", redist / "vcruntime140.dll",
                         redist / "msvcp140.dll", root / "ISCC.exe", materials / "notice"):
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(b"fixture")
            args = ["build.py", "--qt", str(qt), "--redist", str(redist), "--iscc", str(root / "ISCC.exe"),
                    "--materials", str(materials), "--work", str(root / "work"), "--qt-version", "6.8.3",
                    "--dotnet-sdk", "10.0.113", "--msvc-version", "14.0", "--windows-sdk", "10.0",
                    "--iscc-sha256", hashlib.sha256(b"fixture").hexdigest()]
            with patch.object(build.sys, "platform", "win32"), patch.object(build.sys, "argv", args), \
                    patch.dict(build.os.environ, {"VCToolsVersion": "14.0", "WindowsSDKVersion": "10.0"}), \
                    patch.object(build.subprocess, "check_output", side_effect=["6.9.0", "10.0.113"]):
                with self.assertRaises(SystemExit):
                    build.main()
            self.assertFalse((root / "work").exists())


if __name__ == "__main__":
    unittest.main()
