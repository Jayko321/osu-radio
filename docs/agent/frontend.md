# Frontend and desktop GUI

Read [the architecture and capability map](index.md) first. This guide describes the current source; it does not declare Vizia or child-process hosting permanent platform requirements. The confirmed direction is desktop music playback on Linux and Windows, eventually including hosted sources; macOS is unverified and mobile platforms are future goals. Database-backed API contracts and persistence workflows are described in [backend](backend.md) and [database](database.md).

For a GUI task, use the [osu-radio-gui procedure](../../.agents/skills/osu-radio-gui/SKILL.md). Build commands and their limitations belong in [development](development.md); server-side protocol ownership belongs in [backend](backend.md).

## Ownership and change locations

| Concern | Current owner | Where a change belongs |
| --- | --- | --- |
| HTTP transport, independent wire DTOs, errors | [client `api.rs`](../../crates/osu-radio-client/src/api.rs), [client `models.rs`](../../crates/osu-radio-client/src/models.rs) | Reusable frontend/backend communication stays in `osu-radio-client`. DTO details are deliberately excluded from this guide. |
| Session and child lifecycle | [client `session.rs`](../../crates/osu-radio-client/src/session.rs), [client `server.rs`](../../crates/osu-radio-client/src/server.rs) | Change supervision here, coordinating readiness output with the server. |
| Toolkit-free UI data | [client `lib.rs`](../../crates/osu-radio-client/src/lib.rs), [view models](../../crates/osu-radio-client/src/view_models/track.rs) | Shared loading/error representation and track formatting belong here, without Vizia signals, CSS classes, or asset names. |
| Window, UI state, events, async result dispatch | [GUI `app.rs`](../../apps/osu-radio-gui-vizia/src/app.rs), [GUI `main.rs`](../../apps/osu-radio-gui-vizia/src/main.rs) | Keep UI event handling and rendering state in the GUI; send reusable use cases through the client. |
| View structure and bindings | [GUI `views`](../../apps/osu-radio-gui-vizia/src/views/mod.rs) | Change the smallest existing area or shared component that owns the view. |
| Appearance and bundled visuals | [styles](../../apps/osu-radio-gui-vizia/styles), [asset registration](../../apps/osu-radio-gui-vizia/src/assets.rs) | Use the existing area stylesheet and GUI-owned asset mapping. |
| Library and folder settings | [GUI events](../../apps/osu-radio-gui-vizia/src/app.rs), [settings](../../apps/osu-radio-gui-vizia/src/views/settings/mod.rs) | Server data, display selection and picker registration. |

The [client manifest](../../crates/osu-radio-client/Cargo.toml) contains HTTP/serialization/runtime dependencies and no GUI toolkit. The [GUI manifest](../../apps/osu-radio-gui-vizia/Cargo.toml) depends on that client, Tokio, Vizia 0.4.0, and rfd for the native folder chooser. Preserve the domain/backend/client/GUI boundary when replacing today's toolkit or hosting arrangement: a frontend should not start reading osu! installation files directly.

## Reusable client behavior

`ApiClient` owns a reusable HTTP client and normalized base URL. Transport failures, response decoding failures, and unsuccessful HTTP statuses remain distinct `ApiError` cases. Status errors use the response's message where it can be decoded, with the HTTP status as fallback. Keep this handling out of views; this guide intentionally does not freeze endpoint payloads or persistence-dependent status contracts.

`Loading<T>` represents `Idle`, `Pending`, `Ready(T)`, and `Failed(String)` and offers `is_pending`, `value`, `error`, and `from_result`. `describe` walks an error's source chain into a single display message, suppressing cause text already present. These helpers are available to frontends; the current GUI uses a status signal rather than `Loading<T>` for connection status. Unit tests are colocated in [client `lib.rs`](../../crates/osu-radio-client/src/lib.rs).

`library_tracks` groups globally by stored audio-source ID, preserving server set/audio order.
The lowest beatmap ID supplies representative metadata; ordinary title/artist text falls
back to Unicode then explicit unknown labels. The first associated beatmap with a stored
cover reference supplies the cover ID. For sets with multiple distinct audio rows, unique
difficulty names are joined in the subtitle (`Artist | Easy, Hard`); shared-audio sets do
not add difficulty notation. `Track` contains IDs, title, artist, subtitle and optional
`Duration`, with no toolkit assets. Unknown duration is `--:--`. Selection survives a
refresh by audio ID, falling back to the first remaining row. Tests live in
[track.rs](../../crates/osu-radio-client/src/view_models/track.rs).

