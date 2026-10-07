# Backend guidance

Read [the index](index.md) for product direction and [development](development.md)
for checks. This guide describes the current server boundary and hosting protocol.
Read [database](database.md) for SeaORM repositories and persistence workflows,
and the [API skill](../../.agents/skills/osu-radio-api/SKILL.md) when changing contracts.

## Ownership and change locations

| Concern | Owner and source | Where to change |
| --- | --- | --- |
| Environment, process exit, listener | [`config.rs`](../../apps/osu-radio-server/src/config.rs), [`main.rs`](../../apps/osu-radio-server/src/main.rs) | Configuration and server startup belong here. |
| HTTP routing and response shape | [`routes/`](../../apps/osu-radio-server/src/routes/mod.rs) | Handlers call services and map results into route DTOs. Keep both feature variants synchronized. |
| Persisted-model use cases | [`radio-services`](../../crates/radio-services/src/lib.rs) | Shared services compose registration, snapshot replacement and cleanup for server and CLI; keep handlers focused on HTTP translation. |
| Request failures | [`error.rs`](../../apps/osu-radio-server/src/error.rs) | Use `ApiError` for HTTP error translation and safe diagnostics. |
| OpenAPI and Scalar | [`docs.rs`](../../apps/osu-radio-server/src/docs.rs), [`Cargo.toml`](../../apps/osu-radio-server/Cargo.toml) | Documentation metadata and its optional dependencies belong here. |
| Frontend access and process ownership | Client [`api.rs`](../../crates/osu-radio-client/src/api.rs), [`server.rs`](../../crates/osu-radio-client/src/server.rs), [`session.rs`](../../crates/osu-radio-client/src/session.rs) | Keep reusable HTTP calls and supervision in the toolkit-free client. |

The backend owns source-file access, source reading and audio serving. The client
owns temporary downloads and local audio output. Discovery and source parsing remain
reusable scanner work; shared installation services compose folder discovery. Domain
types remain free of I/O. Database queries belong to the persistence component,
whose concrete repositories are documented in [database](database.md).

The GUI calls `osu-radio-client`, not server internals, scanner functions, or
local osu! files. `ApiClient` has its own wire DTOs; the client also owns reusable
view models. See [frontend](frontend.md) for their responsibilities. The server streams original audio files by ID; playback runs locally in the
client. Neither hosted-source providers nor remote hosting are selected.

## Current startup configuration

`ServerConfig::from_env` calls `dotenvy::dotenv()` before reading configuration.
A missing `.env` (`NotFound`) is allowed: process environment alone can supply
configuration. A discovered file still loads without overriding existing process
variables. Other I/O errors and malformed file contents propagate as
`Failed to load .env`, even when the required database URL is already set.

The default SQLite build requires `SQLITE_DATABASE_URL` (a path, SQLite URL or
`:memory:`); the PostgreSQL build requires `POSTGRES_DATABASE_URL`. Startup opens
the pool and applies versioned migrations without resetting. Legacy tables without
SeaORM history are rejected with instructions to use a new database or explicit reset. `OSU_RADIO_SERVER_ADDRESS` defaults to
`127.0.0.1:3000` when its environment lookup fails. An invalid address or a bind
failure propagates with context and exits unsuccessfully; it does not silently
select another port. Keep the loopback default unless a task explicitly changes
the hosting model.

After migration, `AppState::connect` recovers the shared playback queue once.
It retains the list and current position, restores any current entry on pause,
and invalidates callbacks from the previous server lifetime. Recovery commits
before the listener starts; a subsequent Play begins the recovered track from
the start. Queue transitions and revision/token rules belong to
[`QueueService`](../../crates/radio-services/src/queue.rs), not HTTP handlers.

`serve` prepares application state, binds the TCP listener, reads the actual
bound address, prints its startup messages, then enters `axum::serve`. This is
the current order in source, not a separate HTTP health-check protocol.

## Embedded readiness and lifecycle

The current desktop frontend launches a child server. This is today's deployment
choice, not a permanent requirement for Windows, Linux, or future mobile apps.

