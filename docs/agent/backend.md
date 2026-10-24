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

The [Arch recipe](../../packaging/arch/PKGBUILD) installs the server beside Qt
in `/usr/bin` and the complete framework-dependent lazer helper publish under
`/usr/lib/osu-radio/`. It sets `OSU_LAZER_REALM_PARSER_PATH` at build time to
`/usr/lib/osu-radio/osu-lazer-realm-parser`; the existing scanner build script
embeds that path and skips its development helper build. The runtime override
remains available. SQLite and API docs are enabled through the server's default
features. See the [packaging instructions](../../packaging/arch/README.md) for
dependencies and release verification limits; installed acceptance is T05/T06.

`ServerConfig::from_env` calls `dotenvy::dotenv()` before reading configuration,
unless the process variable `OSU_RADIO_SKIP_DOTENV` is exactly `1`. The Linux
client sets this variable on its child server, so embedded startup never searches
the current directory or its ancestors for `.env`. Standalone startup keeps
dotenv loading by default; other switch values do not disable it.
A missing `.env` (`NotFound`) is allowed: process environment alone can supply
configuration. A discovered file still loads without overriding existing process
variables. Other I/O errors and malformed file contents propagate as
`Failed to load .env`, even when a database URL is already set.

On Linux the SQLite build uses `$XDG_DATA_HOME/osu-radio/osu-radio.db` when
`SQLITE_DATABASE_URL` is absent. An absent, empty or relative XDG value falls
back to `$HOME/.local/share/osu-radio/osu-radio.db`; HOME must then be absolute.
Only this default path creates its parent directory. Failure to determine or
create it propagates before readiness. Explicit SQLite paths, URLs and `:memory:`
retain priority and their existing semantics, including missing-parent errors.
Other platforms still require `SQLITE_DATABASE_URL`; PostgreSQL always requires
`POSTGRES_DATABASE_URL` and never uses this fallback. Startup opens
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
bound address, prints its startup messages, starts the queue check worker, then enters `axum::serve`. This is
the current order in source, not a separate HTTP health-check protocol.

## Embedded readiness and lifecycle

