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

- Install Rust through rustup. The repository-wide
  [`rust-toolchain.toml`](../../rust-toolchain.toml) pins Rust 1.98.0 with the
  minimal profile plus rustfmt and Clippy. Run `rustup toolchain install` from
  the repository to install it, and `rustup show` to display the selection.
  Cargo commands in the root or member directories use this pin unless
  explicitly overridden. The workspace
  declares Rust 2024 in [`Cargo.toml`](../../Cargo.toml); the toolchain pin does
  not declare a minimum supported Rust version (`rust-version`).
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

### Linux CI

[`Linux CI`](../../.github/workflows/linux-ci.yml) runs on pushes to `main`,
pull requests and manual dispatch, with separate formatting, tests and Clippy
jobs on Ubuntu 24.04. Each job runs `rustup toolchain install --no-self-update`
after checkout to install the repository-pinned toolchain and components from
[`rust-toolchain.toml`](../../rust-toolchain.toml), without repeating the Rust
version in the workflow. CI disables incremental compilation, installs the
.NET 8 SDK for the real scanner helper build, and installs
`build-essential`, `pkg-config` and `libasound2-dev`. The audio dependency needs
ALSA headers to compile; the tests consume a controlled mixer without opening
an output device. SQLite uses the bundled library from `libsqlite3-sys`;
the selected HTTP dependencies use rustls, so no OpenSSL development package
is needed.

The locked checks cover core/scanner, SQLite repositories/services, CLI, server
with API docs on/off, player, and client library tests/Clippy with `mock` off/on.
Tests run with `--test-threads=1` to avoid the documented process-fixture races.
Ignored benchmarks and opt-in integration probes remain skipped.

Qt/QML and inactive Vizia builds are outside this minimal workflow. Qt requires
6.8+ and additional QML modules/plugins; add those checks only after validating
that setup reproducibly on a hosted runner. PostgreSQL integration, desktop/GPU
interaction, physical audio and Windows validation remain separate. Workspace
formatting still checks both frontends. `--locked` applies to Cargo builds/tests/
Clippy, not `cargo fmt` or the helper's NuGet restore.

## Environment and executable discovery

