# Database and repository contracts

[radio-services](../../crates/radio-services/src/lib.rs) is the cloneable application
entry point for persisted models: `Application → Model service → Repository → Database`.
`Services` privately owns a [radio-db](../../crates/radio-db/src/lib.rs) `Database`.
Server and CLI depend on services, with no direct repository or ORM access.
Services re-export plain models and result types; database handles and transactions
are not part of the service API. There is no generic service or dependency-injection framework.

`radio-db` uses SeaORM 2.0.2. SQLite is the default; exactly one of `sqlite` and
`postgres` must be enabled. Entities, active models, pools and ORM transaction types
are private. The opaque `Transaction` exposes repository accessors and consuming
`commit`/`rollback`; dropping an unfinished transaction rolls it back. Repositories
privately use SeaORM's `DatabaseExecutor` for either a pool or borrowed transaction.

## Schema and ownership

The versioned [initial migration](../../crates/radio-db/src/migrations/m20260913_000001_library.rs)
and subsequent migrations create the current schema below. Migration definitions
are fixed and independent of future entity changes; add another versioned
migration for subsequent changes.

| Table | Identity and relationships |
| --- | --- |
| `user_data` | Singleton settings anchor, primary key constrained to `1`; individual volume mode and global volume percentage. |
| `osu_installations` | Generated `i32` ID; required settings owner; unique marker path; kind, root/marker paths, label, enabled flag and last-scanned timestamp. |
| `beatmap_sets` | Generated `i32` ID; required installation; optional online ID and source hash. |
| `beatmaps` | Generated `i32` ID; required set; difficulty, BPM, native source hash and nullable indexed MD5; optional independent metadata and audio references. Source kind is derived from the installation. |
| `beatmap_metadata` | SHA-256 text primary key; immutable shared imported content. |
| `tags` | Generated `i32` ID; globally unique normalized name, SQLite `BINARY` / PostgreSQL `C` collation. |
| `beatmap_set_tags` | Composite primary key `(beatmap_set_id, tag_id)`; set cascade, restrictive tag reference, index on `tag_id`. |
| `audio_sources` | Generated `i32` ID; globally unique `(kind, location)`; `local`, `copied`, or `online`. |
| `audio_volume` | Composite primary key `(audio_kind, source_location)` and absolute integer `volume_percent` in 0–100. No library foreign key. |
| `listening_history` | Composite primary key `(audio_kind, source_location)` using the audio source's existing identity; UTC millisecond timestamp and last acknowledged playback token. No library foreign key. |
| `playback_queue` | Singleton `id = 1`; nullable current entry ID, `history_len` boundary, playback mode, persisted `waiting_for_check`, nullable launch audio ID/kind/location/generation without foreign keys, BIGINT revision/token and retained last issued entry ID. |
| `queue_entries` | Positive permanent BIGINT ID; unique dense position, independent audio/playlist source keys, nullable advisory IDs, ordinary/Unicode caption and difficulty snapshots, availability and check generation. No library/playlist foreign keys or deletion cascades. |
| `playlists` | Generated `i32` ID and nonempty trimmed name; equal names are allowed. Nullable custom PNG and a retained monotonic cover revision; optional unique source/collection provenance independent of installation deletion. |
| `playlist_items` | Generated `i32` ID; required playlist with deletion cascade; unique `(playlist_id, source_kind, beatmap_hash)`, explicit native/MD5 hash kind and saved ordinary/Unicode title and artist plus difficulty. No library foreign key. |

Installation deletion cascades through sets, beatmaps and set-tag links. Shared references use
restrictive foreign keys. Repository cleanup deletes only unreferenced metadata
and audio rows, plus tags without set links, using correlated `NOT EXISTS` queries.
Join/cleanup foreign keys are indexed. Persistence does not copy
or delete files, including sources marked `copied`.

The additive [background migration](../../crates/radio-db/src/migrations/m20260913_000002_background.rs)
adds nullable `beatmaps.background_path`. Resolved background locations are separate
from immutable metadata hashes. Snapshot replacement resolves `background_file`
through the same named-file map as audio and stores the reference in its existing
transaction. Existing rows remain null until CLI reimport without `--clear`; startup
never scans, backfills, or resets them. Migration and rollback tests cover this
reference alongside the previous snapshot.

The [native-path migration](../../crates/radio-db/src/migrations/m20260914_000003_native_paths.rs)
converts existing installation root/marker text to JSON strings, preserving IDs and
marker uniqueness. New UTF-8 paths use the same representation; non-Unicode paths
use Serde's native `OsStr` JSON (`Unix` bytes or `Windows` UTF-16 units). Repository
and service models return `PathBuf`, so invalid Unicode remains reversible on its
native platform. JSON-shaped literal paths cannot collide with encoded native paths.
Foreign-platform native encodings fail decoding explicitly. Previously lost bytes
cannot be reconstructed; this migration does not permit a lossy downgrade.
HTTP folder fields and CLI output remain display strings; HTTP registration still
accepts Unicode JSON paths. Native scanner/service registration preserves the bytes.

## Metadata identity

The pure [metadata_hash](../../crates/radio-db/src/repositories/beatmap_metadata.rs)
helper accepts imported metadata and returns lowercase SHA-256 hexadecimal text.
Hash input is a deterministic JSON tuple in this exact order:

```text
["radio-db:metadata:v2", title, title_unicode, artist, artist_unicode,
 author-or-null, source, preview_time, audio_file, background_file]
author = [online_id, username, country_code]
```

