#!/usr/bin/env python3
"""Deterministic subprocess fixture for the real Session and HTTP adapter path."""
import json
import os
from pathlib import Path
import sys
import threading
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

counts = {"library": 0, "folders": 0}
library_pending = threading.Event()


def folder(identifier):
    return {"id": identifier, "kind": "lazer", "root_path": "/fixtures/" + str(identifier),
            "marker_path": "/fixtures/" + str(identifier) + "/client.realm",
            "label": "Stored label", "enabled": True, "last_scanned_at": None}


def library(ids):
    return [{"id": 1, "online_id": None, "hash": None,
             "audio_sources": [{"id": i, "kind": "local", "location": str(i)} for i in ids],
             "beatmaps": [{"id": i, "audio_source_id": i, "title": "Track " + str(i),
                           "artist": "Fixture artist", "difficulty_name": "Hard",
                           "has_cover": True} for i in ids]}]


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        with (root / "requests").open("a") as log:
            log.write(self.path + "\n")
        status = 200
        content_type = "application/json"
        if self.path == "/api/beatmap-sets":
            if case == "requests":
                library_pending.set()
                threading.Event().wait(60)
            counts["library"] += 1
            n = counts["library"]
            if n in (1, 3):
                status, payload = 500, {"message": "fixture library unavailable"}
            else:
                payload = library([7, 42, 103] if n == 2 else [103] if n == 4 else [])
        elif self.path == "/api/user-data/osu-folders":
            if case == "requests":
                library_pending.wait(10)
            counts["folders"] += 1
            if counts["folders"] == 2:
                status, payload = 500, {"message": "fixture folders unavailable"}
            else:
                payload = [folder(31), folder(52)]
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
