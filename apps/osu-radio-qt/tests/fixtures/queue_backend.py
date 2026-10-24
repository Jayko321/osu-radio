"""Disposable real-server fixture shared by Qt and the client engine probe.

Only the Realm producer is replaced. Import, persistence, HTTP and file checks
use the built server. Never call this against an existing application directory.
"""

import contextlib
import json
import os
from pathlib import Path
import selectors
import sqlite3
import subprocess
import sys
import time
import urllib.request
import wave


MODS = {"mode": "dt", "speed_percent": 120, "adjust_pitch": True}
GLOBAL_MODS = {"individual_mods_enabled": True,
               "global_profile": {"mode": "dt", "speed_percent": 130, "adjust_pitch": False}}


def request(base, method, path, body=None):
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(base + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=5) as response:
        data = response.read()
        return json.loads(data) if data else None


def fixture_environment(root):
    return {"SQLITE_DATABASE_URL": str(root / "library.sqlite"),
            "OSU_LAZER_REALM_PARSER_PATH": str(root / "realm-helper"),
            "XDG_DATA_HOME": str(root / "data"),
            "OSU_RADIO_SKIP_DOTENV": "1"}


@contextlib.contextmanager
def server(binary, root):
    env = dict(os.environ)
    env.pop("POSTGRES_DATABASE_URL", None)
    env.update(fixture_environment(root))
    env["OSU_RADIO_SERVER_ADDRESS"] = "127.0.0.1:0"
    with (root / "server.log").open("ab") as log:
        child = subprocess.Popen([binary], cwd=root, env=env,
                                 stdout=subprocess.PIPE, stderr=log)
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(child.stdout, selectors.EVENT_READ)
                assert selector.select(10), "real server readiness timeout"
                line = child.stdout.readline().decode()
                assert "listening on " in line, line + (root / "server.log").read_text()
                yield line.split("listening on ")[1].strip()
        finally:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=5)
            child.stdout.close()


def settled(base):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        queue = request(base, "GET", "/api/queue")
        if all(row["availability"] != "checking" for row in queue["entries"]):
            return queue
        time.sleep(0.01)
    raise AssertionError("availability worker did not complete: " + repr(queue))


def generate(root):
    assert not (root / "library.sqlite").exists(), "fixture needs a new private directory"
    source = root / "source"
    source.mkdir()
    (source / "client.realm").write_bytes(b"synthetic marker; never a real Realm")
    for sample, name in enumerate(("A", "B", "C", "rebound")):
        with wave.open(str(source / (name + ".wav")), "wb") as audio:
            audio.setparams((1, 2, 8000, 0, "NONE", "not compressed"))
            audio.writeframes(sample.to_bytes(2, "little") * 8000 * 30)
    sets = []
    for name in ("A", "B", "C", "Ghost"):
        sets.append({"source": "Lazer", "hash": "set-" + name,
                     "files": [{"filename": "audio.wav", "file": {
                         "resolved_path": str(source / (name + ".wav"))}}],
                     "beatmaps": [{"hash": name + difficulty,
                                   "difficulty_name": difficulty,
                                   "metadata": {"title": name, "title_unicode": "曲 " + name,
                                                "artist": "Fixture", "audio_file": "audio.wav"}}
                                  for difficulty in (("Easy", "Hard") if name == "A" else ("Easy",))]})
    (root / "snapshot.ndjson").write_text("".join(json.dumps(s) + "\n" for s in sets))
    helper = root / "realm-helper"
    helper.write_text("#!/usr/bin/env python3\nfrom pathlib import Path\n"
                      "import sys\nsys.stdout.write(Path(__file__).with_name('snapshot.ndjson').read_text())\n")
    helper.chmod(0o700)


