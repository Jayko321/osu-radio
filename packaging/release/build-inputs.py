#!/usr/bin/env python3
"""Read build pins for the manually selected source commit."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys


def main():
    sha = sys.argv[1]
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("source_sha must be a full lowercase commit SHA")
    actual = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    if actual != sha:
        raise ValueError("checkout does not match source_sha")
    pins = json.loads(Path(__file__).with_name("inputs.json").read_text())
    if "GITHUB_OUTPUT" in os.environ:
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write(f"arch_image={pins['arch_image']}\n")
            output.write(f"inputs={json.dumps(pins, separators=(',', ':'))}\n")


if __name__ == "__main__":
    main()