The desktop frontend launches a child server by default. With
`OSU_RADIO_SERVER_URL` set, Qt connects to an existing server instead; see
[remote sessions](frontend.md#session-and-server-supervision). This does not
change the HTTP contracts or select a hosted-source provider.

`EmbeddedServer::start` uses `ServerOptions::address`, whose default is
`127.0.0.1:0`, as the child's `OSU_RADIO_SERVER_ADDRESS`. Port zero requests an
OS-assigned port. The option can be overridden; the supervisor does not always
force port zero. `ServerOptions::working_directory` changes the child's working
directory when present; otherwise it inherits the parent directory. On Linux
the child receives `OSU_RADIO_SKIP_DOTENV=1` regardless of the inherited value.
Other platforms retain dotenv search in the working directory and its ancestors.

[Configuration regressions](../../apps/osu-radio-server/src/config/tests.rs)
run in separate processes with cleared application environments and temporary
working directories. They cover environment-only configuration, the default and
explicit address, the Linux SQLite XDG/HOME fallback (including spaces, missing
HOME and directory creation errors), required URLs on other configurations,
valid dotenv loading, exact skip-switch semantics, process-variable precedence
and fatal parse/read errors. The
[repository dotenv test](../../apps/osu-radio-server/src/config/tests.rs) separately
parses the checkout's optional `.env` through `dotenvy::from_read_iter`, compares
the process environment before/after and withholds values in diagnostics. It
does not install those values, construct application state or connect to the
configured database. A checkout without that optional file skips this read.

The [startup tests](../../apps/osu-radio-server/tests/startup.rs) launch the actual
SQLite server without `.env`, using in-memory storage or private XDG/HOME
paths and an ephemeral loopback port. The default-path test saves a folder and
reads it after restart without a database URL, checking source preservation and
no database in cwd. They also check configuration/connection/bind failures, including a held
loopback port, nonzero exit and no false readiness announcement. Child-only
environment and private working directories keep the application database out of
these checks. Both docs feature states run them; PostgreSQL configuration checks
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
pointing at the reported URL. With `ServerOptions::remote_url`, it instead
validates an HTTP(S) base URL and reads `GET /api/queue` before reporting a
connection. Invalid URLs, HTTP/status/JSON failures and startup timeout fail the
connection; cancellation drops the pending request. Remote session shutdown and
drop never terminate the external server.

The [embedded-server integration test](../../crates/osu-radio-client/tests/embedded_server.rs)
also attaches a remote session to the real SQLite server, checks shared folder
and queue reads, and confirms it still answers after that session shuts down.

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
| `GET /api/user-data/audio-mods` | `{individual_mods_enabled, global_profile: {mode, speed_percent, adjust_pitch}}`; default false + none/100/false. |
| `PATCH /api/user-data/audio-mods` | Optional flat fields `individual_mods_enabled`, `global_mode`, `global_speed_percent`, `global_adjust_pitch`; omitted fields retained, invalid merged profile/null/type/unknown field → 400 without writing. |
| `PUT /api/audio-sources/{id}/mods` | Complete `{mode, speed_percent, adjust_pitch}` profile; 204, invalid profile/body → 400, unknown audio ID → 404. |
| `DELETE /api/audio-sources/{id}/mods` | 204 removes a profile, repeat for known ID is harmless; unknown ID → 404. |
| `GET /api/user-data/audio-settings` | `{individual_volume_enabled, global_volume_percent}`; default false/100. |
| `PATCH /api/user-data/audio-settings` | Optional fields update atomically; percentages are integers 0–100. Omitted fields are retained; null/wrong types fail validation. |
| `PUT /api/audio-sources/{id}/volume` | `{volume_percent}` stores an absolute 0–100 integer by durable audio identity; 204, 400 for range, 404 for unknown ID. |
| `DELETE /api/audio-sources/{id}/volume` | 204 removes an override; repeating for a known source is harmless. Unknown ID is 404. |
| `GET /api/queue` | Full `{entries, current_entry_id, history_len, mode, waiting_for_check, revision, playback_token}` from one service read snapshot, including past/current/future, duplicates and unavailable entries. No checks, writes or history trimming. Entry IDs are positive decimal i64 strings, through `9223372036854775807`. |
| `POST /api/queue/items` | Required `{items, placement:"end"\|"next"}`. Items are `{type:"audio", audio_source_id}` or `{type:"playlist_item", playlist_id, item_id}`. Atomic validation; missing/Online audio and unknown/foreign playlist items return 404. Append never starts playback. Returns the committed assignment. |
| `POST /api/queue/items/{entry_id}/play` | Starts the existing entry without copying it; checking saves waiting intent. Unknown entry is 404, existing unavailable entry is 400. |
| `PATCH /api/queue/items/{entry_id}` | Required `{before_entry_id:"decimal ID"\|null}`; null moves to end, omission fails extraction. Unknown entry/anchor is 404. Current ID/token are retained. |
| `DELETE /api/queue/items/{entry_id}` | Removes that occurrence, applying current-entry transition rules; 404 if absent. Returns the committed assignment. |
| `DELETE /api/queue/upcoming` | Clears future entries, retaining past/current and launch token. Returns the assignment. |
| `DELETE /api/queue` | Clears the whole queue, stops playback and invalidates the launch token. Returns the assignment. |
| `GET /api/playback` | Current assignment with metadata and duration independent of Songs search; the queue list is omitted. |
| `GET /api/playback/events` | Current assignment immediately, followed by increasing committed revisions as NDJSON. Blank-line heartbeat every 15 seconds. Client uses a separate one-hour timeout and reconnects to synchronize. |
| `POST /api/playback/commands` | Tagged `{command:...}`: `play` with optional `audio_source_id`, `pause`, `stop`, `next`, `previous`, `started`, `finished` or `failed`. Start/completion/error commands require `playback_token`; internal device pauses include it too. Stale callbacks return the current assignment without transitioning. |
| `GET /api/playlists` | ID-ordered summaries `{id, name, item_count, cover_beatmap_id, custom_cover_revision}`. Counts include unavailable entries; automatic artwork comes only from the first inserted item. |
| `POST /api/playlists` | `{name}` creates a trimmed nonempty name; 201 with a complete summary, 400 for blank names. Equal names are allowed. |
| `GET /api/playlists/{id}` | `{id, name, items}`; each item includes stable source key, saved ordinary/Unicode title and artist labels, item ID, nullable current library/audio/cover IDs and `last_played_at_ms`. 404 for an absent playlist. |
| `PATCH /api/playlists/{id}` | `{name}` renames; 200 with a complete summary, 400 for blank names, 404 if absent. |
| `DELETE /api/playlists/{id}` | 204 and membership cascade; 404 if absent. Does not change the queue. |
| `POST /api/playlists/{id}/items` | `{beatmap_ids:[...]}` atomically adds current difficulties and returns the full playlist. Repeated source keys are idempotent. Missing beatmap/hash is 400 with no partial additions. |
| `DELETE /api/playlists/{id}/items/{item_id}` | 204 for a member of this playlist, otherwise 404. Does not change the queue. |
| `POST /api/playlists/{id}/queue` | Appends the whole playlist including unavailable items in one service operation; does not launch. Empty append preserves revision/token; unknown playlist is 404. |
| `POST /api/playlists/{id}/play` | Optional JSON `{start_item_id}` (or empty body). Retains history through current inclusive and replaces the future with every playlist item, selecting the requested/first resolved item with checking wait. Returns an assignment. A wholly unavailable nonempty playlist becomes stopped future with no current. Empty playlist or unavailable explicit start is 400 without queue changes; unknown playlist/item is 404. |
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
| `POST /api/user-data/osu-folders/import` | `{marker_path}` validates and fully reads a new source, then saves registration, library snapshot and collections in one transaction. Returns 200 with the stored folder, including already registered sources without changing their snapshot/settings. |
| `POST /api/user-data/osu-folders/{id}/import` | Fully rereads a registered source and atomically replaces its library and imported collections. Returns 200 with the current folder, preserving settings; unknown/deleted ID returns 404. |
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

### Audio mods HTTP contract

[Mods routes](../../apps/osu-radio-server/src/routes/audio_mods.rs) use
`Services::audio_mods()` in both router variants. `mode` is `none|dt|ht`,
`speed_percent` is an integer, and `adjust_pitch` is boolean. None requires
100/false; DT permits 101–200, HT permits 50–99. NC is DT with pitch enabled,
not a separate wire mode. PATCH has flat optional fields, while GET/PATCH return
the complete settings with a nested `global_profile`. Changing only the mode
does not infer speed/pitch defaults: callers must send enough fields for a valid
merged profile. JSON null and unknown fields are rejected. Body deserialization
errors use the same 400 JSON error envelope as semantic validation; unexpected
service errors retain the safe 500 response.

`/api/tracks`, playlist items, full queue entries and playback assignments
(including their nested `track`) always contain `mods`, either null or the
saved individual profile. They never substitute the global effective profile.
OpenAPI marks this field required and nullable. An absent override is distinct
from an explicitly saved none/100/false profile. Assignment mods use the retained
launch source after rebinding, while the queue row may have another profile.
Writes preserve queue revision/token, volume settings and original duration.
The server continues serving the original audio; applying mods is client work.

Client [wire types](../../crates/osu-radio-client/src/models.rs) are
`AudioModsMode`, `AudioModsProfile`, `AudioModsSettings` and `AudioModsPatch`.
The [ApiClient](../../crates/osu-radio-client/src/api.rs) exposes async methods:
`audio_mods() -> Result<AudioModsSettings, ApiError>`,
`update_audio_mods(&AudioModsPatch) -> Result<AudioModsSettings, ApiError>` and
`set_audio_mods(id: i32, profile: Option<AudioModsProfile>) -> Result<(), ApiError>`.
PATCH omits `None` fields; `set_audio_mods(id, None)` sends DELETE, while
`Some(profile)` sends PUT. `Track.mods` and library/playlist/queue/assignment
mappings retain the saved profile without changing source duration or requiring
a separate profile fetch before Play. Settings loading, local persistence queues
and playback application belong to the later client integration cards.

[Route tests](../../apps/osu-radio-server/src/routes/audio_mods/tests.rs) cover
validation/atomic rejection, defaults, projections, original duration, unchanged
queue identity, real ApiClient roundtrips and served OpenAPI. The existing
[rebinding stream test](../../apps/osu-radio-server/src/routes/queue_transport_tests.rs)
checks differing launch/row profiles. Client
[contract tests](../../crates/osu-radio-client/src/api/contract_tests.rs),
[NDJSON tests](../../crates/osu-radio-client/src/api/playback_tests.rs) and model
tests cover request shapes, errors, decoding and retained Track fields.
These fixtures use temporary databases/sources; DSP, UI and native-platform
acceptance are separate.

### Shared playback queue

The [playback routes](../../apps/osu-radio-server/src/routes/playback.rs) use
`Services::queue()` exclusively. Each assignment contains
`{current_entry_id, availability, waiting_for_check, current_audio_source_id,
current_playlist_item_id, track, volume_percent, mods, duration_ms, mode, revision,
playback_token, can_next, can_previous}` in REST and NDJSON. `current_entry_id` is
a nullable decimal string; availability is null or `checking|available|unavailable`.
`waiting_for_check` is persisted intent, not inferred from availability: an assigned
old source may keep playing while its entry's new binding is checking. Duration is
probed only for available assignments without waiting; checking returns null.
`track` uses the existing `/api/tracks` shape. Client waiting behavior is implemented in the shared player/worker; see [frontend](frontend.md#local-audio-playback).
Existing route fixtures complete checks explicitly unless a test starts a worker.

Each queue entry exposes `id`, `position`, saved `title`, `title_unicode`, `artist`,
`artist_unicode`, `difficulty_name`, nullable `audio_source_id`, `playlist_item_id`,
`beatmap_id`, `beatmap_set_id`, `cover_beatmap_id`, `availability`, nullable
`last_played_at_ms`, `volume_percent` and `mods`. Paths, source keys, check generations,
launch sources and migration evidence stay internal. Nullable response properties
are always present and documented as required/nullable.

T04 reconciles queue links in the library transaction after cleanup, before commit.
Assigned playback retains its original audio ID/source/generation and token while the
full queue entries can point to a new checking source. Assignment dates, volume and mods use
the retained source even after library cleanup. A late Failed changes entry availability
only when its binding and generation still match, then follows the existing transition.
A waiting Play whose source disappears advances with playing/paused intent; source
restoration without Play stays stopped. These service contracts and rollback/visibility
checks are in [queue T04 tests](../../crates/radio-services/src/tests/queue_t04.rs).
The worker selects checks from entries, not launch bindings. Import/reimport and folder
deletion wake it after commit; the client preserves an unchanged entry/token engine across these updates.

### Queue availability worker

[`QueueCheckWorker`](../../apps/osu-radio-server/src/queue_check.rs) belongs to
`serve`, which starts it only after state recovery and listener preparation and awaits
`shutdown` when serving returns. `AppState` holds only the shared `Notify` wakeup;
it never owns the worker handle. Tests start workers explicitly. Dropping a handle
signals stop; the supervisor drains its blocking file tasks without applying late
results. Awaited shutdown also joins the supervisor, leaving no worker tasks or
later writes. An already executing apply transaction finishes before this acknowledgement.
There is no forced file-I/O cancellation or new OS-signal protocol: abrupt child
termination still uses the existing embedded-server lifecycle above.

Startup, wakeup, slot completion and a one-second interval select saved `checking`
entries through `pending_checks()`. Each refill reads current priorities; only a
current launch with `waiting_for_check` receives priority. At most four blocking
operations run, with no duplicate `QueueCheck` in flight. Already available or
unavailable entries are not reopened on restart. A task panic or apply failure
releases its slot, logs to stderr and defers that check until the next interval.
File errors yield unavailable rather than a retry loop.

The worker and audio route share
[`open_audio_file`](../../apps/osu-radio-server/src/audio_file.rs): check a regular
file before opening for read, then check its opened descriptor. Local/Copied checks
do not decode media and perform no write transaction. Sequential `apply_check()`
retains the service guards on entry ID, generation and source binding. It never
reports Started or records a listening date. Checked waiting launches preserve
playing/paused intent; append and restored unavailable playlists stay stopped.

Committed revisions use the existing monotone watch/NDJSON path. Every scan reads
and publishes the stored revision even if no checks remain, recovering lost
post-commit library notifications. Queue changes and playlist Play use
`committed_response`; imports and folder deletion call `AppState::wake_queue_checks`
after successful commit. T06 routes reuse that hook through `committed_response`, with
immediate `publish_playback(revision)` when a committed revision is already available.
No new HTTP DTO or service contract was added in T05.

The [worker tests](../../apps/osu-radio-server/src/queue_check/tests.rs) use real
temporary SQLite state with controlled blocking operations, plus actual regular,
missing, directory and Unix FIFO paths. They exercise the four-slot limit, priority,
deduplication, restart, lost wakeups/revisions, panic retry, awaited/drop shutdown,
late removal/clear/reimport results, retained active launch bindings, playing/paused
waiting, no automatic playback/Started/date, and existing HTTP/NDJSON delivery.
These tests do not establish Windows file behavior or physical playback.

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
token, its retained launch source, cleared waiting and Playing/Paused mode, allowing confirmation after
a pause or library removal of that launch. The first confirmation updates source-keyed history and
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

Malformed JSON follows Axum's syntax rejection (400); wrong types, variants and
missing required body fields follow its 422 rejection. Invalid path entry IDs are
400; unknown entries/anchors/playlists and invalid audio/item references are 404.
Playing an existing unavailable entry is 400. Validation failures retain queue,
revision/token and issued IDs and publish no notification. Unexpected database/probe-task
failures retain the safe 500 boundary. Both router variants and OpenAPI register
all queue/playback endpoints. [Route tests](../../apps/osu-radio-server/src/routes/playback_tests.rs)
cover wire transitions, atomic validation, metadata/duration, repeated callbacks,
initial/reconnected snapshots, duplicate-ID tokens, notification ordering,
heartbeat detection and subscriber cancellation. The
[media tests](../../apps/osu-radio-server/src/routes/media.rs) additionally check
that duration probing rejects FIFO sources before opening them.

[T06 transport tests](../../apps/osu-radio-server/src/routes/queue_transport_tests.rs)
exercise actual ApiClient/server roundtrips, atomic 404/extraction failures, exact
BIGINT strings, full snapshots, worker NDJSON, reimport launch-source retention,
late callbacks, and required/nullable OpenAPI fields. Executed checks and limits
are recorded in [T06](../impl/queue-rework/T06.md#отчёт-о-выполнении).

### User playlists

The [playlist routes](../../apps/osu-radio-server/src/routes/playlists.rs) use
`Services::playlists()` and publish committed playback revisions through the same
response path. Ordinary audio queue entries have a null playlist-item assignment;
playlist entries distinguish difficulties sharing one audio ID. Their assignment
and pending queue tracks use the entry's saved single difficulty and `title`,
`title_unicode`, `artist` and `artist_unicode` variants, including after item deletion.
Storage and migration are described in [the database guide](database.md#playback-queue). Nullable
Unicode fields are additive; clients accept older responses that omit them. Queue
snapshots are fixed at insertion and survive subsequent playlist edits. Whole-playlist
Play retains history and every new item; `PlaylistService::append_to_queue` provides
the same snapshots without launching through `POST /api/playlists/{id}/queue`. An empty
append is a revision/token-preserving no-op. Checking is completed explicitly in
service tests and by the T05 worker in the server; client waiting support is implemented in the shared worker.
All twelve playlist
operations are registered in both router configurations and OpenAPI.
Cover upload decodes the complete PNG through `radio-services`, bounded to
512×512 and 2 MiB. The persisted image is independent of the selected source
file. List and rename responses include metadata only; binary bytes are read
by the image endpoint. Reset retains the revision counter for future uploads.
[HTTP checks](../../apps/osu-radio-server/src/routes/playlists/tests.rs) verify
CRUD/statuses, PNG replacement/reset, corrupt/oversized uploads, summary fields,
atomic validation, Unicode playlist/playback/pending queue field roundtrips, queue
history/item IDs, documentation and the real `ApiClient` roundtrip on an isolated loopback server.

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
and both import operations a one-hour request timeout instead of its ordinary 30 seconds.

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

Additional client lifecycle assertions distinguish missing/nonexecutable child
binaries, early exit and invalid UTF-8 output, and observe cleanup after explicit
shutdown, startup cancellation and Drop. Binary resolution precedence is checked
in child-only environment probes with private executable paths; no shared binary
or parent environment is changed. The [coverage worklist](test-coverage.md#client-http-boundaries-and-lifecycle-2026-10-15)
records executed Linux results and unforced failure paths.

The ignored integration test
[`the_embedded_server_answers_on_the_port_it_reports`](../../crates/osu-radio-client/tests/embedded_server.rs)
launches a built server and also exercises database-backed requests. It is not a
pure supervision test; run it only against its isolated test environment. Server
route tests use [`test_support.rs`](../../apps/osu-radio-server/src/test_support.rs),
cover service-backed responses with isolated memory SQLite. Shared workflow and
folder-validation tests live in `radio-services`, as described in [database](database.md).

The [router contracts](../../apps/osu-radio-server/src/routes/mod.rs) now verify
all API paths and method/path pairs with `docs` enabled/disabled, reject
unsupported verbs and resolve every served OpenAPI schema reference. Projection
checks compare complete track/set DTO JSON; rejected settings patches retain old
values. [AppState tests](../../apps/osu-radio-server/src/state.rs) reopen a private
SQLite database to verify paused queue recovery and monotone shared notifications.
The [coverage worklist](test-coverage.md#server-routes-projections-and-recovery-2026-10-15)
records actual runs and the separate benchmark, PostgreSQL compile-only, platform
and physical-output limits.

All behavior above is source-confirmed. Recommended checks and actual validation
results must be reported separately; a source review or compile check is not a
successful server launch or end-to-end playback test.

Queue-rework integration evidence is recorded in [T10](../impl/queue-rework/T10.md#отчёт-о-выполнении).
The opt-in real-backend Qt probe and [private fixture](../../apps/osu-radio-qt/tests/fixtures/queue_backend.py)
use a built SQLite server, actual import routes, real availability worker and
HTTP-created entries above 2^53. The companion [client engine probe](../../crates/osu-radio-client/src/playback/tests/real_backend.rs)
uses the same server with the existing private fake engine; it follows retained
launch identity through HTTP reimport and late callbacks. Only the Realm producer
is synthetic. These probes do not establish real Realm compatibility, physical
audio output or native desktop interaction.