| Input | Current use | Source |
| --- | --- | --- |
| `OSU_LAZER_REALM_PARSER_PATH` | Build-time helper bypass and runtime helper override; prefer an absolute path. | [Build script](../../crates/radio-scanner/build.rs), [lazer process reader](../../crates/radio-scanner/src/lazer/scanner.rs) |
| `OSU_LAZER_REALM_PARSER_BUILT_PATH` | Set by the build script and embedded with `env!`; this is generated build output, not the normal user override. | [Lazer process reader](../../crates/radio-scanner/src/lazer/scanner.rs) |
| `DOTNET_CLI_HOME` | Used by the helper build when supplied; otherwise the build script uses `target/dotnet-home`. | [Build script](../../crates/radio-scanner/build.rs) |
| `OSU_RADIO_SERVER_ADDRESS` | Standalone default is `127.0.0.1:3000`; the supervisor supplies `ServerOptions::address`, default `127.0.0.1:0`. | [Server config](../../apps/osu-radio-server/src/config.rs), [supervisor](../../crates/osu-radio-client/src/server.rs) |
| `OSU_RADIO_SERVER_BIN` | Server executable override after explicit `ServerOptions::binary`, before sibling and `PATH` lookup. | [Supervisor](../../crates/osu-radio-client/src/server.rs) |
| `SQLITE_DATABASE_URL` / `POSTGRES_DATABASE_URL` | Required by SQLite / PostgreSQL CLI and server builds respectively. SQLite accepts paths, SQLite URLs and `:memory:`. | [Server config](../../apps/osu-radio-server/src/config.rs) |
| `.env` and process working directory | Server allows a missing `.env` when the selected database URL is in the process environment. Existing variables take precedence over file values; other I/O/parse errors are fatal. `ServerOptions::working_directory` selects where dotenv searches the child directory and its ancestors. | [Server config](../../apps/osu-radio-server/src/config.rs), [supervisor](../../crates/osu-radio-client/src/server.rs) |

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
persist it. Stable imports use the native database reader; both source kinds use
the same snapshot storage contract. For stable fixtures, pass an explicit
`--marker "/temporary/installation/osu!.db"`; custom `BeatmapDirectory` values
come from that fixture installation's `osu!.*.cfg`. Never use the workspace `.env`
or database for a storage smoke check: run the built CLI from a temporary working
directory with its own `.env` and disposable SQLite file, and remove inherited
database URL variables. Run `store` twice without `--clear` to check snapshot
replacement and shared audio references. Binary fixtures do not establish real
stable-library or Windows compatibility.

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
| Stable binary layouts, mapping, dispatch and media | `cargo test -p radio-scanner --locked stable::` | [Stable fixtures](../../crates/radio-scanner/src/stable/tests.rs): legacy/current boundaries, malformed input, grouping/IDs, BPM, custom paths and artwork. Wine mapping tests are Linux-only; real-library and Windows validation remain separate. |
| Scanner change spanning these paths | `cargo test -p radio-scanner --locked` | Combines the preceding scanner checks; helper build prerequisites still apply. |
| C# helper source | `dotnet build tools/osu-lazer-realm-parser/osu-lazer-realm-parser.csproj --configuration Release --nologo` | Compiles the producer; no automated C# test project currently exists. Follow the [scanner skill](../../.agents/skills/osu-radio-scanner/SKILL.md) for contract checks. |
| CLI wiring | `cargo test -p osu-radio-cli --locked` and `cargo clippy -p osu-radio-cli --all-targets --locked` | Argument tests reject removed `store --count` and retain `--clear`; memory SQLite connection check. No real source import. |
| Local audio engine | `cargo test -p osu-radio-player --locked` | MP3 CBR/VBR, Ogg/Vorbis and WAV decoding/seek with controlled mixer consumption; no physical device. |
| Persisted individual/global volume | Repository/service checks on SQLite/PostgreSQL, both server router variants, client mock off/on and Qt offscreen/lint | Verify defaults, upgrade/reimport, shared audio, A/B selection, debounce/retry/stale replies, volume before play and shutdown flush. Physical output and Linux/Windows slider checks remain manual. |
| Persisted playback queue and assignment protocol | Repository/service tests on each backend, server tests with and without `docs`, client tests with and without `mock`, Qt tests and QML lint below | Use disposable databases; verify recovery, duplicates, stale callbacks/downloads, reconnect and current metadata outside search. Physical output and Windows remain separate. |
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

The server commands also run isolated
[configuration regressions](../../apps/osu-radio-server/src/config/tests.rs) and
an actual [environment-only startup test](../../apps/osu-radio-server/tests/startup.rs).
They use child-only environment variables, temporary directories, in-memory SQLite
and an ephemeral loopback port; no configured application database is opened.
Configuration probes run through an ignored helper test invoked explicitly by
their parent tests. The helper is not an opt-in database integration test.
The read-error fixture uses invalid UTF-8 so it remains effective when run as root.
To check the PostgreSQL-specific configuration without connecting to a database:

```sh
cargo test -p osu-radio-server --no-default-features --features postgres --locked config::tests::
```

Executed on Linux on 2026-10-07 for optional server dotenv loading: locked
SQLite server suites passed with docs enabled/disabled (44/39 unit tests plus
one actual-server environment-only startup test in each build). The seven
configuration regressions also passed in parallel and with PostgreSQL selected,
without a PostgreSQL connection. The existing benchmark stayed ignored; the
ignored configuration helper ran through its parent tests. Server all-target
Clippy with warnings denied passed in both SQLite docs states, as did workspace
formatting, whitespace and affected guide file links. Rust 1.98.0 was used.
The .NET SDK failed to read process metadata in this environment, so these
headless checks used the documented helper build override with `/bin/false`.
Real Realm extraction and PostgreSQL integration were not exercised.

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
Saved pre-fix binaries may still require a directory with `.env`; the current
server accepts the database URL from process environment without that file.

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
| `Failed to load .env` | Inspect the discovered file in the actual child's working directory or ancestors for parse/read errors. Only `NotFound` is ignored; an existing broken file is fatal even with environment configuration. |
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
QuickControls2, Network), QML Basic Controls, Layouts, Effects and Dialogs modules, SVG/image
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