Nulls, empty strings, author presence and original filenames are significant.
Retained metadata fields are not normalized. The database stores author as a
nullable JSON object with those three fields. Tags are separate set relationships
and do not affect metadata identity; persisted metadata has no `tags` or `user_tags`.
Resolved audio locations and background paths are separate beatmap references and never enter the hash. Insertions
compute the hash themselves, reuse an existing row without changing it, and
reject a matching key whose stored content differs.

## Tags and existing versioned databases

[TagService](../../crates/radio-services/src/tag.rs) unions tags across every
metadata-bearing difficulty in a set. `Tags` is split with `split_whitespace()`;
each `UserTags` element remains a whole name, including internal whitespace.
Names are trimmed, converted with Rust `to_lowercase()` (including Unicode), and
empty names are skipped. Deduplication follows normalization. All sources, sets
and installations share one tag ID for an equal normalized name. No Unicode
normalization or full case folding is applied. The
[repository](../../crates/radio-db/src/repositories/tag.rs) normalizes before
lookup/insertion and inserts links idempotently. Import and cleanup use the outer
snapshot transaction; deletion/empty replacement retain tags still linked elsewhere.
Reimport reuses existing tag IDs before cleaning orphan rows. Unreferenced tags
are deleted; a later reintroduction need not retain the deleted ID.

The [tag migration](../../crates/radio-db/src/migrations/m20260919_000004_tags.rs)
upgrades existing versioned databases without a reset or Realm reread. It extracts
old metadata tags, creates links through existing beatmaps to their sets, then
creates v2 metadata hashes and merges equal remaining content, verifying content
on matching hashes. It redirects beatmap references before removing old metadata
rows and both tag columns. Its old entity and hash encoding are frozen locally so
future runtime model changes cannot change this historical migration. All schema,
data and history changes run inside the existing schema lock/transaction. A
failure rolls everything back; downgrade is rejected because original tag spelling,
order and distribution between metadata records cannot be reconstructed.
Scanner import types and Realm/NDJSON retain the original tag fields. This adds no
HTTP endpoints or GUI tag controls.

## Shared service entry points

Access these concrete services through `Services`. Existing operations are exposed;
there are no new standalone beatmap or set creation workflows.

| Accessor | Operations |
| --- | --- |
| `user_data()` | `get`, `overview`; overview composes installation reads through `OsuInstallationService`. |
| `osu_installations()` | `all`, `get`, `register` (resolved scanner marker), `register_folder` (discovery-validated absolute path), `update`, `delete`, `replace_snapshot`, `replace_imported_snapshot`, `register_imported_snapshot`, `reimport_folder`. |
| `beatmap_sets()` | `get`, `for_installation`, `all_with_audio_sources`, `search_with_audio_sources`, `search_tracks`; legacy aggregates retain set/audio/map order; tracks group globally by audio ID. |
| `beatmaps()` | `get`, `for_set`. |
| `beatmap_metadata()` | `get`, `get_or_insert`; the pure `metadata_hash` helper is re-exported. |
| `tags()` | `all`, `get`, `for_set`; lists use `ORDER BY name ASC` with SQLite `BINARY` / PostgreSQL `C`. |
| `audio_sources()` | `get`, `find`, `get_or_insert`. |
| `audio_settings()` | `get`, `update`, `set_volume`; settings and durable per-audio overrides. |
| `queue()` | `get`, `snapshot`, `upcoming`, `playback`, `insert`, `append`, `play_entry`, `move_entry`, `remove_entry`, `clear_upcoming`, `clear`, `pending_checks`, `apply_check`, `command`, `recover`; one persistent server queue. |
| `playlists()` | `all`, `get`, `create`, `rename`, `delete`, `add_items`, `remove_item`, `play`, `cover`, `set_cover`, `clear_cover`, `append_to_queue`; concrete difficulties with source-stable membership. |

[Installation services](../../crates/radio-services/src/osu_installation.rs) own
folder discovery validation, label trimming, snapshot replacement and deletion.
Registration returns `Created` or `AlreadyRegistered`; duplicate marker paths retain
the stored row and label. Updates change only supplied fields in one transaction.
`FolderChanges.label: None` means omitted, `Some(None)` clears it, and an empty string
remains an empty string. Supplied labels are trimmed on update, as in the HTTP contract.

Repositories own SQL and constraints, including immutable metadata identity. Set
and beatmap insertion and shared-row cleanup are internal service operations used
by the import/deletion workflows. Repositories expose the corresponding persistence
primitives through both `Database` and `Transaction`; they never begin nested
transactions. Applications cannot obtain those handles through `Services`.

## Search reads

`Database::begin_read` exposes an opaque transaction for consistent multi-query
reads: a regular SQLite read transaction or PostgreSQL `REPEATABLE READ READ ONLY`.
`BeatmapSetService` reads minimal unique metadata and audio-bearing difficulty
relationships, plus matching set IDs per unique normalized word, inside that
transaction. `TagRepository::matching_set_ids` binds literal substring values to
SQLite `instr` / PostgreSQL `strpos`, selects tag IDs first, and resolves distinct
set IDs through the existing `tag_id` index. It never loads repeated tag names
for search or interprets input as a pattern.

The service normalizes shared metadata once per hash and unions dynamic word
vectors by global audio ID, across sets and difficulties. There is no 64-word
limit. After matching, the same transaction loads full data only for the matching
audio IDs, in chunks of at most 500 parameters, restoring set/audio/map order.
Multiplicity is computed from unfiltered relationships and supplied to the
filtered aggregate loader. No matches skip full loading. Blank queries retain
the original single aggregate query, including empty sets on the legacy API.
`search_tracks` groups that result for Songs; formatting remains in the client.

