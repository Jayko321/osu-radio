# AGENTS.md

## Repository Map

- Rust 2024 workspace; Tokio is the shared async runtime.
- `crates/radio-core`: source-neutral domain and import types only; keep I/O and persistence out.
- `crates/radio-scanner`: OS-wide osu! discovery and source readers. It returns discovered data; it does not choose what the app persists.
- `crates/radio-db`: Diesel persistence. SQLite is the default backend; PostgreSQL is an alternative feature. It depends on `radio-core` so it can persist `ImportedBeatmapSet` values directly.
- `apps/osu-radio-cli`: thin development harness for scanner/import/database behavior. Keep reusable logic in the crates.
- `apps/osu-radio-server`: axum HTTP backend over `radio-db`, layered `routes` -> `services` -> `radio-db` repositories. Routes own HTTP shape, services own use cases, queries stay in the repositories.
- `crates/osu-radio-client`: framework-agnostic frontend library. It supervises an embedded `osu-radio-server` and wraps its HTTP API in DTOs of its own. No GUI toolkit may appear in its dependencies.
- `apps/osu-radio-gui-vizia`: desktop GUI built on vizia 0.4. It is a shell over `osu-radio-client`: signals, views, and events only.
- `tools/osu-lazer-realm-parser`: .NET helper used by `radio-scanner` to read Realm. Its stdout is NDJSON consumed by Rust; diagnostics belong on stderr.
- The intended product boundary is a backend that owns OS access, scanning, persistence, and audio serving. A future frontend should call that backend rather than read osu! files directly.

## Build And Runtime Traps

- Building anything that includes `radio-scanner` runs `dotnet build` from `crates/radio-scanner/build.rs`; install the .NET 8 SDK or set `OSU_LAZER_REALM_PARSER_PATH` to a prebuilt helper. `osu-radio-server` now depends on `radio-scanner` for folder validation, so building the server pays that cost too even though validation never spawns the helper.
- `OSU_LAZER_REALM_PARSER_PATH` is both a build-time bypass and a runtime override. The bundled helper path is embedded at compile time.
- Keep the C# helper's snake_case NDJSON records synchronized with `radio-core::import_types` and `crates/radio-scanner/src/lazer/types.rs`.
- `radio-db` requires exactly one backend. Never use Cargo's `--all-features`: enabling both `sqlite` and `postgres` intentionally fails compilation.
- `Database::connect` configures and checks a connection but does not run `crates/radio-db/src/migrations`. `Database::apply_schema` and `Database::reset_schema` execute the initial migration's `up.sql`/`down.sql`, which is why both files are written idempotently (`IF NOT EXISTS` / `IF EXISTS`). No Diesel CLI/codegen config exists; keep migration SQL, `src/schema/`, and `src/model/` synchronized manually.
- That idempotence only covers whole tables. `IF NOT EXISTS` cannot add a column to a table that already exists, so any change to an existing table's columns silently skips on a database created before it, and queries then fail at runtime with `no such column`. Recreate the local scratch database (`store --clear`, or delete `osu-radio.db`) after such a change until real migrations exist.
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

- `repositories::beatmap::insert_beatmap_sets` is the only beatmap writer. It runs in one transaction, inserts a beatmap set followed by each beatmap's metadata and row, and returns an `ImportSummary`. Its `installation_id` argument stamps every set it writes with the osu! folder they came from.
- `user_data` is a singleton settings row (`USER_DATA_ID` = 1) created by the migration and by `repositories::user_data::ensure_user_data`. `osu_installations` hangs off it and may legitimately be empty: a user who plays from a hosted beatmap source registers no local folder at all, so never assume at least one installation exists.
- `beatmap_sets.installation_id` is nullable and `ON DELETE CASCADE`. Deleting a registered folder deletes every beatmap set imported from it; a `NULL` marks a set that belongs to no local installation. Nothing else may delete `osu_installations` rows casually.
- `repositories::user_data::register_installation` is the only installation writer. It is keyed on `marker_path` (`UNIQUE`), runs in one transaction, and returns `RegisteredInstallation::Created` or `::AlreadyRegistered` instead of failing on a duplicate, so the CLI can reuse a folder while the API answers `409`.
- `last_scanned_at` is RFC 3339 text written by `mark_scanned` after an import, never by a client. It is the only timestamp in the schema; `chrono` is in `radio-db` for that alone.
- `beatmap_set_limit` counts beatmap sets, and sets are never split: a stored set always brings all of its beatmaps. The remainder is reported as `skipped_beatmap_sets`. Never truncate the scanner output before handing it to the repository.
- Audio is referenced, never copied: a resolved audio path becomes a `SourceType::Local` row in `audio_sources`, deduplicated by the table's `UNIQUE (kind, location)` and by an in-run cache.
- Inserts read generated ids back with `RETURNING`, so the `sqlite` feature must keep `diesel/returning_clauses_for_sqlite_3_35` enabled.
- Nothing deduplicates beatmap sets across runs. Re-importing appends; `store --clear` is the way to start over.
- `repositories::beatmap::all_beatmap_sets_with_audio_sources` is the read path behind `Database::beatmap_sets_with_audio_sources`. It runs two queries — every set, then `beatmaps -> beatmap_metadata -> audio_sources` — and groups them in Rust, deduplicating sources shared by a set's difficulties. Sets with no resolvable audio still come back, with an empty list.

