import hashlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location("release_stage", Path(__file__).with_name("stage.py"))
stage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stage)


class ReleaseStageTest(unittest.TestCase):
    def test_candidate_and_failed_handoffs(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            repo = base / "source"
            for relative in ("packaging/release/stage-arch-recipes.py", "packaging/release/inputs.json",
                             "packaging/release/INSTALL.md", "packaging/arch/osu-radio-bin/PKGBUILD",
                             "packaging/arch/osu-radio-git/PKGBUILD"):
                path = repo / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(stage.ROOT / relative, path)
            inputs_path = repo / "packaging/release/inputs.json"
            inputs = json.loads(inputs_path.read_text())
            inputs.update(arch_image="archlinux@sha256:" + "a" * 64, arch_snapshot="2026/10/29",
                          windows_materials_url="https://example.invalid/materials.tar.gz",
                          windows_materials_sha256="b" * 64)
            inputs_path.write_text(json.dumps(inputs))
            for app in ("osu-radio-qt", "osu-radio-server"):
                manifest = repo / "apps" / app / "Cargo.toml"
                manifest.parent.mkdir(parents=True)
                manifest.write_text(f'[package]\nversion = "{stage.VERSION}"\n')
            (repo / "LICENSE").write_text("project license fixture")
            licenses = repo / "packaging/arch/licenses"
            licenses.mkdir(parents=True)
            for name in ("RUST-NOTICES.txt", "HELPER-NOTICES.txt"):
                (licenses / name).write_text(f"synthetic {name}")
            subprocess.run(["git", "init", "-q", str(repo)], check=True)
            subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
            subprocess.run(["git", "-C", str(repo), "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                            "commit", "-qm", "synthetic release source"], check=True)
            sha = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()
            archive = base / f"{stage.ARCH_NAME}.tar.gz"
            files = {
                "BUILD-INFO.txt": f"source commit: {sha}\nworking tree: clean\n".encode(),
                "SOURCE-FILES.sha256": b"synthetic source inventory",
                "usr/bin/osu-radio-qt": b"synthetic GUI binary",
                "usr/bin/osu-radio-server": b"synthetic server binary",
                "usr/lib/osu-radio/osu-lazer-realm-parser": b"synthetic helper",
            }
            for path in [repo / "LICENSE", *licenses.iterdir()]:
                files[f"usr/share/licenses/osu-radio/{path.name}"] = path.read_bytes()

            def write_archive():
                with tarfile.open(archive, "w:gz") as tar:
                    for name, content in files.items():
                        member = tarfile.TarInfo(f"{stage.ARCH_NAME}/{name}")
                        member.size = len(content)
                        tar.addfile(member, io.BytesIO(content))

            write_archive()
            windows = base / "windows"
            (windows / "output").mkdir(parents=True)
            setup = windows / "output" / stage.SETUP_NAME
            setup.write_bytes(b"synthetic Setup")
            (windows / "setup.sha256").write_text(f"{stage.digest(setup)}  {stage.SETUP_NAME}\n")
            (windows / "BUILD-INFO.txt").write_bytes(files["BUILD-INFO.txt"])
            materials = windows / "licenses"
            materials.mkdir()
            for name in stage.WINDOWS_MATERIALS:
                (materials / name).write_text(f"synthetic {name}")
            shutil.copy2(repo / "LICENSE", materials / "LICENSE")
            (materials / "qt-source.tar.xz").write_bytes(b"synthetic Qt source")
            (materials / "QT-SOURCES.json").write_text(json.dumps({
                "qt_version": "6.8.3", "patches": [], "sources": [{
                    "file": "qt-source.tar.xz", "sha256": stage.digest(materials / "qt-source.tar.xz"),
                    "url": "https://example.invalid/qt-source.tar.xz",
                }],
            }))
            hashes = {"osu-radio-qt.exe": "1" * 64, "osu-radio-server.exe": "2" * 64,
                      "helper/osu-lazer-realm-parser.exe": "3" * 64}
            for path in [windows / "BUILD-INFO.txt", *materials.iterdir()]:
                hashes[path.relative_to(windows).as_posix()] = stage.digest(path)
            payload = windows / "payload.sha256"
            payload.write_text("".join(f"{digest}  {name}\n" for name, digest in hashes.items()))

            with patch.object(stage, "ROOT", repo):
                output = base / "candidate"
                stage.assemble(sha, archive, windows, output)
                manifest = json.loads((output / "manifest.json").read_text())
                self.assertEqual(manifest["source_commit"], sha)
                self.assertIsNone(manifest["verified_remote_tag"])
                self.assertEqual(manifest["arch_rust_binaries_sha256"]["usr/bin/osu-radio-qt"],
                                 hashlib.sha256(files["usr/bin/osu-radio-qt"]).hexdigest())
                self.assertIn("arch/osu-radio-bin/.SRCINFO", manifest["files_sha256"])
                self.assertIn("arch/osu-radio-git/.SRCINFO", manifest["files_sha256"])
                for name, expected in manifest["files_sha256"].items():
                    self.assertEqual(stage.digest(output / name), expected, name)
                subprocess.run(["sha256sum", "--check", "--status", "SHA256SUMS"], cwd=output, check=True)
                self.assertIn(stage.digest(archive), (output / "arch/osu-radio-bin/.SRCINFO").read_text())
                with self.assertRaisesRegex(ValueError, "already exists"):
                    stage.assemble(sha, archive, windows, output)

                def rejected(label, pattern, source_sha=sha):
                    destination = base / label
                    with self.assertRaisesRegex((ValueError, FileNotFoundError, KeyError), pattern):
                        stage.assemble(source_sha, archive, windows, destination)
                    self.assertFalse(destination.exists(), "a failed handoff must not leave a candidate")

                rejected("wrong-sha", "source SHA", "f" * 40)
                archive.rename(base / "saved.tar.gz")
                rejected("missing-archive", "missing")
                (base / "saved.tar.gz").rename(archive)
                original = setup.read_bytes()
                setup.unlink()
                rejected("missing-setup", "missing")
                setup.write_bytes(b"altered Setup")
                rejected("altered-setup", "Setup checksum")
                setup.write_bytes(original)
                info = windows / "BUILD-INFO.txt"
                info.write_text(f"source commit: {'b' * 40}\nworking tree: clean\n")
                rejected("mixed-commits", "source commit")
                info.write_bytes(files["BUILD-INFO.txt"])
                material = materials / "DOTNET-NOTICES.txt"
                original = material.read_bytes()
                material.unlink()
                rejected("missing-license", "missing")
                material.write_bytes(b"changed notices")
                rejected("changed-notice", "payload checksum")
                material.write_bytes(original)
                qt_source = materials / "qt-source.tar.xz"
                original = qt_source.read_bytes()
                qt_source.write_bytes(b"different Qt source archive")
                rejected("wrong-qt-source", "Qt source checksum")
                qt_source.write_bytes(original)
                files["BUILD-INFO.txt"] = f"source commit: {sha}\nworking tree: DIRTY\n".encode()
                write_archive()
                rejected("dirty-build", "clean checkout")
                files["BUILD-INFO.txt"] = info.read_bytes()
                del files["usr/bin/osu-radio-server"]
                write_archive()
                rejected("missing-rust-binary", "osu-radio-server")
                (repo / "unexpected-file").touch()
                with self.assertRaisesRegex(ValueError, "clean"):
                    stage.check_source(sha)

    def test_materials_download_rejects_bad_hash_and_unsafe_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "materials"
            buffer = io.BytesIO()
            with tarfile.open(fileobj=buffer, mode="w:gz") as tar:
                member = tarfile.TarInfo("../escape")
                member.size = 1
                tar.addfile(member, io.BytesIO(b"x"))
            data = buffer.getvalue()
            inputs = {"windows_materials_url": "https://example.invalid/materials.tar.gz", "windows_materials_sha256": "0" * 64}
            with patch.object(stage, "read_inputs", return_value=inputs), \
                    patch.object(stage.urllib.request, "urlopen", return_value=io.BytesIO(data)):
                with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                    stage.fetch_materials(destination)
            inputs["windows_materials_sha256"] = hashlib.sha256(data).hexdigest()
            with patch.object(stage, "read_inputs", return_value=inputs), \
                    patch.object(stage.urllib.request, "urlopen", return_value=io.BytesIO(data)):
                with self.assertRaises(tarfile.FilterError):
                    stage.fetch_materials(destination)
            self.assertFalse(destination.exists())
            self.assertFalse((Path(temporary) / "escape").exists())


if __name__ == "__main__":
    unittest.main()
