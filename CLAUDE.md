# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Shared agent contract

@AGENTS.md is the cross-agent contract for this repository and is also what Codex reads. That `@` reference imports it into context, so treat its repository map, build traps, scanner rules, and verification commands as binding here; this file adds only Claude-specific detail and the parts that need more explanation than a bullet.

When a repo-wide rule changes, edit `AGENTS.md` so every agent picks it up, and keep this file for Claude-only workflow notes.

## Commands

Rust 2024 workspace on Tokio. Building any member that pulls in `radio-scanner` runs `dotnet build` from `crates/radio-scanner/build.rs`, so the .NET 8 SDK must be installed or `OSU_LAZER_REALM_PARSER_PATH` must point at a prebuilt helper.

Run from the repository root:

```sh
cargo build --workspace
cargo fmt --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

Focused commands:

```sh
# One crate
cargo test -p radio-scanner

# One test by name
cargo test -p radio-scanner parses_lazer_beatmap_set_json_into_core_type

# Compile/test radio-db with PostgreSQL instead of default SQLite
cargo test -p radio-db --no-default-features --features postgres

# Run the ignored PostgreSQL connection test against a live database
cargo test -p radio-db --no-default-features --features postgres -- --ignored

# Development CLI (arguments follow `--`)
cargo run -p osu-radio-cli -- scan                    # tiered: known paths, then shallow, then full sweep
cargo run -p osu-radio-cli -- scan --depth known      # probe known install paths only, no traversal
cargo run -p osu-radio-cli -- scan --first            # stop at the first installation found
cargo run -p osu-radio-cli -- scan --root <path>      # constrain traversal to one directory
cargo run -p osu-radio-cli -- import --marker <path-to-client.realm>
cargo run -p osu-radio-cli -- database

# Persist scanner output into the database configured in .env
cargo run -p osu-radio-cli -- store --marker <path-to-client.realm>
cargo run -p osu-radio-cli -- store --count 50            # store at most 50 beatmap sets
cargo run -p osu-radio-cli -- store --clear               # drop and recreate the tables first

# HTTP backend over the database configured in .env
cargo run -p osu-radio-server                             # binds OSU_RADIO_SERVER_ADDRESS, default 127.0.0.1:3000
cargo run -p osu-radio-server --no-default-features       # same, without the `docs` feature and its OpenAPI code
# http://127.0.0.1:3000/docs                              # Scalar API reference; the URL is printed at startup
curl http://127.0.0.1:3000/api/beatmap-sets               # every stored beatmap set with its audio sources
curl http://127.0.0.1:3000/api/user-data                  # the settings row with its registered osu! folders

# Desktop GUI (vizia). Build the server first: the GUI spawns it, it does not build it.
cargo build -p osu-radio-server
cargo run -p osu-radio-gui-vizia

# Register, edit, and remove osu! folders
curl -X POST http://127.0.0.1:3000/api/user-data/osu-folders \
  -H 'content-type: application/json' -d '{"path":"D:/osu","label":"Desktop"}'
curl -X PATCH http://127.0.0.1:3000/api/user-data/osu-folders/1 \
  -H 'content-type: application/json' -d '{"enabled":false}'
curl -X DELETE http://127.0.0.1:3000/api/user-data/osu-folders/1

