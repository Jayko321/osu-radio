# Development and verification

Start with [the index](index.md), then the guide for the affected component:
[scanner](scanner.md), [frontend](frontend.md), [database](database.md), or [backend](backend.md).
The commands below are recommendations checked against repository paths and
flags, not a record that they ran successfully.

Database contracts are documented in [database](database.md). Use disposable
databases for persistence checks; do not use application database resets, real
storage imports, or broad machine discovery as routine verification.

## Platforms and prerequisites

The user-confirmed current targets are Linux and Windows. macOS is unverified;
Android and iOS are future goals. OS-specific discovery branches and a successful
Linux build do not establish validation on another platform. Vizia and a child
server describe today's desktop implementation, not requirements for all future
frontends.

- Use Rust and Cargo with Rust 2024 support; the workspace declares its edition
  in [`Cargo.toml`](../../Cargo.toml). Install rustfmt and Clippy for the checks
  below. No repository toolchain pin is currently present.
- Building `radio-scanner` runs [`build.rs`](../../crates/radio-scanner/build.rs),
  which invokes `dotnet build --configuration Release` for the
  [Realm helper project](../../tools/osu-lazer-realm-parser/osu-lazer-realm-parser.csproj).
  It targets `net8.0`; use the .NET 8 SDK or a compatible SDK able to build that
  target, and a runtime capable of launching the produced helper. This build
  dependency also reaches the CLI and server through `radio-scanner`.
- A prebuilt helper can bypass that build via
  `OSU_LAZER_REALM_PARSER_PATH`. Use a real helper for runtime imports; the build
  override itself does not validate that the file exists or works.
- GUI compilation depends on the platform toolchain and the native requirements
  of the currently resolved Vizia/graphics stack. A display session is needed
  to check actual interactions. Consult current upstream documentation through
  Context7 for platform package/setup advice instead of copying package lists
  from another OS or Vizia version.
- Cargo and .NET dependency restoration may need network access on a fresh
  machine. `--locked` protects Cargo.lock; it does not make builds offline or
  prevent generated artifacts under `target/` and the helper build directories.

## Environment and executable discovery

| Input | Current use | Source |
| --- | --- | --- |
| `OSU_LAZER_REALM_PARSER_PATH` | Build-time helper bypass and runtime helper override; prefer an absolute path. | [Build script](../../crates/radio-scanner/build.rs), [lazer process reader](../../crates/radio-scanner/src/lazer/scanner.rs) |
| `OSU_LAZER_REALM_PARSER_BUILT_PATH` | Set by the build script and embedded with `env!`; this is generated build output, not the normal user override. | [Lazer process reader](../../crates/radio-scanner/src/lazer/scanner.rs) |
| `DOTNET_CLI_HOME` | Used by the helper build when supplied; otherwise the build script uses `target/dotnet-home`. | [Build script](../../crates/radio-scanner/build.rs) |
| `OSU_RADIO_SERVER_ADDRESS` | Standalone default is `127.0.0.1:3000`; the supervisor supplies `ServerOptions::address`, default `127.0.0.1:0`. | [Server config](../../apps/osu-radio-server/src/config.rs), [supervisor](../../crates/osu-radio-client/src/server.rs) |
| `OSU_RADIO_SERVER_BIN` | Server executable override after explicit `ServerOptions::binary`, before sibling and `PATH` lookup. | [Supervisor](../../crates/osu-radio-client/src/server.rs) |
| `SQLITE_DATABASE_URL` / `POSTGRES_DATABASE_URL` | Required by SQLite / PostgreSQL CLI and server builds respectively. SQLite accepts paths, SQLite URLs and `:memory:`. | [Server config](../../apps/osu-radio-server/src/config.rs) |
| `.env` and process working directory | Server unconditionally propagates failure to load `.env`; environment variables alone do not permit launch without a discoverable, loadable file. `ServerOptions::working_directory` can select the child directory. | [Server config](../../apps/osu-radio-server/src/config.rs), [supervisor](../../crates/osu-radio-client/src/server.rs) |

Discovery's default candidates also consult OS location inputs such as
`XDG_DATA_HOME` on Linux and `APPDATA`, `LOCALAPPDATA`, `USERPROFILE`, and program
directory variables on Windows. Exact probes are in
[`known.rs`](../../crates/radio-scanner/src/discovery/known.rs). For reproducible
tests use explicit roots instead of modifying the user's environment or invoking
default discovery.

