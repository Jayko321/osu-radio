"""Check Qt verification-tool orchestration with inert processes and temporary files."""
import contextlib
import io
import importlib.util
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from urllib.error import HTTPError
from urllib.request import ProxyHandler, Request, build_opener
from unittest.mock import MagicMock, patch
import struct
import zlib


APP = Path(__file__).resolve().parents[1]


class EmbeddedResourceManifestTests(unittest.TestCase):
    def test_probe_lists_every_bundled_image_and_font_once(self):
        text = (APP / "tests/VisualProbe.qml").read_text()
        entries = re.findall(r'"qrc:/assets/([^"\n]+\.(?:png|svg|ttf))"', text)
        expected = {str(path.relative_to(APP / "assets")) for path in (APP / "assets").rglob("*") if path.suffix.lower() in {".png", ".svg", ".ttf"}}
        self.assertEqual(len(entries), len(set(entries)), "duplicate asset probe entries")
        self.assertEqual(set(entries), expected, "new assets need a decode assertion in the QML probe")


class HttpFixtureTests(unittest.TestCase):
    source = APP / "tests/fixtures/server.py"
    compiled = APP / "tests/fixtures/__pycache__/server.cpython-314.pyc"

    def environment(self, case):
        environment = {"PATH": os.defpath, "PYTHONNOUSERSITE": "1", "OSU_RADIO_QT_PROBE_CASE": case}
        if "SystemRoot" in os.environ:
            environment["SystemRoot"] = os.environ["SystemRoot"]
        return environment

    @contextlib.contextmanager
    def server(self, case, artifact=None, root=None):
        with contextlib.ExitStack() as stack:
            if root is None:
                root = Path(stack.enter_context(tempfile.TemporaryDirectory(prefix="qt-http-fixture-")))
            output = stack.enter_context(tempfile.TemporaryFile())
            process = subprocess.Popen(
                [sys.executable, "-B", str(artifact or self.source)], cwd=root,
                env=self.environment(case), stdout=output, stderr=output,
            )
            try:
                deadline = time.monotonic() + 4
                while True:
                    output.seek(0)
                    log = output.read().decode(errors="replace")
                    match = re.search(r"listening on (http://127\.0\.0\.1:\d+)", log)
                    if match:
                        break
                    self.assertIsNone(process.poll(), log)
                    self.assertLess(time.monotonic(), deadline, log)
                    time.sleep(0.01)
                self.assertEqual((root / "child.pid").read_text(), str(process.pid))
                yield match[1], root
            finally:
                if process.poll() is None:
                    process.terminate()
                try:
                    process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=2)

    def request(self, base, method, path, payload=None):
        data = payload if isinstance(payload, bytes) else json.dumps(payload).encode() if payload is not None else None
        request = Request(base + path, data=data, method=method)
        opener = build_opener(ProxyHandler({}))
        try:
            response = opener.open(request, timeout=3)
        except HTTPError as error:
            response = error
        with response:
            body = response.read()
            if response.headers.get("Content-Length") is not None:
                self.assertEqual(int(response.headers["Content-Length"]), len(body))
            kind = response.headers.get("Content-Type", "")
            value = json.loads(body) if kind == "application/json" else body
            return response.status, value

    def test_startup_retry_and_independent_library_folder_failures_are_deterministic(self):
        with tempfile.TemporaryDirectory(prefix="qt-http-startup-") as temporary:
            root = Path(temporary)
            failed = subprocess.run([sys.executable, "-B", str(self.source)], cwd=root,
                                    env=self.environment("fixture"), capture_output=True, timeout=3)
            self.assertEqual(failed.returncode, 1)
            self.assertIn(b"first startup intentionally unsuccessful", failed.stderr)
            self.assertEqual((root / "starts").read_text(), "1")
            with self.server("fixture", root=root) as (base, _):
                self.assertEqual((root / "starts").read_text(), "2")
                self.assertEqual(self.request(base, "GET", "/api/tracks")[0], 500)
                self.assertEqual([row["id"] for row in self.request(base, "GET", "/api/user-data/osu-folders")[1]], [31, 52])
                self.assertEqual([row["audio_source_id"] for row in self.request(base, "GET", "/api/tracks")[1]], [7, 42, 103])
                self.assertEqual(self.request(base, "GET", "/api/user-data/osu-folders")[0], 500)
                self.assertEqual(self.request(base, "GET", "/api/tracks")[0], 500)
                self.assertEqual([row["audio_source_id"] for row in self.request(base, "GET", "/api/tracks")[1]], [103])
                self.assertEqual(self.request(base, "GET", "/api/tracks"), (200, []))
                self.assertEqual(self.request(base, "GET", "/unknown")[0], 404)

    def test_playlist_cover_retry_keeps_the_created_id_and_binary_payload(self):
        with self.server("playlist-covers") as (base, root):
            png = (root / "source.png").read_bytes()
            status, created = self.request(base, "POST", "/api/playlists", {"name": "  名前 / Тест  "})
            self.assertEqual((status, created["id"], created["name"], created["item_count"]), (201, 1, "名前 / Тест", 0))
            self.assertEqual(self.request(base, "PUT", "/api/playlists/1/cover", png)[0], 500)
            self.assertFalse((root / "source.png").exists())
            self.assertEqual(self.request(base, "PUT", "/api/playlists/1/cover", png)[1]["custom_cover_revision"], 2)
            self.assertEqual(self.request(base, "GET", "/api/playlists/1/cover"), (200, png))
            self.assertEqual((root / "uploaded-cover.png").read_bytes(), png)
            self.assertEqual(len(self.request(base, "GET", "/api/playlists")[1]), 1)
            self.assertEqual(self.request(base, "PATCH", "/api/playlists/1", {"name": " Renamed "})[1]["name"], "Renamed")
            self.assertEqual(self.request(base, "DELETE", "/api/playlists/1/cover")[0], 204)
            self.assertEqual(self.request(base, "GET", "/api/playlists/1/cover")[0], 404)
            self.assertEqual(self.request(base, "DELETE", "/api/playlists/1")[0], 204)
            self.assertEqual(self.request(base, "GET", "/api/playlists/1")[0], 404)
            self.assertEqual(self.request(base, "GET", "/api/playlists"), (200, []))

    def test_queue_ids_survive_position_changes_and_rejections(self):
        with self.server("queue-managed") as (base, root):
            initial = self.request(base, "GET", "/api/queue")[1]
            self.assertEqual(len(initial["entries"]), 5005)
            self.assertEqual(initial["history_len"], 5000)
            maximum = "9223372036854775807"
            duplicate = "9007199254740995"
            checking = "9007199254740997"
            self.assertEqual([row["audio_source_id"] for row in initial["entries"][-4:-2]], [42, 42])
            status, _ = self.request(base, "PATCH", "/api/queue/items/" + checking, {"before_entry_id": maximum})
            self.assertEqual(status, 409)
            self.assertEqual(self.request(base, "GET", "/api/queue")[1], initial)
            self.assertEqual(self.request(base, "PATCH", "/api/queue/items/" + maximum, {"before_entry_id": None})[0], 200)
            self.assertFalse((root / "queue_viewport_requests").exists())
            self.assertEqual(self.request(base, "PATCH", "/api/queue/items/" + maximum, {"before_entry_id": maximum})[0], 200)
            checkpoint = (root / "queue_viewport_requests").read_text()
            self.assertEqual(checkpoint, (root / "requests").read_text())
            self.request(base, "GET", "/api/queue")
            self.request(base, "PATCH", "/api/queue/items/" + maximum, {"before_entry_id": maximum})
            self.assertEqual((root / "queue_viewport_requests").read_text(), checkpoint)
            moved = self.request(base, "GET", "/api/queue")[1]
            self.assertEqual(moved["entries"][-1]["id"], maximum)
            self.assertEqual(self.request(base, "DELETE", "/api/queue/items/" + duplicate)[0], 200)
            self.assertIn(maximum, [row["id"] for row in self.request(base, "GET", "/api/queue")[1]["entries"]])
            self.assertEqual(self.request(base, "POST", "/api/queue/items/100000/play")[0], 200)
            past = self.request(base, "GET", "/api/queue")[1]
            self.assertEqual(past["current_entry_id"], "100000")
            self.assertEqual(len(past["entries"]), 5004)
            self.assertEqual(self.request(base, "DELETE", "/api/queue/upcoming")[0], 200)
            self.assertEqual(len(self.request(base, "GET", "/api/queue")[1]["entries"]), 1)
            self.assertEqual(self.request(base, "DELETE", "/api/queue")[0], 200)
            self.assertEqual(self.request(base, "GET", "/api/queue")[1]["entries"], [])
            operations = [json.loads(line) for line in (root / "queue_operations").read_text().splitlines()]
            self.assertEqual(operations[0]["body"], {"before_entry_id": maximum})
            self.assertEqual(operations[1]["path"], "/api/queue/items/" + maximum)

    def test_insertions_keep_assignment_and_all_playlist_item_snapshots(self):
        with self.server("queue-insert") as (base, _):
            before = self.request(base, "GET", "/api/playback")[1]
            for placement in ("end", "next"):
                status, assignment = self.request(base, "POST", "/api/queue/items", {
                    "items": [{"type": "audio", "audio_source_id": 42}], "placement": placement})
                self.assertEqual(status, 200)
                for key in ("current_entry_id", "playback_token", "mode"):
                    self.assertEqual(assignment[key], before[key])
            self.assertEqual(self.request(base, "POST", "/api/playlists/1/queue")[0], 200)
            confirmed = self.request(base, "GET", "/api/queue")[1]
            rows = confirmed["entries"][-3:]
            self.assertEqual([row["playlist_item_id"] for row in rows], [11, 12, 13])
            self.assertEqual([row["audio_source_id"] for row in rows], [42, 42, None])
            self.assertEqual([row["difficulty_name"] for row in rows], ["Easy", "Hard", "Missing"])
            self.assertEqual(rows[-1]["availability"], "unavailable")
            self.assertEqual(self.request(base, "POST", "/api/queue/items", {
                "items": [{"type": "audio", "audio_source_id": 999}], "placement": "end"})[0], 409)
            self.assertEqual(self.request(base, "GET", "/api/queue")[1], confirmed)

    def test_volume_retry_queue_tokens_and_stale_callbacks_preserve_state(self):
        with self.server("volume") as (base, root):
            self.assertEqual(self.request(base, "GET", "/api/user-data/audio-settings")[0], 500)
            self.assertEqual(self.request(base, "GET", "/api/user-data/audio-settings")[1]["global_volume_percent"], 20)
            self.assertEqual(self.request(base, "PATCH", "/api/user-data/audio-settings", {"global_volume_percent": 40})[1]["global_volume_percent"], 40)
            self.assertEqual(self.request(base, "PUT", "/api/audio-sources/42/volume", {"volume_percent": 10})[0], 500)
            self.assertEqual(self.request(base, "PUT", "/api/audio-sources/42/volume", {"volume_percent": 10})[0], 204)
            self.assertEqual(json.loads((root / "audio-volumes.json").read_text()), {"42": 10})
            self.assertEqual(self.request(base, "GET", "/api/tracks")[1][1]["volume_percent"], 10)
            queue = self.request(base, "GET", "/api/queue")[1]
            self.assertEqual(([e["audio_source_id"] for e in queue["entries"]], queue["current_entry_id"], queue["history_len"], queue["mode"]), ([7, 42, 103], "1", 0, "paused"))
            moved = self.request(base, "POST", "/api/playback/commands", {"command": "next"})[1]
            self.assertEqual((moved["current_audio_source_id"], moved["mode"], moved["playback_token"]), (42, "paused", 2))
            self.assertEqual(self.request(base, "POST", "/api/playback/commands", {"command": "finished", "playback_token": 1})[1], moved)
            self.assertEqual(self.request(base, "POST", "/api/playback/commands", {"command": "previous"})[1]["current_audio_source_id"], 7)
            self.assertEqual(self.request(base, "DELETE", "/api/audio-sources/42/volume")[0], 204)
            self.assertEqual(json.loads((root / "audio-volumes.json").read_text()), {})

    def test_discovery_count_and_import_failures_retry_only_the_requested_marker(self):
        with self.server("folders") as (base, root):
            status, data = self.request(base, "POST", "/api/user-data/osu-folders/discover", {"roots": ["/synthetic"]})
            events = [json.loads(line) for line in data.splitlines()]
            self.assertEqual(status, 200)
            self.assertEqual([event["event"] for event in events], ["candidate"] * 4 + ["complete"])
            self.assertEqual(events[1]["marker_path"], events[2]["marker_path"])
            marker = {"marker_path": "/fixtures/102/client.realm"}
            self.assertEqual(self.request(base, "POST", "/api/user-data/osu-folders/metadata", marker)[0], 500)
            self.assertEqual(self.request(base, "POST", "/api/user-data/osu-folders/metadata", marker), (200, {"beatmap_count": 0}))
            self.assertEqual(self.request(base, "POST", "/api/user-data/osu-folders/import", marker)[0], 500)
            self.assertEqual(self.request(base, "POST", "/api/user-data/osu-folders/import", marker)[1]["id"], 102)
            stable = self.request(base, "POST", "/api/user-data/osu-folders/import", {"marker_path": "/fixtures/101/osu!.db"})[1]
            self.assertEqual((stable["kind"], stable["marker_path"]), ("stable", "/fixtures/101/osu!.db"))
            self.assertEqual(self.request(base, "POST", "/api/user-data/osu-folders/101/import", {})[1]["kind"], "stable")
            self.assertEqual(self.request(base, "DELETE", "/api/user-data/osu-folders/102")[0], 204)
            self.assertEqual([row["id"] for row in self.request(base, "GET", "/api/user-data/osu-folders")[1]], [31, 52, 101])
            log = (root / "requests").read_text()
            self.assertEqual(log.count("POST /api/user-data/osu-folders/import /fixtures/102/client.realm"), 2)

    def test_large_rgba_png_and_duplicate_playlist_rows_match_visual_probe_inputs(self):
        with self.server("visual-flicker") as (base, _):
            tracks = self.request(base, "GET", "/api/tracks")[1]
            self.assertEqual((len(tracks), tracks[0]["title"], tracks[-1]["title"]), (80, "Track 0001", "Track 0080"))
            items = self.request(base, "GET", "/api/playlists/1")[1]["items"]
            self.assertEqual([item["id"] for item in items], [1, 2])
            self.assertEqual([item["audio_source_id"] for item in items], [1, 1])
            png = self.request(base, "GET", "/api/beatmaps/1/cover")[1]
            self.assertEqual(png[:8], b"\x89PNG\r\n\x1a\n")
            chunks = {}; offset = 8
            while offset < len(png):
                size = struct.unpack(">I", png[offset:offset + 4])[0]
                kind = png[offset + 4:offset + 8]
                data = png[offset + 8:offset + 8 + size]
                checksum = struct.unpack(">I", png[offset + 8 + size:offset + 12 + size])[0]
                self.assertEqual(checksum, zlib.crc32(kind + data))
                chunks[kind] = data
                offset += size + 12
            self.assertEqual(offset, len(png))
            self.assertEqual(struct.unpack(">IIBBBBB", chunks[b"IHDR"]), (1280, 1280, 8, 6, 0, 0, 0))
            self.assertEqual(zlib.decompress(chunks[b"IDAT"]), (b"\0" + bytes((80, 100, 120, 255)) * 1280) * 1280)

    def test_retained_python314_bytecode_runs_its_legacy_fixture_contract_without_rewriting(self):
        original = self.compiled.read_bytes()
        self.assertEqual(original[2:4], b"\r\n")
        self.assertIn(int.from_bytes(original[4:8], "little"), (0, 1, 3))
        if original[:4] != importlib.util.MAGIC_NUMBER:
            self.skipTest("executing the retained Python 3.14 bytecode needs its matching interpreter")
        # This retained artifact predates source fixes; validate its supported legacy path separately.
        with self.server("sorting", artifact=self.compiled) as (base, _):
            tracks = self.request(base, "GET", "/api/tracks")[1]
            self.assertEqual([row["audio_source_id"] for row in tracks], [7, 42, 103])
            self.assertEqual([row["artist"] for row in tracks], ["Zulu", "Alpha", "Beta"])
            self.assertEqual(self.request(base, "GET", "/api/audio-sources/7/duration"), (200, {"duration_ms": 125000}))
            self.assertEqual(self.request(base, "GET", "/api/beatmaps/7/cover"), (200, b"P6\n2 2\n255\n" + bytes((80, 100, 120)) * 4))
            self.assertEqual(self.request(base, "POST", "/api/playlists", {"name": " Legacy "})[1]["name"], "Legacy")
            self.assertEqual(self.request(base, "GET", "/api/playlists/1")[1]["items"], [])
            self.assertEqual(self.request(base, "DELETE", "/api/playlists/1")[0], 204)
            self.assertEqual(self.request(base, "GET", "/api/playlists"), (200, []))
        self.assertEqual(self.compiled.read_bytes(), original)


