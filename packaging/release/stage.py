#!/usr/bin/env python3
"""Validate release inputs and assemble a candidate; never publish or move refs."""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import urllib.request


ROOT = Path(__file__).resolve().parents[2]
VERSION = "0.1.0-alpha"
ARCH_NAME = f"osu-radio-{VERSION}-arch-x86_64"
SETUP_NAME = f"osu-radio-{VERSION}-setup.exe"
WINDOWS_MATERIALS = (
    "LICENSE", "QT-NOTICES.txt", "DOTNET-NOTICES.txt", "REALM-NOTICES.txt",
    "MSVC-NOTICES.txt", "RUST-NOTICES.txt", "ASSET-NOTICES.txt",
    "QT-SOURCES.json", "REPLACING-QT.md",
)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def check_source(sha):
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("source SHA must be a full lowercase commit ID")
    if git("rev-parse", "HEAD") != sha or git("rev-parse", f"{sha}^{{commit}}") != sha:
        raise ValueError("checkout does not match source SHA")
    if git("status", "--porcelain", "--untracked-files=all"):
        raise ValueError("source checkout must be clean")
    for app in ("osu-radio-qt", "osu-radio-server"):
        manifest = tomllib.loads((ROOT / "apps" / app / "Cargo.toml").read_text())
        if manifest["package"]["version"] != VERSION:
            raise ValueError(f"{app} version must be {VERSION}")


def read_inputs():
    inputs = json.loads((ROOT / "packaging/release/inputs.json").read_text())
    if not re.fullmatch(r"archlinux@sha256:[0-9a-f]{64}", inputs.get("arch_image") or ""):
        raise ValueError("pin arch_image to a reviewed archlinux@sha256 digest in inputs.json")
    snapshot = inputs.get("arch_snapshot") or ""
    if not re.fullmatch(r"\d{4}/\d{2}/\d{2}", snapshot):
        raise ValueError("pin arch_snapshot to YYYY/MM/DD in inputs.json")
    datetime.datetime.strptime(snapshot, "%Y/%m/%d")
    if not (inputs.get("windows_materials_url") or "").startswith("https://") or not re.fullmatch(
        r"[0-9a-f]{64}", inputs.get("windows_materials_sha256") or ""
    ):
        raise ValueError("reviewed Windows materials HTTPS URL and SHA-256 are required in inputs.json")
    windows = inputs["windows"]
    for key in ("qt", "dotnet_sdk", "msvc", "windows_sdk"):
        if not re.fullmatch(r"\d+(?:\.\d+)+", windows[key]):
            raise ValueError(f"an exact Windows {key} version is required")
    if not windows["dotnet_sdk"].startswith("10."):
        raise ValueError(".NET 10 SDK is required")
    if not windows["inno_url"].startswith("https://") or not re.fullmatch(r"[0-9a-f]{64}", windows["inno_sha256"]):
        raise ValueError("pinned Inno Setup HTTPS URL and SHA-256 are required")
    return inputs


def required_file(path):
    if path.is_symlink() or not path.is_file() or not path.stat().st_size:
        raise ValueError(f"missing, empty or linked file: {path}")
    return path


def check_materials(directory):
    # Presence and hashes are mechanical checks; the license audit remains a human input.
    for name in WINDOWS_MATERIALS:
        required_file(directory / name)
    if digest(directory / "LICENSE") != digest(ROOT / "LICENSE"):
        raise ValueError("Windows project license differs from source")
    if (directory / "NOT-RELEASE-READY.txt").exists():
        raise ValueError("diagnostic T08 materials cannot be used for release staging")
    sources = json.loads((directory / "QT-SOURCES.json").read_text())
    if (sources["qt_version"] != read_inputs()["windows"]["qt"]
            or not isinstance(sources["sources"], list) or not sources["sources"]):
        raise ValueError("Qt source inventory must match the selected Qt version")
    if not isinstance(sources["patches"], list):
        raise ValueError("Qt patches must be listed explicitly (empty for unmodified upstream)")
    for item in sources["sources"] + sources["patches"]:
        relative = PurePosixPath(item["file"])
        if relative.is_absolute() or ".." in relative.parts or "\\" in item["file"]:
            raise ValueError("Qt source paths must stay inside materials")
        path = required_file(directory / relative)
        if not path.resolve().is_relative_to(directory.resolve()) or digest(path) != item["sha256"]:
            raise ValueError(f"Qt source checksum mismatch: {relative}")
        if not item["url"].startswith("https://"):
            raise ValueError("Qt source origin must be recorded as HTTPS")
    return sources


