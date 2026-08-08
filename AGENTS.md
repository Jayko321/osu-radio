# AGENTS.md

## Repository Map

- Rust 2024 workspace; Tokio is the shared async runtime.
- `crates/radio-core`: source-neutral domain and import types only; keep I/O and persistence out.
- `crates/radio-scanner`: OS-wide osu! discovery and source readers. It returns discovered data; it does not choose what the app persists.
- `crates/radio-db`: Diesel persistence. SQLite is the default backend; PostgreSQL is an alternative feature. It depends on `radio-core` so it can persist `ImportedBeatmapSet` values directly.
- `apps/osu-radio-cli`: thin development harness for scanner/import/database behavior. Keep reusable logic in the crates.
- `tools/osu-lazer-realm-parser`: .NET helper used by `radio-scanner` to read Realm. Its stdout is NDJSON consumed by Rust; diagnostics belong on stderr.
- The intended product boundary is a backend that owns OS access, scanning, persistence, and audio serving. A future frontend should call that backend rather than read osu! files directly.

## Build And Runtime Traps

- Building anything that includes `radio-scanner` runs `dotnet build` from `crates/radio-scanner/build.rs`; install the .NET 8 SDK or set `OSU_LAZER_REALM_PARSER_PATH` to a prebuilt helper.
- `OSU_LAZER_REALM_PARSER_PATH` is both a build-time bypass and a runtime override. The bundled helper path is embedded at compile time.
- Keep the C# helper's snake_case NDJSON records synchronized with `radio-core::import_types` and `crates/radio-scanner/src/lazer/types.rs`.
- `radio-db` requires exactly one backend. Never use Cargo's `--all-features`: enabling both `sqlite` and `postgres` intentionally fails compilation.
- `Database::connect` configures and checks a connection but does not run `crates/radio-db/src/migrations`. `Database::apply_schema` and `Database::reset_schema` execute the initial migration's `up.sql`/`down.sql`, which is why both files are written idempotently (`IF NOT EXISTS` / `IF EXISTS`). No Diesel CLI/codegen config exists; keep migration SQL, `src/schema/`, and `src/model/` synchronized manually.
- `osu-radio-cli database` and `osu-radio-cli store` load `.env` and require `SQLITE_DATABASE_URL`; ordinary in-memory SQLite tests do not.

## Scanner Behavior

- Discovery lives in `crates/radio-scanner/src/discovery/` and runs in tiers: `known` probes exact per-OS paths (no directory reads, follows lazer's `storage.ini` to relocated data dirs), `shallow` sweeps the scan roots to a bounded depth, `full` sweeps them without a limit. Each tier is a superset of the ones below it.
- `find_osu_markers_with` is the sync callback core; `discover` wraps it in `spawn_blocking` and streams `OsuMarker` over a `tokio::sync::mpsc` channel. Dropping the `Discovery` stream cancels the walk.
- `DiscoveryOptions` constrains the run at the source: `roots` (replaces the OS defaults), `kind`, `limit`, `depth`. Filtering during the walk is what makes `limit` meaningful, so never filter after collecting.
- Sweeps bypass ignore files and prune selected system/temp/package directories. Pruning is conservative on purpose: osu! lives on secondary drives, under `Program Files`, and in hidden directories.
- CLI `scan --root` now constrains traversal. `scan --first` / `--limit <N>` stop early; `scan --depth known` reads no directories at all. Tests must always pass explicit `roots` so they never trigger OS-wide discovery.
- Stable installations are discovered, but `get_beatmap_sets` supports only lazer imports and returns `UnsupportedSourceError` for stable.
- Lazer audio remains a local content-addressed path in `files[].file.resolved_path`; do not copy audio into app-owned storage in scanner code.

## Persistence Behavior

- `repositories::beatmap::insert_beatmap_sets` is the only writer. It runs in one transaction, inserts a beatmap set followed by each beatmap's metadata and row, and returns an `ImportSummary`.
- `beatmap_set_limit` counts beatmap sets, and sets are never split: a stored set always brings all of its beatmaps. The remainder is reported as `skipped_beatmap_sets`. Never truncate the scanner output before handing it to the repository.
- Audio is referenced, never copied: a resolved audio path becomes a `SourceType::Local` row in `audio_sources`, deduplicated by the table's `UNIQUE (kind, location)` and by an in-run cache.
- Inserts read generated ids back with `RETURNING`, so the `sqlite` feature must keep `diesel/returning_clauses_for_sqlite_3_35` enabled.
- Nothing deduplicates beatmap sets across runs. Re-importing appends; `store --clear` is the way to start over.

## Verification

- Format Rust edits: `cargo fmt`; verify without rewriting: `cargo fmt --check`.
- Focus a crate: `cargo test -p radio-core`, `cargo test -p radio-scanner`, `cargo test -p radio-db`, or `cargo test -p osu-radio-cli`.
- Focus one test by name: `cargo test -p <package> <test-name>`.
- Cross-crate changes: `cargo test --workspace` and `cargo clippy --workspace --all-targets`.
- Check the PostgreSQL variant without a service: `cargo test -p radio-db --no-default-features --features postgres` (the connection test is ignored).
- Run that integration test only with a live database and `POSTGRES_DATABASE_URL`: `cargo test -p radio-db --no-default-features --features postgres -- --ignored`.
- Run the CLI with arguments after `--`, for example `cargo run -p osu-radio-cli -- import --marker <path-to-client.realm>`.
- Exercise persistence against the local scratch database with `cargo run -p osu-radio-cli -- store --marker <path-to-client.realm> --count 5 --clear` (`--count` is a beatmap set count).