[Search contracts](../../crates/radio-services/src/tests/search.rs) cover literal
symbols, cross-reference matches, 70 unique words, three ID chunks, and a committed
replacement from a second pool between projections and full result loading.
Both backends must retain the old snapshot. The existing tag migration is unchanged;
this optimization adds no schema or persistent index.

## Playback queue

The [original queue migration](../../crates/radio-db/src/migrations/m20261001_000005_playback_queue.rs)
created one empty state. The frozen [row migration 12](../../crates/radio-db/src/migrations/m20261016_000012_queue_entries.rs)
replaces the former JSON arrays/index with `queue_entries` and singleton state, inside
the existing schema lock and outer transaction. It preserves every position, duplicate,
current position, mode, revision and token; it neither trims nor starts playback.
The old JSON/index columns are removed. Legacy/unversioned databases still require
an explicit reset or a new database; this upgrade applies to versioned databases only.

Each entry retains the audio's exact `(kind, location)` encoding and, when available,
the playlist item's `(installation kind, hash kind, beatmap hash)`. `PlaylistItem`
now exposes `hash_kind`. Captions prefer retained playlist values, then library
metadata, then the explicit `Unavailable track` title. Lost numeric references
are retained in `migration_audio_source_id` / `migration_playlist_item_id`; missing
paths and hashes are never fabricated. Local/Copied entries begin `checking`, other
entries `unavailable`, all with generation zero. The server availability worker performs the actual file I/O outside database
transactions; services persist availability and waiting intent without probing files.

`QueueEntryId` is a positive `i64` newtype with decimal `Display`; Qt/QML must receive
a string, never a floating point number. `QueueState` exports `entries`,
`current_entry_id`, `history_len`, `last_entry_id`, mode, `waiting_for_check`, `launch_source` and revision/token.
With a current entry, its position equals `history_len`. Without one, entries before
the boundary are past; empty queues use zero and exhausted queues use the list length.
`allocate_id`, `reindex` and `validate` support service transitions.
IDs are not reissued after deletion/clear; all positions/counters fit signed BIGINT,
IDs/order are unique/dense, and Playing/Paused require an existing current entry.
Waiting additionally requires a current `checking` entry and Playing/Paused mode.
The [waiting migration 13](../../crates/radio-db/src/migrations/m20261016_000013_queue_waiting.rs)
infers waiting for active current checking entries without changing list/order/mode/counters.
Downgrade to v12 rejects waiting intent that cannot be reconstructed, including an
already assigned launch that is checking again without waiting. Migrations 1–12 stay frozen.

The [repository](../../crates/radio-db/src/repositories/queue.rs) reads list and singleton
in one LEFT JOIN statement, including pool callers under PostgreSQL READ COMMITTED.
`save` requires a transaction-bound repository and caller-held singleton writer lock;
it validates before writing, rejects counter regression/reused IDs, replaces the rows
and updates the singleton with one outer commit. Advisory IDs can outlive their library
or playlist rows; saved keys/captions never depend on those rows continuing to exist.
Reset includes queue entries and the singleton launch binding.

The additive [launch-source migration 14](../../crates/radio-db/src/migrations/m20261016_000014_queue_launch_source.rs)
adds four nullable singleton fields without library foreign keys. SQL enforces an all-null
or complete binding with nonnegative generation, active current entry and no waiting.
It fills the binding from a complete current entry for active nonwaiting v13 launches;
legacy missing keys stay missing. Downgrade compares this frozen inference and refuses
any state whose source binding v13 cannot reconstruct. Migrations 1–13 stay unchanged.

`QueueService::reconcile_sources_in` runs after shared-row cleanup, before mark-scanned
or outer commit, in installation deletion and the common snapshot replacement workflow.
Both replace APIs, registration with import and reimport therefore use the same writer
lock/transaction. Playlist entries resolve their own `(installation kind, hash kind, hash)`
through `PlaylistRepository::resolve_sources`, independent of item existence; the lowest
beatmap ID with Local/Copied audio wins. Library entries resolve exact `(kind, location)`.
Numeric migration evidence never recovers a missing key. Lost audio/map/set/cover IDs
are cleared; entry IDs, order, item IDs, saved keys and captions remain. A changed/lost
binding advances generation; restored/changed playable sources become checking. Changes
only to map/set/cover references retain availability/generation. A changed queue advances
revision once; an unchanged queue is not saved. Any error/overflow/cancellation rolls
back library, queue, collections and timestamp together.

The assigned launch retains `QueueLaunchSource { audio_source_id, source, check_generation }`
and token across entry rebinding/removal of library sources. New tokens clear the old
binding. Verified assignment or successful waiting check captures it; recovery captures
it from the current entry again, paused and waiting if necessary. A waiting launch that
loses its source advances with its playing/paused intent; restoration alone never selects
or starts a track. `pending_checks()` reads queue entries, never the retained launch source.
[T04 contracts](../../crates/radio-services/src/tests/queue_t04.rs) cover these paths,
late callbacks, exact/shared keys, rollback, cancellation and independent-pool visibility;
[migration contracts](../../crates/radio-db/src/tests/queue.rs) cover v13→v14/reopen/reset
and guarded downgrade on SQLite and disposable PostgreSQL.

Downgrade first reconstructs the complete frozen v11→v12 result and compares every
entry field, order/ID, boundary and last issued ID. A changed snapshot, availability,
generation, nonreconstructible key, extra issued ID or boundary blocks downgrade
before any schema changes; supported states round-trip through populated down/up.