## Scoped CLI inspection

The CLI is a thin harness. Argument definitions are in
[`types.rs`](../../apps/osu-radio-cli/src/types.rs); discovery and marker selection
are in [`helpers.rs`](../../apps/osu-radio-cli/src/commands/helpers.rs).
Run examples from the repository root, replacing quoted placeholders with a
specific installation path. They are examples for an authorized source read,
not checks to run blindly against a user's machine.

```sh
cargo run -p osu-radio-cli --locked -- scan --help
cargo run -p osu-radio-cli --locked -- import --help
cargo run -p osu-radio-cli --locked -- scan --root "/absolute/path/to/osu" --depth known --source lazer --first
cargo run -p osu-radio-cli --locked -- scan --root "/absolute/path/to/selected-parent" --depth shallow --limit 2 --json
cargo run -p osu-radio-cli --locked -- import --marker "/absolute/path/to/osu/client.realm" --limit 5 --json
```

These Cargo arguments also work in PowerShell; use a quoted Windows path such as
`"C:\Games\osu\client.realm"` for the marker. Commands are single lines so shell
continuation syntax does not differ.

`scan --root` replaces default roots. `--source` filters during discovery;
`--first` and `--limit` stop the discovery at its source and cannot be combined.
`scan --limit 0` is rejected. The scan default is `full`, so specify `known` or
`shallow` for bounded inspection. A known-tier run with explicit roots does not
perform a recursive sweep; relocation in `storage.ini` can still lead to a
different data directory. See [scanner](scanner.md) for that distinction.

An explicit `import --marker` avoids discovery. Without it, marker selection
runs full discovery, possibly across OS defaults; do not omit it casually.
`import --limit` limits printed beatmaps, not Realm reads or the number imported
into memory. The `import` command reads and displays source metadata; it does not
persist it. Stable markers are discoverable but stable imports remain unsupported.

## Verification matrix

Choose the smallest row that covers the changed behavior. These are recommended
commands; record exit status, failures, and skipped coverage in the task report.
Do not treat a build as proof of window interactions, a real Realm import, or
playback.

| Change | Recommended check | Evidence and limitation |
| --- | --- | --- |
| Rust formatting | `cargo fmt --check` | Read-only formatting check. Use `cargo fmt` only when authorized to edit source. |
| Core type behavior | `cargo test -p radio-core --locked` | [Core tests](../../crates/radio-core/src/lib.rs); no installation reads. |
| Discovery roots, filters, tiers | `cargo test -p radio-scanner --locked discovery::` | [Discovery tests](../../crates/radio-scanner/src/discovery/mod.rs), [known-path tests](../../crates/radio-scanner/src/discovery/known.rs), [pruning tests](../../crates/radio-scanner/src/discovery/sweep.rs) use explicit temporary roots. |
| Lazer mapping or process handling | `cargo test -p radio-scanner --locked lazer::` | [Parser and fake-helper tests](../../crates/radio-scanner/src/lazer/tests.rs); helper-process tests are Unix-only and do not validate real Realm data. |
| Unsupported-source behavior | `cargo test -p radio-scanner --locked reports_unsupported_scanner_sources` | [Scanner entry-point test](../../crates/radio-scanner/src/lib.rs). |
| Scanner change spanning these paths | `cargo test -p radio-scanner --locked` | Combines the preceding scanner checks; helper build prerequisites still apply. |
| C# helper source | `dotnet build tools/osu-lazer-realm-parser/osu-lazer-realm-parser.csproj --configuration Release --nologo` | Compiles the producer; no automated C# test project currently exists. Follow the [scanner skill](../../.agents/skills/osu-radio-scanner/SKILL.md) for contract checks. |
| CLI wiring | `cargo test -p osu-radio-cli --locked` and `cargo clippy -p osu-radio-cli --all-targets --locked` | Argument tests reject removed `store --count` and retain `--clear`; memory SQLite connection check. No real source import. |
| Local audio engine | `cargo test -p osu-radio-player --locked` | MP3 CBR/VBR, Ogg/Vorbis and WAV decoding/seek with controlled mixer consumption; no physical device. |
| Client formatting, state, readiness parsing | `cargo test -p osu-radio-client --lib --locked` | [Client tests](../../crates/osu-radio-client/src/lib.rs), [Track tests](../../crates/osu-radio-client/src/view_models/track.rs), [readiness tests](../../crates/osu-radio-client/src/server.rs) and controller tests; process fixtures are isolated. |
| GUI code and embedded stylesheet paths | `cargo check -p osu-radio-gui-vizia --release --locked` | Release `include_style!` resolves stylesheet paths at compile time. Debug can defer missing paths to runtime. |
| Server changes, default docs | `cargo clippy -p osu-radio-server --all-targets --locked` | Compiles the documentation-enabled router; does not launch the server. |
| Server changes, docs disabled | `cargo clippy -p osu-radio-server --no-default-features --features sqlite --all-targets --locked` | Compiles the other router and checks OpenAPI annotations remain optional. |
| Covered cross-crate changes | Combine the affected rows above | Do not automatically expand to persistence workflows or real installation reads. |
| Broad review compile/lint coverage when warranted | `cargo clippy --workspace --all-targets --locked` | Builds existing workspace dependencies, including database components; it is not a specification or runtime test of those components. |

