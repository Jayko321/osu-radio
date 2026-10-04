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
| `playback_queue` | Singleton `id = 1`; JSON audio-ID history and parallel nullable playlist-item IDs, nullable current index, playback mode and monotonic revision/token counters. |
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
| `queue()` | `get`, `upcoming`, `playback`, `append`, `clear`, `command`, `recover`; one persistent server queue. |
| `playlists()` | `all`, `get`, `create`, `rename`, `delete`, `add_items`, `remove_item`, `play`, `cover`, `set_cover`, `clear_cover`; concrete difficulties with source-stable membership. |

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

The additive [queue migration](../../crates/radio-db/src/migrations/m20261001_000005_playback_queue.rs)
creates one empty state row without changing the library. The concrete
[repository](../../crates/radio-db/src/repositories/queue.rs) stores the ordered
JSON ID array, including duplicates and played history. A parallel array carries
playlist-item IDs; ordinary queue additions and explicit audio-ID plays carry null.
There is no foreign key from queue history to playlist items. Empty queues have a null
index; exhaustion retains the history with an index equal to its length. No
foreign key binds history to audio rows, so source cleanup can remove deleted
library sources while retaining the queue's previous positions.

[QueueService](../../crates/radio-services/src/queue.rs) opens one transaction,
locks the existing settings singleton before reads, validates every new Local/Copied
ID, applies the transition, saves and commits once. Invalid input leaves the
entire queue unchanged. The same lock coordinates queue changes with snapshot
replacement/deletion across independent pools and processes. Reads assemble the
queue and current metadata within one consistent read transaction.

First append to empty/exhausted history starts the first new entry. Append with
an active current entry retains its mode, including pause. Explicit Play of a
different audio ID inserts immediately after current, before pending items. Play
of the paused current track resumes with its token; Stop followed by Play restarts
with a new token. Next/Previous preserve pause, and select a paused entry when
starting from Stop. Previous at the first playable entry restarts it; from
exhaustion it returns the last playable entry. Transitions skip deleted/Online
IDs while keeping their history entries. Decode/download failure advances via
the client's Failed callback; exhaustion stops. Seeking stays local; persisted
volume settings are described below.

Revision increases once for a persisted change; no-op commands and stale callbacks
retain it. Every new launch, restart, Stop, clear, exhaustion and active-state
recovery of a current entry invalidates the prior playback token. Pause/resume retains it. Finished
requires the current token and Playing mode; Failed requires the current token
and Playing/Paused mode, preserving pause on its transition. `PauseIfCurrent`
requires the current token for retriable device-failure feedback; ordinary user
Pause remains global. Duplicate callbacks
cannot advance twice. Counters use checked arithmetic and checked BIGINT conversion.
Server startup explicitly calls `recover` once after migration: a current entry
becomes Paused with a new token/revision, retaining history and index, including
an entry stopped before shutdown. Already-paused states also invalidate
old-process callbacks; empty/exhausted queues remain stopped. The next Play
begins recovered audio from the start.

Assignments contain only the current audio ID, `LibraryTrack`, mode, revision,
token and transition availability. Metadata loads only that audio's references,
preserving the same lowest-beatmap representative/cover, complete difficulty
list and unfiltered set multiplicity as Songs. It does not depend on a search
result or materialize the complete library. ID validation reads queue IDs in
batches of 500. Physical source availability remains the client's download/
decoding responsibility; extensionless imported locations remain valid.

`upcoming()` reads the queue and pending audio summaries in one read transaction.
It excludes current and history, keeps duplicate positions and skips deleted/Online
sources. Metadata loads only pending audio references and is reused for repeated
audio/item pairs; playlist entries use their saved ordinary/Unicode labels and
single difficulty while the item exists. Other entries use the library audio summary,
independently of Songs search or later playlist ordering. It neither probes files
nor changes the queue.