## Session and server supervision

`Session::start` starts `EmbeddedServer`, then creates an `ApiClient` at the reported URL. `Session` exposes `api`, `base_url`, and asynchronous `shutdown`. It does not itself perform an HTTP readiness request. After `Connected`, the GUI loads library and folders independently, each with loading, empty and error states. Songs retain Refresh / Retry; settings show Retry only after a failure.

The current [supervisor](../../crates/osu-radio-client/src/server.rs) has these behaviors:

- `ServerOptions` accepts `binary`, `working_directory`, `address`, and `startup_timeout`. Defaults are no binary/directory override, `127.0.0.1:0`, and 30 seconds. The address is configurable; the GUI uses those defaults, letting the OS select a port.
- Binary resolution checks the explicit option, `OSU_RADIO_SERVER_BIN`, a sibling of the current executable, then the executable name on `PATH`. The filename uses the platform's executable suffix. Build the server before launching the GUI; its Cargo dependency on the client does not build the server executable.
- The child receives `OSU_RADIO_SERVER_ADDRESS` from the options. It inherits the working directory unless overridden. Server configuration currently requires successfully loading a discoverable `.env`; environment variables alone do not bypass a missing `.env`. See [backend configuration](backend.md) and the [server config source](../../apps/osu-radio-server/src/config.rs).
- Stdin is closed. Stdout and stderr are piped and drained, with `[server]` prefixes. Stderr is forwarded while stdout is inspected for readiness; stdout continues forwarding afterward. Leaving either pipe unread can block the child.
- Readiness depends on a stdout line containing `listening on ` followed by an `http://` or `https://` URL. The server prints its bound address, so the URL contains the actual port rather than `0`. This is a protocol between [server `main.rs`](../../apps/osu-radio-server/src/main.rs) and `parse_ready_line`, not optional diagnostic wording.
- Early stdout EOF waits for the child and reports an exit error; read failure and startup timeout have separate errors. The timeout wraps the ready-line reader, not the subsequent child wait after EOF. Post-start stdout/stderr read loops stop on read errors; there is no automatic restart or ongoing health monitor.
- `shutdown` takes the child once and awaits its termination; repeated shutdown is harmless. `kill_on_drop(true)` and `EmbeddedServer::drop` provide fallback termination, but `Drop` does not await it. A hard kill or crash of the parent cannot be assumed to run destructors or terminate the child.

Readiness parsing has focused unit tests in [server.rs](../../crates/osu-radio-client/src/server.rs). The [ignored embedded-server integration test](../../crates/osu-radio-client/tests/embedded_server.rs) starts a real server and performs database-backed requests; it uses a disposable SQLite database and refuses configured database environment variables. Its prerequisites and scope are recorded in [development](development.md).

## GUI state and event flow

[main.rs](../../apps/osu-radio-gui-vizia/src/main.rs) owns one Tokio runtime that outlives the window. It passes a runtime handle to the app, allowing server shutdown and child reaping during UI teardown. Do not move runtime ownership into a model whose destruction would end it too early.

[app.rs](../../apps/osu-radio-gui-vizia/src/app.rs) builds `AppData`, emits `Connect`, and builds the shell. `AppData` holds an optional `Arc<Session>`, runtime handle, and `UiState`. `UiState` is `Copy` and carries signals for the selected tab, selected audio ID and track, library rows, registered folders and the selected folder ID, independent loading states, artwork revision, two search strings, connection status, and tracked maximize state. Existing builder functions pass it by value.

Background work reports results through `ContextProxy::emit`. `AppEvent::Connected` installs the session and clears status; `Failed` displays a message. UI-thread event handling writes signals with `set`; views project state with `map`. Follow this signal/event pattern rather than introducing older Vizia lens examples or writing signals from runtime tasks.

Window-close events call `stop`, taking the session and spawning asynchronous shutdown. Drop-based termination remains the fallback. Startup failure exposes the error and retry controls. Refresh/shutdown invalidates pending media generations; shutdown aborts media jobs.

