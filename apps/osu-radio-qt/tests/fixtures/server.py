#!/usr/bin/env python3
"""Deterministic subprocess fixture for the real Session and HTTP adapter path."""
import json
import struct
import zlib
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

audio_settings = {"individual_volume_enabled": False, "global_volume_percent": 100}
audio_volumes = {}
volume_loads = 0
volume_puts = 0
if case == "volume":
    audio_settings["global_volume_percent"] = 20
playlists = {}
playlist_covers = {}
cover_puts = 0

def make_png(width, height, color=(80, 100, 120), rgba=False):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6 if rgba else 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress((b"\0" + bytes((*color, 255) if rgba else color) * width) * height)) + chunk(b"IEND", b""))

if case == "playlist-covers":
    (root / "source.png").write_bytes(make_png(800, 400))

if case == "playlist-cover-preview":
    playlists[1] = {"id": 1, "name": "With artwork", "cover_revision": 1, "items": [
        {"id": 1, "playlist_id": 1, "source_kind": "stable", "beatmap_hash": "fixture-7",
         "title": "Track 7", "artist": "Fixture artist", "difficulty_name": "Hard",
         "beatmap_id": 7, "beatmap_set_id": 1, "audio_source_id": 7, "cover_beatmap_id": 7}
    ]}
    playlist_covers[1] = make_png(512, 512)

if case == "visual-flicker":
    playlists[1] = {"id": 1, "name": "Warm artwork", "cover_revision": 1, "items": [
        {"id": identifier, "playlist_id": 1, "source_kind": "stable", "beatmap_hash": "same-audio-" + name,
         "title": "Track 0001", "artist": "Fixture artist", "difficulty_name": name,
         "beatmap_id": identifier, "beatmap_set_id": 1, "audio_source_id": 1, "cover_beatmap_id": 1}
        for identifier, name in [(1, "Easy"), (2, "Hard")]
    ]}
    playlist_covers[1] = make_png(512, 512, (80, 100, 120), True)
flicker_covers = {}

def playlist_summary(identifier):
    playlist = playlists[identifier]
    return {"id": identifier, "name": playlist["name"], "item_count": len(playlist["items"]),
            "cover_beatmap_id": playlist["items"][0]["cover_beatmap_id"] if playlist["items"] else None,
            "custom_cover_revision": playlist.get("cover_revision")}

counts = {"library": 0, "folders": 0, "retry": 0, "queue": 0, "playlists": 0}
library_pending = threading.Event()
playlist_tab_opened = threading.Event()
saved_folders = [31, 52]
imports = {}
metadata_calls = {}
metadata_active = 0
metadata_lock = threading.Lock()


def folder(identifier):
    path = "/fixtures/" + str(identifier)
    if case.startswith("visual"):
        path += "/Очень длинный путь / 日本語 / " + "directory/" * 10
    return {"id": identifier, "kind": "lazer", "root_path": path,
            "marker_path": "/fixtures/" + str(identifier) + "/client.realm",
            "label": "Stored label", "enabled": True, "last_scanned_at": None}


def library(ids):
    tracks = [{"audio_source_id": i, "title": "Track " + str(i), "title_unicode": None,
             "artist": "Fixture artist", "artist_unicode": None, "cover_beatmap_id": i,
             "volume_percent": audio_volumes.get(i),
             "difficulties": [{"beatmap_id": i, "beatmap_set_id": 1, "difficulty_name": "Hard",
                               "set_has_multiple_audio_sources": True}]} for i in ids]
    if case == "sorting":
        for track in tracks:
            identifier = track["audio_source_id"]
            track["artist"] = {7: "Zulu", 42: "Alpha", 103: "Beta"}[identifier]
            track["last_played_at_ms"] = {7: 3000, 42: 1000, 103: None}[identifier]
    if case == "visual-queue":
        titles = {7: "Miku", 42: "Guitar to Kodoku to Aoi Hoshi", 103: "Bling-Bang-Bang-Born (TV Size)"}
        artists = {7: "Anamanaguchi", 42: "kessoku band", 103: "Creepy Nuts"}
        for track in tracks:
            identifier = track["audio_source_id"]
            track["title"] = titles.get(identifier, "Other song")
            track["artist"] = artists.get(identifier, "Artist")
    elif case == "visual-flicker":
        for track in tracks:
            track["title"] = "Track " + str(track["audio_source_id"]).zfill(4)
    elif case.startswith("visual"):
        for track in tracks:
            track["title"] = "長い曲名 — Очень длинное название композиции " + str(track["audio_source_id"]) * 12
            track["artist"] = "Long artist / Исполнитель / アーティスト"
    return tracks