`EmbeddedServer::start` uses `ServerOptions::address`, whose default is
`127.0.0.1:0`, as the child's `OSU_RADIO_SERVER_ADDRESS`. Port zero requests an
OS-assigned port. The option can be overridden; the supervisor does not always
force port zero. `ServerOptions::working_directory` changes the child's working
directory when present; otherwise it inherits the parent directory. This affects
which `.env` the server finds in that directory or its ancestors. No file is
required when the process environment supplies the selected database URL.

[Configuration regressions](../../apps/osu-radio-server/src/config/tests.rs)
run in separate processes with cleared application environments and temporary
working directories. They cover environment-only configuration, the default and
explicit address, the required backend-specific URL, valid dotenv loading,
process-variable precedence and fatal parse/read errors. The
[startup test](../../apps/osu-radio-server/tests/startup.rs) launches the actual
SQLite server without `.env`, using in-memory storage and an ephemeral loopback
port. Both docs feature states run these checks; PostgreSQL configuration checks
need no database connection.

The server emits:

```text
osu-radio server listening on http://127.0.0.1:<bound-port>
```

The client's `parse_ready_line` finds the substring `listening on `, trims the
remainder, and accepts a remainder starting with `http://` or `https://`.
`read_ready_line` waits for this output, with a default 30-second startup deadline.
The deadline also covers waiting for child exit after stdout closes; a live child
that closes stdout cannot bypass it. Startup cancellation and failure kill and
reap the child before returning. A
wording change that removes the marker, or text appended to the address, changes
the protocol. Update producer, parser, and tests together. Readiness means the
bound address has been announced; it does not mean the client has completed an
HTTP health probe.

Both child output pipes are consumed: stderr is forwarded immediately and stdout
is read through readiness and afterward. Logs receive a `[server]` prefix in the
parent. Keep draining both pipes so a full buffer cannot stall the child.
Premature exit, output-read failure, spawn failure, and timeout are exposed as
`ServerError` variants. `Session::start` pairs this server with an `ApiClient`
pointing at the reported URL.

`shutdown` kills and awaits the owned child; repeated shutdown is harmless after
the child has been taken. `Drop` requests termination, and Tokio's child has
`kill_on_drop(true)`. These are normal-lifecycle safeguards, not protection
against a hard-killed parent: abrupt GUI termination can leave an orphan. See
[frontend](frontend.md) for window closure and runtime ownership.

The executable resolution order is an explicit `ServerOptions::binary`, then
`OSU_RADIO_SERVER_BIN`, then a server executable beside the running executable,
then the bare platform executable name resolved through `PATH`. The name uses
the platform's executable suffix. Building the GUI does not build this child
binary; see [development](development.md).

## Error boundary

`ApiError::bad_request` and `ApiError::not_found` carry intentional client-facing
messages and status codes. Other errors converted through `From<E>` become 500
responses containing the fixed message `The server failed to handle the
request.` The underlying error chain is logged to stderr, not returned in the
response. Preserve this separation; do not expose internal paths or storage
diagnostics through unexpected-error messages.

Startup failures print their contextual error chain and return a failing exit
code. The frontend receives supervisor/client errors through the reusable client
boundary and decides how to display them.

## Optional API documentation

The default `docs` feature enables `utoipa`, `utoipa-axum`, and `utoipa-scalar`,
mounts Scalar at `/docs`, and prints its URL. The dependencies are optional;
`--no-default-features --features sqlite` removes this documentation code while
selecting SQLite; `--no-default-features --features postgres` selects PostgreSQL.

`routes::router` has two mutually exclusive implementations: an ordinary axum
router without `docs`, and an `OpenApiRouter` with it. A route change must update
both. Handlers and DTOs use `cfg_attr(feature = "docs", ...)` for OpenAPI
attributes and schema derives. Keep documentation-only field annotations gated
as well. Check both feature states using the commands in
[development](development.md); compiling only one cannot verify the other.

## Database-backed HTTP contracts

`AppState` stores a cloneable `Services` handle; handlers and test fixtures use its
model-service accessors. The server has no `radio-db` dependency. Handlers retain wire DTOs independent of database models. The client
continues to use its own [API DTOs](../../crates/osu-radio-client/src/api.rs).