Default launch is live: first build `osu-radio-server` and supply the selected
database URL in the environment or a loadable `.env`. `--component-gallery` is offline and requires
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
Settings checks activate its navigation tab, return through Songs and Playlists,
retain the selected track and independent queries, and exercise the existing audio
and folder retry controls. The gallery checks that Settings remains disabled.
The `settings_search_filters_visible_labels_and_preserves_layout_and_state`
smoke test uses the `settings-search` probe case: real keyboard edits/clearing,
case-insensitive title/name filtering, whitespace, no-match recovery, conditional
global volume, hidden-row spacing, long wrapped status scroll/reset and retained
query/selection across tabs. Its isolated HTTP fixture also checks that Settings
search never sends a library search query. Run it alone with
`cargo test -p osu-radio-qt --locked --test launch_smoke settings_search_filters_visible_labels_and_preserves_layout_and_state`.
The [media controller tests](../../crates/osu-radio-client/src/controller.rs)
cover replacement of visible demand, canceled HTTP, selected/queue demand,
independent cover and duration completion, stale/repeated acknowledgments and
shutdown. The [artwork continuity probe](../../apps/osu-radio-qt/tests/VisualProbe.qml)
uses the `visual-flicker` fixture: a slow selected duration, duplicate playlist
items, warmed navigation, rapid scrolling and more than 64 MiB of visible 1280px
RGBA images. It checks actual `Image.Ready` states, player/background URL
continuity and unchanged queue delegates. The
[smoke test](../../apps/osu-radio-qt/tests/launch_smoke.rs) also rejects repeated
selected-cover/duration requests and stationary cache-refill loops. Native
[cache/model checks](../../apps/osu-radio-qt/src/native_tests.h) verify the byte
budget, protected cover, installation versions and stale epochs separately.
`OSU_RADIO_QT_SMOKE_TEST=1` is an internal test hook; do not set it interactively.
Test roots stay hidden until the probe configures their screen and geometry.
`OSU_RADIO_QT_PROBE_SCREEN`, `OSU_RADIO_QT_PROBE_WIDTH` and
`OSU_RADIO_QT_PROBE_HEIGHT` affect only this test launch. Screen-targeted desktop
probes require `QT_QPA_PLATFORM=xcb`: Wayland leaves window placement to the
compositor. The probe checks the actual screen/geometry after showing the window.
Probe steps restart their timer after input completes; QtTest input may process
events, so a repeating poll could reenter an unfinished keyboard step.
Help/error paths run with an invalid platform plugin to verify no GUI initialization.
The Unix fixture requires Python 3; keyboard/backdrop probes also require the QtTest QML module. For real SQLite backend coverage:

```sh
cargo build -p osu-radio-server -p osu-radio-qt --locked
cargo test -p osu-radio-qt --locked --test launch_smoke real_backend_uses_a_disposable_database -- --ignored
cargo test -p osu-radio-qt --locked --test launch_smoke real_backend_playlists_use_a_disposable_database -- --ignored
```

These tests remove inherited database URL variables and use temporary `.env`
and SQLite files. They never open the workspace database or run discovery.
The playlist probe seeds synthetic rows after production migrations, then checks
creation, renaming, deletion, concrete difficulty checkboxes, idempotent addition,
row removal and the unavailable-item presentation through the real server/client
and Qt adapter. It deliberately does not start physical audio.

Run [`scripts/lint-qml.sh`](../../apps/osu-radio-qt/scripts/lint-qml.sh) after
building. Set `QMLLINT=/path/to/qmllint` if needed (default `/usr/lib/qt6/bin/qmllint`).
The script stages generated `OsuRadio/qmldir` and `plugin.qmltypes` with the QML
source tree in a temporary import directory, then lints every app/probe QML file.
It requires Bash, Python 3 and ripgrep. Generated QObject type information is
necessary for `Store.qml`; linting only source paths cannot resolve `MockBridge`.

