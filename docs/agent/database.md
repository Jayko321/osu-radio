# Database and repository contracts

[radio-db](../../crates/radio-db/src/lib.rs) uses SeaORM 2.0.2 behind a cloneable
`Database` pool handle. SQLite is the default; exactly one of `sqlite` and
`postgres` must be enabled. Entities, active models, pools and transactions are
private. Public repository methods accept domain inputs and return plain Rust
[models](../../crates/radio-db/src/model/mod.rs) with `anyhow::Result`.

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
and audio rows. Join/cleanup foreign keys are indexed. Persistence does not copy
or delete files, including sources marked `copied`.

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
locations are separate beatmap references and never enter the hash. Insertions
compute the hash themselves, reuse an existing row without changing it, and
reject a matching key whose stored content differs.

## Repository entry points

Access repositories through `Database`; no domain operations live on the handle.

| Accessor | Operations |
| --- | --- |
| `user_data()` | `get()` reads the migrated singleton. |
| `osu_installations()` | `all`, `get`, `register`, `update`, `delete`, `replace_snapshot`. |
| `beatmap_sets()` | `get`, `for_installation`, `all_with_audio_sources`. Aggregate read uses one query, retaining sets without audio and returning distinct sources in ID order. |
| `beatmaps()` | `get`, `for_set`. |
| `beatmap_metadata()` | `get(hash)`, `get_or_insert(imported)`; also exposes the pure hash helper. |
| `audio_sources()` | `get(id)`, `find(source)`, `get_or_insert(source)`. |

[Registration](../../crates/radio-db/src/repositories/osu_installation.rs) returns
`Created` or `AlreadyRegistered`; duplicate marker paths retain the stored row.
Updates change only supplied fields. `label: None` means omitted,
`label: Some(None)` clears it, and an empty string remains an empty string.

## Snapshot replacement

Read the entire scanner result before calling `replace_snapshot`. It requires an
existing installation and matching source kinds. In one transaction it locks the
singleton with a write statement, deletes the old installation sets, inserts all
sets and beatmaps through repositories, cleans up unreferenced shared rows, and
updates `last_scanned_at`. Any error rolls everything back. Empty snapshots clear
that installation's library. Other installations remain intact; shared rows still
referenced elsewhere retain their identities. Snapshot set/beatmap IDs may change.

Deletion takes the same lock and performs cascade deletion plus cleanup in one
transaction. The lock coordinates processes and independent pools; it does not
rely on a server mutex. This serializes snapshot writers globally. Reads remain
available under SQLite WAL and PostgreSQL MVCC. Finer locking would need a new
shared-record cleanup strategy if write throughput becomes a bottleneck.

## Connections, migration and explicit reset

`connect` configures the pool without changing schema. Call `migrate` on ordinary
startup, or explicitly call `reset` when discarding application data is intended.
`check_connection` pings the pool. File SQLite uses WAL, foreign keys and a
five-second busy timeout on every connection. Memory SQLite uses one pooled
connection. Existing plain SQLite paths and `:memory:` remain supported.

Application tables without applied SeaORM migration history produce an actionable
legacy-database error. There is no legacy data migration: choose a new database or
explicit reset. Reset drops only the six application tables and their SeaORM/
Diesel migration history, in child-first order, then recreates the schema in the
same transaction. Unrelated tables are preserved; reset does not use a broad
schema refresh or drop external objects.

The server stores the cloneable handle directly. CLI/server select
`SQLITE_DATABASE_URL` for SQLite or `POSTGRES_DATABASE_URL` for PostgreSQL.
`store` reads a complete scanner snapshot, registers the marker and replaces its
library; `--count` and skipped counts are removed. `--clear` explicitly resets all
application data before registration. Output reports inserted sets/beatmaps and
the distinct audio sources referenced by the snapshot, including reused sources.

## Verification limits

[Repository contracts](../../crates/radio-db/src/tests.rs) run the same workflows
against temporary SQLite files and an explicitly supplied disposable PostgreSQL
database. They cover legacy detection, reset preservation, registration races,
partial updates, generated keys, full/empty replacement, shared cleanup,
restrictive FKs, forced mid-import rollback and independent-pool concurrency.
[Hash tests](../../crates/radio-db/src/repositories/beatmap_metadata.rs) pin the
encoding and test each imported field. See [development](development.md) for
commands. These checks do not exercise a real Realm library, GUI interaction,
audio playback or Windows deployment.