@unittest.skipUnless(os.name == "posix", "visual-audit.py manages Unix process groups")
class VisualAuditTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="qt-tools-test-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.output = self.root / "reports"
        self.runs = []
        self.log_text = "Visual probe finished: 0 issues\n"
        self.exit_code = 0
        self.timeout = False
        self.spawn_error = None
        self.stdout = io.StringIO()
        self.stderr = io.StringIO()

    def spawn(self, command, *, cwd, env, stdout, stderr, start_new_session):
        self.assertEqual(stdout, stderr)
        self.assertTrue(start_new_session)
        working = Path(cwd)
        self.assertEqual((working / "source.png").read_bytes(), (APP / "assets/logos/stable.png").read_bytes())
        self.assertEqual((working / "fixture-server").read_bytes(), (APP / "tests/fixtures/server.py").read_bytes())
        self.assertEqual((working / "fixture-server").stat().st_mode & 0o777, 0o700)
        process = MagicMock(pid=4321)
        process.wait.side_effect = [subprocess.TimeoutExpired(command, 25), 0] if self.timeout else [self.exit_code]
        self.runs.append((command, working, env.copy(), process))
        if self.spawn_error:
            raise self.spawn_error
        stdout.write(self.log_text)
        return process

    def run_script(self, *options):
        environment = {
            "SQLITE_DATABASE_URL": "must-not-open", "POSTGRES_DATABASE_URL": "must-not-open",
            "QT_QUICK_BACKEND": "inherited", "QSG_RHI_BACKEND": "inherited",
            "OSU_RADIO_QT_PROBE_SCREEN": "inherited-screen",
            "XDG_CONFIG_HOME": "must-not-use",
            "DISPLAY": "fixture-display", "WAYLAND_DISPLAY": "fixture-wayland", "UNRELATED": "retained",
        }
        with patch.object(sys, "argv", ["visual-audit.py", str(self.output), *options]), \
                patch.dict(os.environ, environment, clear=True), \
                patch("subprocess.Popen", side_effect=self.spawn), \
                patch("os.killpg") as kill_group, \
                contextlib.redirect_stdout(self.stdout), contextlib.redirect_stderr(self.stderr):
            self.kill_group = kill_group
            with self.assertRaises(SystemExit) as result:
                runpy.run_path(str(APP / "scripts/visual-audit.py"), run_name="__main__")
        return result.exception.code

    def test_default_matrix_is_isolated_and_includes_live_and_offline_gallery(self):
        self.assertEqual(self.run_script(), 0)
        self.assertEqual(len(self.runs), 24)
        rows = json.loads((self.output / "results.json").read_text())
        self.assertEqual(len({row["case"] for row in rows}), 24)
        self.assertTrue(all(row["passed"] and row["exit_code"] == 0 for row in rows))
        for command, working, env, process in self.runs:
            self.assertEqual(command[0], str(APP.parents[1] / "target/debug/osu-radio-qt"))
            self.assertIn(command[1:], ([], ["--component-gallery"]))
            self.assertEqual(env["QT_QPA_PLATFORM"], "offscreen")
            self.assertEqual(env["QT_QUICK_BACKEND"], "software")
            self.assertEqual(env["UNRELATED"], "retained")
            self.assertEqual(env["XDG_CONFIG_HOME"], str(working / "config"))
            for key in ["SQLITE_DATABASE_URL", "POSTGRES_DATABASE_URL", "QSG_RHI_BACKEND", "DISPLAY", "WAYLAND_DISPLAY", "OSU_RADIO_QT_PROBE_SCREEN"]:
                self.assertNotIn(key, env)
            self.assertEqual(env["OSU_RADIO_SERVER_BIN"], str(working / "fixture-server"))
            self.assertEqual(env["OSU_RADIO_QT_SMOKE_TEST"], "1")
            self.assertEqual(env["QML_DISABLE_DISK_CACHE"], "1")
            process.wait.assert_called_once_with(timeout=25)
            self.assertFalse(working.exists(), "temporary fixture must be removed")
        self.kill_group.assert_not_called()

    def test_desktop_requires_an_explicit_screen_before_creating_output(self):
        self.assertEqual(self.run_script("--platform", "xcb"), 2)
        self.assertIn("desktop checks require --screen", self.stderr.getvalue())
        self.assertEqual(self.runs, [])
        self.assertFalse(self.output.exists())

    def test_desktop_screen_and_geometry_preserve_display_without_software_override(self):
        self.assertEqual(self.run_script("--platform", "xcb", "--screen", "Screen 2", "--sizes", "1440x952", "--scales", "1.5"), 0)
        self.assertEqual(len(self.runs), 2)
        for _, _, env, _ in self.runs:
            self.assertEqual(env["OSU_RADIO_QT_PROBE_SCREEN"], "Screen 2")
            self.assertEqual(env["OSU_RADIO_QT_PROBE_WIDTH"], "1440")
            self.assertEqual(env["OSU_RADIO_QT_PROBE_HEIGHT"], "952")
            self.assertEqual(env["QT_SCALE_FACTOR"], "1.5")
            self.assertEqual(env["DISPLAY"], "fixture-display")
            self.assertEqual(env["WAYLAND_DISPLAY"], "fixture-wayland")
            self.assertNotIn("QT_QUICK_BACKEND", env)
            self.assertNotIn("QSG_RHI_BACKEND", env)

    def test_flicker_probe_runs_only_live_and_labels_its_screenshots(self):
        self.assertEqual(self.run_script("--case", "visual-flicker", "--sizes", "1024x640", "--scales", "2"), 0)
        self.assertEqual(len(self.runs), 1)
        command, _, env, _ = self.runs[0]
        self.assertEqual(len(command), 1)
        self.assertEqual(env["OSU_RADIO_QT_PROBE_CASE"], "visual-flicker")
        self.assertEqual(env["OSU_RADIO_QT_PROBE_SCREENSHOT"], str(self.output / "offscreen-1024x640-2-live-visual-flicker"))

    def test_diagnostics_missing_marker_and_exit_errors_cannot_report_success(self):
        for diagnostic in ["TypeError", "ReferenceError", "Binding loop", "VISUAL ISSUE", "watchdog", "missing-marker", "nonzero-exit"]:
            with self.subTest(diagnostic=diagnostic):
                self.exit_code = 7 if diagnostic == "nonzero-exit" else 0
                self.log_text = "unrelated output" if diagnostic == "missing-marker" else "Visual probe finished: 0 issues\n" + diagnostic
                self.assertEqual(self.run_script("--case", "visual-flicker", "--sizes", "1024x640", "--scales", "1"), 1)
                rows = json.loads((self.output / "results.json").read_text())
                self.assertFalse(rows[0]["passed"])
                self.assertEqual(rows[0]["exit_code"], self.exit_code)

    def test_timeout_kills_the_session_reaps_it_and_records_failure(self):
        self.timeout = True
        self.assertEqual(self.run_script("--case", "visual-flicker", "--sizes", "1024x640", "--scales", "1"), 1)
        self.kill_group.assert_called_once_with(4321, signal.SIGKILL)
        process = self.runs[0][3]
        self.assertEqual(process.wait.call_count, 2)
        self.assertEqual(process.wait.call_args_list[0].kwargs, {"timeout": 25})
        self.assertEqual(process.wait.call_args_list[1].kwargs, {})
        self.assertEqual(json.loads((self.output / "results.json").read_text())[0]["exit_code"], -1)
        self.assertFalse(self.runs[0][1].exists())

    def test_spawn_failure_removes_the_temporary_fixture(self):
        self.spawn_error = OSError("fixture executable missing")
        with self.assertRaisesRegex(OSError, "fixture executable missing"):
            self.run_script("--case", "visual-flicker", "--sizes", "1024x640", "--scales", "1")
        self.assertFalse(self.runs[0][1].exists())
        self.assertFalse((self.output / "results.json").exists())