queue = ([1] + list(range(101, 117))) if case == "visual-flicker" else [7, 42, 103] if case in ("queue", "volume", "visual-queue") else []
queue_index = 0
playback_condition = threading.Condition()
playback = {"current_audio_source_id": 7 if queue else None,
            "track": library([7])[0] if queue else None,
            "duration_ms": 125000 if queue else None,
            "mode": "paused" if queue else "stopped", "revision": 1 if queue else 0,
            "playback_token": 1 if queue else 0, "can_next": bool(queue), "can_previous": bool(queue)}
if case == "visual-flicker":
    playback.update(current_audio_source_id=1, track=library([1])[0], duration_ms=None)


def playback_command(body):
    global queue_index
    command = body["command"]
    with playback_condition:
        if (command in ("finished", "failed") or command == "pause" and "playback_token" in body) and body["playback_token"] != playback["playback_token"]:
            return dict(playback)
        if command == "play":
            identifier = body.get("audio_source_id", playback["current_audio_source_id"])
            if identifier is not None and identifier != playback["current_audio_source_id"]:
                queue.insert(min(queue_index + 1, len(queue)), identifier)
                queue_index = queue.index(identifier)
                playback["playback_token"] += 1
            playback["mode"] = "playing"
        elif command == "pause":
            playback["mode"] = "paused"
        elif command == "stop":
            playback["mode"] = "stopped"
            playback["playback_token"] += 1
        elif command == "previous":
            queue_index = max(0, queue_index - 1)
            playback["playback_token"] += 1
        elif command in ("next", "finished", "failed"):
            queue_index += 1
            playback["playback_token"] += 1
        identifier = queue[queue_index] if queue_index < len(queue) else None
        playback["current_audio_source_id"] = identifier
        playback["track"] = library([identifier])[0] if identifier is not None else None
        playback["duration_ms"] = 125000 if identifier is not None else None
        playback["can_next"] = queue_index + 1 < len(queue)
        playback["can_previous"] = bool(queue)
        if identifier is None:
            playback["mode"] = "stopped"
        playback["revision"] += 1
        playback_condition.notify_all()
        return dict(playback)


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
        if self.path == "/api/playback/commands":
            with (root / "playback_commands").open("a") as log:
                log.write(json.dumps(body) + "\n")
            self.reply(200, playback_command(body))
        elif self.path == "/api/playlists":
            identifier = max(playlists, default=0) + 1
            playlists[identifier] = {"id": identifier, "name": body["name"].strip(), "items": []}
            self.reply(201, playlist_summary(identifier))
        elif self.path.endswith("/discover"):
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

    def do_PATCH(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))))
        if self.path == "/api/user-data/audio-settings":
            audio_settings.update(body)
            (root / "audio-settings.json").write_text(json.dumps(audio_settings))
            self.reply(200, dict(audio_settings))
        elif self.path.startswith("/api/playlists/"):
            identifier = int(self.path.split("/")[3])
            playlists[identifier]["name"] = body["name"].strip()
            self.reply(200, playlist_summary(identifier))
        else:
            self.reply(404, {"error": "Unknown fixture operation"})

    def do_PUT(self):
        global volume_puts, cover_puts
        if self.path.startswith("/api/playlists/") and self.path.endswith("/cover"):
            body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
            with (root / "requests").open("a") as log:
                log.write("PUT " + self.path + "\n")
            cover_puts += 1
            if cover_puts == 1:
                (root / "source.png").unlink()
                self.reply(500, {"error": "fixture cover upload unavailable"})
                return
            identifier = int(self.path.split("/")[3])
            playlist_covers[identifier] = body
            (root / "uploaded-cover.png").write_bytes(body)
            playlists[identifier]["cover_revision"] = cover_puts
            self.reply(200, playlist_summary(identifier))
            return
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))))
        identifier = int(self.path.split("/")[3])
        volume_puts += 1
        if case == "volume" and volume_puts == 1:
            self.reply(500, {"error": "fixture volume save unavailable"})
            return
        audio_volumes[identifier] = body["volume_percent"]
        (root / "audio-volumes.json").write_text(json.dumps(audio_volumes))
        self.send_response(204)
        self.end_headers()

    def do_DELETE(self):
        with (root / "requests").open("a") as log:
            log.write("DELETE " + self.path + "\n")
        if self.path.startswith("/api/playlists/"):
            identifier = int(self.path.split("/")[3])
            if self.path.endswith("/cover"):
                playlist_covers.pop(identifier, None)
                playlists[identifier].pop("cover_revision", None)
            else:
                playlists.pop(identifier, None)
            self.send_response(204)
            self.end_headers()
            return
        if self.path.endswith("/volume"):
            audio_volumes.pop(int(self.path.split("/")[3]), None)
            (root / "audio-volumes.json").write_text(json.dumps(audio_volumes))
            self.send_response(204)
            self.end_headers()
            return
        i = int(self.path.rsplit("/", 1)[1])
        if i in saved_folders:
            saved_folders.remove(i)
        self.send_response(204)
        self.end_headers()

    def do_GET(self):
        global volume_loads
        with (root / "requests").open("a") as log:
            log.write(self.path + "\n")
        status = 200
        content_type = "application/json"
        if self.path == "/api/playlists":
            if case == "playlist-cover-preview":
                counts["playlists"] += 1
                if counts["playlists"] == 1:
                    # ShowPlaylists sends a second list request after the tab opens.
                    playlist_tab_opened.wait(5)
                else:
                    playlist_tab_opened.set()
                    time.sleep(0.05)
            payload = [playlist_summary(identifier) for identifier in playlists]
        elif self.path.startswith("/api/playlists/"):
            identifier = int(self.path.split("/")[3])
            if self.path.endswith("/cover"):
                if identifier in playlist_covers:
                    payload, content_type = playlist_covers[identifier], "image/png"
                else:
                    status, payload = 404, {"error": "No custom cover"}
            elif identifier in playlists:
                if case == "visual-flicker":
                    time.sleep(0.12)
                payload = playlists[identifier]
            else:
                status, payload = 404, {"error": "Playlist missing"}
        elif self.path == "/api/user-data/audio-settings":
            volume_loads += 1
            if case == "volume" and volume_loads == 1:
                self.reply(500, {"error": "fixture volume load unavailable"})
                return
            payload = audio_settings
        elif self.path == "/api/queue":
            counts["queue"] += 1
            if case == "queue" and counts["queue"] == 1:
                self.reply(500, {"error": "fixture queue unavailable"})
                return
            with playback_condition:
                payload = {"audio_source_ids": list(queue), "playlist_item_ids": [None] * len(queue),
                           "current_index": queue_index if queue else None,
                           "upcoming_tracks": library(queue[queue_index + 1:]),
                           "mode": playback["mode"], "revision": playback["revision"],
                           "playback_token": playback["playback_token"]}
        elif self.path == "/api/playback":
            with playback_condition:
                payload = dict(playback)
        elif self.path == "/api/playback/events":
            self.send_response(200)
            self.send_header("Content-Type", "application/x-ndjson")
            self.end_headers()
            revision = -1
            try:
                while True:
                    with playback_condition:
                        if revision == playback["revision"]:
                            playback_condition.wait(timeout=0.1)
                        payload = dict(playback)
                    if payload["revision"] != revision:
                        self.wfile.write(json.dumps(payload).encode() + b"\n")
                        revision = payload["revision"]
                    else:
                        self.wfile.write(b"\n")
                    self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                pass
            return
        elif urlsplit(self.path).path == "/api/tracks":
            if case == "requests":
                library_pending.set()
                threading.Event().wait(60)
            counts["library"] += 1
            n = counts["library"]
            if case == "queue":
                query = parse_qs(urlsplit(self.path).query).get("q", [""])[0]
                payload = library([] if query == "missing" else [7, 42, 103])
            elif case == "visual-flicker":
                payload = library(range(1, 81))
            elif case.startswith("visual"):
                query = parse_qs(urlsplit(self.path).query).get("q", [""])[0]
                if query == "visual-error":
                    status, payload = 500, {"message": "visual fixture error: " + "Длинное описание ошибки / 日本語 / " * 6}
                else:
                    payload = library([] if query == "missing" else [7, 42, 103] + list(range(200, 225)))
            elif case in ("playback", "folders", "sorting", "volume", "playlist-covers", "playlist-cover-preview") or case.startswith("native"):
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
            if case == "visual-flicker":
                identifier = int(self.path.split("/")[-2])
                with (root / "media-events").open("a") as log:
                    log.write(str(round(time.time() * 1000)) + " duration " + str(identifier) + "\n")
                time.sleep(1.6 if identifier == 1 else 0.10)
            payload = {"duration_ms": 261000 if case == "visual-queue" else 125000}
        elif self.path.endswith("/cover") and case == "visual-flicker":
            identifier = int(self.path.split("/")[-2])
            with (root / "media-events").open("a") as log:
                log.write(str(round(time.time() * 1000)) + " cover " + str(identifier) + "\n")
            time.sleep(0.025)
            if identifier not in flicker_covers:
                color = (80, 100, 120) if identifier == 1 else (40 + identifier * 37 % 180, 40 + identifier * 53 % 180, 40 + identifier * 79 % 180)
                flicker_covers[identifier] = make_png(1280, 1280, color, True)
            payload, content_type = flicker_covers[identifier], "image/png"
        elif self.path.endswith("/cover") and case == "visual-queue":
            identifier = self.path.split("/")[-2]
            path = root / ("cover-" + identifier + ".png")
            payload = path.read_bytes() if path.exists() else make_png(512, 512)
            content_type = "image/png"
        elif self.path.endswith("/cover") and case.startswith("visual") and (root / "source.png").exists():
            payload = (root / "source.png").read_bytes()
            content_type = "image/png"
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
