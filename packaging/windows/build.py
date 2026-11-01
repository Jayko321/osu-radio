#!/usr/bin/env python3
"""Build a Windows candidate in an MSVC developer environment; no publishing."""

import argparse
import tomllib
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[2]
REQUIRED = (
    "osu-radio-qt.exe", "osu-radio-server.exe", "Qt6Core.dll",
    "platforms/qwindows.dll", "helper/osu-lazer-realm-parser.exe",
    "helper/osu-lazer-realm-parser.dll", "helper/coreclr.dll",
    "helper/Realm.dll", "helper/realm-wrappers.dll",
    "vcruntime140.dll", "msvcp140.dll", "helper/vcruntime140.dll",
    "helper/msvcp140.dll",
)


def check_payload(stage):
    missing = [name for name in REQUIRED if not (stage / name).is_file() or not (stage / name).stat().st_size]
    if missing:
        raise ValueError("missing or empty staged files: " + ", ".join(missing))


def run(args, **kwargs):
    print("+", *map(str, args), flush=True)
    subprocess.run(args, check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qt", type=Path, required=True, help="Qt MSVC x64 installation")
    parser.add_argument("--redist", type=Path, required=True, help="MSVC x64 app-local redistributable DLL directory")
    parser.add_argument("--iscc", type=Path, required=True, help="Inno Setup ISCC.exe")
    parser.add_argument("--materials", type=Path, required=True, help="directory of license and notice files to include")
    parser.add_argument("--work", type=Path, required=True, help="new build and staging directory")
    parser.add_argument("--qt-version", required=True, help="expected exact Qt version")
    parser.add_argument("--dotnet-sdk", required=True, help="expected exact .NET 10 SDK version")
    parser.add_argument("--msvc-version", required=True, help="expected exact VCToolsVersion")
    parser.add_argument("--windows-sdk", required=True, help="expected exact WindowsSDKVersion")
    parser.add_argument("--iscc-sha256", required=True, help="expected ISCC.exe SHA-256")
    args = parser.parse_args()
    if sys.platform != "win32":
        parser.error("run in a Windows MSVC developer environment")
    qt, redist, iscc, materials, work = (p.resolve() for p in (args.qt, args.redist, args.iscc, args.materials, args.work))
    for path in (qt / "bin/qmake.exe", qt / "bin/windeployqt.exe", iscc,
                 redist / "vcruntime140.dll", redist / "msvcp140.dll"):
        if not path.is_file():
            parser.error(f"required tool or runtime file missing: {path}")
    if not materials.is_dir() or not any(materials.iterdir()):
        parser.error("a nonempty license and notice directory is required")
    env = os.environ.copy()
    env["PATH"] = str(qt / "bin") + os.pathsep + env["PATH"]
    env["QMAKE"] = str(qt / "bin/qmake.exe")
    env["CARGO_TARGET_DIR"] = str(work / "cargo-target")
    env["OSU_LAZER_REALM_PARSER_PATH"] = r"helper\osu-lazer-realm-parser.exe"
    env["DOTNET_NOLOGO"] = "1"
    def capture(command):
        return subprocess.check_output(command, cwd=ROOT, env=env, text=True).strip()

    qt_version = capture([str(qt / "bin/qmake.exe"), "-query", "QT_VERSION"])
    dotnet_sdk = capture(["dotnet", "--version"])
    iscc_sha = hashlib.sha256(iscc.read_bytes()).hexdigest()
    actual = {"Qt": qt_version, ".NET SDK": dotnet_sdk, "MSVC": os.environ.get("VCToolsVersion", ""),
              "Windows SDK": os.environ.get("WindowsSDKVersion", "").rstrip("\\/"), "ISCC SHA-256": iscc_sha}
    expected = {"Qt": args.qt_version, ".NET SDK": args.dotnet_sdk, "MSVC": args.msvc_version,
                "Windows SDK": args.windows_sdk.rstrip("\\/"), "ISCC SHA-256": args.iscc_sha256.lower()}
    mismatches = [f"{name}: expected {value}, got {actual[name]}" for name, value in expected.items() if actual[name] != value]
    if not dotnet_sdk.startswith("10."):
        mismatches.append(".NET SDK must be version 10")
    if mismatches:
        parser.error("build tool pins do not match: " + "; ".join(mismatches))
    work.mkdir(parents=True, exist_ok=False)
    stage, output = work / "stage", work / "output"
    stage.mkdir()
    output.mkdir()
    helper = stage / "helper"
    run(["dotnet", "publish", str(ROOT / "tools/osu-lazer-realm-parser/osu-lazer-realm-parser.csproj"),
         "--configuration", "Release", "--runtime", "win-x64", "--self-contained", "true", "--output", str(helper)], cwd=ROOT, env=env)
    run(["cargo", "build", "--release", "--locked", "--target", "x86_64-pc-windows-msvc",
         "-p", "osu-radio-qt", "-p", "osu-radio-server"], cwd=ROOT, env=env)
    for name in ("osu-radio-qt.exe", "osu-radio-server.exe"):
        shutil.copy2(work / "cargo-target/x86_64-pc-windows-msvc/release" / name, stage / name)
    run([qt / "bin/windeployqt.exe", "--release", "--no-compiler-runtime", "--qmldir",
         ROOT / "apps/osu-radio-qt/qml", stage / "osu-radio-qt.exe"], cwd=ROOT, env=env)
    for dll in redist.glob("*.dll"):
        shutil.copy2(dll, stage / dll.name)
        shutil.copy2(dll, helper / dll.name)
    shutil.copytree(materials, stage / "licenses")
    check_payload(stage)
    provenance = [
        f"source commit: {capture(['git', 'rev-parse', 'HEAD'])}",
        "working tree: " + ("DIRTY" if capture(["git", "status", "--porcelain", "--untracked-files=all"]) else "clean"),
        "working tree status:", capture(["git", "status", "--porcelain", "--untracked-files=all"]),
        f"OS: {sys.getwindowsversion()}; architecture: x86_64",
        f"python: {sys.version}",
        f"rust: {capture(['rustc', '--version'])}",
        f"dotnet SDK: {dotnet_sdk}",
        f"Qt: {qt_version}",
        f"MSVC: {actual['MSVC']}; Windows SDK: {actual['Windows SDK']}",
        "cargo: --release --locked --target x86_64-pc-windows-msvc; default features",
        "helper: .NET 10 win-x64 self-contained, Realm 20.1.0",
        "helper runtime: " + json.dumps(json.loads((helper / "osu-lazer-realm-parser.runtimeconfig.json").read_text())["runtimeOptions"], sort_keys=True),
        "ISCC SHA-256: " + iscc_sha,
    ]
    (stage / "BUILD-INFO.txt").write_text("\n".join(provenance) + "\n", encoding="utf-8")
    with (work / "payload.sha256").open("w", encoding="utf-8", newline="\n") as hashes:
        for path in sorted(stage.rglob("*")):
            if path.is_file():
                with path.open("rb") as file:
                    digest = hashlib.file_digest(file, "sha256").hexdigest()
                hashes.write(f"{digest}  {path.relative_to(stage).as_posix()}\n")
    version = tomllib.loads((ROOT / "apps/osu-radio-qt/Cargo.toml").read_text())["package"]["version"]
    run([iscc, f"/DAppVersion={version}", f"/DStageDir={stage}", f"/DOutputDir={output}", ROOT / "packaging/windows/osu-radio.iss"], cwd=ROOT, env=env)
    setup = output / f"osu-radio-{version}-setup.exe"
    if not setup.is_file() or not setup.stat().st_size:
        raise ValueError("Inno Setup did not create the installer")
    with setup.open("rb") as file:
        setup_digest = hashlib.file_digest(file, "sha256").hexdigest()
    (work / "setup.sha256").write_text(f"{setup_digest}  {setup.name}\n", encoding="utf-8")
    print(f"Candidate: {setup}. Review provenance, dependencies and licenses before release.")


if __name__ == "__main__":
    main()