@unittest.skipUnless(os.name == "posix", "lint-qml.sh uses Bash and Unix executable fixtures")
class QmlLintScriptTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="qml-lint-test-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name) / "workspace with spaces"
        self.app = self.root / "apps/osu-radio-qt"
        for part in ["scripts", "qml/components", "tests"]:
            (self.app / part).mkdir(parents=True)
        self.script = self.app / "scripts/lint-qml.sh"
        shutil.copyfile(APP / "scripts/lint-qml.sh", self.script)
        (self.app / "qml/Main.qml").write_text("fixture Main")
        (self.app / "qml/components/Theme.qml").write_text("fixture Theme")
        (self.app / "tests/Probe.qml").write_text("fixture probe")
        self.capture = Path(self.directory.name) / "capture.json"
        self.tools = Path(self.directory.name) / "bin"
        self.tools.mkdir()
        self.linter = self.tools / "fake qmllint"
        self.linter.write_text(f"#!{sys.executable}\n" + """
import json, os, sys
from pathlib import Path
args=sys.argv[1:]
module=Path(args[1])/'OsuRadio'
files=[Path(name) for name in args[4:]]
Path(os.environ['LINT_CAPTURE']).write_text(json.dumps({
    'args':args,'cwd':os.getcwd(),'qmldir':(module/'qmldir').read_text(),
    'types':(module/'plugin.qmltypes').read_text(),'files':{str(p):p.read_text() for p in files}}))
raise SystemExit(int(os.environ.get('LINT_EXIT_CODE','0')))
""")
        self.linter.chmod(0o700)
        # Keep this orchestration test independent of an installed ripgrep binary.
        finder = self.tools / "rg"
        finder.write_text(f"#!{sys.executable}\n" + """
import sys
from pathlib import Path
assert sys.argv[1]=='--files' and sys.argv[-2:]==['-g','*.qml']
import os
if os.environ.get('FINDER_EXIT_CODE'):
    print('fixture file discovery failed',file=sys.stderr)
    raise SystemExit(int(os.environ['FINDER_EXIT_CODE']))
for root in sys.argv[2:-2]:
    for path in Path(root).rglob('*.qml'):
        print(path)
""")
        finder.chmod(0o700)
        self.environment = dict(os.environ, PATH=str(self.tools) + os.pathsep + os.environ.get("PATH", ""), QMLLINT=str(self.linter), LINT_CAPTURE=str(self.capture))
        self.environment.pop("CARGO_TARGET_DIR", None)
        self.scratch = Path(self.directory.name) / "import scratch"
        self.scratch.mkdir()
        self.environment["TMPDIR"] = str(self.scratch)

    def generated_module(self, target, name, modified):
        module = target / f"debug/build/osu-radio-qt-{name}/out/qt-build-utils/qml_modules/OsuRadio"
        module.mkdir(parents=True)
        (module / "qmldir").write_text(name)
        (module / "plugin.qmltypes").write_text("types " + name)
        os.utime(module / "qmldir", (modified, modified))
        return module

    def run_script(self):
        return subprocess.run(["bash", str(self.script)], cwd=self.directory.name, env=self.environment, capture_output=True, text=True, timeout=10)

    def test_newest_generated_module_and_all_qml_files_are_linted_in_registered_paths(self):
        self.generated_module(self.root / "target", "older", 100)
        self.generated_module(self.root / "target", "newer", 200)
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        capture = json.loads(self.capture.read_text())
        self.assertEqual(capture["cwd"], str(self.root))
        self.assertEqual(capture["qmldir"], "newer")
        self.assertEqual(capture["types"], "types newer")
        self.assertEqual(capture["args"][0], "-I")
        self.assertEqual(capture["args"][2:4], ["-W", "0"])
        self.assertEqual(capture["args"][4:], sorted(capture["args"][4:]))
        self.assertEqual(set(capture["files"].values()), {"fixture Main", "fixture Theme", "fixture probe"})
        module = Path(capture["args"][1])
        copied = [name for name in capture["files"] if name.endswith(("Main.qml", "Theme.qml"))]
        self.assertTrue(all(Path(name).is_relative_to(module / "OsuRadio/qml") for name in copied))
        self.assertFalse(module.exists(), "temporary import directory must be removed")

    def test_custom_target_directory_is_used_and_linter_failure_is_propagated(self):
        target = Path(self.directory.name) / "custom target"
        self.generated_module(target, "custom", 100)
        self.environment.update(CARGO_TARGET_DIR=str(target), LINT_EXIT_CODE="9")
        result = self.run_script()
        self.assertEqual(result.returncode, 9, result.stderr)
        capture = json.loads(self.capture.read_text())
        self.assertEqual(capture["qmldir"], "custom")
        self.assertFalse(Path(capture["args"][1]).exists())

    def test_missing_generated_imports_explain_the_build_prerequisite(self):
        result = self.run_script()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Build osu-radio-qt before linting QML.", result.stderr)
        self.assertFalse(self.capture.exists())

    def test_file_discovery_failure_cannot_lint_an_empty_list_successfully(self):
        self.generated_module(self.root / "target", "fixture", 100)
        self.environment["FINDER_EXIT_CODE"] = "7"
        result = self.run_script()
        self.assertEqual(result.returncode, 7, result.stderr)
        self.assertIn("fixture file discovery failed", result.stderr)
        self.assertFalse(self.capture.exists(), "linter must not run after failed file discovery")
        self.assertEqual(list(self.scratch.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