Do not use Cargo `--all-features`: the workspace includes mutually exclusive
database backend features. Feature compatibility is a build constraint here;
select the backend explicitly when disabling default features.

### Repository and backend checks

Use each backend in a separate Cargo invocation. Default tests use SQLite:

```sh
cargo test -p radio-db -p radio-services --locked
cargo test -p osu-radio-cli --locked
cargo test -p osu-radio-server --locked
cargo test -p osu-radio-server --no-default-features --features sqlite --locked
cargo clippy -p radio-db -p radio-services -p osu-radio-cli -p osu-radio-server --all-targets --locked -- -D warnings
cargo clippy -p radio-db -p radio-services -p osu-radio-cli -p osu-radio-server --no-default-features --features postgres --all-targets --locked -- -D warnings
cargo clippy -p osu-radio-server --no-default-features --features postgres,docs --all-targets --locked -- -D warnings
```

The PostgreSQL service and repository contract tests are ignored by default. Provision a **fresh,
disposable local cluster/database**, naming the database `radio_db_test_*`, and
supply only its URL in `RADIO_DB_TEST_POSTGRES_URL`:

```sh
RADIO_DB_TEST_POSTGRES_URL='postgres://USER@127.0.0.1:PORT/radio_db_test_repositories' cargo test -p radio-db --no-default-features --features postgres --locked -- --include-ignored
RADIO_DB_TEST_POSTGRES_URL='postgres://USER@127.0.0.1:PORT/radio_db_test_services' cargo test -p radio-services --no-default-features --features postgres --locked -- --include-ignored --skip synthetic_search_timings
```

Replace USER and PORT with the disposable cluster's values. The test never reads
`POSTGRES_DATABASE_URL`. It exercises explicit reset, creates failure triggers,
and retains an unrelated-table fixture to prove reset scope; use a separate fresh disposable
database for each test command. Stop and remove only that test cluster after testing.
PostgreSQL caller builds compile both router variants; route/service runtime tests
use isolated SQLite. Memory tests, generated keys and independent-pool concurrency
are covered by [service contracts](../../crates/radio-services/src/tests.rs) and
[repository contracts](../../crates/radio-db/src/tests.rs).

Both commands below **must fail** with the explicit exactly-one-backend error:

```sh
cargo check -p radio-db --no-default-features --locked
cargo check -p radio-db --no-default-features --features sqlite,postgres --locked
```

### Search measurements

The opt-in [service measurements](../../crates/radio-services/src/beatmap_set/benchmarks.rs)
separate SQL/ORM reads, Rust matching and full result loading. Synthetic runs use
1,000/10,000/50,000 sets (two audio/four difficulties per set), query `roc hard`,
a warmup and five measured samples. SQLite creates a temporary database;
PostgreSQL resets only an explicitly supplied disposable `radio_db_test_*` database.

```sh
cargo test -p radio-services --locked synthetic_search_timings -- --ignored --nocapture
```