## Server Behavior

- `osu-radio-server` loads `.env`, requires `SQLITE_DATABASE_URL`, and binds `OSU_RADIO_SERVER_ADDRESS` (default `127.0.0.1:3000`).
- `AppState` holds one `Database` behind a `tokio::sync::Mutex`, so requests serialize on a single connection. A diesel-async pool is the upgrade when concurrency matters.
- Startup runs the idempotent `Database::apply_schema` so a fresh database serves instead of erroring; the server never resets the schema.
- Handlers return `ApiError`, which logs the `anyhow` chain to stderr and answers `500` with a fixed message. Never put database errors in a response body.
- `src/services/` holds the use cases. A handler builds a service from `AppState`, awaits one method, and maps the result into DTOs; it never touches `Database` itself. Anything beyond that shape — filtering, pagination, joining more tables — goes in the service or the repository.
- HTTP responses use per-route DTOs (`BeatmapSetResponse`, `AudioSourceResponse`), not `radio-db` models, so the wire shape stays independent of the schema.
- `GET /api/beatmap-sets` currently exposes each audio source's local `location`. That is a real filesystem path leaving the backend; replace it with a streaming URL once the server serves audio. The osu! folder routes also return `root_path`/`marker_path`, but those are paths the client itself submitted, so echoing them back leaks nothing new.
- `UserDataService` owns the settings use case. `/api/user-data` returns the singleton row with its folders; `/api/user-data/osu-folders` is the collection (`GET`, `POST`) and `/api/user-data/osu-folders/{id}` the item (`PATCH`, `DELETE`). Axum 0.8 path params use `{id}`, not `:id`.
- Registration never trusts the client's claim about a folder. `resolve_osu_folder` runs discovery at `DiscoveryDepth::Known` against just that root and derives `OsuKind` itself. A relative path, a folder with no marker, and a folder holding two installations are each a `400` with their own `RegisterFolderError` variant; only `Failed` becomes a `500`.
- `POST` answers `201` with the new folder or `409` with the already-registered one, both as the same `OsuFolderResponse` body. `PATCH` edits `label` and `enabled` only: an absent field is unchanged and an explicit `null` label clears it, which needs `deserialize_present_field` because plain `Option<Option<T>>` collapses both cases. Changing a folder's path means `DELETE` then `POST`, since a different marker is a different installation.
- `ApiError` now carries a status and a client-facing message. Use `bad_request`/`not_found` for messages a client should read; keep letting `?` convert `anyhow` errors, which still log the chain and answer `500` with the fixed message.
- Test fixtures live in `src/test_support.rs` (`#[cfg(test)]`), shared by route and service tests, so neither reaches into the other's test module.
- The `docs` feature (on by default) mounts a Scalar API reference at `/docs` and prints its URL at startup. It is the only thing gating `utoipa`, `utoipa-axum`, and `utoipa-scalar`, which are optional dependencies, so `--no-default-features` produces a binary with no OpenAPI code in it at all. `src/docs.rs` holds `ApiDoc` and `SCALAR_PATH`.
- Because of that gate, `routes::router` exists twice: a plain `axum::Router` under `#[cfg(not(feature = "docs"))]` and an `OpenApiRouter` under `#[cfg(feature = "docs")]`. Only one compiles at a time, so nothing catches a route added to one and not the other — edit both together, and check the stripped build as well as the default one.
- Handlers carry `#[cfg_attr(feature = "docs", utoipa::path(...))]` and DTOs `#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]`, never bare `utoipa` attributes, or the stripped build stops compiling. Fields utoipa cannot infer need `schema(value_type = ...)`: `&'static str`, `PathBuf`, and the `Option<Option<String>>` label all do.

## Frontend Behavior