# Inspect the Realm helper directly
dotnet run --project tools/osu-lazer-realm-parser -- --schema <path-to-client.realm>
dotnet run --project tools/osu-lazer-realm-parser -- <path-to-client.realm>
```

Development happens on Windows with PowerShell 5.1 as the primary shell: `&&` and `||` are parser errors there, so chain with `;` / `if ($?) { ... }`, or use the Bash tool for POSIX syntax.

`radio-db` requires exactly one backend feature; `--all-features` is a deliberate `compile_error!`. The PostgreSQL test reads `POSTGRES_DATABASE_URL`, the CLI `database` command reads `SQLITE_DATABASE_URL`, and both come from `.env` (untracked, already present locally).

## Architecture

The import path is a one-way pipeline across four boundaries, and understanding it requires reading all of them together:

1. **Discovery** (`find_osu_markers` in `crates/radio-scanner/src/helpers.rs`) walks every mounted root/drive for `client.realm` and `osu!.db`, pruning selected system/temp directories, and yields `radio_core::OsuMarker { kind, marker_path, root_path }`.
2. **Process boundary** — `radio-scanner/build.rs` compiles `tools/osu-lazer-realm-parser` and bakes the resulting path into the binary via `OSU_LAZER_REALM_PARSER_BUILT_PATH`. At runtime the scanner spawns that C# helper with a Realm path and reads one beatmap-set JSON object per stdout line. Helper diagnostics must stay on stderr or they corrupt the NDJSON stream.
3. **Type boundary** — the helper emits snake_case records that must stay in lockstep with `radio-scanner/src/lazer/types.rs` (wire shape) and `radio-core/src/import_types.rs` (domain shape). Changing a field means editing `Program.cs` and both Rust modules together.
4. **Domain output** — `radio_scanner::get_beatmap_sets(marker)` dispatches on `OsuKind`, returning `Vec<ImportedBeatmapSet>`. Lazer audio stays at its content-addressed source path (`<lazer-root>/files/<c>/<cc>/<hash>`, resolved by the helper into `files[].file.resolved_path`); scanner code never copies audio into app storage.

`apps/osu-radio-server` is the first piece of that backend, layered `routes` -> `services` -> `radio-db` repositories. A route deserializes, calls one service method, and converts the result into per-route DTOs; the service owns the use case and is the only thing that locks `AppState`'s single `Database`, held behind a `tokio::sync::Mutex`, so concurrent requests queue on one connection until a diesel-async pool replaces it.

Those per-route DTOs are also what the OpenAPI spec is built from, which is the one place the layering costs something. The default `docs` feature makes `utoipa`, `utoipa-axum`, and `utoipa-scalar` compile in and serves a Scalar reference at `/docs`; turning it off strips all of it. The cost is that `routes::router` has to exist twice — `OpenApiRouter` collects paths through its own `routes!` macro, which a plain `axum::Router` has no equivalent for — so the route table is written once per feature state and only one of them ever compiles. Nothing will tell you the two disagree; the stripped build simply serves fewer routes. Check both.

There are two use cases now, and the second one closes the loop back to discovery. `UserDataService` stores which osu! folders the user owns, so a rescan no longer has to re-walk the filesystem to find them: a client `POST`s a path, the service re-runs discovery at `DiscoveryDepth::Known` scoped to that single root to prove there is an installation there and to derive its `OsuKind`, and the resulting `OsuMarker` is what gets stored. That is why the server depends on `radio-scanner` — and why building it now also runs `dotnet build`, even though validation never spawns the Realm helper. Everything imported from a folder carries its `installation_id`, so removing the folder cascades its beatmap sets away.

The desktop GUI sits on the other side of that HTTP boundary and is deliberately split in two. `crates/osu-radio-client` is the framework-agnostic half: it supervises the server process, wraps every endpoint, owns its own DTOs, and carries the small pieces of UI state any frontend needs (`Loading<T>`, `describe`). `apps/osu-radio-gui-vizia` is the vizia half and holds nothing but signals, views, and events. The split is the point: a second frontend should add an app next to it and reimplement views alone, so a GUI toolkit must never reach the client crate.

Its two screens — a songs library and a settings pane, both over a shared now-playing stage — are built to the Figma design, and they run on placeholder content in `src/sample.rs` rather than on anything the server returns. That is deliberate and temporary: the shape of the UI was worth settling before the read path behind it grew. Wiring it up means replacing `sample::TRACKS` with beatmap sets fetched through `ApiClient`, which is what `views.rs` is written to make easy — every view takes its text from a struct field, not from a literal. The look lives entirely in `theme.css`, so layout and colour changes need no recompile in a debug build.

What makes the GUI self-contained is that it owns its server rather than expecting one. `EmbeddedServer` spawns the `osu-radio-server` binary with `OSU_RADIO_SERVER_ADDRESS=127.0.0.1:0`, so the OS picks a free port and an embedded instance never fights the one you may already have on 3000, and then it learns which port it got by reading the child's own `listening on http://...` startup line back off its stdout. That turns a log line into an interface: reword it in the server and startup stops working. It finds the binary next to the running executable, which is why `cargo run` works with no configuration and why the server has to be built first. Both pipes are drained onto the parent's output under a `[server]` prefix, because an unread pipe eventually blocks the child.

Shutdown is the part worth knowing. The window close event kills the server, and `kill_on_drop` plus a `Drop` impl cover the ordinary exit paths, which is also why `main` owns the tokio runtime instead of the model: the runtime has to outlive the vizia context so the child can still be reaped as the model drops. None of that survives a hard kill of the GUI, which runs no destructor and leaves the server behind; a Windows job object is the fix if that becomes a real problem. Note also that vizia 0.4 is the signal API, so any example built around `#[derive(Lens)]` targets a different version, and a few modifiers in the project README exist only on its main branch.

