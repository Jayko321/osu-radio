#!/usr/bin/env python3
"""Measure a saved SQLite server binary against a consistent database backup.

Run from a directory with a .env file (required by the current server startup).
Each query has one warmup and five samples. Payloads stay in --output for exact
comparison and the opt-in Rust captured_client_timings test.
"""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess
import time
from urllib.parse import urlencode
from urllib.request import urlopen

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--binary", type=Path, required=True)
parser.add_argument("--database-backup", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--stage", required=True)
parser.add_argument("--profile", choices=("debug", "release"), required=True)
parser.add_argument("--endpoint", choices=("beatmap-sets", "tracks"), default="beatmap-sets")
args = parser.parse_args()
if not args.database_backup.is_file():
    parser.error("--database-backup must name an existing SQLite backup")
args.output.mkdir(parents=True, exist_ok=True)
prefix = f"{args.stage}-{args.profile}-{args.endpoint}"
env = dict(os.environ, SQLITE_DATABASE_URL=str(args.database_backup.resolve()),
           OSU_RADIO_SERVER_ADDRESS="127.0.0.1:0")
with (args.output / f"{prefix}.stderr").open("w") as errors:
    child = subprocess.Popen([str(args.binary.resolve())], env=env, stdout=subprocess.PIPE,
                             stderr=errors, text=True)
    try:
        line = child.stdout.readline().strip()
        if "listening on " not in line:
            raise RuntimeError(f"Server failed readiness; inspect {prefix}.stderr")
        base_url = line.split("listening on ", 1)[1]
        results = []
        for index, query in enumerate(("rock", "rock hard", "星", "does-not-exist-zzzz", "")):
            timings = []
            for sample in range(6):
                start = time.perf_counter()
                with urlopen(base_url + "/api/" + args.endpoint + "?" + urlencode({"q": query}), timeout=30) as response:
                    data = response.read()
                elapsed = (time.perf_counter() - start) * 1000
                if sample:
                    timings.append(elapsed)
            rows = json.loads(data)
            (args.output / f"{prefix}-{index}.json").write_bytes(data)
            reference = args.output / f"baseline-{args.profile}-beatmap-sets-{index}.json"
            if args.endpoint == "beatmap-sets" and reference.exists():
                assert rows == json.loads(reference.read_bytes()), query
            results.append(dict(query=query, bytes=len(data), rows=len(rows),
                                http_ms=statistics.median(timings), samples_ms=timings))
        (args.output / f"{prefix}-timings.json").write_text(json.dumps(results, ensure_ascii=False, indent=2))
        print(json.dumps(results, ensure_ascii=False, indent=2))
    finally:
        child.terminate()
        try:
            child.wait(timeout=10)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()