- `osu-radio-client` is the only place a frontend talks to the backend. A second GUI should depend on it and reimplement views alone, so anything reusable (server supervision, HTTP calls, DTOs, `Loading<T>`, `describe`) belongs there rather than in an app.
- `src/view_models/` holds the types a UI renders rather than the ones the wire carries. `Track` is the first: owned `title`/`artist`/`duration`, with `duration_label` and `meta` doing the formatting every frontend would otherwise repeat. Keep it toolkit-free and keep presentation out of it — a CSS class or an asset name is the app's business, which is why the placeholder cover art is chosen by list position in `assets::cover_image`/`assets::tint` rather than sitting on the track.
- `EmbeddedServer::start` spawns `osu-radio-server` as a child with `OSU_RADIO_SERVER_ADDRESS=127.0.0.1:0`, so the OS assigns a free port and an embedded server never collides with one already running on 3000. The bound address is read back by parsing the child's `listening on http://...` startup line, which makes that line load-bearing output rather than a nicety: changing its wording breaks startup.
- Both child pipes are drained onto the parent's stdout/stderr with a `[server]` prefix. Never leave a pipe unread; a full pipe buffer blocks the server.
- The binary is resolved as `ServerOptions::binary`, then `OSU_RADIO_SERVER_BIN`, then a sibling of the running executable, then bare `osu-radio-server` on `PATH`. The sibling rule is what makes `cargo run -p osu-radio-gui-vizia` work, since cargo puts both binaries in `target/debug`. Build the server first; the GUI does not build it.
- The child inherits the working directory, and the server loads `.env` from it, so a GUI started outside the repository needs `SQLITE_DATABASE_URL` in its environment or a `working_directory` pointing at the repository.
- Closing the window kills the server: the model handles `WindowEvent::WindowClose`, and `EmbeddedServer` also holds `kill_on_drop(true)` plus a `Drop` that calls `start_kill`. A hard kill of the GUI (Task Manager, a crash) runs no destructor and does orphan the child; a Windows job object would be the fix if that matters.
- The GUI runs one `tokio` runtime owned by `main`, not by the model, so it outlives the vizia context and can still reap the child while the model drops. Requests run on that runtime and report back through `ContextProxy::emit`; signals are UI-thread only, so a background task must emit an event rather than write a signal.
- vizia 0.4 is the signal API, not the older lens API: state is `Signal<T>` built with `Signal::new`, read with `.get()`, written with `.set`/`.update`, and projected with `.map`. Examples written against `#[derive(Lens)]` are for a different version. `.alignment()` exists but some modifiers shown in the repository README are main-branch only; check the vendored source before using an unfamiliar one.
- The window is created with `.decorations(false)`, so `views::top_bar` *is* the title bar. Empty space in it drags the window (`on_mouse_down` -> `WindowEvent::DragWindow`, guarded by `cx.hovered() == cx.current()` because `MouseDown` bubbles from children), and `window_controls` replaces the system minimize/maximize/close buttons. `UiState::maximized` is a tracked bool, not the real window state, which vizia does not expose: an OS-side maximize (aero snap, or the caption double-click Windows performs inside `drag_window`) desyncs it, and the first press of the maximize button afterwards only re-syncs the icon.
- The GUI's look lives in `apps/osu-radio-gui-vizia/styles/`, one sheet per component, each loaded with its own `include_style!` (hot-reloaded from disk in debug, embedded in release). `src/views/` builds the tree and assigns classes; keep styling in the stylesheet rather than in modifier calls. Bundled assets sit in `assets/` and are registered by `src/assets.rs`: Nunito via `add_font_mem`, cover art via `load_image` under the names the CSS references as `url("cover-…")`, and icons as `include_bytes!` SVGs handed to `Svg::new`.
- `src/views/` is one module per screen area: `mod.rs` holds `shell` and `styles`, `background.rs` and `top_bar.rs` hold the frame, and `songs/`, `settings/`, and `player/` each hold their pane in `mod.rs` with its parts in leaves. Shared pieces (`icon`, `icon_button`, `gap`, `hspacer`, `vspacer`, `search_row`, `chip_row`, `sidebar`) live in `views/components/` and are re-exported flat from its `mod.rs`.
- Every view module exposes `pub(crate) fn style() -> CSS` returning its own sheet, and `views::styles` loads them with `base.css` first, since same-specificity rules resolve last-wins. Adding a component means adding a `.rs`, a `.css`, and a line in that list. A wrong stylesheet path is not a build error in debug — `include_style!` reads the file at runtime there — so check it with `cargo check -p osu-radio-gui-vizia --release`, whose `include_str!` form resolves every path at compile time.
- Components are free builder functions taking `cx` plus the data they render, not `View` impls. `UiState` is `Copy` so it passes by value into every one of them; that must stay true.
- The song list, the settings fields, and the now-playing panel are placeholder content in `src/sample.rs`, not data from the backend. Its tracks are real `osu_radio_client::Track` values, so replacing them means changing where the `Vec` comes from, not what the views take. The GUI still starts and supervises the embedded server and still reports a failed start, but it renders none of the server's beatmap sets yet.
- vizia CSS traps, all found the hard way and all silent when violated:
  - `on_press` only fires when the pressed view *is* the event target, so a hoverable child swallows a clickable parent's callback. Give the contents of anything clickable `pointer-events: none`.
  - Hit testing walks the whole tree in draw order and keeps the *last* view whose bounds contain the cursor, so a view drawn early can be stolen from by any later one that overlaps it — even one painted underneath. `.glow-warm` sits at `top: -10%` inside the body and reaches up over the title bar, which silently killed every press right of it until the backdrop and both glows were given `pointer-events: none`. Decorative layers must always be inert.
  - A child's `top` is ignored for spacing inside a stack. Use the parent's `gap`, or an explicit spacer element when siblings need different gaps.
  - `scroll-content` is an element name, not a class: write `.track-list scroll-content`, never `.scroll-content`.
  - `Textbox::placeholder` panics with a subtract overflow in vizia 0.4.0's accessibility pass (`textbox.rs:945`) whenever the box is empty. When the text is empty the guard `line.start_index >= text_len && text_len > 0` stops skipping lines, so the placeholder's line metrics reach `glyph_end - line.start_index` with `glyph_end` clamped to `0`. An empty `Textbox` with no placeholder is fine, so draw the placeholder as a sibling `Label` in a `ZStack` and toggle it with `.display(query.map(|q| q.is_empty()))` — see `search_row` in `src/views/components/search_row.rs`. Do not call `.placeholder(...)` on a textbox that can be empty.
  - Non-standard property names: `corner-radius` not `border-radius`, `size`/`space`/`gap`/`alignment`/`layout-type`, and `1s` for stretch.