Queue-panel verification executed on Linux on 2026-10-01: SQLite and disposable
PostgreSQL service contracts passed, including ordered/duplicate pending entries,
empty/exhausted queues and deleted/Online source filtering. Server tests passed
with `docs` on/off (37/32; one benchmark ignored each). Client tests passed with
`mock` off/on (78/87; two benchmarks ignored each). Qt passed 18 ordinary tests
(two disposable real-backend probes remained ignored); the queue probe checks
retry, metadata/media outside search, live Next, long-list scrolling, empty state,
Escape/close/outside dismissal and focus restoration. Debug server/Qt builds,
scoped all-target Clippy with Rust warnings denied, Vizia compatibility, formatting
and generated-import QML lint passed. Queue visual probes passed at 1440×952 and
1024×640 through offscreen software and a separate xcb/OpenGL llvmpipe window.
Only native rendering establishes artwork/mask appearance; software rendering
omits shader effects. Visual data was synthetic; physical audio and Windows
remain unverified.

After moving the queue button into the title bar immediately before the window
controls, all 18 ordinary Qt tests and generated-import QML lint passed again.
The queue probe checks that placement and opens the panel with a mouse click.

Artwork continuity verification executed on Linux on 2026-10-01 UTC: client
tests passed with `mock` off/on (88/97; two benchmarks ignored each), and all
19 ordinary Qt tests passed (two real-backend probes ignored). Fixture suites
were run with `-- --test-threads=1`: parallel runs had intermittent process
fixture failures and probe timing failures. The final split native helper was
also rerun successfully. Locked Qt build, scoped all-target Clippy with Rust
warnings denied, Vizia release compatibility, formatting, generated-import QML
lint and local guide links passed. Separate xcb/OpenGL windows used the AMD
Radeon RX 6750 XT at 1440×952 and 1024×640. Each captured ten navigation frames
with identical player-artwork pixels and identical pixels in a sampled backdrop
region. Both probes kept 21 distinct 1280px RGBA covers visibly ready under
cache pressure; the stationary checkpoints produced no additional media HTTP
requests. Selected artwork preceded the delayed duration, and navigation fetched
the selected cover/duration once. These are synthetic Linux checks; the user's
real library, physical audio and Windows remain separate validation.

The user performs desktop visual/input checks: the existing gallery matrix above
applies to Qt too, with 1024×640 through 2560×1440 and 100/150/200% scaling.
Check crop/cover changes, typography, blur, scrolling, keyboard navigation,
menu and modal focus restoration/trapping, disabled controls and native window
move/resize/minimize/maximize/close. The automated software smoke tests cannot
establish these results, physical playback or Windows compatibility.

For an explicitly authorized visual audit, the [window-local visual probe](../../apps/osu-radio-qt/tests/VisualProbe.qml) can
save Songs/Settings/player, playlist/editor/menu/modal and gallery screenshots:

```sh
python3 apps/osu-radio-qt/scripts/visual-audit.py /tmp/osu-radio-visual
# Use the confirmed output name and sizes/scales that fit that physical screen.
python3 apps/osu-radio-qt/scripts/visual-audit.py /tmp/osu-radio-gpu --platform xcb --screen DP-2 --sizes 1024x640 --scales 1 1.5 2
python3 apps/osu-radio-qt/scripts/visual-audit.py /tmp/osu-radio-flicker --case visual-flicker --platform xcb --screen DP-2 --sizes 1024x640 1440x952 --scales 1
```

The default runs four logical sizes (1024x640, 1440x952, 1920x1080, 2560x1440)
at 100/150/200%, with isolated fixture processes, temporary working directories
and no workspace database. The fixture returns an audio error before decoding
or opening an output device. Screenshots and `results.json` accompany each log.
The script injects input only into its Qt windows and does not move the desktop
cursor. GPU runs remain necessary to inspect blur, shadows and rounded masks;
the offscreen software renderer cannot establish those effects. Native window
operations and cover-picker cancellation have a separate internal `native` probe;
the folder chooser, physical drag/resize and Windows require separate checks.

## Audio playback verification

Playback uses the client-owned worker and Rodio 0.22.2 with `playback`, `mp3`, `vorbis` and `wav`.
Linux builds need the native audio development libraries required by CPAL/ALSA.
Device initialization occurs on the first playing assignment; galleries and normal
startup do not require an output device. The versioned queue migration preserves
the existing library. Server startup restores queue position on pause; no reset is needed.

