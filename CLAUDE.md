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

`radio-core` is the source-neutral centre — markers plus the nested `ImportedBeatmapSet`/beatmap/metadata/file types — and must stay free of I/O and persistence. `radio-db` is async Diesel: `DatabaseConnection` is a `SyncConnectionWrapper<SqliteConnection>` under the default `sqlite` feature and `AsyncPgConnection` under `postgres`, with `Database::connect` enabling `PRAGMA foreign_keys` (SQLite) or setting `application_name` (PostgreSQL). `osu-radio-cli` is a thin harness: reusable scanning, domain, and persistence logic belongs in the crates, because the intended product is a backend that owns OS access, scanning, persistence, and audio serving, with a frontend that never touches osu! files directly.

## Current state of the work in progress

The scanner path is complete, and a first persistence path now exists end to end. Before extending either, know these seams:

- `crates/radio-db/src/repositories/beatmap.rs` writes `ImportedBeatmapSet` values through `Database::import_beatmap_sets`. The CLI `import` command still only prints scanner output; `store` is the command that persists it.
- `store` is the only consumer of the schema helpers: it calls `Database::apply_schema` (or `reset_schema` under `--clear`) before writing, because `Database::connect` still does not run `crates/radio-db/src/migrations/`. There is no Diesel CLI/codegen config, so migration SQL, `src/schema/` tables, and `src/model/` structs are hand-synchronized — change all three together.
- The schema uses bare `INTEGER PRIMARY KEY`, which auto-assigns on SQLite but not on PostgreSQL. Inserts are untested against a live PostgreSQL database; that variant is only compile-checked.
- `OsuKind::Stable` is discovered but `get_beatmap_sets` returns `UnsupportedSourceError` for it; lazer is the only implemented reader.
- Discovery is tiered and streaming (`crates/radio-scanner/src/discovery/`), so `scan` is cheap by default but `--depth full` (the default ceiling) still ends in an OS-wide walk of every drive. Use `--depth known`, `--first`, or `--root <PATH>` when you just need an install, and `import --marker <path-to-client.realm>` to bypass discovery entirely. Discovery tests must pass explicit `DiscoveryOptions::roots`.
- `osu-radio.db` at the repository root is a local scratch database, not a fixture.

## Repo tooling notes

- `.agents/skills/cr/` is a Codex-format skill (Claude Code loads skills from `.claude/skills/`, so it is not invocable as `/cr` here). Its `references/rust-review-checklist.md` is nonetheless this repo's agreed review standard — panics, resource/task leaks, blocking work on async paths, error-variant specificity, osu!-data safety, path-privacy — so apply it when reviewing Rust changes, and do not propose edits under vendored scanner code unless asked.
- `docs/` is gitignored. Review reports and design notes there are local-only history, useful to read but not part of the tracked repository.
- Serena is configured for this project (`.serena/project.yml`, Rust LSP). Prefer its symbol tools over grep for "where is this defined / who calls this" questions in the Rust crates.
- Consult Context7 (`/find-docs`) rather than memory for Diesel, diesel-async, clap, Tokio, and Realm .NET API details; the versions here (diesel 2, diesel-async 0.7, Tokio 1.52) move faster than training data.
