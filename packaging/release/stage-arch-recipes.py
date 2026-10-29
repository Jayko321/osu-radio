#!/usr/bin/env python3
"""Generate Arch recipes outside the source tree from a clean-SHA archive."""

import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import tarfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("source_sha", help="full source commit ID")
    parser.add_argument("output", type=Path, help="new staging directory")
    args = parser.parse_args()

    name = "osu-radio-0.1.0-alpha-arch-x86_64"
    if args.archive.name != f"{name}.tar.gz":
        parser.error("unexpected archive name")
    if not re.fullmatch(r"[0-9a-f]{40}", args.source_sha):
        parser.error("source SHA must be a full lowercase commit ID")
    root = Path(__file__).resolve().parents[2]
    if subprocess.run(
        ["git", "cat-file", "-e", f"{args.source_sha}^{{commit}}"],
        cwd=root, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    ).returncode:
        parser.error("source commit does not exist in this checkout")
    with tarfile.open(args.archive, "r:gz") as archive:
        info = archive.extractfile(f"{name}/BUILD-INFO.txt").read().decode("utf-8")
    if f"source commit: {args.source_sha}" not in info.splitlines():
        parser.error("archive source commit does not match")
    if "working tree: clean" not in info.splitlines():
        parser.error("archive was not built from a clean working tree")

    with args.archive.open("rb") as archive_file:
        digest = hashlib.file_digest(archive_file, "sha256").hexdigest()
    templates = {}
    for variant in ("osu-radio-git", "osu-radio-bin"):
        result = subprocess.run(
            ["git", "show", f"{args.source_sha}:packaging/arch/{variant}/PKGBUILD"],
            cwd=root, capture_output=True, text=True,
        )
        if result.returncode:
            parser.error(f"source commit has no {variant} recipe")
        templates[variant] = result.stdout
    binary_template = templates["osu-radio-bin"]
    placeholder = "0" * 64
    if binary_template.count(placeholder) != 1:
        parser.error("binary recipe does not contain exactly one checksum placeholder")
    args.output.mkdir(parents=True, exist_ok=False)
    for variant in ("osu-radio-git", "osu-radio-bin"):
        destination = args.output / variant
        destination.mkdir()
        template = templates[variant]
        if variant.endswith("-bin"):
            template = template.replace(placeholder, digest).replace(
                "# T09 replaces this deliberately invalid digest with the final release asset SHA-256.\n", ""
            )
        (destination / "PKGBUILD").write_text(template)
        srcinfo = subprocess.run(
            ["makepkg", "--printsrcinfo"], cwd=destination, check=True, capture_output=True
        ).stdout
        (destination / ".SRCINFO").write_bytes(srcinfo)
    print(f"archive SHA-256: {digest}\nrecipes: {args.output}")


if __name__ == "__main__":
    main()
