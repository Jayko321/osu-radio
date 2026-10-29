import hashlib
import io
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("stage-arch-recipes.py")
NAME = "osu-radio-0.1.0-alpha-arch-x86_64"


class StageRecipesTest(unittest.TestCase):
    def test_recipes_and_rejected_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo = root / "repo"
            repo.mkdir()
            for file in ("packaging/release/stage-arch-recipes.py", "packaging/arch/osu-radio-bin/PKGBUILD", "packaging/arch/osu-radio-git/PKGBUILD"):
                destination = repo / file
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy(SCRIPT.parents[2] / file, destination)
            subprocess.run(["git", "init", "-q", str(repo)], check=True)
            subprocess.run(["git", "-C", str(repo), "add", "packaging"], check=True)
            subprocess.run(["git", "-C", str(repo), "-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "fixture"], check=True)
            sha = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()
            script = repo / "packaging/release/stage-arch-recipes.py"
            archive = root / f"{NAME}.tar.gz"
            with tarfile.open(archive, "w:gz") as tar:
                data = f"source commit: {sha}\nworking tree: clean\n".encode()
                info = tarfile.TarInfo(f"{NAME}/BUILD-INFO.txt")
                info.size = len(data)
                tar.addfile(info, io.BytesIO(data))
            output = root / "staged"

            def run(path, sha, destination):
                return subprocess.run(
                    [sys.executable, str(script), str(path), sha, str(destination)],
                    capture_output=True, text=True,
                )

            self.assertNotEqual(run(archive, "b" * 40, root / "wrong").returncode, 0)
            subprocess.run(["git", "-C", str(repo), "-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "--allow-empty", "-qm", "other"], check=True)
            other_sha = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()
            self.assertNotEqual(run(archive, other_sha, root / "mismatch").returncode, 0)
            self.assertNotEqual(run(root / "absent" / archive.name, sha, root / "missing").returncode, 0)
            (repo / "packaging/arch/osu-radio-bin/PKGBUILD").write_text("dirty working tree")
            result = run(archive, sha, output)
            self.assertEqual(result.returncode, 0, result.stderr)
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            recipe = (output / "osu-radio-bin/PKGBUILD").read_text()
            srcinfo = (output / "osu-radio-bin/.SRCINFO").read_text()
            self.assertIn(f"sha256sums=('{digest}')", recipe)
            self.assertIn(f"sha256sums = {digest}", srcinfo)
            self.assertNotIn("SKIP", srcinfo)
            self.assertIn("sha256sums = SKIP", (output / "osu-radio-git/.SRCINFO").read_text())
            self.assertNotEqual(run(archive, sha, output).returncode, 0)
            with tarfile.open(archive, "w:gz") as tar:
                data = f"source commit: {sha}\nworking tree: DIRTY\n".encode()
                info = tarfile.TarInfo(f"{NAME}/BUILD-INFO.txt")
                info.size = len(data)
                tar.addfile(info, io.BytesIO(data))
            self.assertNotEqual(run(archive, sha, root / "dirty").returncode, 0)


if __name__ == "__main__":
    unittest.main()