| Route | Response and behavior |
| --- | --- |
| `GET /api/beatmap-sets` | Optional `q` searches the library; existing contract retained. Array of `{id, online_id, hash, has_multiple_audio_sources, audio_sources, beatmaps}`. Each audio source has `{id, kind, location}`; distinct sources per set, empty arrays allowed. |
| `GET /api/tracks` | Optional `q`; one record per global audio ID with nullable representative metadata, cover ID, `last_played_at_ms` and all difficulties. Used by Songs. |
| `GET /api/user-data/audio-settings` | `{individual_volume_enabled, global_volume_percent}`; default false/100. |
| `PATCH /api/user-data/audio-settings` | Optional fields update atomically; percentages are integers 0–100. Omitted fields are retained; null/wrong types fail validation. |
| `PUT /api/audio-sources/{id}/volume` | `{volume_percent}` stores an absolute 0–100 integer by durable audio identity; 204, 400 for range, 404 for unknown ID. |
| `DELETE /api/audio-sources/{id}/volume` | 204 removes an override; repeating for a known source is harmless. Unknown ID is 404. |
| `GET /api/queue` | Persisted `{audio_source_ids, playlist_item_ids, current_index, mode, revision, playback_token}` plus `upcoming_tracks` (`TrackResponse` audio summaries). Pending playable entries follow current, preserve order/duplicates and omit deleted/Online IDs. State and metadata come from one read snapshot; viewing does not change playback. Parallel item IDs are nullable; played history remains; an exhausted index equals list length. |
| `POST /api/queue/items` | `{audio_source_ids:[...]}` appends atomically after validating every ID as Local/Copied. Unknown/Online IDs return 400 without changes. An empty/exhausted queue starts the first added item; paused playback stays paused. Returns the current assignment. |
| `DELETE /api/queue` | Clears the queue, stops playback and invalidates the launch token. Returns the current assignment. |
| `GET /api/playback` | Current assignment with metadata and duration independent of Songs search; the queue list is omitted. |
| `GET /api/playback/events` | Current assignment immediately, followed by increasing committed revisions as NDJSON. Blank-line heartbeat every 15 seconds. Client uses a separate one-hour timeout and reconnects to synchronize. |
| `POST /api/playback/commands` | Tagged `{command:...}`: `play` with optional `audio_source_id`, `pause`, `stop`, `next`, `previous`, `started`, `finished` or `failed`. Start/completion/error commands require `playback_token`; internal device pauses include it too. Stale callbacks return the current assignment without transitioning. |
| `GET /api/playlists` | ID-ordered summaries `{id, name, item_count, cover_beatmap_id, custom_cover_revision}`. Counts include unavailable entries; automatic artwork comes only from the first inserted item. |
| `POST /api/playlists` | `{name}` creates a trimmed nonempty name; 201 with a complete summary, 400 for blank names. Equal names are allowed. |
| `GET /api/playlists/{id}` | `{id, name, items}`; each item includes stable source key, saved labels, item ID, nullable current library/audio/cover IDs and `last_played_at_ms`. 404 for an absent playlist. |
| `PATCH /api/playlists/{id}` | `{name}` renames; 200 with a complete summary, 400 for blank names, 404 if absent. |
| `DELETE /api/playlists/{id}` | 204 and membership cascade; 404 if absent. Does not change the queue. |
| `POST /api/playlists/{id}/items` | `{beatmap_ids:[...]}` atomically adds current difficulties and returns the full playlist. Repeated source keys are idempotent. Missing beatmap/hash is 400 with no partial additions. |
| `DELETE /api/playlists/{id}/items/{item_id}` | 204 for a member of this playlist, otherwise 404. Does not change the queue. |
| `POST /api/playlists/{id}/play` | Optional JSON `{start_item_id}` (or empty body). Resolves availability, replaces the queue and starts the requested/first available entry atomically. Returns an assignment. Empty/unavailable playlist or unavailable start is 400 without changing the queue; unknown playlist/item is 404. |
| `GET /api/playlists/{id}/cover` | Custom PNG bytes with `image/png`; 404 for an absent playlist or automatic cover. |
| `PUT /api/playlists/{id}/cover` | Raw 512×512 PNG up to 2 MiB; 200 with updated summary/revision, 400 for corrupt/wrong-size images, 413 for oversized uploads, 404 if absent. |
| `DELETE /api/playlists/{id}/cover` | 204 clears custom artwork and enables automatic fallback, including repeated reset; 404 for an absent playlist. |
| `GET /api/beatmaps/{id}/cover` | Stored background reference bytes, at most 16 MiB; absent/unreadable/oversized cover is 404. No path parameter or caller-supplied filesystem location. |
| `GET /api/audio-sources/{id}/audio` | Original Local/Copied file streamed with `Content-Length` and `application/octet-stream`; 404 for unknown ID or missing/nonregular file, 400 for Online. No caller-supplied paths. |
| `GET /api/audio-sources/{id}/duration` | `{duration_ms}` with nullable duration; absent audio ID is 404, missing/corrupt/unsupported media is null. Local/copied sources only. |
| `GET /api/user-data` | `{id, osu_folders}` with singleton `id = 1`. |
| `GET /api/user-data/osu-folders` | Folder array, including disabled entries. |
| `POST /api/user-data/osu-folders` | Body `{path, label?}`. Discovery validates an absolute path. New folder: 201; duplicate marker: 409 with the existing folder body, leaving its label unchanged. Invalid/ambiguous folder: 400. |
| `POST /api/user-data/osu-folders/discover` | Optional `{roots, depth}`; depth is `known`, `shallow`, or default `full`. Absolute explicit roots only; omitted/empty roots use scanner defaults. Streams NDJSON candidates and an explicit completion event. Dropping the body drops the scanner stream. |
| `POST /api/user-data/osu-folders/metadata` | `{marker_path}` validates an absolute Stable/Lazer marker, reads through the existing scanner reader and returns `{beatmap_count}` for individual difficulties, including zero. Preview never persists data. |
| `POST /api/user-data/osu-folders/import` | `{marker_path}` validates and fully reads a new source, then saves registration and snapshot in one transaction. Returns 200 with the stored folder, including already registered sources without changing their snapshot/settings. |
| `PATCH /api/user-data/osu-folders/{id}` | Optional `label` and `enabled`; 200 with folder, or 404. Omitted fields remain unchanged; explicit null clears label. Supplied non-null labels are trimmed by the service. |
| `DELETE /api/user-data/osu-folders/{id}` | 204 when removed, 404 when absent. Transactional cascade and shared cleanup; no file deletion. |

