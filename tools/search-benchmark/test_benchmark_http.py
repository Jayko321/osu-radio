"""Exercise benchmark orchestration without starting a server or opening a database."""
import contextlib
import io
import json
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch
from urllib.parse import parse_qs, urlsplit


SCRIPT = Path(__file__).with_name("benchmark_http.py")


class BenchmarkTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="radio-benchmark-test-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.backup = self.root / "backup.sqlite"
        self.backup.write_bytes(b"inert backup fixture")
        self.output = self.root / "reports"
        self.binary = self.root / "saved server"
        self.child = MagicMock()
        self.child.stdout.readline.return_value = "osu-radio server listening on http://127.0.0.1:43210\n"
        self.response = MagicMock()
        self.response.__enter__.return_value.read.return_value = b'[{"id":1}]'
        self.stdout = io.StringIO()
        self.stderr = io.StringIO()

    def run_script(self, endpoint=None, profile="debug"):
        arguments = [str(SCRIPT), "--binary", str(self.binary),
                     "--database-backup", str(self.backup), "--output", str(self.output),
                     "--stage", "stage3", "--profile", profile]
        if endpoint is not None:
            arguments.extend(["--endpoint", endpoint])
        with patch.object(sys, "argv", arguments), \
                patch("subprocess.Popen", return_value=self.child) as spawn, \
                patch("urllib.request.urlopen", return_value=self.response) as request, \
                patch("time.perf_counter", side_effect=[0.0, 0.001] * 30), \
                contextlib.redirect_stdout(self.stdout), contextlib.redirect_stderr(self.stderr):
            self.spawn = spawn
            self.request = request
            runpy.run_path(str(SCRIPT), run_name="__main__")

    def test_queries_samples_raw_payloads_and_timings_are_preserved(self):
        self.output.mkdir()
        # Semantic equality allows whitespace differences in an existing baseline.
        (self.output / "baseline-debug-beatmap-sets-0.json").write_text('[\n{"id": 1}\n]')
        self.run_script()
        self.assertEqual(self.request.call_count, 30)
        expected = [query for query in ["rock", "rock hard", "星", "does-not-exist-zzzz", ""] for _ in range(6)]
        for call, query in zip(self.request.call_args_list, expected, strict=True):
            url = urlsplit(call.args[0])
            self.assertEqual(url.path, "/api/beatmap-sets")
            self.assertEqual(parse_qs(url.query, keep_blank_values=True), {"q": [query]})
            self.assertEqual(call.kwargs, {"timeout": 30})
        results = json.loads(self.stdout.getvalue())
        self.assertEqual(len(results), 5)
        for index, row in enumerate(results):
            payload = self.output / f"stage3-debug-beatmap-sets-{index}.json"
            self.assertEqual(payload.read_bytes(), b'[{"id":1}]')
            self.assertEqual(row["rows"], 1)
            self.assertEqual(row["bytes"], len(payload.read_bytes()))
            self.assertEqual(row["samples_ms"], [1.0] * 5)
            self.assertEqual(row["http_ms"], 1.0)
            self.assertEqual(row["query"], expected[index * 6])
        self.assertEqual(json.loads((self.output / "stage3-debug-beatmap-sets-timings.json").read_text()), results)
        self.assertEqual(self.backup.read_bytes(), b"inert backup fixture")
        self.assertEqual(self.spawn.call_args.args, ([str(self.binary.resolve())],))
        environment = self.spawn.call_args.kwargs["env"]
        self.assertEqual(environment["SQLITE_DATABASE_URL"], str(self.backup.resolve()))
        self.assertEqual(environment["OSU_RADIO_SERVER_ADDRESS"], "127.0.0.1:0")
        self.child.terminate.assert_called_once_with()
        self.child.wait.assert_called_once_with(timeout=10)
        self.child.kill.assert_not_called()

    def test_tracks_release_uses_its_endpoint_without_comparing_legacy_payloads(self):
        self.output.mkdir()
        (self.output / "baseline-release-beatmap-sets-0.json").write_text('["different legacy shape"]')
        self.run_script(endpoint="tracks", profile="release")
        self.assertTrue(all(urlsplit(call.args[0]).path == "/api/tracks" for call in self.request.call_args_list))
        self.assertTrue((self.output / "stage3-release-tracks-timings.json").is_file())

    def test_missing_backup_is_rejected_before_starting_a_child(self):
        self.backup.unlink()
        with self.assertRaises(SystemExit) as error:
            self.run_script()
        self.assertEqual(error.exception.code, 2)
        self.assertIn("existing SQLite backup", self.stderr.getvalue())
        self.spawn.assert_not_called()
        self.assertFalse(self.output.exists())

    def test_readiness_failure_reports_diagnostic_filename_and_reaps_child(self):
        self.child.stdout.readline.return_value = "unexpected startup output\n"
        with self.assertRaisesRegex(RuntimeError, r"stage3-debug-beatmap-sets\.stderr"):
            self.run_script()
        self.request.assert_not_called()
        self.child.terminate.assert_called_once_with()
        self.child.wait.assert_called_once_with(timeout=10)

    def test_http_failure_still_terminates_and_reaps_child(self):
        self.response.__enter__.return_value.read.side_effect = TimeoutError("fixture request timeout")
        with self.assertRaisesRegex(TimeoutError, "fixture request timeout"):
            self.run_script()
        self.child.terminate.assert_called_once_with()
        self.child.wait.assert_called_once_with(timeout=10)
        self.assertFalse((self.output / "stage3-debug-beatmap-sets-timings.json").exists())

    def test_invalid_json_is_rejected_and_reaps_child(self):
        self.response.__enter__.return_value.read.return_value = b"invalid JSON"
        with self.assertRaises(json.JSONDecodeError):
            self.run_script()
        self.child.terminate.assert_called_once_with()
        self.child.wait.assert_called_once_with(timeout=10)

    def test_baseline_mismatch_rejects_the_measurement_and_reaps_child(self):
        self.output.mkdir()
        (self.output / "baseline-debug-beatmap-sets-0.json").write_text("[]")
        with self.assertRaisesRegex(AssertionError, "rock"):
            self.run_script()
        self.child.terminate.assert_called_once_with()
        self.child.wait.assert_called_once_with(timeout=10)
        self.assertFalse((self.output / "stage3-debug-beatmap-sets-timings.json").exists())

    def test_slow_termination_escalates_to_kill_and_waits_again(self):
        self.child.wait.side_effect = [subprocess.TimeoutExpired("fixture child", 10), 0]
        self.run_script()
        self.child.terminate.assert_called_once_with()
        self.child.kill.assert_called_once_with()
        self.assertEqual(self.child.wait.call_args_list[0].kwargs, {"timeout": 10})
        self.assertEqual(self.child.wait.call_args_list[1].kwargs, {})


if __name__ == "__main__":
    unittest.main()
