#!/usr/bin/env python3
"""Run the window-local visual probe; desktop rendering requires an explicit screen."""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("output", type=Path)
parser.add_argument("--platform", choices=["offscreen", "xcb"], default="offscreen")
parser.add_argument("--screen")
parser.add_argument("--case", choices=["visual", "visual-flicker"], default="visual")
parser.add_argument("--sizes", nargs="+", default=["1024x640", "1440x952", "1920x1080", "2560x1440"])
parser.add_argument("--scales", nargs="+", default=["1", "1.5", "2"])
args = parser.parse_args()
if args.platform == "xcb" and not args.screen:
    parser.error("desktop checks require --screen")
repo = Path(__file__).resolve().parents[3]
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=True)
results = []
for size in args.sizes:
    width, height = size.split("x")
    for scale in args.scales:
        for gallery in ((False, True) if args.case == "visual" else (False,)):
            label = f"{args.platform}-{size}-{scale}-{'gallery' if gallery else 'live'}"
            if args.case != "visual":
                label += "-" + args.case
            with tempfile.TemporaryDirectory(prefix="osu-visual-") as temp:
                working = Path(temp)
                server = working / "fixture-server"
                shutil.copy(repo / "apps/osu-radio-qt/tests/fixtures/server.py", server)
                server.chmod(0o700)
                shutil.copy(repo / "apps/osu-radio-qt/assets/logos/stable.png", working / "source.png")
                env = os.environ.copy()
                for key in ("SQLITE_DATABASE_URL", "POSTGRES_DATABASE_URL", "QT_QUICK_BACKEND", "QSG_RHI_BACKEND", "OSU_RADIO_QT_PROBE_SCREEN"):
                    env.pop(key, None)
                env.update(QT_QPA_PLATFORM=args.platform, QT_SCALE_FACTOR=scale,
                           QT_FORCE_STDERR_LOGGING="1", QT_LOGGING_RULES="qml.info=true;qml.warning=true",
                           QML_DISABLE_DISK_CACHE="1", OSU_RADIO_QT_SMOKE_TEST="1",
                           OSU_RADIO_QT_PROBE_CASE=args.case, OSU_RADIO_SERVER_BIN=str(server),
                           OSU_RADIO_QT_PROBE_WIDTH=width, OSU_RADIO_QT_PROBE_HEIGHT=height,
                           OSU_RADIO_QT_PROBE_SCREENSHOT=str(output / label))
                if args.screen:
                    env["OSU_RADIO_QT_PROBE_SCREEN"] = args.screen
                if args.platform == "offscreen":
                    env.update(QT_QUICK_BACKEND="software")
                    env.pop("DISPLAY", None)
                    env.pop("WAYLAND_DISPLAY", None)
                command = [str(repo / "target/debug/osu-radio-qt")]
                if gallery:
                    command.append("--component-gallery")
                with (output / f"{label}.log").open("w") as log:
                    process = subprocess.Popen(command, cwd=working, env=env, stdout=log, stderr=log, start_new_session=True)
                    try:
                        code = process.wait(timeout=25)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait()
                        code = -1
                text = (output / f"{label}.log").read_text()
                ok = code == 0 and "Visual probe finished: 0 issues" in text and not any(
                    error in text for error in ("TypeError", "ReferenceError", "Binding loop", "VISUAL ISSUE", "watchdog"))
                results.append({"case": label, "passed": ok, "exit_code": code})
                (output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
                print(f"{'PASS' if ok else 'FAIL'} {label}", flush=True)
raise SystemExit(0 if all(row["passed"] for row in results) else 1)