def prepare(binary, root):
    generate(root)
    wrapper = root / "fixture-server"
    wrapper.write_text("#!/usr/bin/env python3\nimport os\n"
                       "os.environ.pop('POSTGRES_DATABASE_URL', None)\n"
                       "os.environ.update(" + repr(fixture_environment(root)) + ")\n"
                       "os.execv(" + repr(binary) + ", [" + repr(binary) + "])\n")
    wrapper.chmod(0o700)
    with server(binary, root) as base:
        folder = request(base, "POST", "/api/user-data/osu-folders/import",
                         {"marker_path": str(root / "source/client.realm")})
        tracks = request(base, "GET", "/api/tracks")
        main = request(base, "POST", "/api/playlists", {"name": "Queue source"})
        maps = [d["beatmap_id"] for t in tracks if t["title"] != "C" for d in t["difficulties"]]
        main = request(base, "POST", f'/api/playlists/{main["id"]}/items', {"beatmap_ids": maps})
        other = request(base, "POST", "/api/playlists", {"name": "Other"})
        b = next(t for t in tracks if t["title"] == "B")
        request(base, "POST", f'/api/playlists/{other["id"]}/items',
                {"beatmap_ids": [b["difficulties"][0]["beatmap_id"]]})
        request(base, "PUT", f'/api/audio-sources/{tracks[0]["audio_source_id"]}/volume', {"volume_percent": 29})
        request(base, "PATCH", "/api/user-data/audio-mods",
                {"individual_mods_enabled": True, "global_mode": "dt", "global_speed_percent": 130})
        request(base, "PUT", f'/api/audio-sources/{tracks[0]["audio_source_id"]}/mods', MODS)
        main = request(base, "GET", f'/api/playlists/{main["id"]}')
    # Only the ID allocator is seeded directly, after shutdown. Every entry and
    # transition below is created through HTTP, exercising real BIGINT transport.
    with sqlite3.connect(root / "library.sqlite") as db:
        db.execute("UPDATE playback_queue SET last_entry_id=?", (2**53,))
    with server(binary, root) as base:
        request(base, "POST", f'/api/playlists/{main["id"]}/queue')
        queue = settled(base)
        assert len(queue["entries"]) == 4 and queue["mode"] == "stopped"
        assert queue["entries"][-1]["availability"] == "unavailable"
        current = queue["entries"][1]["id"]
        request(base, "POST", f"/api/queue/items/{current}/play")
        request(base, "POST", "/api/playback/commands", {"command": "pause"})
        queue = request(base, "GET", "/api/queue")
        assert queue["current_entry_id"] == current and queue["history_len"] == 1
        assert all(int(e["id"]) > 2**53 for e in queue["entries"])
        assert all(e["last_played_at_ms"] is None for e in queue["entries"])
        # Only A's audio has its own profile; the rest have none, whatever the global set is.
        mods = {e["audio_source_id"]: e["mods"] for e in queue["entries"]}
        assert mods.pop(tracks[0]["audio_source_id"]) == MODS and not any(mods.values())
        assert request(base, "GET", "/api/queue") == queue, "GET must be pure"
    (root / "queue_fixture.json").write_text(json.dumps({"folder": folder, "tracks": tracks,
                                                        "main": main, "other": other, "queue": queue}))


def verify(binary, root):
    with server(binary, root) as base:
        assert request(base, "GET", "/api/queue")["entries"] == []
        assert len(request(base, "GET", "/api/tracks")) == 4
        assert len(request(base, "GET", "/api/playlists")) == 2
        fixture = json.loads((root / "queue_fixture.json").read_text())
        assert request(base, "GET", f'/api/playlists/{fixture["main"]["id"]}') == fixture["main"]
        tracks = request(base, "GET", "/api/tracks")
        assert all(t["last_played_at_ms"] is None for t in tracks)
        assert tracks[0]["volume_percent"] == 29
        assert tracks[0]["mods"] == MODS and all(t["mods"] is None for t in tracks[1:])
        assert request(base, "GET", "/api/user-data/audio-mods") == GLOBAL_MODS
        assert request(base, "GET", "/api/playback")["mode"] == "stopped"
        # The Qt probe turned shuffle and repeat All on before exiting; a new server keeps them.
        assert request(base, "GET", "/api/queue")["shuffle"] is True
        assert request(base, "GET", "/api/queue")["repeat"] == "all"
        playback = request(base, "GET", "/api/playback")
        assert playback["shuffle"] is True and playback["repeat"] == "all"
    assert (root / "source/A.wav").is_file()
    print("Real queue fixture: persistence, library, playlists, dates, volume and mods passed")


if __name__ == "__main__":
    mode, binary = sys.argv[1:]
    {"prepare": prepare, "verify": verify}[mode](str(Path(binary).resolve()), Path.cwd())