Run the player tests, client tests with and without `mock`, server tests with and
without `docs`, and both frontend checks listed above. Scope Clippy to these five
packages, testing the docs-disabled server separately. Never select all database
features together. Audio tests must consume decoded data with a controlled mixer,
not open the machine's physical device. HTTP tests use disposable sources and
databases. They verify bytes, status codes, bounded downloads and cancellation;
controller tests cover selection independence, stale requests and cleanup.

Manual acceptance remains separate: play MP3, Ogg/Vorbis and WAV files, select another row while audio
continues, explicitly play that row, pause/resume, seek forward/backward and after
EOF, adjust global volume, and close during a download. Enqueue several tracks
through the API, check automatic advance and Qt Next/Previous while playing and
paused, then restart the server and resume from the restored queue position.
Test slider dragging without tick interference in Qt. Repeat physical playback
and lifecycle checks on Windows. Queue-list UI, shuffle, repeat, device selection
and volume persistence across launches remain deferred.

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

Executed on Linux on 2026-10-01 for the server queue: SQLite repository and
service suites passed (5 and 12 tests; two service benchmarks ignored), including
the populated-library migration, reopen, rollback and independent-pool queue
contracts. PostgreSQL repository/service contracts passed on separate fresh
temporary databases; service contracts were repeated after recovery and
token-bound device-pause fixes. Server suites passed with docs enabled/disabled
(29/26, one benchmark ignored in each), then the added conditional-pause regression
passed in both routers and the OpenAPI test was repeated. Client suites passed
without/with `mock` (50/56, two benchmarks ignored in each). Player tests passed
(6), CLI tests passed (2), and Qt passed its 13 ordinary tests, including eight
offscreen scenarios, plus its normally ignored real-backend startup probe on
disposable SQLite. A final actual-server HTTP smoke passed 30 requests covering
NDJSON, insertion/history, callbacks, duplicate IDs, clear and process restart.
The independent source review's confirmed races were fixed and rechecked.

Server/Qt builds, scoped all-target Clippy on SQLite and both PostgreSQL router
configurations, generated-import QML lint, formatting and affected guide paths
passed. Loopback/process checks ran outside the sandbox. Native Qt headers emit
a compiler warning; Rust Clippy with warnings denied passed. These checks do
not establish physical audio, desktop input/GPU rendering or Windows behavior.
Queue-list UI, shuffle and repeat were not added; the gallery remains offline.

## Playlist verification

Run the SQLite/PostgreSQL repository and service contracts separately, both
server router configurations, client tests with `mock` off/on, the Qt suites and
the disposable real-backend playlist probe above. Follow the existing backend
matrix for scoped Clippy; include `osu-radio-qt` and generated-import QML lint.
Never point these contracts at the configured application database.

[`Repository tests`](../../crates/radio-db/src/tests/playlists.rs) cover upgrading
an existing library/queue without reset and cascade boundaries.
[`Service tests`](../../crates/radio-services/src/tests/playlists.rs) cover
CRUD/reopen, atomic/idempotent adds across independent pools, unavailable items,
reimport with new IDs, folder deletion/restoration, copy selection, batches larger
than 500 items, same-audio difficulties, queue replacement/snapshot semantics,
selected start and stale playback callbacks. Both routers exercise the
[`HTTP contract`](../../apps/osu-radio-server/src/routes/playlists/tests.rs).
Client and Qt tests additionally cover stale view responses, independent
selection, item-specific pause/resume, retained retry choices and offline gallery
actions.

Manual acceptance remains separate: create/edit/remove playlists through desktop
input, add only chosen difficulties, switch between library and playlist, select
two difficulties sharing audio and explicitly play each, check Pause/Resume and
Next/Previous, remove/reimport a folder and restart the application. Repeat on
Windows and check actual audio output. Offscreen software rendering and isolated
backend checks do not establish these platform/device results. Collection import,
manual reordering, descriptions and collection synchronization
remain deferred.