[Repository queue contracts](../../crates/radio-db/src/tests/queue.rs) cover a
populated pre-queue upgrade, concurrent migration, reopen, rollback, position
validation and explicit reset. [Service queue contracts](../../crates/radio-services/src/tests/queue.rs)
cover atomic validation, copied/Online inputs, insertion/history, duplicate IDs,
EOF/failure, pause/resume/Stop, boundaries, stale/duplicate callbacks across pools,
concurrent append/clear, recovery, removed sources and search-independent metadata.
Both run through the ordinary disposable SQLite/PostgreSQL contract harnesses.
These deterministic storage/transition checks do not establish physical output.

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
writer lock and outer transaction. It validates the current token and playable
source in Playing/Paused mode, then accounts for that source/token once. A new
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
creates the two playlist tables, indexes `beatmaps.hash` for resolution, and adds
the nullable queue JSON column without clearing the library or queue. A null
column in a pre-playlist queue reads as one null item ID per existing audio ID.
The repository validates equal array lengths on every queue save.

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

`play(id, start_item_id)` resolves the playlist and replaces the queue in one
writer-locked transaction. Unavailable entries are skipped. An empty or wholly
unavailable playlist, unknown start item, or unavailable requested start produces
a typed error and leaves the old queue untouched. A successful launch stores
both audio and item IDs, selects the requested or first available item and gets
a new launch token. Later playlist edits/deletion leave the queue composition
unchanged. Assignments retain `current_playlist_item_id` even after item deletion;
while the item exists its saved labels and single difficulty are projected into
the track. Deleted-item metadata falls back to the normal audio track projection.
Next/Previous and completion callbacks reuse the existing position/token rules.

[Migration/repository checks](../../crates/radio-db/src/tests/playlists.rs) cover
a populated five-migration upgrade, an eight-migration upgrade with existing
playlists, a nine-migration Unicode upgrade/downgrade, minimum usable-ID backfill,
source-kind separation, old queue preservation, summary counts/first-item artwork,
cover revision retention, cascades and reset.
[Service checks](../../crates/radio-services/src/tests/playlists.rs) cover CRUD,
reopen, concurrent idempotent addition, atomic validation, reimported IDs, source
removal/return, copy priority, distinct source kinds, 503 ordered items, PNG
validation, replacement/reset, persistence after deleting the original file, queue
replacement, shared audio, selected start, pause/resume and stale callbacks.
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
explicit reset. Reset drops only the twelve application tables and their SeaORM/
Diesel migration history, in child-first order, then recreates the schema in the
same transaction. Unrelated tables are preserved; reset does not use a broad
schema refresh or drop external objects.

The server stores `Services` in `AppState`; CLI uses the same service accessors. CLI/server select
`SQLITE_DATABASE_URL` for SQLite or `POSTGRES_DATABASE_URL` for PostgreSQL.
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

[Repository contracts](../../crates/radio-db/src/tests.rs) retain legacy detection,
reset scope, restrictive foreign keys, explicit/drop rollback and immutable metadata
checks. They also cover 20 independent-pool startup/upgrade races and reset races,
native path registration/readback/duplicates, existing text-path migration (including
JSON-shaped filenames), ordered aggregates with empty sets and shared audio, and
cleanup with null references. On Linux, native-path checks use distinct invalid
UTF-8 bytes; Windows surrogate checks require running the contracts on Windows.
[Hash tests](../../crates/radio-db/src/repositories/beatmap_metadata.rs) pin
v2 encoding, each retained field, and exclusion of tags.
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

Executed on Linux on 2026-09-14: repository/service contracts passed with SQLite
and a fresh disposable PostgreSQL cluster; CLI and both SQLite server router
variants passed their tests. Scoped Clippy passed for SQLite and PostgreSQL callers.
A single PostgreSQL `EXPLAIN (ANALYZE, BUFFERS)` run with 50,000 metadata rows,
50,000 audio rows, 200,000 beatmaps and 4 MiB `work_mem` used hash anti joins:
metadata cleanup took 40.2 ms and audio cleanup 33.4 ms. All shared rows were
referenced; deletion/null-reference correctness is covered by the contracts.
Benchmark setup was rolled back. Windows native-path execution remains unverified.