For a real-library comparison, save the initial working tree (including untracked
source files) and debug/release server binaries before editing. Use SQLite's backup
API to create one consistent copy; never copy only the main file from a live WAL
database. Run each stage on that same backup. The
[HTTP measurement script](../../tools/search-benchmark/benchmark_http.py) captures exact
payloads and medians for `rock`, `rock hard`, `星`, no matches and blank input.
Use stage labels `baseline`, `stage1`, `stage2`, `stage3`; stage 3 uses `tracks`.
Run from a directory with `.env`, as required by current server startup.

```sh
python3 tools/search-benchmark/benchmark_http.py --binary /absolute/saved/server-debug --database-backup /absolute/backup.sqlite --output /tmp/search-results --stage baseline --profile debug
RADIO_SEARCH_BENCH_DB=/absolute/backup.sqlite cargo test -p radio-services --locked snapshot_search_timings -- --ignored --nocapture
RADIO_SEARCH_BENCH_DB=/absolute/backup.sqlite cargo test -p osu-radio-server --locked snapshot_serialization_timings -- --ignored --nocapture
RADIO_SEARCH_BENCH_DIR=/tmp/search-results cargo test -p osu-radio-client --locked captured_client_timings -- --ignored --nocapture
```

Repeat with `--release` and release binary/payload labels. Snapshot service timing
keeps test-only reference implementations of the original and tag-filtered paths;
each result is compared exactly against the original. Serialization timing also
compares full client tracks. Captured client timing measures Rust decoding,
conversion and controller notification separately, excluding rendering/media.
The original mapper is retained only in the benchmark to measure the initial
client behavior (before it retained all difficulties).

`controller_http_timings` measures HTTP through controller completion, with and
without 200 ms debounce. Set `RADIO_SEARCH_BENCH_BINARY` to the saved server,
`RADIO_SEARCH_BENCH_WORKDIR` to a directory whose `.env` selects the backup, and
`RADIO_SEARCH_BENCH_COMPACT=0` for the original endpoint or `1` for the new endpoint.
Unset inherited `SQLITE_DATABASE_URL` / `POSTGRES_DATABASE_URL` when running it.
The compact case exercises the actual controller task; the original case replays
the old request/mapper into the same controller. Child startup, GUI painting,
media fetching and source import are excluded. Do not run other CPU-intensive
checks concurrently with measurements. Report medians/variability, not CI thresholds.

PostgreSQL `--include-ignored` contracts must retain `--skip synthetic_search_timings`
to avoid concurrent resets. Snapshot benchmarks are SQLite-only. Actual measurements
and executed checks are recorded in [the search report](search-performance.md).

### Store and schema lifecycle

`store` reads a full scanner result before opening the replacement transaction.
It registers the selected marker and replaces that installation's snapshot;
`--count` is no longer accepted. `--clear` explicitly resets all application data,
including registered folders. Plain startup migrates without clearing, and rejects
legacy unversioned tables. Use a new database when retaining a legacy database is
necessary. See [database](database.md) for lock, hash and cleanup guarantees.

For a GUI interaction check, first build the server with
`cargo build -p osu-radio-server --locked`, then launch the GUI with
`cargo run -p osu-radio-gui-vizia --locked` only in an already prepared, authorized
runtime environment. The GUI starts a server and therefore uses the current
database-dependent startup path. This is not part of documentation-only
validation. Check the affected interactions using the [GUI skill](../../.agents/skills/osu-radio-gui/SKILL.md).

The ignored [embedded-server test](../../crates/osu-radio-client/tests/embedded_server.rs)
also launches a real server and exercises database-backed requests. It is not a
replacement for the isolated readiness parser tests; it creates its own temporary SQLite database and `.env`, and requires a built SQLite server. Run with `env -u SQLITE_DATABASE_URL -u POSTGRES_DATABASE_URL cargo test -p osu-radio-client --test embedded_server --locked -- --ignored`. It covers registration, duplicates, label clearing, enabled edits, deletion, media route registration and restart persistence.

## Component gallery

```sh
cargo run -p osu-radio-gui-vizia --locked -- --component-gallery
```

This launch mode is chosen before constructing the normal app or Tokio runtime.
It needs no server binary, `.env`, database or osu! installation; examples and
state live in memory. `--help` prints the launch syntax without opening a window.

Headless verification for component changes:

```sh
cargo test -p osu-radio-gui-vizia --locked
cargo check -p osu-radio-gui-vizia --release --locked
cargo clippy -p osu-radio-gui-vizia --all-targets --locked -- -D warnings
cargo fmt -p osu-radio-gui-vizia --check
```

The CSS test uses the exact Vizia 0.4 parser and checks errors, recovery warnings,
unknown custom declarations and unresolved property forms. The release check alone
does not parse stylesheets. Unit tests also cover launch flags, tag cycles,
single tab selection, case-insensitive menu search, keyboard-index transitions,
and scroll direction, fractional deltas, scaling, routing and unaffected non-scroll events.

The user runs and visually checks the GUI. Suggested manual matrix (not an
executed result):

- Resize at 1024×640, 1440×952, 1920×1080 and 2560×1440; repeat at 100%, 150%
  and 200% display scaling. Check scroll access, text clipping, covers and 90px song rows.
- Inspect all four button variants, 24px icons/16px window glyphs, Poppins weights
  and fallback text. Hover, press and keyboard-focus every control.
  Buttons, tabs, window controls, menu options and switches must not show a focus
  outline, but Tab navigation and keyboard activation must still work. Song
  outlines and the white input-container focus outline must remain.
- Edit empty/filled fields, inspect sibling placeholders, toggle switches, select
  tabs and cycle each filter tag through all three states.
- Check white selected-tab backgrounds with dark labels/icons in both the gallery
  and Songs/Settings navigation. Focus fields/searches by mouse and keyboard: one
  white outline must surround the input container. Verify a white blinking caret
  before typing, while editing, and after clearing; blur/disable must hide the
  empty caret, and the separate placeholder must remain visible when empty.
- Scroll in both directions through the gallery, song list, settings, long menus
  and modal at 100%, 150% and 200% scaling. Target 60 logical pixels per wheel unit;
  test fractional/touchpad motion, Shift/horizontal scrolling, nested routing and
  both scroll bounds. The toolkit combines wheel/touchpad deltas, so both change.
- Search the playlist menu (including no results), scroll long/wrapped names,
  use arrows and Home/End then Enter; dismiss with Escape and outside press.
  Check focus returns to the menu trigger.
- Open the modal with keyboard and mouse. Tab/Shift+Tab must stay inside;
  background controls must not respond. Test close icon, Escape and backdrop,
  then confirm focus returns to Create playlist. Resize while the dialog is open.
  Check 24px panel padding/content gaps, preserved outer clearance and access to
  the bottom of long content.
- Toggle Disable demo controls: the counter, fields, switches, tags, tabs and menus
  must stop responding. The disable switch itself must remain usable.
  Text/icons must remain white with a single 40% fade; light-filled controls and
  selected tabs retain dark foregrounds. Nested fields must not fade repeatedly.
- Inspect Regular/Thick/Thin over the colour bands: background is blurred,
  text/icons are sharp. Check the same colours on Songs, Settings and the player.
- In an authorized normal-app environment, check Refresh/Retry, folder menu and
  native picker, Songs/Settings navigation and custom window controls. The gallery
  does not exercise these backend/window integrations.

## Troubleshooting and limits

| Symptom | Inspect first |
| --- | --- |
| `cargo` or `rustc` unavailable | Verify the active shell's `PATH` and installed Rust toolchain; do not claim checks passed. |
| Scanner/CLI/server build cannot start `dotnet` | Check SDK availability and the helper override. The server transitively builds the scanner even if the use case only validates folders. |
| Helper cannot launch at runtime | Confirm the resolved helper exists, has the platform executable form/permissions, and has a compatible .NET runtime; preserve its stderr in reported failures. |
| `Failed to load .env` | Check the actual child's working directory and a loadable `.env`; setting `SQLITE_DATABASE_URL` alone does not avoid the load call. |
| GUI cannot find the server binary | Build the server first and check explicit option, environment override, sibling executable, then `PATH`. |
| Embedded startup times out | Check forwarded stderr and the readiness marker before changing timeout settings. |
| Style appears ignored or a control cannot be clicked | Follow [frontend pitfalls](frontend.md) and inspect the owning stylesheet, inclusion, overlapping layers, and child hit testing. |
| A nonempty textbox works but an empty placeholder crashes | Use the existing sibling-label pattern described in [frontend](frontend.md); do not add `.placeholder(...)` to an empty Vizia textbox. |