## Verification

- Format Rust edits: `cargo fmt`; verify without rewriting: `cargo fmt --check`.
- Focus a crate: `cargo test -p radio-core`, `cargo test -p radio-scanner`, `cargo test -p radio-db`, `cargo test -p osu-radio-cli`, or `cargo test -p osu-radio-server`.
- Focus one test by name: `cargo test -p <package> <test-name>`.
- Cross-crate changes: `cargo test --workspace` and `cargo clippy --workspace --all-targets`.
- Check the PostgreSQL variant without a service: `cargo test -p radio-db --no-default-features --features postgres` (the connection test is ignored).
- Run that integration test only with a live database and `POSTGRES_DATABASE_URL`: `cargo test -p radio-db --no-default-features --features postgres -- --ignored`.
- Run the CLI with arguments after `--`, for example `cargo run -p osu-radio-cli -- import --marker <path-to-client.realm>`.
- Exercise persistence against the local scratch database with `cargo run -p osu-radio-cli -- store --marker <path-to-client.realm> --count 5 --clear` (`--count` is a beatmap set count). `store` registers the marker's osu! folder, imports against its id, and stamps `last_scanned_at`; `--clear` now also drops the registered folders.
- Serve the stored data with `cargo run -p osu-radio-server`, then `curl http://127.0.0.1:3000/api/beatmap-sets`.
- Build the server before the GUI, which spawns it rather than building it: `cargo build -p osu-radio-server`, then `cargo run -p osu-radio-gui-vizia`.
- Exercise the supervisor without a window: `cargo test -p osu-radio-client --test embedded_server -- --ignored`. It is ignored by default because it needs the built server binary and a database.
- Browse the API at `http://127.0.0.1:3000/docs`; the server prints that URL on startup.
- Any change to the server's routes or DTOs must be checked against both feature states, because each one compiles a different `router`: `cargo clippy -p osu-radio-server --all-targets` and `cargo clippy -p osu-radio-server --no-default-features --all-targets`.
- Exercise the settings routes against a running server:
  - `curl http://127.0.0.1:3000/api/user-data`
  - `curl -X POST http://127.0.0.1:3000/api/user-data/osu-folders -H 'content-type: application/json' -d '{"path":"<absolute-path-to-osu-folder>","label":"Desktop"}'`
  - `curl -X PATCH http://127.0.0.1:3000/api/user-data/osu-folders/1 -H 'content-type: application/json' -d '{"enabled":false}'`
  - `curl -X DELETE http://127.0.0.1:3000/api/user-data/osu-folders/1`
- `cargo test -p radio-db --no-default-features --features postgres` compiles but its repository tests fail: they open `:memory:`, which only SQLite understands. That predates the user-data work. Treat the postgres command as a compile check (`cargo clippy -p radio-db --no-default-features --features postgres --all-targets`) until those tests are gated behind the `sqlite` feature.
