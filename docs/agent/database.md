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
creates the following schema. It is a fixed migration definition, independent of
future entity changes; add another versioned migration for subsequent changes.

| Table | Identity and relationships |
| --- | --- |
| `user_data` | Singleton settings anchor, primary key constrained to `1`. |
| `osu_installations` | Generated `i32` ID; required settings owner; unique marker path; kind, root/marker paths, label, enabled flag and last-scanned timestamp. |
| `beatmap_sets` | Generated `i32` ID; required installation; optional online ID and source hash. |
| `beatmaps` | Generated `i32` ID; required set; difficulty, BPM, source hash; optional independent metadata and audio references. Source kind is derived from the installation. |
| `beatmap_metadata` | SHA-256 text primary key; immutable shared imported content. |
| `audio_sources` | Generated `i32` ID; globally unique `(kind, location)`; `local`, `copied`, or `online`. |

Installation deletion cascades through sets and beatmaps. Shared references use
restrictive foreign keys. Repository cleanup deletes only unreferenced metadata
and audio rows, using correlated `NOT EXISTS` queries against beatmap references.
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
["radio-db:metadata:v1", title, title_unicode, artist, artist_unicode,
 author-or-null, source, tags, user_tags, preview_time, audio_file, background_file]
author = [online_id, username, country_code]
```

Nulls, empty strings, author presence, tag order and original filenames are
significant. There is no normalization. The database stores author as a nullable
JSON object with those three fields and user tags as a JSON array. Resolved audio
locations and background paths are separate beatmap references and never enter the hash. Insertions
compute the hash themselves, reuse an existing row without changing it, and
reject a matching key whose stored content differs.

## Shared service entry points

Access these concrete services through `Services`. Existing operations are exposed;
there are no new standalone beatmap or set creation workflows.

| Accessor | Operations |
| --- | --- |
| `user_data()` | `get`, `overview`; overview composes installation reads through `OsuInstallationService`. |
| `osu_installations()` | `all`, `get`, `register` (resolved scanner marker), `register_folder` (discovery-validated absolute path), `update`, `delete`, `replace_snapshot`. |
| `beatmap_sets()` | `get`, `for_installation`, `all_with_audio_sources`; aggregate read retains its single repository query and returns `BeatmapSetWithAudio` records, grouped directly into a vector in SQL set-ID order. |
| `beatmaps()` | `get`, `for_set`. |
| `beatmap_metadata()` | `get`, `get_or_insert`; the pure `metadata_hash` helper is re-exported. |
| `audio_sources()` | `get`, `find`, `get_or_insert`. |

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

## Snapshot replacement

Read the entire scanner result before calling `OsuInstallationService::replace_snapshot`. It requires an
existing installation and matching source kinds. In one transaction it locks the
singleton with a write statement, deletes the old installation sets, inserts all
sets through `BeatmapSetService::add`, beatmaps through `BeatmapService::add`, and
shared records through metadata/audio services. It then cleans up unreferenced shared rows and
updates `last_scanned_at`. Every participating service uses repositories bound to that same transaction, with
no pool fallback. Only the outer workflow commits. Errors or cancellation before
commit roll back the old snapshot, shared rows and timestamp together. Empty snapshots clear
that installation's library. Other installations remain intact; shared rows still
referenced elsewhere retain their identities. Snapshot set/beatmap IDs may change.

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
explicit reset. Reset drops only the six application tables and their SeaORM/
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
encoding and each imported field. See [development](development.md) for commands.
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