Executed on Linux on 2026-10-01 for user playlists: SQLite repository/service
suites passed (5/12 tests; two service benchmarks ignored), and PostgreSQL
repository/service suites passed (4/1 tests) on separate disposable databases.
Both server router suites passed (33 with docs, 29 without; serialization
benchmarks ignored). Client library suites passed without/with `mock` (54/61;
two benchmarks ignored in each). Qt passed its 13 ordinary tests, then the real
server playlist probe passed on temporary SQLite, including adding from a
playlist whose song is absent from the active library search. Server/Qt builds,
Vizia compatibility check, scoped all-target Clippy with warnings denied,
generated-import QML lint, formatting and affected guide links passed. Native
Qt headers still emit their existing C++ compiler warning. Loopback/process
checks ran outside the restricted sandbox; physical audio, desktop input/GPU
rendering and Windows were not exercised.

The separate Playlists tab and covers add the `playlist-covers` fixture probe
in [launch_smoke.rs](../../apps/osu-radio-qt/tests/launch_smoke.rs). It checks the
inline editor's create-success/upload-failure retry against the same ID, retains
prepared bytes after the source file is deleted, cancels/stales picker results,
loads a saved custom cover and resets it. The real-backend playlist probe checks
tab restoration, independent search, counts, editing and keyboard context removal.
Native [image checks](../../apps/osu-radio-qt/src/native_tests.h) exercise center
crop, JPEG EXIF orientation, invalid/oversized input and separate cache versions.

Manual acceptance also covers native image choice, replacing/resetting covers,
application restart after deleting the source image, keyboard traversal and
1024×640 through 2560×1440 at 100/150/200% scaling. Compare the gallery's list
and detail screens with the supplied designs; offscreen software checks establish
neither desktop/GPU rendering nor Windows behavior.

Executed on Linux on 2026-10-01 for the separate Playlists tab and covers:
SQLite repository/service suites passed (5/12), PostgreSQL suites passed (4/1)
on two fresh disposable databases, and both HTTP configurations passed
(37 with docs, 32 without; serialization benchmarks ignored). Client suites
passed without/with `mock` (76/85; two benchmarks ignored each). Qt passed all
18 tests including both real-backend probes, the image/EXIF/cache checks,
offline gallery and create/upload retry after source-file deletion. The final
read-only review found no remaining confirmed issue. SQLite/PostgreSQL and Qt
all-target Clippy with warnings denied, Vizia compatibility, formatting,
generated-import `qmllint` and 342 local guide paths passed. Two offscreen
gallery screenshots were inspected for list/detail geometry; the original
reference images were not supplied in this context, so exact comparison was
not performed. Desktop input, native picker appearance, GPU effects, scaling,
physical audio and Windows remain unverified. Qt's existing C++ header warning
still appears during compilation.

## Sorting and listening-history verification

Use the repository/backend and Qt matrices above: SQLite and fresh disposable
PostgreSQL separately, server `docs` on/off, client `mock` on/off, Qt offscreen
and disposable real-backend probes, Vizia compatibility, scoped Clippy, QML lint,
formatting and guide-link checks. The
[repository history checks](../../crates/radio-db/src/tests/listening_history.rs)
cover a populated upgrade, source identity, rollback, batched reads and reset;
[service history checks](../../crates/radio-services/src/tests/listening_history.rs)
cover durable dates, source removal/return, queue clear and token acknowledgements.
[Client sorting checks](../../crates/osu-radio-client/src/controller/sorting/tests.rs)
cover display keys, late responses and media-preserving reorder; worker and queue
tests cover successful start, failures and callback retries. Qt model and QML
probes exercise item identity, cache retention and menu keyboard controls.

Physical sound, desktop input/rendering and Windows remain manual checks. Play
from the library, a search and a sorted playlist; check recent ordering after
start, pause/resume, Stop/Play, automatic advance, duplicate audio difficulties
and application restart. Verify that sorting changes display order while playlist
playback follows saved order, then remove/reimport a folder and check its dates.

Executed on Linux on 2026-10-01 for sorting and listening history: SQLite
repository/service suites passed (5/12), and PostgreSQL suites passed (4/1) on
separate fresh disposable databases. Server suites passed with `docs` on/off
(34/30), and client suites passed with `mock` off/on (62/70); existing timing
benchmarks remained ignored. Qt passed 14 ordinary tests and both disposable
actual-server probes; the final native model regression was repeated after lint
fixes. CLI passed its two tests, Vizia compatibility compiled, and server/Qt
debug builds passed. Scoped all-target Clippy with Rust warnings denied passed
for SQLite and both PostgreSQL router configurations. Generated-import QML lint,
workspace formatting, whitespace and 324 local guide links/anchors passed.
An independent read-only review found no actionable issues. Loopback/process
checks ran outside the restricted sandbox; native Qt headers retain their existing
C++ compiler warning. Physical audio, desktop input/rendering and Windows were
not exercised.