def fetch_materials(destination):
    inputs = read_inputs()
    if destination.exists():
        raise ValueError("materials destination already exists")
    with tempfile.TemporaryDirectory(dir=destination.parent) as temporary:
        archive = Path(temporary) / "materials.tar.gz"
        with urllib.request.urlopen(inputs["windows_materials_url"], timeout=120) as response, archive.open("wb") as output:
            shutil.copyfileobj(response, output)
        if digest(archive) != inputs["windows_materials_sha256"]:
            raise ValueError("Windows materials archive checksum mismatch")
        extracted = Path(temporary) / "extracted"
        with tarfile.open(archive, "r:gz") as tar:
            # No links/devices: these materials are files, never runtime symlinks.
            if any(not (member.isfile() or member.isdir()) for member in tar.getmembers()):
                raise ValueError("materials archive must contain only files and directories")
            tar.extractall(extracted, filter="data")
        check_materials(extracted)
        extracted.rename(destination)


def check_provenance(info, sha):
    lines = info.splitlines()
    if [line for line in lines if line.startswith("source commit:")] != [f"source commit: {sha}"]:
        raise ValueError("artifact source commit does not match")
    if [line for line in lines if line.startswith("working tree:")] != ["working tree: clean"]:
        raise ValueError("artifact was not built from a clean checkout")