[QueueService](../../crates/radio-services/src/queue.rs) opens one transaction,
locks the existing settings singleton before reads, validates every new Local/Copied
ID, applies the transition, saves and commits once. Invalid input leaves the
entire queue unchanged. The same lock coordinates queue changes with snapshot
replacement/deletion across independent pools and processes. Reads assemble the
queue and current metadata within one consistent read transaction.

`insert(Vec<QueueItemRef>, QueuePlacement)` accepts Local/Copied audio IDs or a
concrete `(playlist_id, item_id)`. It checks membership atomically and retains unresolved
playlist items as unavailable with their saved keys/captions. `End` appends; `Next`
inserts immediately after current, or at the future boundary without current.
`append` delegates to this path. Both placements preserve order/duplicates, allocate
independent permanent IDs and never select a track, start music or lift pause.

`play_entry` selects the existing ID without copying it. Unknown entry/anchor IDs
return `QueueError::UnknownEntry`; explicit Play of an unavailable entry returns
`UnavailableEntry`. Invalid audio and foreign/missing playlist items return
`InvalidAudioSource` / `InvalidPlaylistItem`. Errors roll back the entire mutation.
Library Play of different audio inserts immediately after current (or at the future
boundary) and launches it. Resume retains the token; another entry, including the same
audio, gets a new token. Without current, Play starts at the future boundary;
if no future remains, explicit Play replays from the beginning.

`move_entry(id, before_id)` moves before an anchor, or to the end for `None`.
It keeps current ID/token and follows current with the history boundary. Without
current it retains the count of past entries. Removing past decreases the boundary;
removing a different entry or `clear_upcoming` keeps the current launch. Removing
current selects the next available/checking entry with Playing/Paused intent, or
stops if none remains. `clear` invalidates the launch and retains the issued-ID counter.

Next/Previous follow list order and skip unavailable or unresolved/Online sources.
They select checking entries and persist `waiting_for_check`, preserving Playing/Paused
intent. Previous at the first playable entry restarts it; from exhaustion it returns
the last candidate. Stop followed by Play restarts with a new token. Every new launch,
Stop, clear, exhaustion and recovery of current invalidates old playback callbacks.
Pause/resume and list edits that retain current do not change its token.

`pending_checks()` returns saved checking jobs with current waiting entry first.
`QueueCheck` carries entry ID, generation, audio ID and exact `SourceType`. `apply_check`
accepts a result only if all four still match and availability is still checking.
Waiting success clears waiting without changing token/mode; failure marks the entry
unavailable and advances with the same intent. Availability updates of an already
assigned launch keep current/token/mode. Accepted Failed marks its current entry
unavailable only if audio ID, exact source and generation still match the retained
launch binding, then advances using the existing intent. Deleted/cleared IDs, changed generation/source and duplicate
results are no-ops. Checks never record listening. Started and Finished are ignored
while waiting; Started additionally requires current token, active mode and the retained
launch source. `PauseIfCurrent` remains token-bound. Checked counters and BIGINT conversions
roll back failures, including history writes in the same transaction.

After actual mutations or recovery, service transitions remove only the earliest
past entries above 5000, counting unavailable entries. Current and future are retained;
without current, `history_len` defines past. Reads, no-op edits, duplicate Started and
stale callbacks/results never trim or change revision/token. Revision increases once
per committed queue/history mutation. Recovery pauses current, invalidates old callbacks,
restores waiting if current is checking and trims history without autostart. Empty or
exhausted queues stay stopped; oversized past is still trimmed on recovery.

`snapshot()` returns singleton fields and every ordered `QueueSnapshotEntry`, containing
the retained `QueueEntry`, listening date and individual volume override. One read
transaction covers all three projections. Captions come from entries, never a changed
playlist. Existing history/volume services resolve dates/overrides by the saved source
key through batched repository queries, even when advisory IDs are null or library rows
have disappeared. Duplicates, unavailable rows and nullable references remain visible;
reading does not resolve sources, probe files or trim history.

Service `PlaybackAssignment` includes current entry ID, availability and waiting in
addition to audio/item IDs, `LibraryTrack`, mode, revision/token and transport flags.
The existing HTTP DTO still omits the three new service fields until T06. Playlist
assignments use saved captions/single difficulty after item deletion; ordinary audio
assignments retain captions and current library difficulty/cover references.
`upcoming()` remains the legacy filtered pending projection. Whole-playlist append
and history-preserving launch are implemented by T03 below; import reconciliation is T04.
The server file worker and client waiting behavior remain T05 and T07 respectively.
For assigned playback, audio ID, listening date and volume use the retained launch source,
including a deleted library source; full queue entries expose their current resolved links.

Automated T02 evidence is recorded in [the card](../impl/queue-rework/T02.md).
[Service contracts](../../crates/radio-services/src/tests/queue_t02.rs) exercise identity,
insertions/edits, checks, history boundaries, source-keyed projections and rollback;
[repository contracts](../../crates/radio-db/src/tests/queue.rs) cover v12→v13/reopen and
downgrade guards. Physical audio, Qt and native Windows acceptance are separate.

## Audio volume settings

The additive [audio-settings migration](../../crates/radio-db/src/migrations/m20261001_000008_audio_settings.rs)
adds `individual_volume_enabled` (false) and `global_volume_percent` (100) to
`user_data`, and creates `audio_volume` without resetting existing library,
queue, playlists or listening history. Both stored percentages have SQL range
constraints. The [settings service](../../crates/radio-services/src/audio_settings.rs)
validates 0–100 and writes through the singleton writer lock and one transaction.
An omitted patch field is retained; removing an override is idempotent for an
existing audio ID. Unknown audio IDs are rejected.