The songs pane uses Vizia `VirtualList::new_generic` with a checked optional row lookup (the stock indexer can panic while a list shrinks). It uses 106px slots containing 90px cards and 16px spacing. Visible row construction and selection request media. A queue runs at most four tasks; each task fetches cover and duration sequentially. HTTP requests time out after 30 seconds. Image responses are bounded to 16 MiB on both client and server; raster decoding runs in a blocking task, cached covers are scaled proportionally to at most 1280px on the longest side, and the GUI-owned LRU cache is bounded to 64 MiB. Unavailable, corrupt or oversized artwork remains neutral. Duration failures leave `--:--`; refreshing permits another attempt.

The General section shows an **osu! folders** dropdown with names formatted as `{kind} - {root_path}` and an adjacent plus button. The first returned folder is selected initially; selection is retained by ID on refresh and is session-local presentation only. An empty list shows `No osu! folders`. Long dropdown entries wrap, and the collapsed field has a full-name tooltip. Selecting a folder closes the popup and does not filter songs.

The plus button opens `rfd::AsyncFileDialog` for a directory and immediately registers the chosen path through the client API with no label. Cancellation leaves folder state unchanged. Adding is disabled during connection, loading, picking and registration. Duplicate registration selects the existing stored folder without adding a duplicate or changing its saved values. Loading/registration errors appear inline; Retry reloads a failed list or reopens the picker after failed registration. Settings have no manual path entry, label editing, enabled toggle, save/delete controls or last-scan display. Existing labels and backend CRUD contracts remain intact. Registration does not import songs; existing libraries obtain cover references after CLI reimport without `--clear`.

## Implemented interactions and placeholders

These are source-confirmed bindings, not a claim of interactive verification on every platform.

| Surface | Current behavior | Source |
| --- | --- | --- |
| Songs/Settings tabs | Change `Tab`; both panes are constructed and visibility follows the tab signal. The player remains visible. | [shell](../../apps/osu-radio-gui-vizia/src/views/mod.rs), [top bar](../../apps/osu-radio-gui-vizia/src/views/top_bar.rs) |
| Track cards | Change the selected audio-source ID, card highlight, cover/backdrop, title, artist, and duration. This does not start audio playback. | [track card](../../apps/osu-radio-gui-vizia/src/views/songs/track_card.rs), [player](../../apps/osu-radio-gui-vizia/src/views/player/mod.rs), [background](../../apps/osu-radio-gui-vizia/src/views/background.rs) |
| Search fields | Edit independent query signals and toggle placeholder labels. Neither songs nor settings are filtered yet. | [search row](../../apps/osu-radio-gui-vizia/src/views/components/search_row.rs), [track list](../../apps/osu-radio-gui-vizia/src/views/songs/track_list.rs), [settings pane](../../apps/osu-radio-gui-vizia/src/views/settings/mod.rs) |
| Song filter chips | Static labels and visual hover treatment; no filter or picker actions. | [chip](../../apps/osu-radio-gui-vizia/src/views/components/chip.rs) |
| Folder settings | Display-only dropdown selection, native directory picker with immediate registration, and failure-only retry. No GUI editing/removal, import or output-device selection. | [settings](../../apps/osu-radio-gui-vizia/src/views/settings/mod.rs) |
| Transport, volume, add, stack icon | Visual controls without action handlers. `icon_button` itself only adds a CSS class. | [controls](../../apps/osu-radio-gui-vizia/src/views/player/controls.rs), [icon helpers](../../apps/osu-radio-gui-vizia/src/views/components/icon.rs), [top bar](../../apps/osu-radio-gui-vizia/src/views/top_bar.rs) |
| Progress and elapsed time | Zero progress and `00:00` elapsed; optional duration follows the selected track. No seeking or playback clock. | [progress](../../apps/osu-radio-gui-vizia/src/views/player/progress.rs), [player stylesheet](../../apps/osu-radio-gui-vizia/styles/player.css) |
| Window controls | Custom minimize/maximize/close actions and title-bar dragging; double-click handler requests maximize toggling. | [top bar](../../apps/osu-radio-gui-vizia/src/views/top_bar.rs), [events](../../apps/osu-radio-gui-vizia/src/app.rs) |

The app disables native decorations, so empty title-bar space and custom controls are operational UI. Dragging is guarded by `cx.hovered() == cx.current()` because mouse-down events bubble from children. The double-click handler currently checks the button only. The maximize icon follows a local boolean, not an observed OS window state; an OS-side maximize can desynchronize it, and a later button press may only bring the tracked state back into agreement.

## View, stylesheet, and asset ownership