Each `beatmaps` entry adds `id`, `audio_source_id`, `difficulty_name`, `title`,
`title_unicode`, `artist`, `artist_unicode`, and `has_cover`. Cover availability
means a stored reference exists, not that the file is currently readable. The
single repository aggregate joins difficulties and metadata while retaining a
distinct, ID-ordered audio-source list per set. Existing fields remain unchanged.

### Audio settings

[Audio-settings routes](../../apps/osu-radio-server/src/routes/audio_settings.rs)
use `Services::audio_settings()` and exist in both router variants. Tracks,
playlist items and current assignments expose nullable `volume_percent`; omitted
fields are accepted by the client as no override. Values remain available while
individual mode is disabled. Settings writes do not change queue tokens/revisions;
the editing client applies them locally immediately. Queue transitions read their
stored override in the same snapshot as metadata. Settings load failures prevent
client playback until retry succeeds. Unexpected errors use the existing safe
error response. [Route checks](../../apps/osu-radio-server/src/routes/audio_settings.rs)
exercise public URLs, field validation, projections and idempotent removal.

### Shared playback queue

The [playback routes](../../apps/osu-radio-server/src/routes/playback.rs) use
`Services::queue()` exclusively. Each assignment contains
`{current_audio_source_id, current_playlist_item_id, track, volume_percent, duration_ms, mode, revision, playback_token,
can_next, can_previous}`; `track` uses the existing `/api/tracks` record shape.
Modes are `stopped`, `paused` and `playing`. Track/ID are null after exhaustion
or clearing. Metadata includes the cover ID and every difficulty even when the
current audio falls outside the active search. Duration reuses the content-based
Lofty probe; unreadable or unsupported media produces null, without preventing
the client from reporting a failed download/decode and advancing the queue.

User Pause needs only `{"command":"pause"}`. Device-failure feedback sends
`{"command":"pause","playback_token":T}`; the service pauses only that
current launch. This callback can be retried after transport failure without
pausing a newer assignment committed by another API writer.