Overrides are absolute percentages keyed by the audio source's exact existing
`(kind, location)` identity, using the same native-path encoding as listening
history. All difficulties and playlists sharing audio share its setting. Library
cleanup retains overrides; reimport of the same source resolves them for new IDs.
Changing kind or moving the file creates a new identity. Explicit application
reset clears settings and overrides. Disabling individual mode retains overrides.

The [volume repository](../../crates/radio-db/src/repositories/audio_volume.rs)
resolves IDs in batches of 500. Library tracks, available playlist items and
playback assignments expose nullable `volume_percent` within their consistent
read transaction, including while the mode is disabled. Playback assignments
also carry the value at their top level. The client combines it with the loaded
mode/global settings before starting the engine.
[Repository checks](../../crates/radio-db/src/tests/audio_settings.rs) cover the
seven-migration upgrade, constraints, read snapshots, rollback, reopen, native
keys, source removal/return, batching and reset on SQLite/PostgreSQL.
[Service checks](../../crates/radio-services/src/tests/audio_settings.rs) cover
shared difficulty/playlist projections, retained overrides and reimport.

## Listening history

The additive [listening-history migration](../../crates/radio-db/src/migrations/m20261001_000007_listening_history.rs) preserves the library, playlists and
queue. Existing queue positions do not prove successful playback and are not
backfilled. A concrete [history repository](../../crates/radio-db/src/repositories/listening_history.rs)
and [service](../../crates/radio-services/src/listening_history.rs) read dates in batches
inside the transaction that loads library tracks, playlist items or the current
assignment.

`QueueService` records `Started { playback_token }` within its existing singleton
writer lock and outer transaction. It validates the current token, cleared waiting
and retained launch source in Playing/Paused mode, then accounts for that source/token
once, including after its library source has been removed or rebound. A new
date increments queue revision while retaining the launch token, position and
mode. Pausing and continuing the same launch do not update history; a new launch
can update it again. Duplicate or stale callbacks leave revision and date intact.

History stores the source's kind and stored location independently of transient
audio IDs. Folder deletion and shared-source cleanup can remove every library
reference without removing history or retaining audio-source rows. Reimporting
the same source reconnects its date; moving the file creates another identity.
Queue clear preserves history. Explicit application reset removes it.

[Repository checks](../../crates/radio-db/src/tests/listening_history.rs) and
[service checks](../../crates/radio-services/src/tests/listening_history.rs) cover
the migration and source/token lifecycle on each database backend.

## User playlists

The additive [playlist migration](../../crates/radio-db/src/migrations/m20261001_000006_playlists.rs)
added playlists, saved item snapshots and a historical nullable queue JSON column
without clearing the library or queue. Migration 12 consumes that column; a null
historical value means one null item ID per old audio ID. Current queue storage
uses entry rows, with no parallel persistent JSON arrays.

The additive [cover migration](../../crates/radio-db/src/migrations/m20261001_000009_playlist_covers.rs)
adds a nullable binary PNG and a nonnegative `i64` revision without rewriting
existing playlists, membership, the library or queue. Upload increments the
revision under the singleton writer lock; resetting the cover keeps the counter
so a later upload gets a different version. The service accepts fully decodable
512×512 PNG files up to 2 MiB, with bounded PNG-only `image` decoding. It stores
image bytes, independently of the original file. `cover` is the only read that
selects those bytes; detail, rename and list responses omit binary data.

The additive [Unicode-name migration](../../crates/radio-db/src/migrations/m20261003_000010_playlist_unicode_names.rs)
adds nullable `title_unicode` and `artist_unicode` snapshots to `playlist_items`.
Backfill uses the lowest beatmap ID with Local/Copied audio for the exact source
kind and beatmap hash, matching availability resolution. Each complete import
refreshes those two fields for manually created playlists inside its existing transaction; unresolved keys
retain their last known Unicode names. Ordinary names and difficulty in manually created playlists stay as
originally saved; imported collections refresh all saved labels when metadata is available. New items save the raw ordinary and Unicode variants separately.
Older unavailable items without retained metadata cannot recover Unicode text
until matching source data returns. Snapshot changes roll back with failed imports.

`PlaylistSummary` contains `id`, `name`, `item_count`, `cover_beatmap_id` and
`custom_cover_revision`. The list uses one aggregated repository statement in a
consistent read transaction, without loading each playlist's contents. Counts
include unavailable entries. Automatic artwork resolves the first inserted
item only, using its representative audio cover; it remains null if that item
has no available artwork. A custom cover exposes its revision, otherwise the
revision is null. Name changes return the same complete summary.

[`PlaylistService`](../../crates/radio-services/src/playlist.rs) trims names and
rejects whitespace-only names. Additions accept current beatmap IDs, validate all
of them and their nonempty chosen hashes before inserting, and commit once under
the existing singleton writer lock. Repeated keys are idempotent; first insertion
order is retained by item ID. Names need not be unique. Current beatmap row IDs
are returned when reading but never own membership.

The [repository](../../crates/radio-db/src/repositories/playlist.rs) resolves
`(installation kind, hash kind, beatmap hash)` against the current library in bound
queries of at most 500 parameters. For an equal key it chooses the lowest beatmap
ID with Local/Copied audio. Without such a copy, current beatmap/audio/cover IDs are
null; saved labels remain visible. Playlist reads use one consistent transaction.
Entries sharing audio use its lowest-ID stored cover reference, matching the
library projection and the existing audio-based client media scheduler.
Snapshot replacement, empty snapshots and installation deletion retain membership;
returning matching source data restores availability. Physical file existence and
decoding remain the existing playback worker's responsibility.