Views use free builder functions, with one private custom `Artwork` view for raster rendering. Shared builders are re-exported by [components/mod.rs](../../apps/osu-radio-gui-vizia/src/views/components/mod.rs): `icon`, `icon_button`, `gap`, `hspacer`, `search_row`, `chip_row`, and `sidebar`. Songs, settings, and player each have an area module with leaf modules for their parts.

Stylesheets are owned by **areas**, not by every Rust view module. [The stylesheet registry](../../apps/osu-radio-gui-vizia/src/views/mod.rs) loads `base.css` first, then the six area `style()` functions in this order:

| Module | Sheet and covered views |
| --- | --- |
| [background.rs](../../apps/osu-radio-gui-vizia/src/views/background.rs) | [background.css](../../apps/osu-radio-gui-vizia/styles/background.css): body, backdrop, glows, pane layout |
| [top_bar.rs](../../apps/osu-radio-gui-vizia/src/views/top_bar.rs) | [top-bar.css](../../apps/osu-radio-gui-vizia/styles/top-bar.css): navigation, connection status, window controls |
| [components/mod.rs](../../apps/osu-radio-gui-vizia/src/views/components/mod.rs) | [components.css](../../apps/osu-radio-gui-vizia/styles/components.css): shared sidebar, search, chips, icon buttons |
| [songs/mod.rs](../../apps/osu-radio-gui-vizia/src/views/songs/mod.rs) | [songs.css](../../apps/osu-radio-gui-vizia/styles/songs.css): list and cards |
| [settings/mod.rs](../../apps/osu-radio-gui-vizia/src/views/settings/mod.rs) | [settings.css](../../apps/osu-radio-gui-vizia/styles/settings.css): list, sections, fields |
| [player/mod.rs](../../apps/osu-radio-gui-vizia/src/views/player/mod.rs) | [player.css](../../apps/osu-radio-gui-vizia/styles/player.css): cover, metadata, progress, controls |

[base.css](../../apps/osu-radio-gui-vizia/styles/base.css) supplies global font/window styling and spacers. Most appearance belongs in these sheets. Existing view modifiers bind dynamic state, asset choices, visibility, and explicit gap sizes. Adding a leaf component normally reuses its area's sheet; it does not inherently require another sheet or registry entry.

The current `include_style!` workflow hot-reloads files in debug and embeds them in release. A debug build alone does not prove stylesheet paths resolve; the release check in [development](development.md) covers embedded paths. Stylesheet-load failures are printed to stderr by the registry and do not abort app construction.

[assets.rs](../../apps/osu-radio-gui-vizia/src/assets.rs) embeds Nunito and SVG icons. Source artwork arrives through the cover endpoint and is decoded with Skia into a bounded GUI cache keyed by beatmap ID. Bundled reference artwork remains on disk and supplies a decoding fixture, not sample songs. `tint` keeps the existing GUI tint classes. The shared [Artwork view](../../apps/osu-radio-gui-vizia/src/views/components/artwork.rs) draws the same cached images in cards, cover and backdrop with a centered source crop, uniform scaling, linear sampling and clipping to the styled rounded path. Selection bindings request a redraw. Overlays stay separate from the source assets; the backdrop has 6% opacity and 20px blur, with the existing glow hues. The body clips decorative overflow below the top bar; radial fades finish inside their bounds to avoid visible rectangular edges at large sizes.

`Svg::new` receives embedded icon bytes. The shared `.icon` uses `fill: transparent` so Vizia preserves embedded white fills and asset opacity; navigation does not multiply the already translucent Settings glyph's opacity. Icons remain 24px, with 16px window glyphs. Card overlays darken artwork directionally while text stays white, and the selected card retains its cyan border. The font's license is bundled as [OFL.txt](../../apps/osu-radio-gui-vizia/assets/fonts/OFL.txt).

## Adaptive player layout

[app.rs](../../apps/osu-radio-gui-vizia/src/app.rs) sets a 1024 × 640 logical-pixel minimum window size. The top bar remains 50px, the sidebar 480px and song rows 90px. Typography and icons retain their sizes as the window grows.

The private `PlayerGeometry` in [player/mod.rs](../../apps/osu-radio-gui-vizia/src/views/player/mod.rs) derives sizes from the available player pane. Its geometry callback converts physical bounds to logical pixels before binding styles; Vizia applies display scaling once. Artwork grows from 340px at a 960px pane width up to 640px, and content grows from 650px to 960px. Content is also limited to 85% of the pane width. The 138:172 horizontal margin ratio preserves the reference content span x=618–1268 at 1440 × 952.