## Qt folder modal verification

The folder-selection change adds isolated tests in
[client stream tests](../../crates/osu-radio-client/src/api/folder_tests.rs),
[controller tests](../../crates/osu-radio-client/src/controller/folders_tests.rs),
[service tests](../../crates/radio-services/src/tests/folder_selection.rs),
[route tests](../../apps/osu-radio-server/src/routes/folder_selection_tests.rs),
and the [Qt probe](../../apps/osu-radio-qt/tests/AdapterProbe.qml). They use explicit
discovery roots, temporary source files/databases, and an isolated fixture process.
The Qt gallery remains offline. No application database is used for these checks.

Manual acceptance remains separate: check live GPU scene blur, native chooser and
browser launch, long real installation paths, desktop input/scaling and Windows.
Automated Qt software rendering verifies modal focus/dismissal, responsive geometry,
registered/pending/error row states and sequential partial-success retry, not those
platform integrations. Counts read metadata only; real Lazer Realm compatibility is
separate from fixture tests.

Executed on Linux on 2026-09-28: client tests without/with `mock`, SQLite
service tests, server tests with and without `docs`, Qt tests/build, the actual
backend Qt startup probe on a disposable SQLite database, Vizia tests, generated-import
QML lint, scoped all-target Clippy and formatting passed. The folder tests cover
byte/chunk-split NDJSON, incremental delivery and response cancellation, per-opening
stale-result rejection, unique versus ambiguous scoped selection, alias/ID matching,
zero and individual-difficulty counts, count errors/retry, staged toggles, one-hour
timeout overrides, duplicate imports, forced mid-import rollback, shared-row removal,
and sequential partial-success retry. The gallery keyboard probes cover Tab/Shift+Tab
containment, focus restoration, Escape and backdrop dismissal, row states and
1024x640 geometry. A software-rendered folder-modal screenshot was inspected; this
does not verify live GPU blur, native picker/browser behavior, actual Lazer Realm
reading, Windows interaction or PostgreSQL execution. Existing benchmark tests
remained ignored.

## Qt Settings tab verification

Executed on Linux on 2026-10-01: `cargo test -p osu-radio-qt --locked` passed
18 tests; the two opt-in real-backend tests remained ignored. Generated-import
QML lint and whitespace checks passed. Loopback fixtures required execution
outside the restricted sandbox. The existing visual-audit script passed live
fixture and offline gallery checks at 1440×952 and 1024×640, at scale 1.
Settings screenshots were inspected, including wrapped folder options and a long
audio status; the probe checks vertical scrolling, Tab traversal into the folder
menu and Escape focus restoration. These checks use software rendering and
temporary fixtures; desktop/GPU interaction and Windows remain unverified.
Figma metadata was available, but its Starter tool limit blocked the source
screenshot and asset export. Exact visual comparison and exported-icon equality
were therefore not verified; icon origins are recorded in
[asset sources](../../apps/osu-radio-qt/assets/SOURCES.md).

## Unicode name preferences verification

Use the existing SQLite/PostgreSQL, server `docs` on/off, client `mock` on/off,
Qt offscreen/lint and scoped Clippy matrix above. The
[client preference tests](../../crates/osu-radio-client/src/controller/preferences/tests.rs)
cover all four title/artist combinations, whitespace fallback, retained suffixes,
late responses, display sorting, pending playlist captions and unchanged media.
[Playlist contracts](../../crates/radio-db/src/tests/playlists.rs) and
[service contracts](../../crates/radio-services/src/tests/playlists.rs) cover the
Unicode migration/backfill, source removal/return, snapshot refresh and rollback.
The [Qt probe](../../apps/osu-radio-qt/tests/launch_smoke.rs) isolates Linux native
settings with temporary `XDG_CONFIG_HOME`: two launches, changed-key writes,
rapid toggles, Space activation, reconnect and read/write errors. The gallery
remains memory-only.