`append_to_queue(id) -> PlaybackAssignment` resolves and appends every playlist
item under the singleton writer lock, in one transaction with one outer commit.
It preserves current entry, mode, waiting, history boundary and playback token;
a nonempty append increments revision once. An empty playlist is a no-op returning
the existing assignment without changing revision/token or trimming history.
An unknown playlist returns `PlaylistError::NotFound`.

`play(id, start_item_id)` uses the same transaction discipline. It retains the old
list through current inclusive (without current: through `history_len`), replaces
only the future with the entire playlist, and selects the requested or first
resolved item. Every new position has a fresh entry ID, original item ID, source key
including `hash_kind`, and saved ordinary/Unicode captions and difficulty.
Resolved items start `checking`, unresolved ones `unavailable`. The launch waits
for checking through T02 and invalidates the previous playback token; neither
insertion nor completion of checks records a listening date.

A wholly unavailable nonempty playlist is retained as future with the boundary
before it, no current entry, Stopped mode and a new token. Returning sources does
not create Play. An empty Play returns `NoAvailableItems`; an unknown/foreign start
returns `ItemNotFound`, an explicitly unavailable start `UnavailableItem`.
All errors leave the queue and counters untouched. Both operations apply the
5000-past-entry limit after changes. Later playlist edits/deletion preserve queue
composition, keys and snapshots, including item IDs and single-difficulty playback
projections. Next/Previous and callbacks reuse T02's entry/token transitions.

[T03 contracts](../../crates/radio-services/src/tests/queue_t03.rs) cover append in
empty/exhausted/playing/paused states, repeated difficulties sharing audio,
source/MD5 keys and unavailable items, history-preserving and explicit starts,
errors/no-op, stale callbacks/checks, no false listening dates, detached playlist
snapshots, history trimming, rollback and independent-pool writers. Actual checks
and limits are recorded in [T03](../impl/queue-rework/T03.md).

[Migration/repository checks](../../crates/radio-db/src/tests/playlists.rs) cover
a populated five-migration upgrade, an eight-migration upgrade with existing
playlists, a nine-migration Unicode upgrade/downgrade, minimum usable-ID backfill,
source-kind separation, old queue preservation, summary counts/first-item artwork,
cover revision retention, cascades and reset.
[Service checks](../../crates/radio-services/src/tests/playlists.rs) cover CRUD,
reopen, concurrent idempotent addition, atomic validation, reimported IDs, source
removal/return, copy priority, distinct source kinds, 503 ordered items, PNG
validation, replacement/reset, persistence after deleting the original file, queue
history preservation, shared audio, selected start, pause/resume and stale callbacks.
Unicode contracts cover playback/pending queue projections, import refresh without
changing saved ordinary labels, unavailable snapshots, source return and
failed-import rollback.

### Imported osu! collections

The additive [collection migration](../../crates/radio-db/src/migrations/m20261004_000011_imported_collections.rs)
adds nullable indexed `beatmaps.md5_hash`, `playlist_items.hash_kind` (`source` by default,
or `md5`) and nullable playlist provenance with a unique `(origin_source, origin_collection)`
pair. Existing native hashes and item uniqueness `(playlist_id, source_kind, beatmap_hash)`
are preserved. Existing lazer MD5 stays null until the source is reread; migration never
fabricates MD5 from native `Hash`. Downgrade refuses collections or MD5 membership.

Complete scanner snapshots synchronize collections into ordinary playlist views under
the existing writer lock and snapshot transaction. Source identity is the canonical marker
path encoded by the existing lossless path codec. Lazer collection identity is its Guid;
stable identity is JSON `[original_name, zero_based_occurrence_among_equal_names]`.
An empty or whitespace-only name displays as `Unnamed collection`; original names stay
in stable identity. Stable rename creates a new playlist and retains the old one; lazer
rename updates the existing playlist. Equal names and empty collections are supported.

Synchronization updates name and game membership, preserves custom PNG/revision, and
keeps IDs/order of remaining items. New members append, repeated MD5 references collapse,
and separate difficulties sharing audio stay separate. Manual membership edits are
replaced on explicit import. Missing collections and playlists of deleted installations
remain saved. Missing beatmaps keep saved labels and become unavailable until a matching
source returns. New unknown MD5 members have no saved metadata yet. Imported playlist
manual additions prefer MD5 when available, so adding the same difficulty is idempotent.
Manual playlists retain native hashes. Availability, artwork and Unicode refresh all
resolve the recorded hash kind with the same Local/Copied priority.

[Migration contracts](../../crates/radio-db/src/tests/collections.rs) cover the populated
migration-10 upgrade, native default/null MD5, uniqueness, valid hash kinds and guarded
downgrade. [Collection service contracts](../../crates/radio-services/src/tests/collections.rs)
cover rename, duplicate/empty names, membership and cover retention, native/MD5 collision,
unknown/removing/returning maps, source deletion/re-add and failure after collection changes.
The shared harness runs these contracts with temporary SQLite or disposable PostgreSQL.
Historical migration fixtures insert original columns directly, independent of current entities.


## Snapshot replacement