def assemble(sha, archive, windows, output):
    check_source(sha)
    inputs = read_inputs()
    if output.exists():
        raise ValueError("staging destination already exists")
    required_file(archive)
    if archive.name != f"{ARCH_NAME}.tar.gz":
        raise ValueError("unexpected Arch archive name")
    setup = required_file(windows / "output" / SETUP_NAME)
    if required_file(windows / "setup.sha256").read_text().strip() != f"{digest(setup)}  {SETUP_NAME}":
        raise ValueError("Setup checksum mismatch")
    windows_info = required_file(windows / "BUILD-INFO.txt").read_text(encoding="utf-8")
    check_provenance(windows_info, sha)
    materials = windows / "licenses"
    qt_sources = check_materials(materials)
    payload = required_file(windows / "payload.sha256")
    payload_hashes = {}
    for line in payload.read_text().splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  (.+)", line)
        if not match or match[2] in payload_hashes:
            raise ValueError("invalid or duplicate Windows payload hash")
        payload_hashes[match[2]] = match[1]
    for name in ("osu-radio-qt.exe", "osu-radio-server.exe", "helper/osu-lazer-realm-parser.exe"):
        if name not in payload_hashes:
            raise ValueError(f"missing Windows payload hash: {name}")
    material_files = sorted(path for path in materials.rglob("*") if path.is_file())
    if {path.relative_to(windows).as_posix() for path in material_files} != {
        name for name in payload_hashes if name.startswith("licenses/")
    }:
        raise ValueError("Windows license handoff does not match the full payload inventory")
    for path in [windows / "BUILD-INFO.txt", *material_files]:
        name = path.relative_to(windows).as_posix()
        if digest(required_file(path)) != payload_hashes.get(name):
            raise ValueError(f"Windows payload checksum mismatch: {name}")

    with tarfile.open(archive, "r:gz") as tar:
        members = tar.getmembers()
        if len({item.name for item in members}) != len(members):
            raise ValueError("duplicate Arch archive paths")

        def read_member(name):
            member = tar.getmember(f"{ARCH_NAME}/{name}")
            if not member.isfile() or not member.size:
                raise ValueError(f"missing regular nonempty archive file: {name}")
            return tar.extractfile(member)

        with read_member("BUILD-INFO.txt") as stream:
            arch_info = stream.read().decode("utf-8")
        check_provenance(arch_info, sha)
        binary_hashes = {}
        for name in ("usr/bin/osu-radio-qt", "usr/bin/osu-radio-server"):
            with read_member(name) as stream:
                binary_hashes[name] = hashlib.file_digest(stream, "sha256").hexdigest()
        for name in ("usr/lib/osu-radio/osu-lazer-realm-parser", "SOURCE-FILES.sha256",
                     "usr/share/licenses/osu-radio/LICENSE", "usr/share/licenses/osu-radio/RUST-NOTICES.txt",
                     "usr/share/licenses/osu-radio/HELPER-NOTICES.txt"):
            with read_member(name):
                pass
        for path in [ROOT / "LICENSE", *sorted((ROOT / "packaging/arch/licenses").iterdir())]:
            with read_member(f"usr/share/licenses/osu-radio/{path.name}") as stream:
                if hashlib.file_digest(stream, "sha256").hexdigest() != digest(path):
                    raise ValueError(f"Arch license material differs from source: {path.name}")

    # Publish the directory only after every operation succeeds; no partial success manifest.
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent) as temporary:
        stage = Path(temporary) / "candidate"
        stage.mkdir()
        shutil.copy2(archive, stage / archive.name)
        shutil.copy2(setup, stage / setup.name)
        shutil.copytree(materials, stage / "licenses/windows")
        shutil.copytree(ROOT / "packaging/arch/licenses", stage / "licenses/arch")
        shutil.copy2(ROOT / "LICENSE", stage / "licenses/arch/LICENSE")
        shutil.copy2(ROOT / "packaging/release/INSTALL.md", stage / "INSTALL.md")
        shutil.copy2(ROOT / "packaging/release/inputs.json", stage / "build-inputs.json")
        (stage / "build-info").mkdir()
        (stage / "build-info/arch.txt").write_text(arch_info, encoding="utf-8")
        (stage / "build-info/windows.txt").write_text(windows_info, encoding="utf-8")
        shutil.copy2(payload, stage / "build-info/windows-payload.sha256")
        subprocess.run([sys.executable, str(ROOT / "packaging/release/stage-arch-recipes.py"),
                        str(archive), sha, str(stage / "arch")], check=True)
        source_name = f"osu-radio-{VERSION}-source.tar.gz"
        subprocess.run(["git", "archive", "--format=tar.gz", f"--prefix=osu-radio-{VERSION}/",
                        f"--output={stage / source_name}", sha], cwd=ROOT, check=True)
        hashes = {path.relative_to(stage).as_posix(): digest(required_file(path))
                  for path in sorted(stage.rglob("*")) if path.is_file()}
        manifest = {
            "version": VERSION, "status": "candidate; final tag and platform acceptance pending",
            "source_commit": sha, "source_archive": source_name,
            "planned_tag": f"v{VERSION}", "verified_remote_tag": None,
            "targets": ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu (Arch)"],
            "arch_git_source": "https://github.com/Jayko321/osu-radio.git#branch=main",
            "inputs": inputs, "qt_sources": qt_sources,
            "ci": {key: os.environ.get(key) for key in (
                "GITHUB_REPOSITORY", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_WORKFLOW_SHA", "GITHUB_WORKFLOW_REF"
            )},
            "build_info": {"windows": windows_info, "arch": arch_info},
            "arch_rust_binaries_sha256": binary_hashes,
            "windows_payload_sha256": payload_hashes, "files_sha256": hashes,
        }
        (stage / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        hashes["manifest.json"] = digest(stage / "manifest.json")
        (stage / "SHA256SUMS").write_text("".join(f"{value}  {name}\n" for name, value in sorted(hashes.items())), encoding="utf-8")
        stage.rename(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    check = commands.add_parser("check")
    check.add_argument("source_sha")
    materials = commands.add_parser("materials")
    materials.add_argument("output", type=Path)
    stage = commands.add_parser("assemble")
    stage.add_argument("source_sha")
    stage.add_argument("archive", type=Path)
    stage.add_argument("windows", type=Path)
    stage.add_argument("output", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "check":
            check_source(args.source_sha)
            inputs = read_inputs()
            if os.environ.get("GITHUB_OUTPUT"):
                with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
                    output.write(f"arch_image={inputs['arch_image']}\ninputs={json.dumps(inputs)}\n")
        elif args.command == "materials":
            fetch_materials(args.output)
        else:
            assemble(args.source_sha, args.archive.resolve(), args.windows.resolve(), args.output.resolve())
    except (ValueError, KeyError, OSError, tarfile.TarError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"release staging: {error}\n")


if __name__ == "__main__":
    main()