## Skill selection and upstream documentation

| Request | Procedure | Technical guide |
| --- | --- | --- |
| Modify GUI layout, interaction, styles, or assets | [`osu-radio-gui`](../../.agents/skills/osu-radio-gui/SKILL.md) | [Frontend](frontend.md) |
| Modify discovery, import mapping, or Realm helper | [`osu-radio-scanner`](../../.agents/skills/osu-radio-scanner/SKILL.md) | [Scanner](scanner.md) |
| Review staged/unstaged work and write findings only | [`cr`](../../.agents/skills/cr/SKILL.md) | This verification matrix plus the affected guide |
| Database or server/client API work | [API skill](../../.agents/skills/osu-radio-api/SKILL.md) | [Database](database.md), [backend](backend.md) |

Keep task procedures in skills and technical facts in guides. Claude uses the
same procedures linked from `CLAUDE.md`; do not create separate copies.

## Context7 and source evidence

For library/framework/SDK/API/CLI/cloud-service questions, including syntax,
configuration, migration, library-specific debugging and setup, fetch current
documentation even for familiar technologies. Prefer Context7 over web search
for this purpose; follow the shared requirement in [AGENTS.md](../../AGENTS.md).

1. Resolve the official library name with proper punctuation:
   `npx ctx7@latest library <name> "<specific question>"`.
   Run `library` first unless the user supplied an ID such as `/org/project`.
2. Select the best ID using exact name, relevant description, snippet count,
   source reputation and benchmark score. Try an alternate name/query if results
   do not match. Use a versioned ID returned by the resolver for version-specific
   advice.
3. Fetch `npx ctx7@latest docs <libraryId> "<specific concept>"`. Use separate
   queries for distinct concepts unless asking how they interact, and use at
   most three commands per question. Base API advice on the fetched material.

Run Context7 requests outside the default sandbox. If DNS/network errors occur,
rerun outside it rather than retrying inside. On quota errors, report the failure
and suggest `npx ctx7@latest login` or `CONTEXT7_API_KEY`; do not silently substitute
remembered advice. Never put credentials or other sensitive information in queries.

Repository source establishes what this project currently does. Current upstream
documentation establishes supported library APIs; verify against the resolved
version and installed source when the two differ. General code review, business
logic debugging, refactoring, writing scripts from scratch, general programming
concepts and documentation tracing do not themselves require Context7. A
library-specific question encountered within such a task still does.

When behavior changes, update the affected guide, source/test links, and any
procedure whose steps changed in the same task. Leave unrelated guides alone.
Keep task-specific actual validation results in the task report rather than
turning this recommended-command matrix into a stale record of passing checks.

## Qt frontend

Prerequisites: Qt **6.8+** development libraries/tools (Core, Gui, Qml, Quick,
QuickControls2, Network), QML Basic Controls, Layouts and Effects modules, SVG/image
plugins, a compatible C++ compiler, and Rust. CXX-Qt 0.10 finds Qt through `qmake`;
set `QMAKE=/path/to/qmake6` if multiple installations exist. The runtime must also
be able to locate Qt libraries and plugins. QML/fonts/icons/covers are embedded,
but the executable is not a standalone Qt distribution. Windows packaging and
execution are unverified.

```sh
cargo run -p osu-radio-qt --locked
cargo run -p osu-radio-qt --locked -- --component-gallery
cargo run -p osu-radio-qt --locked -- --help
```

Default launch is live: first build `osu-radio-server` and provide a `.env`
pointing to the intended database. `--component-gallery` is offline and requires
no server, `.env`, database or osu! installation. `--help` and argument errors are handled before GUI initialization.
Recommended automated checks:

```sh
cargo test -p osu-radio-client --lib --locked
cargo test -p osu-radio-client --features mock --lib --locked
cargo test -p osu-radio-qt --locked
cargo build -p osu-radio-qt --locked
cargo clippy -p osu-radio-client --features mock -p osu-radio-qt --all-targets --locked -- -D warnings
cargo fmt -p osu-radio-client -p osu-radio-qt --check
```

