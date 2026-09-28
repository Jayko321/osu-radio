#!/usr/bin/env python3
"""Deterministic subprocess fixture for the real Session and HTTP adapter path."""
import json
import os
from pathlib import Path
import sys
import threading
import time
from urllib.parse import urlsplit, parse_qs
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

root = Path.cwd()
(root / "child.pid").write_text(str(os.getpid()))
starts_file = root / "starts"
starts = int(starts_file.read_text()) + 1 if starts_file.exists() else 1
starts_file.write_text(str(starts))
case = os.environ.get("OSU_RADIO_QT_PROBE_CASE", "fixture")
if starts == 1 and case == "fixture":
    print("fixture: first startup intentionally unsuccessful", file=sys.stderr)
    sys.exit(1)

counts = {"library": 0, "folders": 0, "retry": 0}
library_pending = threading.Event()
saved_folders = [31, 52]
imports = {}
metadata_calls = {}
metadata_active = 0
metadata_lock = threading.Lock()


def folder(identifier):
    return {"id": identifier, "kind": "lazer", "root_path": "/fixtures/" + str(identifier),
            "marker_path": "/fixtures/" + str(identifier) + "/client.realm",
            "label": "Stored label", "enabled": True, "last_scanned_at": None}


def library(ids):
    return [{"audio_source_id": i, "title": "Track " + str(i), "title_unicode": None,
             "artist": "Fixture artist", "artist_unicode": None, "cover_beatmap_id": i,
             "difficulties": [{"beatmap_id": i, "beatmap_set_id": 1, "difficulty_name": "Hard",
                               "set_has_multiple_audio_sources": True}]} for i in ids]


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def reply(self, status, payload):
        data = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        try:
            self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def do_POST(self):
        global metadata_active
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))))
        marker = body.get("marker_path", "")
        with (root / "requests").open("a") as log:
            log.write("POST " + self.path + " " + marker + "\n")
        if self.path.endswith("/discover"):
            events = [{"event": "candidate", "kind": "lazer", "root_path": "/fixtures/31",
                       "marker_path": "/fixtures/31/client.realm", "registered_id": 31}]
            for i in [101, 101, 102]:
                events.append({"event": "candidate", "kind": "stable" if i == 101 else "lazer",
                               "root_path": "/fixtures/" + str(i),
                               "marker_path": "/fixtures/" + str(i) + "/" + ("osu!.db" if i == 101 else "client.realm"),
                               "registered_id": i if i in saved_folders else None})
            events.append({"event": "complete"})
            data = b"".join(json.dumps(event).encode() + b"\n" for event in events)
            self.send_response(200)
            self.send_header("Content-Type", "application/x-ndjson")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            try:
                for offset in range(0, len(data), 23):
                    self.wfile.write(data[offset:offset + 23])
                    self.wfile.flush()
                    time.sleep(0.003)
            except (BrokenPipeError, ConnectionResetError):
                pass
        elif self.path.endswith("/metadata"):
            with metadata_lock:
                metadata_active += 1
                maximum = root / "metadata_max"
                maximum.write_text(str(max(metadata_active, int(maximum.read_text()) if maximum.exists() else 0)))
                metadata_calls[marker] = metadata_calls.get(marker, 0) + 1
                n = metadata_calls[marker]
            time.sleep(0.12)
            if "/102/" in marker and n == 1:
                self.reply(500, {"error": "fixture preview unavailable"})
            else:
                self.reply(200, {"beatmap_count": 0 if "/102/" in marker else 17})
            with metadata_lock:
                metadata_active -= 1
        elif self.path.endswith("/import"):
            i = int(marker.split("/")[2])
            imports[i] = imports.get(i, 0) + 1
            time.sleep(0.08)
            if i == 102 and imports[i] == 1:
                self.reply(500, {"error": "fixture import unavailable"})
            else:
                if i not in saved_folders:
                    saved_folders.append(i)
                result = folder(i)
                result["kind"] = "stable" if i == 101 else "lazer"
                result["marker_path"] = marker
                self.reply(200, result)
        else:
            self.reply(404, {"error": "Unknown fixture operation"})

    def do_DELETE(self):
        with (root / "requests").open("a") as log:
            log.write("DELETE " + self.path + "\n")
        i = int(self.path.rsplit("/", 1)[1])
        if i in saved_folders:
            saved_folders.remove(i)
        self.send_response(204)
        self.end_headers()

    def do_GET(self):
        with (root / "requests").open("a") as log:
            log.write(self.path + "\n")
        status = 200
        content_type = "application/json"
        if urlsplit(self.path).path == "/api/tracks":
            if case == "requests":
                library_pending.set()
                threading.Event().wait(60)
            counts["library"] += 1
            n = counts["library"]
            if case in ("playback", "folders"):
                payload = library([7, 42, 103])
            elif case == "search":
                query = parse_qs(urlsplit(self.path).query).get("q", [""])[0]
                if query == "retry":
                    counts["retry"] += 1
                if query == "retry" and counts["retry"] == 1:
                    status, payload = 500, {"message": "fixture search unavailable"}
                else:
                    payload = library([7, 42, 103] if query == "" else [] if query == "missing" else [42])
            elif n in (1, 3):
                status, payload = 500, {"message": "fixture library unavailable"}
            else:
                payload = library([7, 42, 103] if n == 2 else [103] if n == 4 else [])
        elif self.path == "/api/user-data/osu-folders":
            if case == "requests":
                library_pending.wait(10)
            counts["folders"] += 1
            if case == "folders":
                payload = [folder(i) for i in saved_folders]
            elif counts["folders"] == 2:
                status, payload = 500, {"message": "fixture folders unavailable"}
            else:
                payload = [folder(31), folder(52)]
        elif self.path.endswith("/audio"):
            time.sleep(0.3)
            status, payload = 404, {"error": "fixture audio unavailable"}
        elif self.path.endswith("/duration"):
            payload = {"duration_ms": 125000}
        elif self.path.endswith("/cover"):
            # Valid PPM is decoded by Qt's built-in image plugins without external fixtures.
            payload = b"P6\n2 2\n255\n" + bytes([80, 100, 120]) * 4
            content_type = "image/x-portable-pixmap"
        else:
            status, payload = 404, {"message": "Unknown fixture route"}
        data = payload if isinstance(payload, bytes) else json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        try:
            self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError):
            pass


server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
print("listening on http://127.0.0.1:" + str(server.server_port), flush=True)
server.serve_forever()