Read the entire scanner result before calling `OsuInstallationService::replace_imported_snapshot`
(`replace_snapshot` remains a sets-only compatibility API). It requires an
existing installation and matching source kinds. In one transaction it locks the
singleton with a write statement, deletes the old installation sets, inserts all
sets through `BeatmapSetService::add`, beatmaps through `BeatmapService::add`, and
shared records through metadata/audio services and set links through `TagService`.
It synchronizes supplied collection playlists and refreshes saved Unicode names, then cleans up unreferenced shared rows and updates `last_scanned_at`. Every participating service uses repositories bound to that same transaction, with
no pool fallback. Only the outer workflow commits. Errors or cancellation before
commit roll back the old snapshot, collection playlists, shared rows and timestamp together. Empty snapshots clear
that installation's library. Other installations remain intact; shared rows still
referenced elsewhere retain their identities. Snapshot set/beatmap IDs may change.

[`register_imported_snapshot` / `import_folder`](../../crates/radio-services/src/folder_selection.rs)
add atomic registration plus replacement for Qt Apply. Source reading finishes
before the transaction opens. The transaction locks the singleton, matches resolved
marker identity, registers a new installation and calls the existing replacement
workflow through the same transaction. Only the outer workflow commits; any error
rolls back registration as well as snapshot/shared rows. Already registered sources
return their stored row without reading/replacing their metadata or changing settings.
Identity matching repeats under the writer lock so concurrent imports are idempotent.
Preview metadata reads never write. `reimport_folder(id)` explicitly reads the full source,
then reloads the same ID under the writer lock and checks source identity before replacing.
Deletion during reading returns absence and never recreates the folder. Concurrent label/enabled
changes are preserved; only the scan timestamp changes. [Temporary folder checks](../../crates/radio-services/src/tests/folder_selection.rs)
cover bad collection reads and a delayed helper across concurrent settings/deletion at this boundary.

Deletion takes the same lock and performs cascade deletion plus cleanup in one
transaction. The lock coordinates processes and independent pools; it does not
rely on a server mutex. This serializes snapshot writers globally. Reads remain
available under SQLite WAL and PostgreSQL MVCC. Finer locking would need a new
shared-record cleanup strategy if write throughput becomes a bottleneck.

## Connections, migration and explicit reset

`Services::connect` configures the pool without changing schema. Call `migrate` on ordinary
startup, or explicitly call `reset` when discarding application data is intended.
`check_connection` pings the pool. File SQLite uses WAL, foreign keys and a
five-second busy timeout on every connection. Memory SQLite uses one pooled
connection. Existing plain SQLite paths and `:memory:` remain supported.

Migration and explicit reset acquire the same database-level schema lock before
checking history or selecting pending migrations: SQLite `BEGIN IMMEDIATE`, or a
PostgreSQL transaction advisory lock under `READ COMMITTED`. All checks, migrations
and history writes use that transaction; commit, errors and cancellation release
its lock. Independent pools/processes therefore cannot select the same pending work.

Application tables without applied SeaORM migration history produce an actionable
legacy-database error. There is no legacy data migration: choose a new database or
explicit reset. Reset drops only application-owned tables and their SeaORM/
Diesel migration history, in child-first order, then recreates the schema in the
same transaction. Unrelated tables are preserved; reset does not use a broad
schema refresh or drop external objects.