After successfully starting its audio engine, the client sends
`{"command":"started","playback_token":T}`. The service checks the current
token, a playable source and Playing/Paused mode, allowing confirmation after
a pause of that launch. The first confirmation updates source-keyed history and
increments revision without changing mode or token. Duplicate and stale starts
are no-ops. The resulting assignment carries `track.last_played_at_ms` and uses
the existing event stream, so clients update dates without restarting audio.
Track and playlist date reads share their list's consistent transaction. Nullable
dates are UTC milliseconds; clients accept a missing field as unknown.

Every mutation publishes its committed revision before asynchronous duration
probing. `AppState` uses a watch channel that only accepts increasing revisions,
so a delayed response cannot replace a newer notification. The events handler
subscribes before reading its initial snapshot and emits only increasing
revisions afterward. Heartbeats also reread the persisted assignment to detect
independent writers or cancellation between commit and notification. Watch
notifications may coalesce intermediate revisions; every emitted assignment
reflects a committed snapshot. The body directly owns its receiver and read
stream, so dropping it releases the subscription without a detached task or an
open transaction waiting for an event.

Malformed or incomplete JSON commands follow Axum's 422 rejection; typed invalid
audio IDs use the deliberate 400 error body. Unexpected database/probe-task
failures retain the safe 500 boundary. Both router variants and OpenAPI register
all six endpoints. [Route tests](../../apps/osu-radio-server/src/routes/playback_tests.rs)
cover wire transitions, atomic validation, metadata/duration, repeated callbacks,
initial/reconnected snapshots, duplicate-ID tokens, notification ordering,
heartbeat detection and subscriber cancellation. The
[media tests](../../apps/osu-radio-server/src/routes/media.rs) additionally check
that duration probing rejects FIFO sources before opening them.

### User playlists

The [playlist routes](../../apps/osu-radio-server/src/routes/playlists.rs) use
`Services::playlists()` and publish committed playback revisions through the same
response path. Ordinary audio queue entries have a null playlist-item assignment;
playlist entries distinguish difficulties sharing one audio ID. While the item
exists, its assignment track contains that single difficulty. Queue content is
fixed at launch and survives subsequent playlist edits. All eleven playlist
operations are registered in both router configurations and OpenAPI.
Cover upload decodes the complete PNG through `radio-services`, bounded to
512×512 and 2 MiB. The persisted image is independent of the selected source
file. List and rename responses include metadata only; binary bytes are read
by the image endpoint. Reset retains the revision counter for future uploads.
[HTTP checks](../../apps/osu-radio-server/src/routes/playlists/tests.rs) verify
CRUD/statuses, PNG replacement/reset, corrupt/oversized uploads, summary fields,
atomic validation, queue replacement/item IDs, documentation and
the real `ApiClient` roundtrip on an isolated loopback server.

### Folder discovery and selection

The [folder routes](../../apps/osu-radio-server/src/routes/folder_selection.rs) are
registered in both router variants and the OpenAPI document. Candidate lines are
`{"event":"candidate","kind":"stable","root_path":"/osu","marker_path":"/osu/osu!.db","registered_id":null}`;
completion is `{"event":"complete"}` followed by a newline. Resolved marker paths
deduplicate aliases, and registered IDs match existing folders even when their
stored path is an alias. The response owns the cancellable scanner stream directly.

Metadata and import share server-side marker validation. Relative paths, unsupported
filenames and missing/nonregular markers return 400; source-reading or storage
failures use the existing safe 500 boundary. Stable and Lazer readers supply preview
counts without importing into the database. The client gives discovery, metadata
and import a one-hour request timeout instead of its ordinary 30 seconds.