Executed on Linux on 2026-10-03: SQLite repository/service suites passed (5/12;
two service benchmarks ignored), and PostgreSQL suites passed (4/1) on separate
fresh disposable databases. Server suites passed with `docs` on/off (37/32;
one benchmark ignored each); client suites passed with `mock` off/on (93/102;
two benchmarks ignored each). Qt passed five unit tests, fifteen ordinary
offscreen integration tests and both disposable real-backend probes. Scoped
all-target Clippy with Rust warnings denied passed for SQLite, both PostgreSQL
server configurations, the mock client and Qt. The fresh SQLite server build,
Vizia release compatibility, generated-import QML lint, formatting, whitespace
and affected local guide paths passed. Independent implementation and focused
Settings-layout reviews found no actionable production defect. Loopback/process
checks ran outside the restricted sandbox; native Qt headers retain their
existing C++ compiler warning.

Offscreen Space activation does not establish desktop keyboard/wheel scrolling,
native rendering, physical playback or Windows storage behavior. Those remain
manual checks. Older unavailable playlist entries without retained metadata
cannot recover Unicode names until a matching source is imported again.

## osu! collection import verification

Use the scoped scanner, SQLite/PostgreSQL, server `docs` on/off, CLI,
client `mock` on/off and serial Qt matrices above. All collection fixtures
and persistence checks use temporary sources and databases. The
[stable binary fixtures](../../crates/radio-scanner/src/stable/tests.rs)
exercise empty/duplicate/Unicode names, missing maps, repeated MD5 and corrupt
collection data. The [lazer checks](../../crates/radio-scanner/src/lazer/tests.rs)
generate genuine Realm files with the
[fixture generator](../../tools/osu-lazer-realm-parser/fixture-generator/Program.cs)
and run both the production helper and Rust importer, including distinct native
`Hash`/`MD5Hash`, an absent collection type and malformed required fields.

[Repository contracts](../../crates/radio-db/src/tests/collections.rs) cover
migration 11, historical native hashes and guarded downgrade. Shared
[service contracts](../../crates/radio-services/src/tests/collections.rs) cover
repeat import, preserved item IDs/order/custom covers, overwritten manual edits,
missing/restored maps, source removal/return and rollback after collection writes.
[Folder checks](../../crates/radio-services/src/tests/folder_selection.rs) cover
bad source reads and concurrent settings/deletion across the read boundary.
[Client checks](../../crates/osu-radio-client/src/controller/folders_tests.rs)
exercise staged refresh and independent playlist reload after a library error;
playlist tests cover stale responses during a concurrent mutation. The
[Qt probe](../../apps/osu-radio-qt/tests/AdapterProbe.qml) activates the refresh
control and checks its staged action through HTTP using an isolated backend fixture.

These checks do not exercise a user's game installation, physical audio,
desktop pointer/rendering behavior or Windows. Real-library and native-platform
acceptance remain separate from temporary Realm and offscreen fixture evidence.

Executed on Linux on 2026-10-04: core/scanner suites passed (2/48), including
the genuine temporary Realm production-helper/importer checks. SQLite repository
and service suites passed (5/14; two service benchmarks ignored); PostgreSQL
passed (4/1) on separate fresh disposable databases, and its test cluster was
stopped and removed. Both server router configurations passed (37/32; one
benchmark ignored each), CLI passed two tests, and client `mock` off/on passed
(96/105; two benchmarks ignored each). Qt passed five unit and fifteen ordinary
serial offscreen integration tests plus both disposable real-backend probes.
The folder fixture was corrected to preserve the stable marker across discovery,
import and list responses and to finish metadata work before reporting completion;
the serial rerun retained the exact two-worker concurrency assertion.

Scoped all-target Clippy with Rust warnings denied passed for scanner/core,
SQLite repositories/services/CLI/server, client/Qt and both PostgreSQL server
configurations. Server/Qt debug builds, Vizia release compatibility, generated
QML lint, workspace formatting, whitespace and 426 local guide paths passed.
Independent source reviews found no actionable correctness issue. The refresh
control was exercised through native Space activation; desktop pointer behavior,
GPU rendering, Windows and real game installations were not exercised. Existing
Qt native-header compiler warnings remain.