The Qt integration test launches both roots using the offscreen/software backend
from temporary working directories. A deterministic executable fixture exercises
the real Session subprocess and HTTP path, including failed startup/retry,
independent list errors, media notifications, ID selections, shrinking/empty
refreshes and child cleanup. Gallery mode uses no backend. The bundled
[test probe](../../apps/osu-radio-qt/tests/AdapterProbe.qml) exits only after
explicit assertions complete, with a native watchdog and outer process timeout.
`OSU_RADIO_QT_SMOKE_TEST=1` is an internal test hook; do not set it interactively.
Help/error paths run with an invalid platform plugin to verify no GUI initialization.
The Unix fixture requires Python 3. For real SQLite backend coverage:

```sh
cargo build -p osu-radio-server -p osu-radio-qt --locked
cargo test -p osu-radio-qt --locked --test launch_smoke real_backend_uses_a_disposable_database -- --ignored
```

That test removes inherited database URL variables and uses a temporary `.env`
and SQLite file. It never opens the workspace database or runs discovery.

Run [`scripts/lint-qml.sh`](../../apps/osu-radio-qt/scripts/lint-qml.sh) after
building. Set `QMLLINT=/path/to/qmllint` if needed (default `/usr/lib/qt6/bin/qmllint`).
The script stages generated `OsuRadio/qmldir` and `plugin.qmltypes` with the QML
source tree in a temporary import directory, then lints every app/probe QML file.
It requires Bash, Python 3 and ripgrep. Generated QObject type information is
necessary for `Store.qml`; linting only source paths cannot resolve `MockBridge`.

The user performs desktop visual/input checks: the existing gallery matrix above
applies to Qt too, with 1024×640 through 2560×1440 and 100/150/200% scaling.
Check crop/cover changes, typography, blur, scrolling, keyboard navigation,
menu and modal focus restoration/trapping, disabled controls and native window
move/resize/minimize/maximize/close. The automated software smoke tests cannot
establish these results, physical playback or Windows compatibility.

## Audio playback verification

Playback uses the client-owned worker and Rodio 0.22.2 with `playback`, `mp3`, `vorbis` and `wav`.
Linux builds need the native audio development libraries required by CPAL/ALSA.
Device initialization occurs on the first Play; galleries and normal startup do
not require an output device. No database migration or reset is needed.

Run the player tests, client tests with and without `mock`, server tests with and
without `docs`, and both frontend checks listed above. Scope Clippy to these five
packages, testing the docs-disabled server separately. Never select all database
features together. Audio tests must consume decoded data with a controlled mixer,
not open the machine's physical device. HTTP tests use disposable sources and
databases. They verify bytes, status codes, bounded downloads and cancellation;
controller tests cover selection independence, stale requests and cleanup.

Manual acceptance remains separate: play MP3, Ogg/Vorbis and WAV files, select another row while audio
continues, explicitly play that row, pause/resume, seek forward/backward and after
EOF, adjust global volume, and close during a download. Test slider dragging
without tick interference in Qt and Vizia. Repeat physical playback and lifecycle
checks on Windows. Queue, automatic advance, shuffle, repeat, device selection and
volume persistence across launches are outside this stage.

Executed on Linux on 2026-09-20 for this playback change: player tests passed
(6), client library tests passed with `mock` disabled (35) and enabled (41),
and server tests passed with `docs` enabled (21) and disabled (20). Existing
benchmark tests stayed ignored. Player/client/server scoped all-target Clippy
passed with warnings denied. Vizia passed 20 tests, scoped Clippy and the release
check. Qt passed 11 tests (including six offscreen scenarios), scoped Clippy and
generated-import `qmllint`; its existing real-backend test stayed ignored. HTTP
tests required execution outside the sandbox because loopback socket binding is
blocked inside it. Debug builds of the server and Qt frontend also passed;
workspace formatting and changed guide links were checked. These are deterministic
engine, isolated HTTP and controller checks, not physical audio or Windows tests.

Executed on Linux on 2026-09-22 for the format extension: all six player tests
passed with MP3 CBR/VBR, Ogg/Vorbis and PCM WAV fixtures, including extensionless
decoding and seek/EOF/replay coverage. Client tests passed without/with `mock`
(35/41; two benchmarks ignored in each). Scoped player/client all-target Clippy
with warnings denied and formatting passed. Physical playback and Windows were
not exercised in this follow-up.