Apply orchestration belongs to the client, with one independent import/deletion
transaction per action. A failed row cannot roll back successful sibling actions.
See the [database registration/import contract](database.md#snapshot-replacement)
and [client folder workflow](frontend.md#qt-frontend).

### Library search

The optional `q` parameter is split with Rust `split_whitespace()`. Every word must
match a literal substring of a track's tags, artist, title or difficulty; words
may match different fields or different difficulties sharing that audio ID.
Artist/title Unicode variants are included. Query and metadata use Unicode
`to_lowercase()`, matching the stored normalized tags; this is not full Unicode
case folding or accent normalization. Special characters have no SQL wildcard or
regular-expression meaning. Missing/whitespace-only queries preserve the complete
library response, including empty sets, and existing order is retained without ranking.

Both endpoints use the same optimized service search. Minimal unique metadata,
audio relationships and per-word matching tag set IDs are read in one snapshot
(SQLite transaction, PostgreSQL repeatable-read/read-only). The service matches
shared metadata once per hash, unions matches by audio ID, then loads complete
results only for matched audio in batches of at most 500 parameters inside that
same transaction. No matches skip full loading; blank queries keep the full
aggregate path. See [database](database.md).
All audio references participate globally by ID. Tags apply to all audio in their
set; matching a difficulty does not include unrelated audio from the same set.
The response retains only matched audio and all their metadata references, keeping
representative titles/covers unchanged. `has_multiple_audio_sources` is computed
before filtering so clients retain difficulty annotations when only one audio remains.
`search_tracks` groups returned references into unique audio tracks in existing order.

The compact [tracks route](../../apps/osu-radio-server/src/routes/tracks.rs) returns
`{audio_source_id, title, title_unicode, artist, artist_unicode, cover_beatmap_id,
difficulties}`. Text and cover ID are nullable. Representative text comes from the
lowest beatmap ID, even if null; cover uses the lowest ID with a stored reference.
Each difficulty is `{beatmap_id, beatmap_set_id, difficulty_name,
set_has_multiple_audio_sources}`, ordered by beatmap ID; duplicate names with
different IDs remain. The client retains every difficulty in `Track`, choosing
ordinary/Unicode/unknown text and unique subtitle names only for multi-audio sets.
Server and client DTOs are independent. Both router variants register this route.
[HTTP equivalence tests](../../apps/osu-radio-server/src/routes/tracks.rs) compare
complete client tracks against the legacy endpoint, including null text, cover
selection, split sets, shared audio, repeated names and encoded query symbols.

Matching takes O(K × S) for K query words and S searchable text/relationships.
Database reads, sorting, response serialization and transport are separate costs;
this is not an O(n) promise for the whole HTTP request. No search migration is needed.
[Service search contracts](../../crates/radio-services/src/tests/search.rs) exercise
matching and concurrent replacement across a read snapshot on both backends.
[HTTP tests](../../apps/osu-radio-server/src/routes/beatmap_sets.rs) exercise actual
query extraction and literal symbol decoding under both documentation configurations.

[Media handlers](../../apps/osu-radio-server/src/routes/media.rs) resolve IDs through
services, then use `spawn_blocking` for filesystem reads and Lofty probing. Lofty
sniffs contents rather than extensions and disables tag reading, supporting lazer
hash filenames. Files stay in place. Expected media failures yield neutral UI
states; unexpected database/task failures retain the safe 500 boundary. Both
router/documentation feature variants register cover, duration and audio endpoints.

Folder paths are display strings; native installation paths remain lossless `PathBuf`
values in services/storage (see [path encoding](database.md#schema-and-ownership)).
Folder JSON fields remain `id`, `kind`, `root_path`, `marker_path`, `label`,
`enabled`, and `last_scanned_at`. Unexpected failures retain the safe 500 error
boundary described above. Registration does not scan/import a library; the CLI
`store` workflow performs complete snapshot replacement. Audio locations in these
responses remain references; clients fetch bytes by ID through the audio endpoint.
The backend owns source-file access and streams bounded chunks without materializing
the whole file. The client owns downloading, its temporary file and audio output.
No schema migration is required.

## Evidence and verification limits

Client [`server.rs` tests](../../crates/osu-radio-client/src/server.rs) include
`the_bound_address_is_read_back_from_the_startup_line` and
`other_startup_lines_are_not_mistaken_for_the_address`. They exercise parsing
without launching a server. Additional Unix process regressions cover missing
binaries, a silent child, a live child closing stdout and startup cancellation,
including awaited child cleanup.

The ignored integration test
[`the_embedded_server_answers_on_the_port_it_reports`](../../crates/osu-radio-client/tests/embedded_server.rs)
launches a built server and also exercises database-backed requests. It is not a
pure supervision test; run it only against its isolated test environment. Server
route tests use [`test_support.rs`](../../apps/osu-radio-server/src/test_support.rs),
cover service-backed responses with isolated memory SQLite. Shared workflow and
folder-validation tests live in `radio-services`, as described in [database](database.md).

All behavior above is source-confirmed. Recommended checks and actual validation
results must be reported separately; a source review or compile check is not a
successful server launch or end-to-end playback test.