The server stores `Services` in `AppState`; CLI uses the same service accessors. CLI/server select
`SQLITE_DATABASE_URL` for SQLite or `POSTGRES_DATABASE_URL` for PostgreSQL.
The CLI requires its selected URL. Only the Linux SQLite server has a
[user-data fallback](backend.md#current-startup-configuration) when its URL is
absent; connection creation, migrations and explicit reset are shared unchanged.
`store` reads a complete scanner snapshot, registers the marker and replaces its
library; `--count` and skipped counts are removed. `--clear` explicitly resets all
application data before registration. Output reports inserted sets/beatmaps and
the distinct audio sources referenced by the snapshot, including reused sources.

## Verification limits

[Service contracts](../../crates/radio-services/src/tests.rs) cover registration races,
partial updates, generated IDs, full/empty replacement, shared cleanup, forced
mid-import rollback, independent-pool replacement/deletion, cancellation after all
workflow writes but before commit, and source-file preservation. The same contracts
run on temporary SQLite files and fresh disposable PostgreSQL databases. A separate
test-only ORM connection installs failure triggers and checks row counts; there is
no public raw-SQL service escape hatch. Folder-validation tests use explicit temporary roots.
[Marker/reimport assertions](../../crates/radio-services/src/tests/folder_selection.rs)
check exact relative/missing/directory/wrong-name errors without registration and
reject changed source kind/path before replacement, retaining all previous data.

[Repository contracts](../../crates/radio-db/src/tests.rs) retain legacy detection,
reset scope, restrictive foreign keys, explicit/drop rollback and immutable metadata
checks. They also cover 20 independent-pool startup/upgrade races and reset races,
native path registration/readback/duplicates, existing text-path migration (including
JSON-shaped filenames), ordered aggregates with empty sets and shared audio, and
cleanup with null references. On Linux, native-path checks use distinct invalid
UTF-8 bytes; Windows surrogate checks require running the contracts on Windows.
[Hash tests](../../crates/radio-db/src/repositories/beatmap_metadata.rs) pin
v2 encoding, each retained field, and exclusion of tags.
Plain declarations are tested through these consumers: folder patches distinguish
omitted/null/empty values, queue contracts compare complete state across reopen and
rollback, and tag contracts check normalized identities and composite links.
[Audio settings](../../crates/radio-db/src/tests/audio_settings.rs) and
[listening history](../../crates/radio-db/src/tests/listening_history.rs) compare
source-keyed values across independent pools, read snapshots, rollback and reset.
[Queue boundary assertions](../../crates/radio-db/src/tests/queue.rs) reject
duplicate IDs, sparse order, invalid empty/current/boundary states and counters
outside BIGINT without changing the persisted list or singleton. The [service queue tests](../../crates/radio-services/src/tests/queue.rs)
also seed the persisted revision/token/ID counters at BIGINT's maximum: failed commands retain
the exact queue and playback assignment. A missing settings row is tested inside
a rolled-back fixture; get/settings/update/lock report the missing row and never
recreate it implicitly.
The [coverage worklist](test-coverage.md) keeps declaration/consumer evidence
separate from source-line measurements and remaining projection audits.
[Field projection contracts](../../crates/radio-db/src/tests/model_projections.rs)
use explicit expected metadata/set/beatmap values and compare complete library,
search and playlist projections on SQLite and disposable PostgreSQL. They cover
empty sets, absent metadata/audio, Unicode text, chunk-boundary deduplication,
filtered multiplicity, persisted custom covers and detached playlist labels after
library deletion. All fixture writes are rolled back. Playlist volume/history
enrichment belongs to the service layer; repository projections leave those fields null.
[Track projection assertions](../../crates/radio-services/src/beatmap_set/tracks.rs)
compare all library-track/difficulty fields while merging audio across sets,
preserving first-seen audio order, choosing labels by the globally lowest
difficulty ID and selecting a cover independently from the label representative.
[Service history assertions](../../crates/radio-services/src/tests/listening_history.rs)
bound acknowledged timestamps by wall-clock Unix milliseconds. [Volume assertions](../../crates/radio-services/src/tests/audio_settings.rs)
check stored 0/100 overrides, omitted-field updates and typed validation errors.
[Playlist assertions](../../crates/radio-services/src/tests/playlists.rs) cover
damaged PNG pixel data and exact NotFound errors across mutations of a deleted
playlist, while comparing retained covers and queue state.
[Migration lifecycle contracts](../../crates/radio-db/src/tests/migration_lifecycle.rs)
first verify the frozen v1 schema's singleton/kind/required-field/uniqueness and
foreign-key constraints, lookup indexes and installation cascade while retaining
shared metadata/audio rows. They then step through migrations 5–12 in both directions, checking migration
history, table/column/index presence, retained installation/difficulty/background
references and an unrelated table, then reapply the latest schema and verify
default queue/audio settings. A separate fixture rolls background migration 2
down/up and confirms native-path migration 3 refuses a lossy downgrade; migration
4's tag downgrade also leaves data/history intact on rejection. The same lifecycle
contract runs in SQLite and the disposable PostgreSQL repository suite.

The [service benchmark fixture](../../crates/radio-services/src/beatmap_set/benchmarks.rs)
creates its own six-set SQLite database, asserts tag/difficulty/Unicode/empty
search cardinalities and runs all three snapshot benchmark reference stages.
Library rows and an unrelated sentinel table remain unchanged. The large synthetic
benchmark separately seeds temporary 1,000/10,000/50,000-set databases; its debug
instrumented timings do not establish production performance.
[Connection tests](../../crates/radio-db/src/connection.rs) exercise supported
memory URL forms without creating files, and plain file paths/SQLite URLs with
WAL, busy timeout, enforced foreign keys and reopen/readback. Pure
[source identity tests](../../crates/radio-db/src/model/audio_source.rs) retain
location spelling and reject unknown or differently cased/whitespace-padded kinds.
[Migration tag contracts](../../crates/radio-db/src/tests/tag_migration.rs) cover
filled upgrades, shared links, metadata merging, collision rollback, dropped
columns, repeat/concurrent migration and rejected downgrade.
[Service tag contracts](../../crates/radio-services/src/tests/tags.rs) cover Unicode
lowercasing, whitespace, whole user tags, ordering, global IDs across sources and
installations, repeated/empty replacement and shared cleanup. The existing failure
and cancellation contracts also compare tag rows and set links before/after rollback.
See [development](development.md) for commands.
These checks do not exercise a real Realm library, GUI interaction, audio playback
or Windows deployment.

Executed on Linux on 2026-10-15 for the per-file test audit: SQLite repository
tests passed 10 checks, and PostgreSQL 18.6 passed six repository checks (including
the shared migration lifecycle) and the service contract on two fresh
`radio_db_test_*` databases. SQLite service regressions and scoped denied-warning
Clippy results are recorded in the [coverage worklist](test-coverage.md).
The temporary loopback PostgreSQL cluster was stopped and removed after the run;
the configured application database was not opened. These are fixture-backed
persistence checks, not native Windows or frontend acceptance.

Executed on Linux on 2026-09-14: repository/service contracts passed with SQLite
and a fresh disposable PostgreSQL cluster; CLI and both SQLite server router
variants passed their tests. Scoped Clippy passed for SQLite and PostgreSQL callers.
A single PostgreSQL `EXPLAIN (ANALYZE, BUFFERS)` run with 50,000 metadata rows,
50,000 audio rows, 200,000 beatmaps and 4 MiB `work_mem` used hash anti joins:
metadata cleanup took 40.2 ms and audio cleanup 33.4 ms. All shared rows were
referenced; deletion/null-reference correctness is covered by the contracts.
Benchmark setup was rolled back. Windows native-path execution remains unverified.

The [T10 integration report](../impl/queue-rework/T10.md#отчёт-о-выполнении)
records fresh SQLite/PostgreSQL migration/repository/service contracts, including
populated v11→v12, v12→v13 and v13→v14 fixtures, downgrade guards, reset scope,
rollback/overflow and independent-connection visibility. Real-backend probes use
private versioned SQLite, HTTP import/reimport and paused recovery. The fixture
seeds only the retained allocator above 2^53 with test-only SQL after shutdown;
entries and operations still traverse HTTP/services/repositories.