`radio-core` is the source-neutral centre — markers plus the nested `ImportedBeatmapSet`/beatmap/metadata/file types — and must stay free of I/O and persistence. `radio-db` is async Diesel: `DatabaseConnection` is a `SyncConnectionWrapper<SqliteConnection>` under the default `sqlite` feature and `AsyncPgConnection` under `postgres`, with `Database::connect` enabling `PRAGMA foreign_keys` (SQLite) or setting `application_name` (PostgreSQL). `osu-radio-cli` is a thin harness: reusable scanning, domain, and persistence logic belongs in the crates, because the intended product is a backend that owns OS access, scanning, persistence, and audio serving, with a frontend that never touches osu! files directly.

## Current state of the work in progress

The scanner path is complete, and a first persistence path now exists end to end. Before extending either, know these seams:

- `crates/radio-db/src/repositories/beatmap.rs` writes `ImportedBeatmapSet` values through `Database::import_beatmap_sets`. The CLI `import` command still only prints scanner output; `store` is the command that persists it.
- Reading back goes through `all_beatmap_sets_with_audio_sources`, which returns each set paired with the audio sources its beatmaps reference. Beatmaps and metadata are still not exposed; adding them means extending that repository function, not the handler.
- Folders are registered and stamped with `last_scanned_at`, but nothing rescans them yet. The pieces a rescan endpoint needs are all in place: read the folder, rebuild its `OsuMarker` from the stored `kind`/`marker_path`, and import against its id. Deciding what to do with the previous import for that folder is the open question — nothing deduplicates, so a naive rescan appends.
- `store` and the server startup are the consumers of the schema helpers: both call `Database::apply_schema` (`reset_schema` under `store --clear`) before writing, because `Database::connect` still does not run `crates/radio-db/src/migrations/`. There is no Diesel CLI/codegen config, so migration SQL, `src/schema/` tables, and `src/model/` structs are hand-synchronized — change all three together.
- Because `apply_schema` is only idempotent per table, adding a column to an existing table does nothing to a database created earlier. Adding `beatmap_sets.installation_id` is exactly that case: a pre-existing `osu-radio.db` keeps serving until a query touches the column and fails with `no such column: beatmap_sets.installation_id`. Recreate it with `store --clear` after any column change.
- The settings path is `user_data` (one row, id 1) -> `osu_installations` (zero or more). Zero is a real state, not an error — a user on a hosted beatmap source registers no folder. `repositories/user_data.rs` owns all of it; `RegisteredInstallation` distinguishes a fresh registration from an existing one so the CLI can reuse a folder while the API answers `409`.
- The schema uses bare `INTEGER PRIMARY KEY`, which auto-assigns on SQLite but not on PostgreSQL. Inserts are untested against a live PostgreSQL database; that variant is only compile-checked.
- `OsuKind::Stable` is discovered but `get_beatmap_sets` returns `UnsupportedSourceError` for it; lazer is the only implemented reader.
- Discovery is tiered and streaming (`crates/radio-scanner/src/discovery/`), so `scan` is cheap by default but `--depth full` (the default ceiling) still ends in an OS-wide walk of every drive. Use `--depth known`, `--first`, or `--root <PATH>` when you just need an install, and `import --marker <path-to-client.realm>` to bypass discovery entirely. Discovery tests must pass explicit `DiscoveryOptions::roots`.
- `osu-radio.db` at the repository root is a local scratch database, not a fixture.

## Repo tooling notes

- `.agents/skills/cr/` is a Codex-format skill (Claude Code loads skills from `.claude/skills/`, so it is not invocable as `/cr` here). Its `references/rust-review-checklist.md` is nonetheless this repo's agreed review standard — panics, resource/task leaks, blocking work on async paths, error-variant specificity, osu!-data safety, path-privacy — so apply it when reviewing Rust changes, and do not propose edits under vendored scanner code unless asked.
- `docs/` is gitignored. Review reports and design notes there are local-only history, useful to read but not part of the tracked repository.
- Serena is configured for this project (`.serena/project.yml`, Rust LSP). Prefer its symbol tools over grep for "where is this defined / who calls this" questions in the Rust crates.
- Consult Context7 rather than memory for Diesel, diesel-async, clap, Tokio, and Realm .NET API details; the versions here (diesel 2, diesel-async 0.7, Tokio 1.52) move faster than training data. Delegate the lookup to the `context7-docs` subagent, which runs on Sonnet and keeps doc retrieval off the main model; fall back to the `/find-docs` skill or the Context7 MCP tools directly when that agent is not configured.