| Window (100% scaling) | Square cover | Content width |
| --- | --- | --- |
| 1024 × 640 | 340px | 462.4px |
| 1440 × 952 | 340px | 650px |
| 1600 × 900 | 396.7px | 758.3px |
| 1920 × 1080 | 510px | 960px |
| 2560 × 1440 | 640px | 960px |

The cover is centered in the flexible region above a 173px metadata/progress/control group, with 52px bottom spacing. Its square size is capped by both available width and height. Player titles/artists, card text and connection status stay on one line and truncate overflowing text. The progress knob is centered on the fill endpoint as width changes; progress remains zero until playback is implemented.

Both `font-weight` and the Nunito `wght` variation specify the intended weight.

Colocated unit tests cover the five sizing targets, short/empty bounds, centered cropping with equal axis scaling, invalid image bounds and valid/corrupt image decoding. These are mathematical/resource checks; visual and input checks require a running GUI as described below.

## Known Vizia pitfalls and verification limits

These constraints come from the checked-in implementation and its workaround comments. They are repository-specific evidence, not freshly reproduced upstream bug reports. For unfamiliar Vizia APIs or version changes, use Context7 as required by [AGENTS.md](../../AGENTS.md), then check compatibility with the locked version and actual dependency source.

- **Clickable children:** hoverable contents can become the press target and prevent a parent's `on_press`. The [top-bar](../../apps/osu-radio-gui-vizia/styles/top-bar.css) and [song-card](../../apps/osu-radio-gui-vizia/styles/songs.css) rules make clickable contents inert with `pointer-events: none`. Keep the parent interactive and avoid disabling children that are intended to have their own actions.
- **Decorative overlap:** the backdrop and oversized glows extend beyond their apparent area. [background.css](../../apps/osu-radio-gui-vizia/styles/background.css) records the hit-testing problem where a later overlapping view steals presses from the title bar. Keep decorative layers inert even when visually underneath controls.
- **Empty textboxes:** [search_row.rs](../../apps/osu-radio-gui-vizia/src/views/components/search_row.rs) records a Vizia 0.4.0 accessibility subtraction overflow for an empty textbox using `Textbox::placeholder`. The current workaround is a separate label in a `ZStack`, visible while the query is empty, with pointer events disabled in [components.css](../../apps/osu-radio-gui-vizia/styles/components.css). Preserve it until a dependency change is verified to remove the issue.
- **Stack spacing:** [layout.rs](../../apps/osu-radio-gui-vizia/src/views/components/layout.rs) records that a child's `top` is ignored for spacing inside a stack. Use the parent's `gap` for regular spacing or the existing explicit `gap` element for different sibling spacing.
- **Selectors and property dialect:** `scroll-content` is an element selector in [songs.css](../../apps/osu-radio-gui-vizia/styles/songs.css) and [settings.css](../../apps/osu-radio-gui-vizia/styles/settings.css), not a class. Existing sheets use `corner-radius`, `layout-type`, `size`, `gap`, `alignment`, and `1s` stretch syntax. Do not assume browser CSS or main-branch Vizia examples apply unchanged.
- **Padding syntax:** Vizia 0.4 accepts one value for `padding`; use `padding-top`, `padding-bottom`, `padding-left` and `padding-right` for different sides. A two-value declaration such as `padding: 10px 12px` clears preceding declarations in that rule during parser recovery, which collapsed the folder dropdown rows. Compilation checks embedded paths but do not validate stylesheet syntax.

Client unit tests cover shared loading/error behavior, track formatting, and readiness parsing. The headless `folder_selection_survives_refresh_and_duplicate_registration` test in [app.rs](../../apps/osu-radio-gui-vizia/src/app.rs) covers empty lists, selection retention, additions and duplicate registrations without opening a window or reading installations. GUI geometry, artwork and folder-state tests do not constitute an interaction test suite. Compilation and stylesheet-path checks do not validate clicks, native pickers, empty-search accessibility, title-bar dragging, OS window-state behavior, layout, audio output, or child cleanup after abrupt process death. Select scoped checks from [development](development.md), and record actual automated and manual results separately.

When behavior changes, update the affected paragraph or table here and relevant source links. Keep reusable procedures in the skill and common build commands in the development guide; ordinary UI edits do not require rewriting all agent documentation.
