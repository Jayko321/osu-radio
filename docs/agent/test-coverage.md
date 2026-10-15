# Repository test coverage worklist

Every file in this inventory has explicit assertion/consumer test evidence.
The goal is **tests for every repository file**. This inventory contains
320 tracked/nonignored worktree files as of 2026-10-15; new files must be
added when the inventory is refreshed. A passing suite, compilation, a test module
or a filename match alone does not establish complete behavioral coverage.

For executable code, inspect its branches and callers and connect assertions to
observable behavior. Files containing only declarations are exercised through
their consumers. Assets, fixtures, configuration and prose need appropriate
consumer/static checks rather than artificial unit tests. Test infrastructure also
needs review; executing it does not prove its assertions are strong.

## Executed focused checks

- `cargo test -p osu-radio-cli -p radio-core --locked --offline -j 1`:
  15 CLI unit tests, 11 actual-CLI subprocess scenarios and seven core tests
  passed on Linux. Five core tests and 24 CLI tests are new (29 total).
- CLI fixture discovery always uses explicit temporary roots/markers. Child
  processes use a private working directory and their own `.env`, with inherited
  database/helper variables removed. SQLite assertions reopen only that fixture's
  database through `Services`.
- The focused tests read synthetic stable databases and inert audio/artwork
  bytes. They prove reference/persistence behavior, not decoding or physical sound.
- The broader Linux baseline passed 279 tests (seven ignored):
  `cargo test -p radio-core -p radio-scanner -p radio-db -p radio-services -p osu-radio-cli -p osu-radio-server -p osu-radio-player -p osu-radio-client --locked --offline -j 1 -- --test-threads=1`.
  This includes 48 scanner tests with the production helper on synthetic Realm,
  128 client tests, 44 server unit tests and one server startup check, using
  default SQLite/docs features and no client `mock` feature. Socket fixtures
  required execution outside the sandbox after sandbox `bind` failures.
  Passing this baseline does not complete the per-file assertion audit below.
- `cargo test -p osu-radio-cli --no-default-features --features postgres --locked --offline -j 1`:
  14 unit tests and eight storage-independent subprocess scenarios passed. This
  checks PostgreSQL feature compilation, not a live PostgreSQL connection or store.
- `cargo clippy -p osu-radio-cli -p radio-core --all-targets --locked --offline -j 1 -- -D warnings`,
  workspace formatting and whitespace checks passed. The inventory matches all
  current tracked/nonignored files, and local file targets in the affected
  agent guides exist. These checks preceded the scoped measurement below.

Entries marked **focused** below have the specific assertion evidence described
in the last column. Observed lines are a separate measurement; neither column
claims that every possible behavior/platform is covered. **Pending** means the file still needs an
explicit assertion/consumer audit, even if existing tests already exercise it.

## Scoped measurements (2026-10-15)

The observed-line column below combines Linux SQLite/PostgreSQL repository and
service runs, SQLite server `docs` off/on, client `mock` off/on and the Qt
unit/offscreen/backend probes. Current raw JSON is `target/coverage/workspace-linux.json`;
earlier snapshots remain at `target/coverage/headless-sqlite-postgres.json` and
`target/coverage/headless-sqlite.json`. Benchmark Python trace output is in
`target/coverage/python-summary.txt` and `target/coverage/python/`. These generated
artifacts are ignored; the table preserves snapshots for 108 Rust/build-script files.

**Limits:** LLVM line counts include inline test code and ignored benchmark
bodies. A dash means absent from this scoped report (including declarations,
generated mappings and unmeasured languages/features), not proof of zero tests.
A line hit does not prove assertion quality or branch coverage. The successful
server probe is force-killed and cannot flush a complete process profile; its
readiness assertions still run. QML and C++ native files have probe evidence but
no LLVM source-line instrumentation here. PostgreSQL HTTP routes, desktop/GPU,
physical sound and native Windows execution remain separate. No whole-repository
coverage percentage is claimed.

Executed in the first measurement stage:

- The eight-package default headless run under coverage passed 293 tests:
  CLI 15 unit + 12 subprocess, client 128, player 6, server 44 unit + 1 startup,
  core 7, repositories 5, scanner 61 and services 14; seven ignored.
- Client `mock`: 137 library tests passed, two ignored.
- Server without `docs`: 39 unit + four startup tests passed, two ignored.
  All four startup tests also passed with `docs` enabled.
- Scanner, CLI and server all-target Clippy with denied warnings passed.
  The eight benchmark unittest cases passed and execute all 57 measured script
  lines with mocked HTTP/processes. They are included in Linux CI.
- This continuation adds 17 Rust and eight Python tests (25 total).
  Together with the prior 29 additions, the goal has 54 new tests so far.

### Database continuation (2026-10-15)

- Added five Rust tests: two source-identity checks, two actual SQLite connection
  checks and one SQLite migration-lifecycle entry. PostgreSQL invokes that same
  lifecycle contract inside its existing guarded repository test.
- SQLite coverage run: 10 repository tests and 14 service tests passed (two
  service benchmarks ignored). Clippy for radio-db all targets passed separately
  with SQLite and PostgreSQL, with denied warnings.
- PostgreSQL 18.6: six repository tests and the full service contract passed on
  separate fresh `radio_db_test_repositories` / `radio_db_test_services` databases.
  A private temporary cluster listened only on loopback at an allocated port.
  `target/coverage/postgres-run.json` records both suite exits and confirmed
  removal; `postgres-run.log` / `postgres-server.log` preserve the run evidence.
  The test cluster was stopped and its directory removed in the runner's `finally`.
- This brings the goal to 59 new tests. Product behavior/schema/dependencies are
  unchanged. Qt/native checks were outside that stage; the following stage adds
  offscreen/native-fixture evidence. PostgreSQL server routes and Windows remain separate.

### Qt, resources and verification tools (2026-10-15)

- Added 14 tests: one Rust runtime lifecycle contract, one embedded-resource Qt
  probe, and 12 Python checks for the resource manifest and verification tools.
  This brings the goal to 73 new tests.
- Qt 6.12.0/Rust 1.98.0: six library/binary unit tests and 18 ordinary serial
  offscreen probes passed under coverage. Both opt-in actual SQLite-backend
  probes also passed on private databases. No real library or output device is used.
- The offline asset probe decodes all 36 bundled SVG/PNG/TTF resources, checking
  image readiness/dimensions and font family names. A manifest assertion fails
  when a resource is added without a decode entry.
- Four real software-rendered live/gallery visual scenarios passed at 1024x640
  and 1440x952, scale 1. Logs/screenshots/results are in
  `target/coverage/qt-visual/`. This does not establish desktop/GPU rendering.
- Tool tests reproduced two issues before their fixes: failed file discovery
  returned a successful empty QML lint, and visual-audit inherited native settings.
  QML discovery now propagates pipeline failure; visual-audit uses a private
  `XDG_CONFIG_HOME`. The 12 tool tests, real generated-import QML lint, Bash syntax
  checks, formatting and whitespace checks passed. The Python tests are in CI.
- Qt all-target Clippy with denied Rust warnings passed on the current Qt 6.12
  build, without repository compiler/dependency changes.
- Python trace exercised all 70 counted visual-audit script lines;
  `target/coverage/qt-python-summary.txt` and `target/coverage/qt-python/` contain
  that scoped trace. Mock tool tests do not validate desktop input or real process
  group termination. Rust line coverage still excludes C++ and QML source lines.

### Scanner defaults and repository metadata (2026-10-15)

- Added seven Rust scanner tests, five POSIX build-script subprocess tests and
  seven repository metadata/parser checks: 19 additions, 92 new tests in total.
- The 68 Rust scanner tests passed under coverage, including the production
  helper on synthetic Realm. Scanner all-target Clippy with denied warnings passed.
  Linux candidate generation used only private HOME/XDG directories, including
  unset/empty XDG and absent HOME; no full-machine discovery was run.
  Canonical aliases preserve the first readable marker path, symlinked files/trees
  are skipped by sweeps, and missing roots do not prevent scanning a valid root.
  The runtime-helper compatibility APIs validate collections before returning
  set-only results. Stable collection/configuration read errors abort with context
  and leave source fixture bytes/directories unchanged.
- Build-script tests compile the unchanged `build.rs` using `rustc` and isolate
  PATH/.NET home. The fake executable records exact arguments/environment; tests
  assert Cargo directives, prebuilt override bypass, missing/failed `dotnet` and
  success without an output executable. Five tests passed; no real .NET build is
  claimed for this fake-process suite.
- Metadata checks cover local Markdown files and heading anchors, Cargo workspace
  membership/path dependencies/lock entries, declared targets, ignore rules and
  one inventory row per repository file. Two parser checks cover fences and
  heading slug/duplicate handling. They do not validate prose facts, external
  URLs or all Markdown syntax. Both new Python suites are included in Linux CI;
  hosted execution has not been observed.
- `__pycache__/` is now ignored to avoid adding generated Python caches. One cache
  was already tracked before this task; it remains in the inventory and on disk.
  Product behavior, schema and dependencies remain unchanged in this stage.

### Real Realm producer continuation (2026-10-15)

- Added four Rust tests and strengthened the existing genuine-Realm check:
  96 new tests across the goal. Minimal fixtures retain the absent-field schema;
  a separate mapped rich schema exercises the production dynamic reader.
- All 72 scanner tests passed under Rust coverage. The five `real_` cases also
  passed after adding boundary assertions for two-character hashes, a null author
  and object-list schema links. They build the generator in temporary directories
  and run the actual production helper; no configured game files are opened.
- Rich export checks title/artist variants, author fields, ordered user tags,
  float BPM, negative preview time, Int32 overflow, missing metadata and file/hash
  links. Null/empty/one-character hashes have no path; two characters suffice.
  Realm bytes are unchanged after export/schema checks, and no media is created.
- Direct helper checks cover usage/missing/directory/corrupt inputs, exit codes
  2/3/4, stderr-only failure diagnostics, sorted schema NDJSON, scalar/list/set/
  dictionary types, nullable values, object links, primary keys and indexes. A
  schema without BeatmapSet can be inspected but export rejects it.
- Full production-helper Release rebuild succeeded with zero errors. Its existing
  NuGet assets reported NU1900 (vulnerability endpoint unavailable); Fody also
  reported the expected empty default schema for this dynamic-reader-only project.
  These warnings do not establish a current vulnerability audit.
- C# line instrumentation, every possible Realm property/schema type, real-library
  compatibility and native Windows/macOS execution remain unverified. Product
  helper/protocol/dependencies are unchanged; only fixtures/tests/docs changed.

### Persisted declaration/consumer audit (2026-10-15)

- Inspected the plain models and ORM entity declarations against their concrete
  repository/service assertions. No implementation change or artificial tests were
  needed for the declarations marked focused in this stage.
- Re-ran SQLite coverage contracts: 10 repository tests and 14 service tests passed;
  two service timing benchmarks remained ignored. Fixtures use temporary databases.
  Assertions cover omitted/null/empty folder patches, native path identity, queue
  state round trips/rollback, tag normalization/link constraints, settings defaults
  and bounds, source-keyed volume/history and transaction snapshot consistency.
- Entity/model module declarations are covered by these consumers rather than
  source-line instrumentation. Beatmap/metadata/search and playlist projections
  remain pending their field-by-field assertion audit. PostgreSQL results from the
  earlier shared-contract run remain recorded separately; it was not rerun here.

### Persisted field/projection continuation (2026-10-15)

- Added a shared field-projection contract inside the existing SQLite/PostgreSQL
  repository suite, plus one pure LibraryTrack regression test. The goal now
  has 97 new entry tests; persisted projection cases strengthen existing tests.
- Explicit expectations cover all BeatmapMetadata, BeatmapSet and Beatmap fields,
  SearchMetadata/SearchDifficulty rows, BeatmapDetails and aggregate audio groups.
  Distinct ordinary/Unicode strings, source/MD5/native identities and absent
  metadata/audio expose swapped or omitted projection fields. Empty sets remain
  visible; a difficulty without audio is absent from the search relationships.
- Complete available/detached Playlist/PlaylistItem/PlaylistSummary fields and
  all PlaylistBeatmap fields are checked. Duplicate additions retain one item;
  custom cover bytes/revision persist; removing the library clears live IDs/cover
  references while retaining saved labels and source keys. Repository volume/date
  fields are null here; service enrichment remains separately audited.
- The shared SQLite contract passed. All six PostgreSQL repository tests passed
  on a fresh `radio_db_test_projections` database in a private PostgreSQL 18.6
  loopback-only cluster. `target/coverage/model-projections-postgres.json` records
  exit zero, successful stop and directory removal; the sibling logs preserve
  build/test/server output. No configured application database or .env was read.
- Both backend Clippy checks with denied warnings passed. Product schema,
  repository behavior and dependencies are unchanged. The contract rolls back
  its own fixture before the existing repository scenarios continue.

### Service delegation/search/track audit (2026-10-15)

- The plain beatmap/metadata/audio/user-data services delegate to the asserted
  repositories; folder overview, source variants, snapshot retention and cleanup
  have concrete shared service assertions. Tag import is checked for Unicode
  lowercasing, whitespace, whole user tags, duplicate global identities, order,
  replacement/removal, missing metadata and empty input.
- Existing search contracts assert Unicode/literal substring matching, AND across
  global audio references, per-source filtering with multiplicity, consistent read
  snapshots, deduplicated query words, 70-word queries and 1002 audio sources over
  three SQL chunks. Their fixture builders/assertions were read during this audit.
- Added one complete LibraryTrack/TrackDifficulty comparison: first-seen audio
  order differs from numeric ID order, globally lowest difficulty ID supplies all
  label variants, cover selection can come from another difficulty, and every
  difficulty retains its owning set/multiplicity. Empty input is also checked.
- SQLite service coverage passed 15 tests (two benchmarks ignored). The pure new
  test passed with PostgreSQL selected too, without a PostgreSQL connection; the
  earlier full PostgreSQL service run remains distinct. Both backend all-target
  Clippy checks passed. These are model/service checks, not playback validation.

### Queue/playlist/history/settings continuation (2026-10-15)

- Strengthened existing shared entry tests; the goal remains at 97 new entry
  tests. Repository queue validation now checks mismatched parallel arrays,
  absent/current indices, invalid empty modes and counters beyond BIGINT, retaining
  the previous row after every rejected save. Service commands at persisted
  revision/token `i64::MAX` fail without changing queue or playback assignment.
- A transaction removes only its fixture settings singleton; get/settings,
  no-op/partial update and writer lock report the missing row without recreating
  it. Rollback restores the original row and defaults.
- Deleted-playlist mutations now assert exact NotFound variants. An otherwise
  correctly sized PNG with damaged IDAT bytes is rejected without replacing the
  saved cover. Existing copy/MD5/native identity, Unicode refresh, chunking,
  detached-label and concurrent queue/playlist assertions were audited.
- Service volume tests read back both 0 and 100, retain settings on an all-omitted
  update, and compare typed invalid-percent errors plus the retained override.
  History acknowledges Unix milliseconds within the surrounding wall-clock interval;
  duplicate/stale/delayed acknowledgements and durable projections remain checked.
- SQLite coverage passed 10 repository + 15 service tests; two timing benchmarks
  were ignored. Fresh PostgreSQL 18.6 databases passed six repository + two service
  tests (one full shared scenario and one pure track test); timing fixtures were
  skipped. `target/coverage/state-contracts-postgres-final.json` records successful
  suite exits, stop and removal of the private loopback-only cluster, with sibling
  runner/server logs. No configured application database or .env was used.
- Backend service/repository sources and their specific consumer assertions were
  inspected before marking the corresponding rows focused. SQLite/PostgreSQL
  all-target Clippy with denied warnings passed. Product behavior, schemas,
  dependencies, devices and externally hosted services are unchanged.

### Folder boundaries, documents and fixture consumers (2026-10-15)

- Added two SQLite service entry tests and four stdlib metadata/parser checks:
  103 new entry tests across the goal. Marker preview/import rejects relative,
  missing, directory and wrong-name inputs with exact path-bearing error variants
  and no new records. Reimport's guarded kind/path mismatch retains the original
  folder, library, timestamp and source bytes.
- Folder workflow/import sources and the tag-migration fixture were read against
  their existing atomic registration, duplicate/alias, source-read/deletion,
  shared cleanup, collision/upgrade/rollback and timestamp assertions. New folder
  cases passed with the seven-case folder suite; full service results follow
  below. Tag migration uses the previously recorded shared backend suites, with
  unchanged source/fixture code.
- All six player tests passed after strengthening the real CBR/VBR MP3, Ogg and
  WAV consumer: channels=2, rate=44100 and duration 1.4–1.7 seconds, audible decoded
  samples, extensionless copies, mixer transport/seek/EOF/replacement and decoder
  reclamation without device callbacks. This does not validate a physical device.
- All nine server media tests passed (SQLite/docs): fixture duration detection,
  exact bounded HTTP bytes/download cleanup, missing/nonregular/online/FIFO
  handling and original JPEG response bytes. These validate the fixture consumers;
  original artwork attribution and physical sound remain separate.
- Eleven repository Python tests passed. Every Markdown file now has a content,
  UTF-8 and closed-fence check in addition to local target/anchor checks; negative
  parser cases reject unclosed/mismatched fences. Documentation rows marked
  focused record this structural evidence, not automated factual verification of
  every prose claim or external URL. Player README tone names match the files.
- Bundled OFL/ISC/Apache notice texts are pinned in full by SHA-256, with UTF-8 and
  universal newline handling; SOURCES.md must link each notice. This protects the
  supplied attribution payload from silent removal/truncation. Asset provenance
  is not inferred from a passing integrity check.
- Full SQLite service coverage passed 17 tests (two timing benchmarks ignored).
  Service/player all-target Clippy with denied warnings passed, as did formatting,
  whitespace and the updated repository metadata suite.
- No product implementation, dependencies, game files or existing notices were
  changed. These are fixture/test/documentation changes; hosted CI and native
  Windows execution are separate from the executed Linux checks.

### Client DTOs and presentation state (2026-10-15)

- Added eleven client entry tests (114 new entry tests across the goal): four
  DTO/wire contracts, two loading/error helper cases, three library transitions
  and two queue cases. Existing patch and track assertions were strengthened.
- All four volume-bearing wire DTOs accept integer 0 and 100, reject out-of-range,
  negative, fractional, string and boolean values, and distinguish optional
  null/absence from required global volume. Older response extension defaults,
  conditional pause/full-width callback tokens, playback modes, folder labels,
  request paths and created/duplicate accessors have explicit expected values.
- Library tests check abort acknowledgement, wrapping request epochs, stale
  completion status isolation, connection/retry state, selection outside search
  for the current track, absent selection rejection, empty search messages,
  monotone history and selective volume/duration updates. Nested error causes
  remain readable without repeating already-present cause text.
- Queue tests check offline/closed panel behavior, abort acknowledgement, late
  failure isolation and HTTP refetch when a queue snapshot predates the current
  assignment. The old snapshot never replaces displayed rows. Existing queue
  tests retain ordered duplicates, independent selection, media demand, Unicode,
  stream reconnect and token-bound callback/command serialization assertions.
- Full Linux client coverage passed 139 unit tests without `mock` and 148 with
  it, with two fixture-dependent timing benchmarks ignored in each. The final
  models/library subsets passed again after removing duplicate command examples
  and replacing a wall-clock deadline assertion with captured time bounds.
- The explicit `embedded_server` test passed separately after a fresh SQLite/docs
  server build. DB URL variables were removed from its environment; only the
  fixture's temporary `.env`, SQLite file and marker were used. Real HTTP asserts
  readiness port, registration/duplicate behavior, null patch, persistence across
  shutdown/restart, removal and unchanged marker bytes. It does not import Realm
  contents, use a real game folder or open an audio device.
- Existing preference, sorting, volume, folder and HTTP fixture assertions were
  inspected and rerun. Their per-file evidence is recorded below; the larger
  client controller/player/settings/playlist/API owners still need their own
  complete audit. All-target client Clippy passed with `mock` off/on. No product
  behavior, dependencies, ownership or wire protocol changed.

### Client HTTP boundaries and lifecycle (2026-10-15)

- Added fifteen entry tests (129 new entry tests across the goal): seven HTTP
  boundary contracts, seven server/session cases and one workflow cancellation
  contract. One new Rust test file brings the inventory to 320 files.
- All 36 public HTTP operations now assert method, path/query, exact JSON or PNG
  request bytes/fields and retained 422 status/path/server error. Separate success
  cases check clear-queue assignment counters, volume PUT/reset DELETE at 0/100
  and escaped Unicode legacy search. Malformed 200/201/409 bodies remain decode
  errors, including duplicate registration; `error`/legacy `message` and invalid
  error-body fallback retain intentional HTTP status semantics.
- Ordinary covers accept exactly 16 MiB and discard missing/header/stream excess;
  discovery/playback accept a 1 MiB heartbeat line and reject larger lines without
  emitting events. Truncated bodies retain transport errors; malformed event JSON
  retains stream-decode errors. Audio accepts exact/zero byte limits, cleans
  temporary files and preserves the cause on file-creation failure. Existing
  cancellation and oversized-audio checks remain active.
- Linux child fixtures assert distinct missing/nonexecutable spawn errors, early
  exit and invalid UTF-8 output after reaping, session API/address agreement,
  repeated acknowledged shutdown and eventual child cleanup on Drop. Child-only
  environment probes verify explicit binary, environment override, adjacent file
  and PATH precedence using a private hard link to the test executable; no shared
  executable or parent environment is edited. Error Display/source cases retain
  context. The workflow watch-cancellation case observes resource Drop before
  Cancelled completion and an unchanged subsequent ready result.
- Full client coverage passed 154 unit tests without `mock` and 163 with it,
  with two timing benchmarks ignored in each. The real-server integration remains
  opt-in; its prior isolated run is recorded above. Final seven HTTP tests passed
  again after strengthening payload/exact-download-bound assertions. Full logs
  and exit summaries are in ignored `target/coverage/client-http-final-*` files.
  Client all-target Clippy passed in both feature modes; formatting, whitespace,
  metadata/inventory checks and JSON/text line reports were refreshed.
- Product implementation, dependencies, ownership and protocols are unchanged.
  Physical output, native Windows, forced disk write/flush exhaustion and actual
  HTTP-client-builder failure cleanup are not established by these checks; the
  latter error wrapper is tested independently.

### Server routes, projections and recovery (2026-10-15)

- Added eight entry tests (137 new entry tests across the goal). The 320-file
  inventory is unchanged; tests live beside the existing server code.
- Both router variants now exercise an explicit contract of 25 API paths and
  35 method/path pairs across GET/POST/PUT/PATCH/DELETE/OPTIONS/TRACE: registered
  methods reach their handlers and unregistered methods return 405. Invalid JSON
  prevents discovery/import actions during this routing check; positive behavior
  is asserted by the endpoint consumers. Dropping the event response releases its
  watch receiver. `/docs` is present only with the feature enabled.
- The served OpenAPI document has exact paths/methods, title/version/description
  and five tags, with every local schema reference resolved. No CDN/network fetch
  or browser rendering is inferred from this check.
- Complete track and legacy set/audio/beatmap DTOs are compared to explicit JSON,
  including distinct ordinary/Unicode fields, dates, zero volume, nullable fields,
  every difficulty's identity/multiplicity, all source kinds and empty lists.
  Client decoding succeeds; internal installation IDs are omitted from the wire.
- Rejected null/wrong/unknown/mixed-valid-invalid settings patches retain both
  persisted settings fields and the track override; empty patch is a no-op.
  Registration's relative/missing/ambiguous/unexpected errors retain exact public
  status/JSON. Contextual internal errors return safe 500 JSON without the cause.
- Private file-SQLite reconnect checks AppState queue recovery: IDs/index remain,
  playing becomes paused and revision/token advance; cloned state shares monotone
  notifications through u64::MAX without writing those notification values to the
  database. Marker bytes and subscriber cleanup are checked.
- Existing folder tests now assert canonical candidate paths and all three depths
  with explicit private roots, plus relative-root rejection. Media checks now
  include cache headers, exact 16 MiB cover acceptance, excess/directory rejection.
  Playback, playlist, configuration and fixture assertions were inspected against
  their complete current source before marking their file audit focused.
- Full SQLite server coverage passed 52 unit tests with `docs`, 46 without it and
  four actual startup tests in each feature mode. Two unit entries stay ignored:
  the opt-in snapshot timing benchmark and the configuration helper (the latter
  runs explicitly through its seven parent tests). The expanded method matrix was
  rerun in both modes. Detailed logs/exit summaries are retained under ignored
  `target/coverage/server-route-final-*`.
- Server all-target Clippy passed for SQLite with/without `docs` and PostgreSQL
  with `docs`; PostgreSQL was compile-only in this stage. Product implementation,
  dependencies, persistence schema and wire protocols were not changed. Physical
  output, native Windows, configured/production DBs and the snapshot timing
  benchmark remain outside these executed checks.

### Client player, settings and playlist owners (2026-10-15)

- Added eight entry tests (145 new entry tests across the goal): three player
  presentation/command/seek cases, two folder alias/chooser cases and three
  playlist draft/list/mutation cases. Existing folder selection/getter and playlist
  item-selection assertions were strengthened. The inventory remains 320 files.
- Player presentation checks connection/settings/selection gates independently
  from local seek, distinct playlist-item identity for shared audio, zero/unknown
  duration, metadata fallback, loading/error priority and unavailable rows. Seek
  effects carry the expected audio ID and launch token; a different duplicate-audio
  item emits no seek. Offline transport/playlist commands report their error
  without opening a worker, starting tasks or changing library selection.
- Canonical folder aliases abort the old preview, replace its queued path once,
  discard late old-path counts and retain the canonical result. Cancellation
  awaits both scan/count fixtures and clears queued work across epoch wrap.
  Chooser requests are single; stale/current-cancelled/closed/busy replies do not
  start discovery. Folder getters/selection retain valid IDs and reject missing
  IDs. All candidates are synthetic; no configured discovery or game reads occur.
- Playlist cases retain an offline/busy draft and its prior PNG after oversized
  input; serialized state omits raw editor PNG bytes. Current list failure retains
  detail, while a removed active playlist clears only its own selection and picks
  a remaining add target. Unicode filtering, invalid/hidden item selection,
  missing target/unchecked additions and create/rename/delete result transitions
  leave the independent library choice intact.
- The entire player/settings/playlist owner source and all 18 existing/new
  playlist fixture flows were inspected against their assertions. Nine shared
  VM cases, seven folder cases and 18 playlist cases passed before the full
  feature runs. Queue/callback, preference/volume and media consumers remain part
  of those full suites; their source owners keep separate audit entries.
- Full Linux client coverage passed 162 unit tests without `mock` and 171 with it,
  with two timing benchmarks ignored in each. The opt-in real-server integration
  was skipped in these runs; its prior isolated result remains recorded above.
  Detailed logs/exit summaries are in ignored `target/coverage/client-vm-final-*`.
  All-target client Clippy passed in both feature modes, and formatting,
  metadata/inventory and line-report checks were refreshed.
- Product behavior, dependencies and state ownership were unchanged. Physical
  sound, native Windows and live Qt interactions were not checked by these
  toolkit-free/fake-worker tests.

### Client worker and offline reducers (2026-10-15)

- Added five entry tests (150 new entry tests across the goal) and strengthened
  the existing disabled-gallery guard. The inventory remains 320 files: 266
  focused audits are complete and 54 files remain pending. The file audit count
  is separate from executable line/branch coverage.
- Audited all ten playback tests, including the real worker thread with a fake
  engine: current/stale generations, download feedback, Started before Finished,
  periodic Playing snapshots, replaced/stale-file deletion, joined shutdown,
  closed channels and harmless repeated shutdown/send. Invalid assignment volume
  preserves the playing source, mode, token and volume while exposing an error.
  Existing core tests assert cached restarts, duplicate IDs, seek serials,
  pause-during-download and token-bound decode/play/device failures.
- Audited eight mock reducer tests and four mock playlist tests. Folder actions
  cover invalid paths, staged refresh/removal/addition, partial success/retry,
  failed-count replacement, pending browse guards, reset and disabled isolation.
  Playlist assertions cover editor/cover epochs, the 2 MiB boundary and errors,
  cancellation, revision-specific lookup/removal, serialization exclusion,
  independent selection/sort/saved order and offline playback errors.
- Full Linux client coverage passed 164 unit tests without `mock` and 176 with
  it; two existing timing benchmarks were ignored in each. The opt-in real-server
  integration remained ignored; its earlier private-SQLite evidence is separate.
  Logs/exit summaries are in ignored `target/coverage/client-worker-final-*`.
  Scoped all-target Clippy passed with `mock` off/on; formatting, whitespace and
  all 11 repository metadata checks passed. The JSON/text reports and all 108
  measured inventory rows were refreshed.
- Production behavior, dependencies and architecture were unchanged. These
  tests use temporary files and in-memory data; they do not validate physical
  audio, a native picker, desktop Qt interaction or native Windows execution.

### Discord presence, lifecycle and Unix IPC (2026-10-15)

- Added four entry tests (154 new entry tests across the goal), strengthened the
  watch notification test and expanded IPC helper/reply assertions. Audited the
  two production files and both separate test files: 270 of 320 file audits are
  now focused, with 50 pending. Production behavior and dependencies are unchanged.
- The 12 presence/lifecycle tests assert exact UTF-8/byte bounds, EOF/extreme
  decoder timestamp handling, all source/metadata gates and latest-state replay.
  Watch consumers now prove that seek, token, title, state, duration and clearing
  changes notify, while position ticks and repeated absence remain coalesced.
  A mock-installed service reuses one worker across toggles and stops on explicit
  shutdown or Drop; no installed Discord connection is used by that test.
- Four transport tests cover first-byte cancellation/buffering, timeout/disable,
  disconnected operations, nonce wrapping and cancellation restoration after a
  failed clear. Six child probes clear the inherited environment and point every
  runtime/temp root to a private directory: XDG runtime, TMPDIR, TMP, TEMP,
  compatible-client subpaths, index 9, Snap runtime parent and socket absence.
  Successful probes use the actual discovery and byte-read path. Child coverage
  profiles are preserved; no process mutates the parent's environment.
- All 12 existing real Unix IPC flows were reviewed and passed. Helpers now
  assert handshake version, PID and nonempty string nonces; unrelated/non-object,
  missing/numeric-nonce and wrong-event replies stay connected without an extra
  Activity or artwork fallback. Existing assertions retain bounded/fragmented
  frames, PING/PONG bytes, EOF, cancellation, stale replies and retry limits.
- Focused coverage passed all 28 Discord tests. Full Linux client coverage then
  passed 168 unit tests without `mock` and 180 with it; two timing benchmarks and
  the opt-in real-server integration remained ignored in each feature run.
  Detailed logs/exit summaries are in ignored `target/coverage/client-discord-*`.
  All-target Clippy passed with `mock` off/on; formatting, whitespace and all 11
  repository metadata checks passed. JSON/text reports and all 108 measured table
  rows were refreshed.
- Unix fixtures do not establish installed Discord/Vesktop acceptance, sandbox
  packaging permissions, artwork propagation, physical audio or native Windows
  named-pipe/GUI behavior. Those platform checks remain separate.

### Controller coordination and media lifetime (2026-10-15)

- Added eight entry tests (162 new entry tests across the goal) and strengthened
  viewport validation. Audited `controller.rs`, `coordinator.rs` and `media.rs`,
  and reviewed all 13 current VM-coordination assertion flows. There are now 273
  focused file audits and 47 pending, with the same 320-file inventory. Production
  behavior, dependencies and state ownership were unchanged.
- Generation-wrap tests abort and await pending HTTP tasks, retain an active
  decoder's slot until its stale ACK, reset every retained queue demand and reject
  stale successful installation. Missing HTTP covers and failed decode ACKs cache
  unavailability; unknown durations cache `None`, survive navigation and retry
  only after explicit invalidation. Viewport snapshots reject negative/unknown IDs.
- Direct controller and media lookups assert the same precedence across selected
  duplicate playlist items, current playback, library, queue and selected-track
  fallback, including an invalid library index. Duration effects update duplicate
  playlist/queue rows, current/selected tracks and the library while preserving
  independent selection/playback and emitting only the relevant row/queue updates.
- Fake-engine worker tests assert absent/stale/offline download guards, stale-file
  removal without clearing current work, valid file handover and public Seek
  dispatch with the launch token. Blocked semaphore downloads abort and closed
  gates complete as Cancelled before HTTP. Temporary audio files disappear when
  the worker is joined; no output device is opened.
- Connection checks preserve a pending query's remaining debounce across session
  installation and prevent duplicate startup tasks. A private shell child and
  controlled late Connected completion assert Linux PID reaping before shutdown
  returns. The focused controller run passed 36 tests; two timing benchmarks were
  ignored. These are deterministic fixtures, not a production startup-load test.
- Full Linux client coverage passed 176 unit tests without `mock` and 188 with it;
  two timing benchmarks and the opt-in real-server integration remained ignored
  in each feature run. Logs/exit summaries are in ignored
  `target/coverage/client-coordinator-*`. All-target Clippy passed with `mock`
  off/on, as did formatting, whitespace and all 11 metadata checks. JSON/text
  reports and all 108 measured inventory rows were refreshed. Physical audio,
  live Qt/desktop interaction and native Windows execution remain separate.

### Benchmark fixtures, initial schema and service contracts (2026-10-15)

- Added two ordinary entry tests (164 new entry tests across the goal) and
  strengthened the existing migration lifecycle fixture. Audited both benchmark
  files, the initial migration, its frozen tag-upgrade mapping and the complete
  shared service-contract fixture. The inventory remains 320 files: 278 focused
  audits and 42 pending. Product behavior, schema definitions and dependencies
  were unchanged; every database/process fixture is private.
- The ordinary Unix client fixture writes five legacy/compact payload pairs,
  checks explicit representative/cover/fallback/deduplication values and invokes
  the captured benchmark plus both actual HTTP benchmark paths in separate child
  processes with cleared environments. It checks 30 timing-result rows and 120
  query/path requests, including Unicode, empty results and both debounce modes.
  Its HTTP child serves only those inert fixtures, with no database or media I/O.
- The service fixture seeds six sets in its own SQLite file, asserts exact
  tag/difficulty/Unicode/empty result cardinalities, then invokes the actual
  snapshot benchmark entry in a cleared child environment. All 15 reference-stage
  results are checked, and library rows plus an unrelated sentinel remain intact.
  The opt-in synthetic bulk benchmark also passed explicitly at 1,000, 10,000
  and 50,000 sets. These debug/instrumented measurements are functional evidence,
  not production performance results.
- Initial-schema assertions cover all six v1 tables, frozen metadata columns,
  lookup indexes, singleton/kind/required-field/unique/foreign-key rejection and
  cascade versus shared-row retention. Populated tag-upgrade consumers establish
  the frozen legacy mapping. Existing service assertions were reviewed for
  two-pool registration, omitted/null updates, shared snapshot cleanup, forced
  failure/cancellation rollback, concurrent replacement/deletion, source identity
  and unchanged temporary source files.
- Full Linux client coverage passed 177 tests without `mock` and 189 with it.
  Their standalone benchmark entries and real-server integration remained ignored
  at the top level; the ordinary fixture ran the benchmark entries with controlled
  inputs. Full SQLite coverage passed 10 repository and 18 service tests; the
  latest service-only repeat also passed after routing its snapshot fixture through
  the actual entry point. Its two opt-in benchmark entries stayed ignored at the
  top level; the bulk benchmark was run separately and the snapshot entry ran in
  the fixture's child. Logs are in ignored `target/coverage/bench-*`.
- Fresh private PostgreSQL databases passed all six repository tests and both
  service tests, including the initial-schema assertions. The PostgreSQL bulk
  benchmark was skipped. `bench-postgres-results.json` records successful cluster
  stop/removal. SQLite/client, mock-client and PostgreSQL repository/service
  all-target Clippy passed; formatting, whitespace and all 11 metadata checks
  passed. JSON/text and all 108 measured table rows were refreshed.
- Child benchmark profiles are retained. The force-killed HTTP fixture child
  cannot flush complete line profiles, although its request assertions run.
  Measurements still include inline test/ignored feature code; physical audio,
  real-library performance, hosted CI and native Windows remain separate checks.

### Repository configuration and CI consumers (2026-10-15)

- Added five entry tests (169 new entry tests across the goal). The 320-file
  inventory now has 290 focused audits and 30 pending, all in Qt/QML and its
  supporting declarations/fixtures. Product behavior, dependencies and the
  configuration files audited in this stage were unchanged.
- All 15 standard-library Python metadata tests passed. New or strengthened
  assertions exercise Git ignore rules and nonignored lookalikes, empty
  submodule metadata with no gitlinks, the empty legacy migration lock and the
  pinned toolchain's format/lint components. CR/Serena checks guard selected
  committed scalar/list fields; they are not a general YAML/schema validator and
  do not start Serena.
- CI command checks assert the scoped package/backend/feature matrix, locked
  dependency resolution, serialized test runs, Clippy targets and Bash syntax.
  Actionlint 1.7.12 also accepted the workflow locally, with external ShellCheck
  and Pyflakes integrations disabled. Its binary is under ignored `target/`;
  hosted GitHub Actions were not executed.
- The repository dotenv test parses the optional checkout file with the actual
  dotenv iterator and checks that the process environment is unchanged. Values
  are withheld from diagnostics, including read/parse failures; no application
  state or database connection is constructed. An absent optional file returns
  without parsing. This is a file-consumer check, not startup acceptance.
- Full Linux SQLite server coverage passed 53 unit tests with docs and 47
  without docs, plus four startup tests in each mode. Two entries remained
  ignored at the top level: the timing benchmark and the configuration child
  probe invoked by seven parent tests. All-target Clippy passed for SQLite docs
  on/off and PostgreSQL/docs; PostgreSQL was compile/lint only in this stage.
  Logs and exit summaries are under ignored `target/coverage/config-*`.
- Accumulated LLVM JSON/text reports and all 108 measured inventory rows were
  refreshed. Formatting, whitespace and metadata checks passed. Per-file audit
  status remains separate from measured lines and platform/runtime acceptance.

### Qt controls, popup rules and native artwork (2026-10-15)

- Added two offline integration entry tests (171 new entry tests across the goal)
  and strengthened the existing native artwork contract. Fifteen per-file audits
  are complete in this stage: the small shared controls, Theme declaration/fonts,
  Store payload conversion and native artwork boundary. The inventory remains
  320 files: 305 focused audits and 15 pending. Product behavior, dependencies
  and the approved state/GUI boundaries were unchanged.
- The real gallery root hosts production component instances. QtTest keyboard
  and pointer events assert button/switch/tab/tag activation and disabled input
  suppression, exact edit/toggle counts, Unicode selection/clearing and state
  precedence. Font readiness, artwork source clearing, material source mapping,
  layout invalidation and visual property variants are checked explicitly.
  Store dispatch preserves zero, false, Unicode and omitted payload semantics.
- Menu checks distinguish original IDs from row indices, reject missing/disabled
  choices, edit search text, wrap with arrows and use Home/End/Space/Return.
  They assert selected-row focus and close-on-disable. Modal checks assert the
  initial field, contained Tab traversal, nondismissible Escape/backdrop/close
  suppression and restored opener focus. Existing gallery checks separately
  exercise dismissible Escape/backdrop and responsive modal proportions.
- Native checks retain existing scaling/pixels, replacement versions, protected
  LRU/budget and EXIF/crop assertions, adding automatic/draft/nested key isolation,
  malformed/stale provider requests, reset state and stale draft rejection.
  Missing local files, unsupported BMP, byte/pixel limits and damaged compressed
  PNG pixels are rejected; a later successful preparation clears its prior error.
  The damaged PNG deliberately produces a libpng diagnostic. Native assertion
  failures now retain their descriptive message.
- Full Linux instrumented Qt coverage passed four library tests, two binary tests
  and 20 serial offscreen integration scenarios. The two opt-in actual-server
  probes remained ignored in this stage. The tests use private working/settings
  directories and inert media; offline cases have an unavailable server path.
  Logs are under ignored `target/coverage/qt-components-*`. All 12 Qt Python tool
  tests, all 15 repository metadata checks, generated-import QML lint with zero
  warnings, all-target Qt Clippy with Rust warnings denied, formatting and
  whitespace checks passed. Accumulated LLVM JSON/text and all 108 measured
  inventory rows were refreshed.
- Property/input checks are separate from GPU blur/masking, desktop window-manager
  behavior, physical sound and native Windows acceptance. QML/C++ have explicit
  consumer evidence here, not LLVM source-line instrumentation.

### Final Qt panels and repository verification (2026-10-15)

- Added two ordinary Qt entries and six Python HTTP fixture entries (179 new
  entry tests across the goal), strengthened Gallery/Main/Player/Songs assertions
  and finished the remaining per-file consumer audit. All 320 inventory files now
  have focused assertion/consumer evidence; zero per-file audits remain pending.
  This counts files with tests, not complete line/branch/platform coverage.
- The composed Qt fixture checks blank/preparing/busy editor gates, exact
  marker/playlist/difficulty IDs, empty/offline/detail/menu states, disabled
  empty-playlist queue actions, modal mutation gates and queue retry/duplicate
  delegate registration/removal. The title-bar fixture uses actual pointer input
  with a window double to assert tab/queue actions, all eight resize edge flags,
  move/minimize/maximize/restore/close routing and fullscreen/maximized visibility.
  It never moves or resizes the user's desktop windows.
- Gallery keyboard actions now traverse its real Store bindings. Main retains
  every existing delegate when removing an absent delegate; Songs refresh buttons
  traverse their QML callbacks for failed/shrinking/empty responses. Player
  checks the 1024-by-640 layout, valid progress range and disabled unavailable
  modes. Existing Settings search/preference and live playlist/media assertions
  were audited against the corresponding panel sources.
- The normal composed-dialog close exposed a real QML lifetime error:
  `FolderSelectionModal` read `parent.width` after a row's parent disappeared.
  Its width now falls back to zero without a parent. The same scenario passed
  only after that guard, including the outer rejection of QML diagnostics.
  This is the only product behavior fix in this stage; ownership, protocols,
  storage, dependencies and configured application data were unchanged.
- Six direct HTTP tests check startup retry and independent list failures,
  playlist/cover and volume CRUD/retry, stale playback tokens, marker/count/import
  partial retry, visual duplicate memberships and every generated PNG chunk CRC
  plus decompressed RGBA pixels. Every child has a private working directory,
  cleared application configuration, a loopback port and verified process reaping.
  The retained `.pyc` is exercised separately on its common sorting/media/playlist
  paths and remains byte-for-byte unchanged. Its Python 3.14 magic must match for
  execution; other interpreters skip that case. It predates source fixes and is
  not claimed to be identical to the source fixture used by normal Qt launches.
- Final Linux headless default-feature verification passed 374 tests, seven
  ignored: CLI 15 unit/12 subprocess, client 177, player six, server 53/four startup,
  core seven, repositories ten, scanner 72 and services 18. The scanner tests
  build/run the actual helper and synthetic Realm generator and check preserved
  source bytes. The client without/with `mock` passed 177/189; both top-level
  timing entries stayed ignored but their controlled ordinary fixture invokes
  the actual benchmark paths. Server docs off passed 47 plus four startup tests;
  its benchmark and seven-parent configuration child probe stayed top-level
  ignored. PostgreSQL CLI passed 14 unit and nine storage-independent subprocess
  tests; that CLI run does not establish PostgreSQL CLI database runtime.
- Final Qt coverage passed four library, two binary and 22 ordinary offscreen
  scenarios. Both opt-in actual-server Qt probes passed separately on private
  SQLite, as did the client's actual-server restart/persistence check and four
  instrumented startup checks. Fresh PostgreSQL repository/service runs passed
  six/two tests in separate databases. Their private cluster was stopped and
  removed successfully; the PostgreSQL bulk performance benchmark was skipped.
  Earlier explicit SQLite bulk measurements remain recorded above.
- All 18 Qt Python checks passed under Python 3.14. Logs/results for the final
  feature/lint matrix are in ignored `target/coverage/final-*`; panel regressions
  are under `qt-panels-*`. Accumulated LLVM JSON/text and all 108 measured table
  rows were refreshed after the final instrumented runtime checks. C++/QML and
  other unmeasured languages retain explicit test evidence rather than an LLVM
  source-line percentage. Final all-target Clippy passed for the headless SQLite
  packages, Qt, the PostgreSQL database/services/CLI/server packages and the
  PostgreSQL server with docs. Unsupported zero-backend and dual-backend builds
  both failed with the required exactly-one-backend diagnostic. All five scanner
  Python protocol checks, eight benchmark-tool checks, 15 repository metadata
  checks, generated-import QML lint, actionlint, formatting and whitespace checks
  passed. Actionlint's external ShellCheck/Pyflakes integrations and hosted
  GitHub Actions were not run.

Reproduce after installing the [coverage tool](https://github.com/taiki-e/cargo-llvm-cov)
(tool 0.9.1 and Rust 1.98.0 LLVM tools were used here). Preserve `LLVM_PROFILE_FILE`
in isolated child environments so their coverage is retained. Socket/process
fixtures require a runner permitting local sockets and child observation.

```sh
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.9.1 --locked
cargo llvm-cov -p radio-core -p radio-scanner -p radio-db -p radio-services -p osu-radio-cli -p osu-radio-server -p osu-radio-player -p osu-radio-client --no-report --locked --offline -j 1 -- --test-threads=1
cargo llvm-cov -p osu-radio-client --features mock --lib --no-report --locked --offline -j 1 -- --test-threads=1
cargo llvm-cov -p osu-radio-server --no-default-features --features sqlite --no-report --locked --offline -j 1 -- --test-threads=1
cargo llvm-cov -p osu-radio-server --test startup --no-report --locked --offline -j 1 -- --test-threads=1
mkdir -p target/coverage
# Provision separate fresh PostgreSQL test databases as described in development.md first.
RADIO_DB_TEST_POSTGRES_URL='postgres://USER@127.0.0.1:PORT/radio_db_test_repositories' cargo llvm-cov -p radio-db --no-default-features --features postgres --no-report --locked --offline -j 1 -- --include-ignored --test-threads=1
RADIO_DB_TEST_POSTGRES_URL='postgres://USER@127.0.0.1:PORT/radio_db_test_services' cargo llvm-cov -p radio-services --no-default-features --features postgres --no-report --locked --offline -j 1 -- --include-ignored --test-threads=1 --skip synthetic_search_timings
cargo llvm-cov -p osu-radio-qt --no-report --locked --offline -j 1 -- --test-threads=1
# The two real-backend probes need a built SQLite server beside the instrumented Qt binary.
cargo llvm-cov -p osu-radio-qt --test launch_smoke --no-report --locked --offline -j 1 -- real_backend --ignored --test-threads=1
cargo llvm-cov report --include-build-script --json --output-path target/coverage/workspace-linux.json
python3 -B -m unittest discover -s apps/osu-radio-qt/tests -p 'test_*.py' -v
apps/osu-radio-qt/scripts/lint-qml.sh
python3 -B -m unittest discover -s tools/search-benchmark -v
python3 -B -m unittest discover -s crates/radio-scanner/tests -p 'test_*.py' -v
python3 -B -m unittest discover -s tests -v
python3 -B -m trace --count --missing --summary --coverdir target/coverage/python --module unittest discover -s tools/search-benchmark -v > target/coverage/python-summary.txt
```

These commands accumulate profiles. Cargo-llvm-cov 0.9.1 supports
`LLVM_PROFILE_FILE_NAME=<fresh-run-tag>-%p-%m.profraw` to isolate an invocation's
raw filenames. A repeated instrumented service test encountered an incompatible
existing raw-profile merge (test exit remained zero); rerunning it with a unique
filename completed without warnings, and JSON/text reports were refreshed. Keep
old profiles when comparing stages, and use a new tag after recompilation or PID
namespace reuse. The installed tool source and upstream README document this
filename override.

Use `cargo llvm-cov clean --workspace`
before a fresh measurement; preserve artifacts if comparing changes. Select
mutually exclusive database backends in separate invocations.

## Completion and remaining acceptance

All 320 current inventory files have focused test/consumer evidence and the final
scoped runtime, feature, lint, metadata, link and measurement checks passed.
There are no pending per-file audits. New repository files must extend the
inventory and supply corresponding assertions/consumer evidence.

This is file coverage, not 100% line/branch/platform coverage. Tests do not replace
hosted GitHub Actions, physical audio, desktop input/window-manager/GPU effects,
native Discord or native Windows acceptance. PostgreSQL HTTP routes were compiled;
runtime PostgreSQL checks exercised repositories/services in private databases.
The retained Python 3.14 bytecode was tested separately from the current source;
it is not a second supported production backend. These limits do not represent
untested files in this inventory.

Use the [verification matrix](development.md#verification-matrix); incompatible
backend features must never be combined with `--all-features`. No configured
application database or real game installation is a test fixture.

## Rust, C#, C++, QML and executable tools

| File | Audit | Observed lines | Evidence / remaining action |
| --- | --- | --- | --- |
| [apps/osu-radio-cli/src/commands/database.rs](../../apps/osu-radio-cli/src/commands/database.rs) | Focused | 34/34 (100.00%) | Memory connection plus child-isolated dotenv success/failure tests (SQLite). |
| [apps/osu-radio-cli/src/commands/helpers.rs](../../apps/osu-radio-cli/src/commands/helpers.rs) | Focused | 322/323 (99.69%) | Unit/subprocess tests: explicit roots, marker selection, JSON, errors and Unicode truncation. |
| [apps/osu-radio-cli/src/commands/import.rs](../../apps/osu-radio-cli/src/commands/import.rs) | Focused | 318/319 (99.69%) | Unit/subprocess tests: cross-set limits, metadata/nulls, table formatting and source preservation. |
| [apps/osu-radio-cli/src/commands/mod.rs](../../apps/osu-radio-cli/src/commands/mod.rs) | Focused | — | Compiled by the tested CLI; module declarations only. |
| [apps/osu-radio-cli/src/commands/scan.rs](../../apps/osu-radio-cli/src/commands/scan.rs) | Focused | 38/38 (100.00%) | Subprocess tests: JSON/text, empty/filtered roots, first/limit and errors. |
| [apps/osu-radio-cli/src/commands/store.rs](../../apps/osu-radio-cli/src/commands/store.rs) | Focused | 83/84 (98.81%) | Subprocess tests: replacement/reopen, duplicate registration, explicit reset and failed reads (SQLite). |
| [apps/osu-radio-cli/src/consts.rs](../../apps/osu-radio-cli/src/consts.rs) | Focused | — | CLI help, default preview limit and table output assertions. |
| [apps/osu-radio-cli/src/main.rs](../../apps/osu-radio-cli/src/main.rs) | Focused | 23/23 (100.00%) | CLI subprocess tests: command dispatch, help/version and exit codes. |
| [apps/osu-radio-cli/src/types.rs](../../apps/osu-radio-cli/src/types.rs) | Focused | 51/51 (100.00%) | Unit and subprocess tests: defaults, invalid arguments and enum mapping. |
| [apps/osu-radio-cli/tests/commands.rs](../../apps/osu-radio-cli/tests/commands.rs) | Focused | — | Executed: 12 subprocess scenarios, including child-scoped Lazer helper override/diagnostics. |
| [apps/osu-radio-cli/tests/support/mod.rs](../../apps/osu-radio-cli/tests/support/mod.rs) | Focused | — | Executed by the 12 scenarios; stable binary/source fixtures, child isolation and coverage propagation. |
| [apps/osu-radio-qt/build.rs](../../apps/osu-radio-qt/build.rs) | Focused | 63/63 (100.00%) | QML/resource registration exercised by both roots, native adapters and decoding all 36 embedded image/font files; recursive resource discovery measured. |
| [apps/osu-radio-qt/qml/Gallery.qml](../../apps/osu-radio-qt/qml/Gallery.qml) | Focused | — | Real offline root: keyboard field/tag/button/disable actions reach Store, Settings stays unavailable, sorting/search keep IDs, playlist create/edit/cover/reset/add/remove/delete and modal focus/backdrop/layout paths asserted without a server. |
| [apps/osu-radio-qt/qml/Main.qml](../../apps/osu-radio-qt/qml/Main.qml) | Focused | — | Actual startup/retry and independent panel navigation, retained media-delegate removal, selected/background artwork continuity, offline/error/empty states, picker cancellation and folder/playlist modal wiring asserted; both real-server probes pass on private SQLite. |
| [apps/osu-radio-qt/qml/PlayerPanel.qml](../../apps/osu-radio-qt/qml/PlayerPanel.qml) | Focused | — | Minimum-window geometry and valid progress range, disabled seek/placeholder modes, selection/loading/download-error projection, volume editor focus/mode/selection closure, retry and next/previous/queue controls asserted without physical output. |
| [apps/osu-radio-qt/qml/PlaylistsPanel.qml](../../apps/osu-radio-qt/qml/PlaylistsPanel.qml) | Focused | — | Live and actual-server probes assert independent list/detail models and queries, item-ID selection, sorting, create/edit/cover/reset/add/remove/delete forwarding, retained media and unavailable/duplicate-audio presentation. |
| [apps/osu-radio-qt/qml/SettingsPanel.qml](../../apps/osu-radio-qt/qml/SettingsPanel.qml) | Focused | — | Actual keyboard/clear/whitespace/case/no-match search and scroll/layout recovery, independent retained queries, conditional global volume, folder/audio retries and offline preference persistence/error/toggle bindings asserted. |
| [apps/osu-radio-qt/qml/SongsPanel.qml](../../apps/osu-radio-qt/qml/SongsPanel.qml) | Focused | — | Debounced search/retry/clear, refresh-control callbacks with failed/shrinking/empty responses, sort selection, ID/media roles and visible-demand scrolling/cache continuity asserted through real Qt adapters. |
| [apps/osu-radio-qt/qml/Store.qml](../../apps/osu-radio-qt/qml/Store.qml) | Focused | — | Offline controls probe asserts numeric zero, false, Unicode and omitted values through actual Store.send and observes the parsed MockBridge state. |
| [apps/osu-radio-qt/qml/WindowBar.qml](../../apps/osu-radio-qt/qml/WindowBar.qml) | Focused | — | Real pointer input checks tab/queue routing, unavailable gallery controls, unoccupied-space move, all eight edge/corner flags, minimize/maximize/restore/close calls and resize visibility through a window double; actual desktop window-manager behavior remains separate. |
| [apps/osu-radio-qt/qml/components/AppButton.qml](../../apps/osu-radio-qt/qml/components/AppButton.qml) | Focused | — | Actual Space activation, disabled pointer/keyboard suppression and all four button-variant backgrounds asserted in the offline controls probe. |
| [apps/osu-radio-qt/qml/components/AppField.qml](../../apps/osu-radio-qt/qml/components/AppField.qml) | Focused | — | Native keyboard edit and textEdited count, disabled suppression and Ctrl+A/Backspace clearing of Unicode content asserted. |
| [apps/osu-radio-qt/qml/components/AppIcon.qml](../../apps/osu-radio-qt/qml/components/AppIcon.qml) | Focused | — | Native icon source, requested dimensions and decorative disabled child asserted; every embedded SVG also decodes through Qt. |
| [apps/osu-radio-qt/qml/components/AppMenu.qml](../../apps/osu-radio-qt/qml/components/AppMenu.qml) | Focused | — | Value-role IDs, empty/out-of-range/disabled choices, query editing, wrap/Home/End/Space/Return navigation, selected-row focus and disable-close behavior asserted with real QtTest input. |
| [apps/osu-radio-qt/qml/components/AppModal.qml](../../apps/osu-radio-qt/qml/components/AppModal.qml) | Focused | — | Initial/restored focus, nondismissible Escape/backdrop/close suppression and Tab containment asserted; existing gallery also tests dismissible Escape/backdrop and responsive bounds. |
| [apps/osu-radio-qt/qml/components/AppSwitch.qml](../../apps/osu-radio-qt/qml/components/AppSwitch.qml) | Focused | — | Pointer/Space toggles emit exactly once; disabling preserves checked state and suppresses input in the offline controls probe. |
| [apps/osu-radio-qt/qml/components/AppTabs.qml](../../apps/osu-radio-qt/qml/components/AppTabs.qml) | Focused | — | Space activates the second rendered tab with its original index and selected treatment; disabled pointer input preserves the choice. |
| [apps/osu-radio-qt/qml/components/AppTag.qml](../../apps/osu-radio-qt/qml/components/AppTag.qml) | Focused | — | Pointer selection fallback and explicit neutral/included/excluded precedence, border colors and disabled state/foreground asserted. |
| [apps/osu-radio-qt/qml/components/Artwork.qml](../../apps/osu-radio-qt/qml/components/Artwork.qml) | Focused | — | Embedded image reaches Ready with centered crop and cache disabled; clearing its source reaches Image.Null. GPU mask appearance remains separate. |
| [apps/osu-radio-qt/qml/components/FolderSelectionModal.qml](../../apps/osu-radio-qt/qml/components/FolderSelectionModal.qml) | Focused | — | Marker-bound row/footer signals, pending/count/error/action projection, applying/picker gates, partial-success retry, responsive focus/dismissal and normal teardown asserted. Regression fixes parent.width after delegate parent removal. |
| [apps/osu-radio-qt/qml/components/IconButton.qml](../../apps/osu-radio-qt/qml/components/IconButton.qml) | Focused | — | Actual keyboard activation, disabled pointer suppression and native icon URL asserted through the offline controls probe. |
| [apps/osu-radio-qt/qml/components/MaterialSurface.qml](../../apps/osu-radio-qt/qml/components/MaterialSurface.qml) | Focused | — | Foreground sizing, source/null origins, position/width invalidation and all material tint variants asserted. GPU blur remains separate. |
| [apps/osu-radio-qt/qml/components/PlaylistEditor.qml](../../apps/osu-radio-qt/qml/components/PlaylistEditor.qml) | Focused | — | Actual name editing/Return save, blank/preparing/busy gates, cover/reset/cancel forwarding, first focus, create/edit labels, retained/custom/automatic preview and responsive Save sizing asserted. |
| [apps/osu-radio-qt/qml/components/PlaylistPane.qml](../../apps/osu-radio-qt/qml/components/PlaylistPane.qml) | Focused | — | Empty/query/offline/detail states, card IDs, empty-playlist queue suppression/Edit focus, three-action menu navigation, inline editor forwarding, sorting, duplicate item identities and independent lists asserted in composed/gallery/live probes. |
| [apps/osu-radio-qt/qml/components/PlaylistTrackCard.qml](../../apps/osu-radio-qt/qml/components/PlaylistTrackCard.qml) | Focused | — | Typed available/unavailable and duplicate-audio/item-ID presentation, selection, anchored keyboard removal menu, contained/restored focus and removal of the chosen difficulty asserted in gallery and actual-server probes; GPU shadow appearance remains separate. |
| [apps/osu-radio-qt/qml/components/PlaylistsModal.qml](../../apps/osu-radio-qt/qml/components/PlaylistsModal.qml) | Focused | — | Actual target and checkbox input forwards original playlist/difficulty IDs; Add requires target plus checked candidate, busy locks controls/dismissal, focus/Tab/Escape and concrete difficulty addition asserted through composed/gallery/live probes. |
| [apps/osu-radio-qt/qml/components/QueuePanel.qml](../../apps/osu-radio-qt/qml/components/QueuePanel.qml) | Focused | — | Loading/error/retry/empty states, duplicate row presentation, visible-demand registration and removal on shrink, scrolling, placement, focus/Escape/backdrop/close and warmed artwork/delegate stability asserted. |
| [apps/osu-radio-qt/qml/components/Theme.qml](../../apps/osu-radio-qt/qml/components/Theme.qml) | Focused | — | Singleton exposes every font as Ready and Poppins as the resolved family; all bundled faces also decode through the asset probe. |
| [apps/osu-radio-qt/scripts/lint-qml.sh](../../apps/osu-radio-qt/scripts/lint-qml.sh) | Focused | — | Four Bash orchestration cases cover latest/custom imports, missing imports, failure/cleanup and failed discovery; real generated-import qmllint passed. |
| [apps/osu-radio-qt/scripts/visual-audit.py](../../apps/osu-radio-qt/scripts/visual-audit.py) | Focused | — | Seven mocked-process cases check geometry/screens/matrix, private settings/DB environment, diagnostics and timeout cleanup; four real software visual scenarios also passed. |
| [apps/osu-radio-qt/src/app_bridge.rs](../../apps/osu-radio-qt/src/app_bridge.rs) | Focused | 680/711 (95.64%) | Native model/cache/QObject destruction assertions plus live session, search, media, playlists, volume and settings offscreen probes. |
| [apps/osu-radio-qt/src/artwork.h](../../apps/osu-radio-qt/src/artwork.h) | Focused | — | Native contracts assert image pixels/scaling, replacement versions/LRU/protection/budget, track/custom/automatic/draft keys, nested provider paths and stale/reset rejection; local URL/read/format/decode/byte/pixel errors and successful error clearing are exercised. |
| [apps/osu-radio-qt/src/bridge.rs](../../apps/osu-radio-qt/src/bridge.rs) | Focused | 122/128 (95.31%) | Unknown/malformed actions and original menu indices checked by unit tests; offline gallery interaction probes exercise dispatch/projection. |
| [apps/osu-radio-qt/src/launch.rs](../../apps/osu-radio-qt/src/launch.rs) | Focused | 28/28 (100.00%) | Default/gallery/help/duplicate/unknown and Unix non-Unicode arguments checked before GUI startup. |
| [apps/osu-radio-qt/src/lib.rs](../../apps/osu-radio-qt/src/lib.rs) | Focused | — | Module/export declarations compiled through Qt binary, library tests and native bridge consumers. |
| [apps/osu-radio-qt/src/main.rs](../../apps/osu-radio-qt/src/main.rs) | Focused | 57/66 (86.36%) | Help/error exit, live/offline selection, QML roots and orderly shutdown exercised by unit/offscreen/backend probes. |
| [apps/osu-radio-qt/src/native_tests.h](../../apps/osu-radio-qt/src/native_tests.h) | Focused | — | Executed native assertions cover image/EXIF/crop/error recovery, track/playlist/automatic/draft keys, malformed/stale/reset provider state, decoded cache budgets/LRU/protection and model/QObject-lifetime contracts. |
| [apps/osu-radio-qt/src/qt_view_models.rs](../../apps/osu-radio-qt/src/qt_view_models.rs) | Focused | 1001/1074 (93.20%) | Typed model identity/media, settings/preferences, folder/playlist/player/queue projection asserted through native and QML probes. |
| [apps/osu-radio-qt/src/runtime.h](../../apps/osu-radio-qt/src/runtime.h) | Focused | — | Offscreen engine/probe setup and native preferences exercised; actual desktop screen placement/GPU/input remain separate. |
| [apps/osu-radio-qt/src/runtime.rs](../../apps/osu-radio-qt/src/runtime.rs) | Focused | 172/177 (97.18%) | New unit test verifies finished-task pruning, context clearing, controller cleanup, pending-task abort and waiting for already-started blocking work. |
| [apps/osu-radio-qt/src/view_models.h](../../apps/osu-radio-qt/src/view_models.h) | Focused | — | Separate VM/TrackListModel instances and role/update semantics exercised by native model and QML consumers. |
| [apps/osu-radio-qt/tests/AdapterProbe.qml](../../apps/osu-radio-qt/tests/AdapterProbe.qml) | Focused | — | Existing selection/list/folder/playback/search/sort/volume/playlist/preference/shutdown probes and two new offline control/menu/modal cases all passed; input counts, disabled suppression, value roles, focus and visual property bindings audited. |
| [apps/osu-radio-qt/tests/VisualProbe.qml](../../apps/osu-radio-qt/tests/VisualProbe.qml) | Focused | — | All 36 image/font resources decoded; live/gallery software visual scenarios at 1024x640 and 1440x952 plus artwork continuity probe passed. |
| [apps/osu-radio-qt/tests/fixtures/server.py](../../apps/osu-radio-qt/tests/fixtures/server.py) | Focused | — | Six direct private HTTP tests plus Qt subprocess scenarios assert startup retry, independent list failures, marker/count/import partial retry, playlist/cover and volume CRUD/retry, token/queue state, PNG CRC/pixels and visual duplicate rows; every child is reaped. |
| [apps/osu-radio-qt/tests/launch_smoke.rs](../../apps/osu-radio-qt/tests/launch_smoke.rs) | Focused | — | Latest run passes six library/binary tests and 20 ordinary offscreen scenarios; both opt-in actual-backend probes passed in the earlier Qt stage. All use private settings/sources/databases. |
| [apps/osu-radio-qt/tests/test_tools.py](../../apps/osu-radio-qt/tests/test_tools.py) | Focused | — | 12 Python checks passed: visual runner, Bash lint orchestration and complete embedded resource manifest; no Qt needed for these unit checks. |
| [apps/osu-radio-server/src/config.rs](../../apps/osu-radio-server/src/config.rs) | Focused | 17/17 (100.00%) | Environment/default/database selection and optional dotenv branches audited against seven isolated child configuration probes; both SQLite/docs suites run them, with earlier PostgreSQL-specific selection checks recorded above. |
| [apps/osu-radio-server/src/config/tests.rs](../../apps/osu-radio-server/src/config/tests.rs) | Focused | — | Seven isolated configuration probes retain child coverage and clear app configuration; the eighth ordinary check parses the repository dotenv without mutation, value diagnostics or a database connection. |
| [apps/osu-radio-server/src/docs.rs](../../apps/osu-radio-server/src/docs.rs) | Focused | — | Served Scalar path, title/version/description, exact five tags, all 25 API paths/35 methods and every local schema reference asserted by the new OpenAPI contract; docs-off URL returns 404. |
| [apps/osu-radio-server/src/error.rs](../../apps/osu-radio-server/src/error.rs) | Focused | 100/100 (100.00%) | Exact status/content-type/single-field JSON asserted for intentional borrowed/owned messages and contextual internal errors; private cause text stays out of the 500 response. |
| [apps/osu-radio-server/src/main.rs](../../apps/osu-radio-server/src/main.rs) | Focused | 23/34 (67.65%) | SQLite startup subprocesses check readiness and six failing configuration/connection/bind cases in both docs states; successful server is force-killed, so its profile is incomplete. |
| [apps/osu-radio-server/src/routes/audio_settings.rs](../../apps/osu-radio-server/src/routes/audio_settings.rs) | Focused | 461/463 (99.57%) | All DTO/validation/service/error paths audited against HTTP defaults/partial updates/absolute overrides, bounds, unknown IDs, no-op patches, null/unknown/wrong fields and exact retained settings/override after rejection; docs bounds checked. |
| [apps/osu-radio-server/src/routes/beatmap_sets.rs](../../apps/osu-radio-server/src/routes/beatmap_sets.rs) | Focused | 365/369 (98.92%) | Every response field/source kind/null/empty case compared to explicit JSON and decoded by client; live service HTTP search asserts literal Unicode/symbol filtering, no unrelated audio, clone replacement/deletion and empty library. |
| [apps/osu-radio-server/src/routes/folder_selection.rs](../../apps/osu-radio-server/src/routes/folder_selection.rs) | Focused | 95/95 (100.00%) | Explicit-root known/shallow/full NDJSON candidate paths/kind/registration/completion and relative-root rejection asserted; preview/import/reimport tests cover read-only preview, atomic duplicate/failed-refresh retention and missing ID. No unscoped discovery runs. |
| [apps/osu-radio-server/src/routes/folder_selection_tests.rs](../../apps/osu-radio-server/src/routes/folder_selection_tests.rs) | Focused | — | Three fixture flows audited and passed: only private roots and synthetic Stable data; exact candidate paths, all depths, relative errors, source refresh/duplicate/rollback and served OpenAPI are observed. |
| [apps/osu-radio-server/src/routes/media.rs](../../apps/osu-radio-server/src/routes/media.rs) | Focused | 730/743 (98.25%) | Entire source/fixture audit: Local/Copied exact bounded streaming, headers, growth cap, unavailable/online/nonregular errors with hidden paths, temporary download cleanup, extensionless duration/FIFO guards and exact JPEG bytes; cover accepts exact 16 MiB and rejects excess/directory. |
| [apps/osu-radio-server/src/routes/mod.rs](../../apps/osu-radio-server/src/routes/mod.rs) | Focused | 214/216 (99.07%) | Both router implementations checked against the same explicit 25-path/35-method contract across seven HTTP verbs, rejected unsupported methods, docs feature URL and event-response receiver release; served OpenAPI exact methods/tags/references checked alongside existing positive endpoint consumers. |
| [apps/osu-radio-server/src/routes/playback.rs](../../apps/osu-radio-server/src/routes/playback.rs) | Focused | 173/182 (95.05%) | DTO/command mappings, commit-before-notify ordering and response-owned NDJSON loop audited against atomic HTTP queue/command, token/history/duration/metadata, initial-snapshot/reconnect, stale notification, dropped receiver and unnotified heartbeat-commit assertions. |
| [apps/osu-radio-server/src/routes/playback_tests.rs](../../apps/osu-radio-server/src/routes/playback_tests.rs) | Focused | — | All six flows audited: explicit expected queue/assignment values, stale callback equality, dates/revisions, subscriber counts, real heartbeat and OpenAPI shape; synthetic media/private memory SQLite only. |
| [apps/osu-radio-server/src/routes/playlists.rs](../../apps/osu-radio-server/src/routes/playlists.rs) | Focused | 191/193 (98.96%) | Entire DTO/HTTP/service/error wiring audited against client roundtrip, CRUD/idempotence/atomic invalid additions, duplicate-audio item identity, queue retention, Unicode/date/volume projection and bounded PNG cover validation/reset contracts. |
| [apps/osu-radio-server/src/routes/playlists/tests.rs](../../apps/osu-radio-server/src/routes/playlists/tests.rs) | Focused | — | Four assertion/fixture flows audited and passed: real loopback client contract, all-route schemas, exact PNG/content-type/revision and oversized/corrupt/missing cases, stateful CRUD/queue identity and missing-record statuses. |
| [apps/osu-radio-server/src/routes/tracks.rs](../../apps/osu-radio-server/src/routes/tracks.rs) | Focused | 178/400 (44.50%) | Full ordinary/Unicode/date/volume/nullable/difficulty JSON projection asserted; compact actual HTTP results equal legacy grouping for blank/literal/Unicode/difficulty/no-match queries. Snapshot timing benchmark stays opt-in and is not a behavioral coverage claim. |
| [apps/osu-radio-server/src/routes/user_data.rs](../../apps/osu-radio-server/src/routes/user_data.rs) | Focused | 465/465 (100.00%) | Full registration/list/overview/update/delete wiring audited; 201/409 retained values, label absent/null/enabled patches, missing IDs and all four typed registration error mappings assert exact status and safe JSON. |
| [apps/osu-radio-server/src/state.rs](../../apps/osu-radio-server/src/state.rs) | Focused | 109/109 (100.00%) | Private file-SQLite reconnect asserts queue recovery to paused with retained IDs/index and invalidated counters; cloned state shares strictly monotone watch notifications through u64::MAX without mutating persistence. Source marker bytes and subscriber release asserted. |
| [apps/osu-radio-server/src/test_support.rs](../../apps/osu-radio-server/src/test_support.rs) | Focused | 72/72 (100.00%) | Fixture constructors read and checked through persisted exact DTO/search/queue/playlist consumers; only private memory SQLite and explicit synthetic markers/import snapshots are used. Helper functions never invoke game discovery implicitly. |
| [apps/osu-radio-server/tests/startup.rs](../../apps/osu-radio-server/tests/startup.rs) | Focused | — | Four actual-server scenarios passed with docs off/on, using memory SQLite, private environment and an ephemeral occupied-port fixture. |
| [crates/osu-radio-client/src/api.rs](../../crates/osu-radio-client/src/api.rs) | Focused | 850/865 (98.27%) | All 36 public HTTP operations audited against method/path/error assertions plus DTO and success consumers; new clear/volume reset/legacy search, cover and NDJSON boundaries, error taxonomy and temporary-file failure checks pass. OS write/flush exhaustion is not injected. |
| [crates/osu-radio-client/src/api/contract_tests.rs](../../crates/osu-radio-client/src/api/contract_tests.rs) | Focused | — | Seven loopback HTTP contracts: all 36 wrapper method/path/error cases, missing mutation successes, status/decode/transport/file errors, byte/line limits and partial response cleanup; executed on Linux. |
| [crates/osu-radio-client/src/api/folder_tests.rs](../../crates/osu-radio-client/src/api/folder_tests.rs) | Focused | — | Three executed stream/timeout fixture flows audited: byte-split Unicode delivery before completion, missing/invalid/truncated completion, cancellation EOF and one-hour operation override using paused time. |
| [crates/osu-radio-client/src/api/playback_tests.rs](../../crates/osu-radio-client/src/api/playback_tests.rs) | Focused | — | Three executed contracts audited: exact command JSON, byte-split Unicode/heartbeats/cancellation and invalid/truncated stream plus timeout override; real loopback only. |
| [crates/osu-radio-client/src/api/playlist_tests.rs](../../crates/osu-radio-client/src/api/playlist_tests.rs) | Focused | — | Two executed fixture consumers audited: exact PNG request/response bytes/content type and revision, missing artwork, upload bound and header/stream download bounds; no image-decoder claim. |
| [crates/osu-radio-client/src/controller.rs](../../crates/osu-radio-client/src/controller.rs) | Focused | 1586/1612 (98.39%) | Command/completion routing and single-loop lifecycle audited against 36 controller checks plus domain consumers: independent errors/selection, debounce/retry, single-flight connect, retained startup deadline, late-ready child reaping and cancelled HTTP/worker cleanup use private fixtures. |
| [crates/osu-radio-client/src/controller/benchmarks.rs](../../crates/osu-radio-client/src/controller/benchmarks.rs) | Focused | 386/693 (55.70%) | Ordinary Unix fixture runs the actual captured benchmark and both HTTP modes in private child processes: explicit legacy/compact metadata goldens, five payload variants, 30 timing rows and all 120 path/query requests; no database or media I/O, standalone entries remain opt-in. |
| [crates/osu-radio-client/src/controller/coordinator.rs](../../crates/osu-radio-client/src/controller/coordinator.rs) | Focused | 858/878 (97.72%) | VM input/effect forwarding and ownership audited against domain/controller consumers; new tests assert all duration projections/duplicate-item events, identical track precedence, seek-to-worker forwarding, stale/current file ownership, offline download failure and gate abort/closure without a device. |
| [crates/osu-radio-client/src/controller/media.rs](../../crates/osu-radio-client/src/controller/media.rs) | Focused | 443/455 (97.36%) | Scheduler branches audited against nine media flows and domain consumers: four retained decode slots, generation/serial tickets, selection/queue/visible priority, invalid rows, upgrade/reentry, cache preservation/invalidation, actual decode-failure suppression and acknowledged HTTP cancellation across generation wrap. |
| [crates/osu-radio-client/src/controller/view_model_tests.rs](../../crates/osu-radio-client/src/controller/view_model_tests.rs) | Focused | — | Thirteen assertion flows audited: independent list/item identity, controls/presentation/offline settings, seek effects and worker errors; new lookup precedence, duplicate duration/event fanout, stale/current audio ownership, public seek and blocked/closed download gate tests use a fake engine and private sessions. |
| [crates/osu-radio-client/src/controller/workflow.rs](../../crates/osu-radio-client/src/controller/workflow.rs) | Focused | 65/66 (98.48%) | VM/controller consumers exercise ordered effect delivery and request execution; direct watch-cancellation test asserts in-flight resource Drop before Cancelled completion and unchanged ready result after reset. |
| [crates/osu-radio-client/src/discord.rs](../../crates/osu-radio-client/src/discord.rs) | Focused | 345/347 (99.42%) | All presence gates, bounded UTF-8 fields, valid/absent/extreme timestamps, watch coalescing versus key-change notification, service teardown and nonce-correlated RPC/fallback/reconnect state audited against 28 deterministic mock/isolated Unix IPC checks; native Discord acceptance remains separate. |
| [crates/osu-radio-client/src/discord/tests.rs](../../crates/osu-radio-client/src/discord/tests.rs) | Focused | — | Twelve assertion flows audited: output metadata/timestamps and source gates, exact UTF-8 bounds, watcher notifications/coalescing, mock service reuse/teardown, latest-state replay, failed writes and bounded reconnect delays; tests start no installed Discord connection. |
| [crates/osu-radio-client/src/discord/tests/ipc.rs](../../crates/osu-radio-client/src/discord/tests/ipc.rs) | Focused | — | Twelve real private-Unix-socket flows audited: bounded/fragmented READY/ACK, raw PING/PONG, EOF/timeouts/cancellation, nonce/key-bound fallback, rejected clears, stale/unrelated/malformed reply shapes and reconnect delays; helpers assert version/PID/nonempty nonce and bounded reads. |
| [crates/osu-radio-client/src/discord/transport.rs](../../crates/osu-radio-client/src/discord/transport.rs) | Focused | 494/499 (99.00%) | Four transport tests plus IPC consumers assert cancellation-safe first-byte buffering, timeout/disable, disconnected operations/clear restoration/nonce wrap, six child-isolated runtime/temp/Snap/index/absence discovery cases and real byte reads; Windows named-pipe execution remains separate. |
| [crates/osu-radio-client/src/lib.rs](../../crates/osu-radio-client/src/lib.rs) | Focused | 84/84 (100.00%) | All Loading states/accessors/default/result conversions and nested error-chain deduplication asserted; public facade consumed by the full client suites. |
| [crates/osu-radio-client/src/mock.rs](../../crates/osu-radio-client/src/mock.rs) | Focused | 423/424 (99.76%) | Eight reducer tests audited: stable selection/sorting, controls/menu/search and disabled isolation; new folder flow asserts staged refresh/remove/add, partial failure/retry, count errors, pending browse guard and reset without I/O. |
| [crates/osu-radio-client/src/mock/playlists.rs](../../crates/osu-radio-client/src/mock/playlists.rs) | Focused | 519/541 (95.93%) | Four in-memory assertion flows audited: duplicate/unavailable items and edits, editor epochs and cover bounds/errors/cancellation, revision lookup/delete/serialization, sorted selection versus saved order, invalid targets and offline playback errors. |
| [crates/osu-radio-client/src/models.rs](../../crates/osu-radio-client/src/models.rs) | Focused | 260/260 (100.00%) | DTO volume boundaries/types/null/absence, compatibility defaults, mode and full-width token wire values, folder request labels/paths and created/duplicate accessors asserted; remaining DTOs exercised by HTTP/track/playlist consumers. |
| [crates/osu-radio-client/src/playback.rs](../../crates/osu-radio-client/src/playback.rs) | Focused | 843/895 (94.19%) | Ten tests audited: token/generation gates, seek/resume/cache/volume and failure feedback; real fake-engine worker thread asserts download/Started/Finished ordering, periodic ticks, stale/replaced-file deletion and joined/idempotent shutdown cleanup without audio hardware. |
| [crates/osu-radio-client/src/server.rs](../../crates/osu-radio-client/src/server.rs) | Focused | 538/541 (99.45%) | Binary priority isolated in child probes; readiness parsing, spawn/missing/early-exit/invalid-output errors, timeout/cancellation, repeated shutdown and Drop child cleanup asserted. Every error Display/source branch checked; process assertions are Linux-only. |
| [crates/osu-radio-client/src/session.rs](../../crates/osu-radio-client/src/session.rs) | Focused | 68/71 (95.77%) | Public startup/address/client/shutdown wiring exercised by real private-SQLite integration and fake-child lifecycle; missing binary maps to StartError::Server and both error variants retain display/source context. HTTP-client builder failure is not forced. |
| [crates/osu-radio-client/src/view_models/folders.rs](../../crates/osu-radio-client/src/view_models/folders.rs) | Focused | 8/8 (100.00%) | Folder state/event declarations consumed by five staged-selection tests: per-opening epochs, counts, duplicate/scoped discovery, partial apply, refresh/remove precedence and playlist refresh after library failure. |
| [crates/osu-radio-client/src/view_models/folders_tests.rs](../../crates/osu-radio-client/src/view_models/folders_tests.rs) | Focused | — | Seven fixture/assertion flows audited and run; canonical alias/count cancellation, chooser epochs, scoped candidates, staged partial apply and refresh/import failures use synthetic data and private fake sessions. |
| [crates/osu-radio-client/src/view_models/library.rs](../../crates/osu-radio-client/src/view_models/library.rs) | Focused | 317/317 (100.00%) | Request cancellation/wrap/stale acceptance, connection/error/retry state, current/off-list selection, empty search messages, monotone dates and selective volume/duration updates asserted alongside controller search/media tests. |
| [crates/osu-radio-client/src/view_models/mod.rs](../../crates/osu-radio-client/src/view_models/mod.rs) | Focused | — | Declaration/re-export surface consumed by asserted client controller/VM/HTTP suites; no independent runtime behavior. |
| [crates/osu-radio-client/src/view_models/player.rs](../../crates/osu-radio-client/src/view_models/player.rs) | Focused | 527/535 (98.50%) | Entire state/handler source audited against queue/controller and new presentation/offline/seek contracts: selected/current/loading identity, revision/token restarts and callback retry ordering, transport/worker errors, local seek and metadata/volume restoration. Physical worker creation is separate. |
| [crates/osu-radio-client/src/view_models/playlists.rs](../../crates/osu-radio-client/src/view_models/playlists.rs) | Focused | 805/854 (94.26%) | Entire owner audited against 18 tests: typed item identity, selection/search/sort/media/date/volume merges, stale epochs, editor draft/upload retry, image slots/current keys, mutation guards/results and list/deletion cleanup. New serialization check omits editor PNG bytes. |
| [crates/osu-radio-client/src/view_models/playlists/tests.rs](../../crates/osu-radio-client/src/view_models/playlists/tests.rs) | Focused | — | All 18 fixture/assertion flows audited and run: new offline/busy/oversized editor, Unicode list/error/deletion and create/rename/delete/add guards complement real loopback upload retry and stale import/detail/image contracts. Private fake sessions only. |
| [crates/osu-radio-client/src/view_models/preferences.rs](../../crates/osu-radio-client/src/view_models/preferences.rs) | Focused | — | Test-only imports/module wrapper for the three executed preference contracts; no product state ownership. |
| [crates/osu-radio-client/src/view_models/preferences/tests.rs](../../crates/osu-radio-client/src/view_models/preferences/tests.rs) | Focused | — | Four independent Unicode choices, blank fallbacks, suffix/media retention, late rows, loading selection captions and old-wire defaults asserted in three passing tests. |
| [crates/osu-radio-client/src/view_models/queue.rs](../../crates/osu-radio-client/src/view_models/queue.rs) | Focused | 113/114 (99.12%) | Offline/closed/aborted request behavior and revision-gated HTTP refetch asserted; ordered duplicate rows, preferences, media/duration restoration and error/late-response rules covered by 19 queue tests. |
| [crates/osu-radio-client/src/view_models/queue/tests.rs](../../crates/osu-radio-client/src/view_models/queue/tests.rs) | Focused | — | All 19 assertion flows audited and passed with fake worker/temporary sessions/loopback HTTP; command serialization, callback retry tokens, reconnect and independent queue demand are observed. |
| [crates/osu-radio-client/src/view_models/settings.rs](../../crates/osu-radio-client/src/view_models/settings.rs) | Focused | 863/925 (93.30%) | Entire owner audited against preference/volume/folder tests; new canonical-alias preview abort and cancellation acknowledgements, chooser single/stale/closed guards and folder getter/selection checks join existing scoped staging/partial-apply, debounce/retry and shutdown-flush contracts. |
| [crates/osu-radio-client/src/view_models/sorting.rs](../../crates/osu-radio-client/src/view_models/sorting.rs) | Focused | 55/55 (100.00%) | Every sort compares Unicode folded primary/secondary text, dates and stable IDs; library/playlist consumers preserve identity, selection, media and saved queue order. |
| [crates/osu-radio-client/src/view_models/sorting/tests.rs](../../crates/osu-radio-client/src/view_models/sorting/tests.rs) | Focused | — | Three tests assert exact sort order/empty input, unchanged media jobs/generation/duration, repeat-sort no-op and stale search/history merges; passed in both client modes. |
| [crates/osu-radio-client/src/view_models/track.rs](../../crates/osu-radio-client/src/view_models/track.rs) | Focused | 261/261 (100.00%) | Grouping/cover/lowest-map metadata, split-audio annotation/dedup/difficulty retention, filtered equivalence, fallback preferences, duration/meta labels and refreshed selection asserted by track/preference/playlist consumers. |
| [crates/osu-radio-client/src/view_models/volume.rs](../../crates/osu-radio-client/src/view_models/volume.rs) | Focused | 6/6 (100.00%) | State/change/save declarations and same-target coalescing exercised by absolute override, stale edit/read, paused assignment gating, debounce/retry and ordered shutdown-save tests. |
| [crates/osu-radio-client/src/view_models/volume/tests.rs](../../crates/osu-radio-client/src/view_models/volume/tests.rs) | Focused | — | Four fixture flows audited and passed, including exact float bits, invalid slider inputs, independent save targets and success/failure HTTP shutdown flushing without rollback. |
| [crates/osu-radio-client/tests/embedded_server.rs](../../crates/osu-radio-client/tests/embedded_server.rs) | Focused | — | Executed against a freshly built SQLite server with DB URL variables removed and private .env/database: reported random port, user/library HTTP, new/duplicate folder, null patch, restart persistence, removal and marker-byte preservation. |
| [crates/osu-radio-player/src/lib.rs](../../crates/osu-radio-player/src/lib.rs) | Focused | 178/197 (90.36%) | All transport/source lifetime helpers audited against six controlled-mixer/decoder tests: real formats, extensionless files, volume, seek/EOF/replay, failed replacement and no-callback shutdown. Physical constructor/device paths remain unverified. |
| [crates/osu-radio-player/src/tests.rs](../../crates/osu-radio-player/src/tests.rs) | Focused | — | Fixtures/assertions audited and strengthened with stereo/44100 Hz/1.4–1.7 s bounds; six decoder/mixer/state/error/release cases pass without opening an output device. |
| [crates/radio-core/src/import_types.rs](../../crates/radio-core/src/import_types.rs) | Focused | 138/138 (100.00%) | Executed: audio/background references, absent values, exact/first match and Unix native paths. |
| [crates/radio-core/src/lib.rs](../../crates/radio-core/src/lib.rs) | Focused | 26/26 (100.00%) | Executed: source-name round trips, strict rejection and error formatting. |
| [crates/radio-db/src/connection.rs](../../crates/radio-db/src/connection.rs) | Focused | 198/198 (100.00%) | Memory URL forms keep data without files; file paths/SQLite URLs enforce WAL, foreign keys and 5000 ms busy timeout with reopen/readback. PG connection path exercised by disposable contracts. |
| [crates/radio-db/src/entities/audio_source.rs](../../crates/radio-db/src/entities/audio_source.rs) | Focused | — | ORM mappings exercised by Local/Copied/Online persistence, source identity/dedup, shared cleanup and restrictive-FK assertions in repository/migration contracts. |
| [crates/radio-db/src/entities/audio_volume.rs](../../crates/radio-db/src/entities/audio_volume.rs) | Focused | — | Composite source identity, persisted percent, validation, 1002-ID batches, snapshot isolation, rollback, delete and reset asserted in tests/audio_settings.rs. |
| [crates/radio-db/src/entities/beatmap.rs](../../crates/radio-db/src/entities/beatmap.rs) | Focused | — | Shared projection contract round-trips every nullable/non-null field through insert/get/for_set and joins; library deletion and transaction rollback are asserted on SQLite/PostgreSQL. |
| [crates/radio-db/src/entities/beatmap_metadata.rs](../../crates/radio-db/src/entities/beatmap_metadata.rs) | Focused | — | Every persisted column is asserted against distinct explicit input values and search projections, including JSON author, Unicode/source/media fields; immutable/corrupt-row/hash contracts pass on both backends. |
| [crates/radio-db/src/entities/beatmap_set.rs](../../crates/radio-db/src/entities/beatmap_set.rs) | Focused | — | Full set identity/installation/nullable online/hash values and empty/populated joins asserted; per-installation deletion detaches playlist live references without erasing saved labels. |
| [crates/radio-db/src/entities/beatmap_set_tag.rs](../../crates/radio-db/src/entities/beatmap_set_tag.rs) | Focused | — | Composite links assert idempotent linking, invalid foreign keys, restrictive tag deletion, installation cascade and orphan cleanup in tag_repository_contracts. |
| [crates/radio-db/src/entities/listening_history.rs](../../crates/radio-db/src/entities/listening_history.rs) | Focused | — | Composite source identity and timestamp/token persistence asserted across migration, repeated token, read snapshot, rollback, source recreation and reset in tests/listening_history.rs. |
| [crates/radio-db/src/entities/mod.rs](../../crates/radio-db/src/entities/mod.rs) | Focused | — | Private ORM module exports compiled and consumed by the asserted repository/service contracts for all 13 application-owned tables. |
| [crates/radio-db/src/entities/osu_installation.rs](../../crates/radio-db/src/entities/osu_installation.rs) | Focused | — | Entity/model path mapping exercised with invalid-Unicode native paths and separate pools; registration, label/enabled patch semantics, deletion and cascades asserted in repository/services contracts. |
| [crates/radio-db/src/entities/playlist.rs](../../crates/radio-db/src/entities/playlist.rs) | Focused | — | Projection contract verifies create/summary/custom cover bytes and revision with retained detached items; existing migration/collection contracts assert provenance uniqueness and retained identity. |
| [crates/radio-db/src/entities/playlist_item.rs](../../crates/radio-db/src/entities/playlist_item.rs) | Focused | — | Every saved label/identity and live relationship field is asserted across add/get/item/library deletion; duplicate stable-key insertion and collection hash-kind guards remain checked by shared contracts. |
| [crates/radio-db/src/entities/queue.rs](../../crates/radio-db/src/entities/queue.rs) | Focused | — | Singleton JSON/state fields round trip across migration/reopen and transaction rollback/drop; modes, indices, revisions/tokens and playlist-item identity asserted by queue/playlist/history consumers. |
| [crates/radio-db/src/entities/tag.rs](../../crates/radio-db/src/entities/tag.rs) | Focused | — | Normalized unique names and tag IDs persist through get/link/filter/delete/cascade/cleanup assertions in tag_repository_contracts. |
| [crates/radio-db/src/entities/user_data.rs](../../crates/radio-db/src/entities/user_data.rs) | Focused | — | Singleton defaults/settings fields are read and compared across migrate/reset and independent pools; mode/percentage updates, SQL bounds and rollback asserted in tests/audio_settings.rs. |
| [crates/radio-db/src/lib.rs](../../crates/radio-db/src/lib.rs) | Focused | 152/152 (100.00%) | Pool/opaque transaction wiring audited; connect/migrate/reset, commit/explicit/drop rollback, cancelled writes, independent pools and read snapshots asserted on SQLite/PostgreSQL. Backend compile guards retain scoped builds. |
| [crates/radio-db/src/migrations/m20260913_000001_library.rs](../../crates/radio-db/src/migrations/m20260913_000001_library.rs) | Focused | 28/28 (100.00%) | Frozen schema verified on SQLite and private PostgreSQL: singleton/kind/required-field/marker/audio uniqueness, every initial foreign key, lookup indexes, absent later columns/tables and cascade of installation-owned rows while shared metadata/audio survive; populated upgrade/rollback consumers also passed. |
| [crates/radio-db/src/migrations/m20260913_000002_background.rs](../../crates/radio-db/src/migrations/m20260913_000002_background.rs) | Focused | 7/7 (100.00%) | Background column drops/reapplies on both backends while retaining the difficulty row. |
| [crates/radio-db/src/migrations/m20260914_000003_native_paths.rs](../../crates/radio-db/src/migrations/m20260914_000003_native_paths.rs) | Focused | 8/10 (80.00%) | Existing path conversion contracts plus explicit lossy-downgrade rejection with unchanged installation/history on SQLite/PostgreSQL. |
| [crates/radio-db/src/migrations/m20260919_000004_tags.rs](../../crates/radio-db/src/migrations/m20260919_000004_tags.rs) | Focused | 118/125 (94.40%) | Existing tag upgrade/hash/rollback contracts plus lifecycle rejection of irreversible downgrade with unchanged library/schema/history on both backends. |
| [crates/radio-db/src/migrations/m20260919_000004_tags_legacy.rs](../../crates/radio-db/src/migrations/m20260919_000004_tags_legacy.rs) | Focused | — | Frozen SeaORM mapping consumed by populated tag upgrade tests on SQLite/PostgreSQL: old tags/user tags, full metadata identity/author/path fields, shared hashes and all map references survive the v1-to-v2 conversion; collision rollback and rejected downgrade are asserted. |
| [crates/radio-db/src/migrations/m20261001_000005_playback_queue.rs](../../crates/radio-db/src/migrations/m20261001_000005_playback_queue.rs) | Focused | 7/7 (100.00%) | Shared SQLite/PostgreSQL lifecycle checks up/down/reapply, affected table/column/index presence, history and retained library/unrelated rows; feature-specific data contracts also passed. |
| [crates/radio-db/src/migrations/m20261001_000006_playlists.rs](../../crates/radio-db/src/migrations/m20261001_000006_playlists.rs) | Focused | 7/7 (100.00%) | Shared SQLite/PostgreSQL lifecycle checks up/down/reapply, affected table/column/index presence, history and retained library/unrelated rows; feature-specific data contracts also passed. |
| [crates/radio-db/src/migrations/m20261001_000007_listening_history.rs](../../crates/radio-db/src/migrations/m20261001_000007_listening_history.rs) | Focused | 7/7 (100.00%) | Shared SQLite/PostgreSQL lifecycle checks up/down/reapply, affected table/column/index presence, history and retained library/unrelated rows; feature-specific data contracts also passed. |
| [crates/radio-db/src/migrations/m20261001_000008_audio_settings.rs](../../crates/radio-db/src/migrations/m20261001_000008_audio_settings.rs) | Focused | 7/7 (100.00%) | Shared SQLite/PostgreSQL lifecycle checks up/down/reapply, affected table/column/index presence, history and retained library/unrelated rows; feature-specific data contracts also passed. |
| [crates/radio-db/src/migrations/m20261001_000009_playlist_covers.rs](../../crates/radio-db/src/migrations/m20261001_000009_playlist_covers.rs) | Focused | 7/7 (100.00%) | Shared SQLite/PostgreSQL lifecycle checks up/down/reapply, affected table/column/index presence, history and retained library/unrelated rows; feature-specific data contracts also passed. |
| [crates/radio-db/src/migrations/m20261003_000010_playlist_unicode_names.rs](../../crates/radio-db/src/migrations/m20261003_000010_playlist_unicode_names.rs) | Focused | 7/7 (100.00%) | Shared SQLite/PostgreSQL lifecycle checks up/down/reapply, affected table/column/index presence, history and retained library/unrelated rows; feature-specific data contracts also passed. |
| [crates/radio-db/src/migrations/m20261004_000011_imported_collections.rs](../../crates/radio-db/src/migrations/m20261004_000011_imported_collections.rs) | Focused | 7/8 (87.50%) | Shared SQLite/PostgreSQL lifecycle checks up/down/reapply, affected table/column/index presence, history and retained library/unrelated rows; feature-specific data contracts also passed. |
| [crates/radio-db/src/migrations/mod.rs](../../crates/radio-db/src/migrations/mod.rs) | Focused | 65/65 (100.00%) | Version/history/reapply assertions and existing 20 independent-pool migration/reset races passed on SQLite/PostgreSQL; configured database is excluded. |
| [crates/radio-db/src/model/audio_source.rs](../../crates/radio-db/src/model/audio_source.rs) | Focused | 37/37 (100.00%) | All source kinds round-trip exact locations; empty, unknown, case-changed and padded kinds produce the expected error. |
| [crates/radio-db/src/model/beatmap.rs](../../crates/radio-db/src/model/beatmap.rs) | Focused | — | All fields compared to independent expected values after insert/get/for_set, including native/MD5 identities, metadata/audio/cover references and all-null difficulty in shared SQLite/PostgreSQL projections. |
| [crates/radio-db/src/model/beatmap_metadata.rs](../../crates/radio-db/src/model/beatmap_metadata.rs) | Focused | — | All retained fields compared to explicit ordinary/Unicode/author/source/preview/media values after insert and read; existing hash/collision/null identity assertions also pass on both backends. |
| [crates/radio-db/src/model/beatmap_set.rs](../../crates/radio-db/src/model/beatmap_set.rs) | Focused | — | Complete set/audio/detail and search-projection expectations cover empty/populated sets, null metadata/audio, filtered multiplicity, no-audio exclusion and repeated IDs across 500-row query chunks on both backends. |
| [crates/radio-db/src/model/mod.rs](../../crates/radio-db/src/model/mod.rs) | Focused | — | Plain model exports consumed by repository/service, server and CLI contracts; behavioral projections retain their separate per-file audit status. |
| [crates/radio-db/src/model/osu_installation.rs](../../crates/radio-db/src/model/osu_installation.rs) | Focused | 3/3 (100.00%) | Registration/native path round trips and service patch assertions distinguish omitted/null/empty labels and enabled false/true while preserving the other fields. |
| [crates/radio-db/src/model/playlist.rs](../../crates/radio-db/src/model/playlist.rs) | Focused | — | All library/add/get/item/summary fields asserted with available and detached entries, saved labels/native/MD5 keys, covers/revisions and duplicate additions; repository date/volume fields are null and service enrichment is separate. |
| [crates/radio-db/src/model/queue.rs](../../crates/radio-db/src/model/queue.rs) | Focused | — | Shared migration/queue contracts compare all QueueState fields across save, reopen, rollback/drop and invalid indices; play/pause modes are also exercised by playlist/history migrations. |
| [crates/radio-db/src/model/tag.rs](../../crates/radio-db/src/model/tag.rs) | Focused | — | Tag and set-tag projections asserted through normalization, repeated link dedup, foreign-key failures, per-set reads, substring matching and cleanup contracts. |
| [crates/radio-db/src/model/user_data.rs](../../crates/radio-db/src/model/user_data.rs) | Focused | 6/6 (100.00%) | Singleton id and AudioSettings defaults/partial updates/bounds are asserted across migrations, independent pools, read snapshots, rollback and reset. |
| [crates/radio-db/src/repositories/audio_source.rs](../../crates/radio-db/src/repositories/audio_source.rs) | Focused | 75/75 (100.00%) | Local/Copied/Online identity, get/find/upsert, cleanup/restricted references and source-file preservation asserted; playable-ID filtering covers missing/online/copied IDs and large queue/playlist batches. |
| [crates/radio-db/src/repositories/audio_volume.rs](../../crates/radio-db/src/repositories/audio_volume.rs) | Focused | 54/54 (100.00%) | Source-keyed upsert/delete and 500-ID chunked joins audited against migration, read snapshot, rollback, 1002-ID, source-recreation/kind distinction, range and reset assertions on both backends. |
| [crates/radio-db/src/repositories/beatmap.rs](../../crates/radio-db/src/repositories/beatmap.rs) | Focused | 55/55 (100.00%) | Entire insert/get/for_set mapping audited; shared SQLite/PostgreSQL assertions compare every field, null/default references and ordered rows, plus absent read after fixture rollback. |
| [crates/radio-db/src/repositories/beatmap_metadata.rs](../../crates/radio-db/src/repositories/beatmap_metadata.rs) | Focused | 195/195 (100.00%) | Every persisted/search field and immutable get-or-insert/corrupt-row/cleanup path asserted; golden v2 encoding, every identity field, null/empty/author presence and excluded tags are covered on both backends. |
| [crates/radio-db/src/repositories/beatmap_set.rs](../../crates/radio-db/src/repositories/beatmap_set.rs) | Focused | 219/220 (99.55%) | Insert/get/per-installation/delete and all library/search/filtered projection paths audited against explicit rows, empty sets/null joins, multiplicity, unknown IDs and repeated IDs across query chunks on SQLite/PostgreSQL. |
| [crates/radio-db/src/repositories/listening_history.rs](../../crates/radio-db/src/repositories/listening_history.rs) | Focused | 65/65 (100.00%) | Source-keyed timestamp/token upsert, duplicate-token no-op, BIGINT conversion and chunked joins audited against snapshot, rollback, source-recreation/kind distinction and reset assertions. |
| [crates/radio-db/src/repositories/mod.rs](../../crates/radio-db/src/repositories/mod.rs) | Focused | — | Repository declarations/re-exports consumed through every Database/Transaction accessor and Services contracts; concrete SQL/error/transaction evidence is recorded per repository. |
| [crates/radio-db/src/repositories/osu_installation.rs](../../crates/radio-db/src/repositories/osu_installation.rs) | Focused | 126/126 (100.00%) | Created/duplicate identity, native/JSON-shaped path encoding, partial null/empty updates, scan timestamps, get/delete/cascade and independent-pool races asserted by shared repository/service contracts; native Windows unverified. |
| [crates/radio-db/src/repositories/playlist.rs](../../crates/radio-db/src/repositories/playlist.rs) | Focused | 415/415 (100.00%) | All mapping/query/mutation paths audited against independent projections, cover revisions, copied/local/online preference, native/MD5 keys, Unicode refresh, collection replacement/retention/rollback and missing/ownership errors on both backends. |
| [crates/radio-db/src/repositories/queue.rs](../../crates/radio-db/src/repositories/queue.rs) | Focused | 62/63 (98.41%) | Persisted JSON/state mapping and validation audited; reopen/rollback/drop, invalid indices/lengths/empty modes and out-of-BIGINT counters assert exact retained state; schema-invalid corruption remains defensive. |
| [crates/radio-db/src/repositories/tag.rs](../../crates/radio-db/src/repositories/tag.rs) | Focused | 147/147 (100.00%) | Normalization/upsert/link/get/order/bulk relationships, literal substring queries, FK restrictions, duplicate linking and cleanup audited against shared tag/search and migration/service contracts on both backends. |
| [crates/radio-db/src/repositories/user_data.rs](../../crates/radio-db/src/repositories/user_data.rs) | Focused | 55/55 (100.00%) | Singleton/default/partial settings and writer lock audited; shared missing-row fixture asserts read/update/lock errors without implicit creation, plus defaults/range/snapshot/rollback/reset contracts. |
| [crates/radio-db/src/tests.rs](../../crates/radio-db/src/tests.rs) | Focused | — | Entry/fixture/assertion audit: fresh backend guard, migration races/legacy reset scope/native paths, field projections, FK and commit/rollback/drop, immutable content, tags and new missing-singleton scenario pass on both backends. |
| [crates/radio-db/src/tests/audio_settings.rs](../../crates/radio-db/src/tests/audio_settings.rs) | Focused | — | Upgrade/read-snapshot/rollback/range/source-kind/recreation/reset fixture assertions audited; shared SQLite/PostgreSQL contract passes. |
| [crates/radio-db/src/tests/collections.rs](../../crates/radio-db/src/tests/collections.rs) | Focused | — | Legacy row/default hash kind, provenance uniqueness, native-versus-MD5 membership constraints and guarded downgrade/reapply assertions audited; shared backend contract passes. |
| [crates/radio-db/src/tests/listening_history.rs](../../crates/radio-db/src/tests/listening_history.rs) | Focused | — | Upgrade/token/date/snapshot/batching/rollback/source-lifetime/reset assertions audited against explicit saved values; shared SQLite/PostgreSQL contract passes. |
| [crates/radio-db/src/tests/migration_lifecycle.rs](../../crates/radio-db/src/tests/migration_lifecycle.rs) | Focused | — | Initial v1 constraints/indexes/cascade fixture plus additive 5-11 up/down, library/sentinel/default retention, background downgrade and lossy-path/tag guards audited and passed on SQLite and the private PostgreSQL repository contract. |
| [crates/radio-db/src/tests/playlists.rs](../../crates/radio-db/src/tests/playlists.rs) | Focused | — | Legacy populated playlist/cover/Unicode upgrade and downgrade fixtures use independent old-column SQL; retained queue, cover revision, source preference, Unicode snapshots, cascade and reset assertions pass on both backends. |
| [crates/radio-db/src/tests/queue.rs](../../crates/radio-db/src/tests/queue.rs) | Focused | — | Same populated migrated row checked across reopen, rollback/drop, index rules and six new invalid-save variants; rejected saves retain exact state on both backends. |
| [crates/radio-db/src/tests/tag_migration.rs](../../crates/radio-db/src/tests/tag_migration.rs) | Focused | — | Independent v1 SQL/tuple fixtures audited against normalized tags/links, immutable v2 merge, retained paths/maps/index/history, repeated/concurrent upgrade, refused down and forced collision rollback; prior shared SQLite/PostgreSQL run passes unchanged code. |
| [crates/radio-scanner/build.rs](../../crates/radio-scanner/build.rs) | Focused | 34/44 (77.27%) | Five actual-build-script subprocess contracts: override bypass, exact build args/env/Cargo output, absent dotnet, nonzero exit and missing helper; fake dotnet on POSIX. |
| [crates/radio-scanner/src/discovery/known.rs](../../crates/radio-scanner/src/discovery/known.rs) | Focused | 282/285 (98.95%) | Known-root/relocation/dedup and 64 KiB UTF-8 regular-file boundary assertions; isolated HOME/XDG Linux default-path cases without a sweep; other OS branches unverified. |
| [crates/radio-scanner/src/discovery/mod.rs](../../crates/radio-scanner/src/discovery/mod.rs) | Focused | 294/305 (96.39%) | Explicit-root known/shallow/full tiers, kind/limit/callback stop, cross-tier dedup, sync/async equivalence, drop flag, canonical aliases retaining the first path and symlink/missing-root cases; no machine sweep. |
| [crates/radio-scanner/src/discovery/sweep.rs](../../crates/radio-scanner/src/discovery/sweep.rs) | Focused | 226/234 (96.58%) | Walk assertions cover overlapping roots in either order, per-root shallow depth, explicit roots under pruned directories, Git/global ignore independence and no symlink traversal; Unix exclusions tested, Windows branches unverified. |
| [crates/radio-scanner/src/lazer/mod.rs](../../crates/radio-scanner/src/lazer/mod.rs) | Focused | — | Module/re-export declarations compiled through public helper APIs and CLI. |
| [crates/radio-scanner/src/lazer/scanner.rs](../../crates/radio-scanner/src/lazer/scanner.rs) | Focused | 96/111 (86.49%) | Helper arguments/order, pipes, diagnostics, invalid stdout/stderr, failure and Linux kill/reap/cancellation assertions; runtime override and collection validation through default/set-only APIs. OS pipe/forwarding failure injection remains limited. |
| [crates/radio-scanner/src/lazer/tests.rs](../../crates/radio-scanner/src/lazer/tests.rs) | Focused | — | 72 scanner tests passed; parser/process and five genuine-Realm tests assert metadata/media/schema/CLI/source preservation. Linux process cleanup and isolated runtime wrappers are covered; C# lines unmeasured. |
| [crates/radio-scanner/src/lazer/types.rs](../../crates/radio-scanner/src/lazer/types.rs) | Focused | 99/99 (100.00%) | Full-field mapping plus optional defaults, required/type validation, every Guid character and MD5; real Realm round trip remains synthetic. |
| [crates/radio-scanner/src/lib.rs](../../crates/radio-scanner/src/lib.rs) | Focused | 51/51 (100.00%) | Strict ASCII MD5 validation, compatibility error formatting and explicit Stable/Lazer helper dispatch; default Lazer entry exercised through CLI. |
| [crates/radio-scanner/src/stable/media.rs](../../crates/radio-scanner/src/stable/media.rs) | Focused | 158/165 (95.76%) | Assertions cover safe relative references, custom/absolute/conflicting/unreadable configs, Linux nearest-Wine mappings and background event quoting/malformed artwork fallback. Native Windows/macOS branches unverified. |
| [crates/radio-scanner/src/stable/mod.rs](../../crates/radio-scanner/src/stable/mod.rs) | Focused | 91/93 (97.85%) | Public dispatch/group/order/set-ID/file-dedup and resolved references checked; collections retain order/identity and compatibility wrappers validate complete snapshots. Missing/unreadable sources fail without changing fixture bytes. |
| [crates/radio-scanner/src/stable/reader.rs](../../crates/radio-scanner/src/stable/reader.rs) | Focused | 308/316 (97.47%) | Version/layout/tag-width boundaries, full truncation sweeps, lengths/counts/tags/UTF-8/booleans/footer checks, field mapping, weighted BPM and collection corruption/order/MD5 asserted against binary fixtures. |
| [crates/radio-scanner/src/stable/tests.rs](../../crates/radio-scanner/src/stable/tests.rs) | Focused | — | Synthetic binary/media fixtures audited against parser/import assertions, including collection and configuration read failures with source preservation; original game libraries are not read. |
| [crates/radio-services/src/audio_settings.rs](../../crates/radio-services/src/audio_settings.rs) | Focused | 79/79 (100.00%) | Validation/partial updates and source-keyed fill/remove audited against exact 0/100 readback, typed invalid/unknown errors, no-op patch, playlist/track/assignment projections, reopen/reimport and rollback assertions. |
| [crates/radio-services/src/audio_source.rs](../../crates/radio-services/src/audio_source.rs) | Focused | 16/16 (100.00%) | Thin get/find/get-or-insert/cleanup delegation audited against shared Local/Copied/Online identity, independent-pool reads, cleanup and source-file preservation service assertions. |
| [crates/radio-services/src/beatmap.rs](../../crates/radio-services/src/beatmap.rs) | Focused | 27/27 (100.00%) | Thin add/get/per-set forwarding audited against shared snapshot/replacement/get/ID/BPM/reference/rollback assertions; repository tests independently compare every persisted field. |
| [crates/radio-services/src/beatmap_metadata.rs](../../crates/radio-services/src/beatmap_metadata.rs) | Focused | 12/12 (100.00%) | Thin get/get-or-insert/cleanup delegation audited against shared identity reuse, JSON author, immutable rows, replacement and shared/orphan cleanup assertions; repository field contract gives explicit expected values. |
| [crates/radio-services/src/beatmap_set.rs](../../crates/radio-services/src/beatmap_set.rs) | Focused | 145/145 (100.00%) | Service/query/matching logic audited against get/per-installation/snapshot consumers and complete search cases: Unicode/literals, AND across global audio, null matches, deduplicated words, multiplicity, read snapshots and 1002-source chunks. |
| [crates/radio-services/src/beatmap_set/benchmarks.rs](../../crates/radio-services/src/beatmap_set/benchmarks.rs) | Focused | 593/601 (98.67%) | Private six-set SQLite fixture asserts tag/difficulty/Unicode/empty search counts and all three snapshot reference stages while preserving rows and a sentinel; the opt-in bulk benchmark runs separately on temporary 1k/10k/50k sets. Timings are debug/instrumented correctness evidence only. |
| [crates/radio-services/src/beatmap_set/tracks.rs](../../crates/radio-services/src/beatmap_set/tracks.rs) | Focused | 144/144 (100.00%) | New pure full-structure assertion covers first-seen audio order, cross-set grouping, globally lowest-ID labels, independent cover choice, every difficulty owner/multiplicity and empty input; SQLite/PostgreSQL feature test passes. |
| [crates/radio-services/src/folder_selection.rs](../../crates/radio-services/src/folder_selection.rs) | Focused | 180/182 (98.90%) | All preview/discovery/import/register/reimport paths audited: read-only previews, aliases/duplicates, atomic import and read/write boundary; new exact invalid-marker errors and stale-kind/path rejection retain all prior data (SQLite fixtures). |
| [crates/radio-services/src/lib.rs](../../crates/radio-services/src/lib.rs) | Focused | 69/69 (100.00%) | Facade/accessor/cloneable database wiring audited through explicit connect/migrate/reset, all model services and shared contract counts/transaction/rollback/source-preservation scenarios; ORM handles remain private. |
| [crates/radio-services/src/listening_history.rs](../../crates/radio-services/src/listening_history.rs) | Focused | 54/54 (100.00%) | Current-token acknowledgements, Unix-millisecond bounds, duplicate/stale/paused acknowledgements, track/playlist/assignment fill and durable source-keyed reimport/reset assertions audited on both backends. |
| [crates/radio-services/src/osu_installation.rs](../../crates/radio-services/src/osu_installation.rs) | Focused | 367/368 (99.73%) | Registration/update/delete and shared replacement internals audited against source-kind validation, counts/shared references, tags/collections/Unicode refresh, empty replacement, rollback/cancellation, source preservation and concurrent import/delete assertions. |
| [crates/radio-services/src/playlist.rs](../../crates/radio-services/src/playlist.rs) | Focused | 214/220 (97.27%) | Names/mutations/play launch/cover validation and fill audited against copy/native/MD5/Unicode/503-item cases, detached/live snapshots, typed deleted-playlist errors and damaged-PNG rejection with retained state. |
| [crates/radio-services/src/queue.rs](../../crates/radio-services/src/queue.rs) | Focused | 393/405 (97.04%) | All command/recovery/upcoming/assignment helpers audited against queue order, token-bound callbacks, pause/stop/failure, concurrency, missing sources, metadata and new persisted-counter-limit rollback assertions. |
| [crates/radio-services/src/tag.rs](../../crates/radio-services/src/tag.rs) | Focused | 42/43 (97.67%) | Whole import/normalization/delegation path audited against Unicode lowercasing, whitespace, whole user tags, BTreeSet ordering/global identity, missing metadata, repeated/empty replacements and shared cleanup assertions. |
| [crates/radio-services/src/tests.rs](../../crates/radio-services/src/tests.rs) | Focused | — | Complete fixture/helper/assertion audit: two-pool registration, omitted/null updates, snapshot/shared cleanup, forced-failure and cancellation rollback, concurrent replacement/deletion, source-kind identity and unchanged temporary source files; full SQLite and fresh private PostgreSQL contracts passed. |
| [crates/radio-services/src/tests/audio_settings.rs](../../crates/radio-services/src/tests/audio_settings.rs) | Focused | — | Scenario/assertions audited: default/partial/no-op settings, exact 0/100 override readback, typed validation errors and unchanged override, shared projections, reopen/removal/reimport and source cleanup. |
| [crates/radio-services/src/tests/collections.rs](../../crates/radio-services/src/tests/collections.rs) | Focused | — | Minimal/full/missing/imported/manual collection scenarios audited for native/MD5 collisions, ordering/IDs/provenance, repeat/omission, forced rollback, source removal/reinstall and empty/renamed Stable identities. |
| [crates/radio-services/src/tests/folder_selection.rs](../../crates/radio-services/src/tests/folder_selection.rs) | Focused | — | Seven SQLite tests with explicit roots/markers audited; new invalid marker and stale identity cases assert error variants plus retained rows/source bytes, alongside helper-controlled deletion/settings/read boundaries. |
| [crates/radio-services/src/tests/listening_history.rs](../../crates/radio-services/src/tests/listening_history.rs) | Focused | — | Scenario/assertions audited including measured Unix-ms acknowledgement bounds, duplicate/stale/concurrent callbacks, pause/resume/failure, projections and source/reimport/relocation/reset lifetimes. |
| [crates/radio-services/src/tests/playlists.rs](../../crates/radio-services/src/tests/playlists.rs) | Focused | — | Fixtures/assertions audited for mutations/atomic add/play, names/covers including damaged pixels, exact missing-playlist errors, source-copy resolution, 503 items, Unicode refresh/rollback and detached data. |
| [crates/radio-services/src/tests/queue.rs](../../crates/radio-services/src/tests/queue.rs) | Focused | — | All helper assertions audited: atomic validation, order/history/navigation, guarded callbacks, concurrency, recovery, missing sources and metadata; new BIGINT ceiling cases preserve complete state and assignment. |
| [crates/radio-services/src/tests/search.rs](../../crates/radio-services/src/tests/search.rs) | Focused | — | Fixture/assertion audit covers literal/Unicode/AND/global-audio semantics, preserved unrelated references, transaction snapshot versus writer replacement and 70-word/1002-source chunk cases; SQLite service run passes. |
| [crates/radio-services/src/tests/tags.rs](../../crates/radio-services/src/tests/tags.rs) | Focused | — | Fixture/assertion audit covers normalization/order/global identity and links over Lazer/Stable, repeat/remove/empty/missing metadata cases, plus tag identity independent of immutable metadata; SQLite service run passes. |
| [crates/radio-services/src/user_data.rs](../../crates/radio-services/src/user_data.rs) | Focused | 20/20 (100.00%) | Settings singleton/get/overview and transaction lock delegation exercised by fresh-database, registration, populated overview and replacement concurrency/rollback assertions. |
| [tools/osu-lazer-realm-parser/Program.cs](../../tools/osu-lazer-realm-parser/Program.cs) | Focused | — | Actual helper tested on minimal/rich Realm: collection validation, metadata/author/tags, numeric overflow, file-path boundaries, CLI errors and sorted typed schema output with source-byte preservation; C# lines not instrumented. |
| [tools/osu-lazer-realm-parser/fixture-generator/Program.cs](../../tools/osu-lazer-realm-parser/fixture-generator/Program.cs) | Focused | — | Fixture generator compiled and run in TempDir by four real-helper tests; output covers minimal/rich/missing-set and corrupt-collection schemas, metadata/file/null/overflow boundaries and all schema collection kinds. |
| [tools/search-benchmark/benchmark_http.py](../../tools/search-benchmark/benchmark_http.py) | Focused | — | Eight unittest scenarios verify requests, warmup/samples, raw payloads, baseline comparison and child cleanup using mocks; Python trace: 57/57 executable lines. |
| [tools/search-benchmark/test_benchmark_http.py](../../tools/search-benchmark/test_benchmark_http.py) | Focused | — | Eight unittest scenarios passed; runpy exercises the actual script with isolated paths and controlled HTTP/process objects. |

## Configuration and build metadata

| File | Audit | Observed lines | Evidence / remaining action |
| --- | --- | --- | --- |
| [.agents/skills/cr/agents/openai.yaml](../../.agents/skills/cr/agents/openai.yaml) | Focused | — | Static metadata consumer verifies the interface fields, nonempty labels and default-prompt $cr token against the existing SKILL.md name; no skill execution or external integration is claimed. |
| [.env](../../.env) | Focused | — | Actual dotenvy iterator parses the current checkout file without loading values; the test asserts unchanged process environment, prints no contents and opens no configured database. Missing optional checkout files skip this read. |
| [.github/workflows/linux-ci.yml](../../.github/workflows/linux-ci.yml) | Focused | — | Actionlint 1.7.12 accepted syntax/expressions locally; regression checks validate Bash syntax, job/toolchain wiring, all scoped package/feature/test/Clippy variants, Python consumers and required locking/serialization. Hosted execution and external linter integrations remain unverified. |
| [.gitignore](../../.gitignore) | Focused | — | Actual git check-ignore verifies output/database/cache patterns and agent-guide/source exceptions; already tracked cache is preserved. |
| [.gitmodules](../../.gitmodules) | Focused | — | Git successfully parses the empty file and the current index contains no 160000 gitlinks; metadata regression guards the empty/no-submodule state without fetching or initializing anything. |
| [.serena/.gitignore](../../.serena/.gitignore) | Focused | — | Real git check-ignore --no-index verifies root cache/project.local.yml exclusion and nested lookalike retention; .serena/project.yml remains visible. |
| [.serena/project.yml](../../.serena/project.yml) | Focused | — | Selected committed scalar/list guards verify project/encoding, Rust server, local root and no additional workspaces/startup command, ignore integration and positive activation timeout; source inspected, Serena not launched and general schema parsing is not claimed. |
| [Cargo.lock](../../Cargo.lock) | Focused | — | Workspace package names/versions match local lock entries; scoped locked SQLite/PostgreSQL/docs/mock/Qt builds consume the dependency graph. |
| [Cargo.toml](../../Cargo.toml) | Focused | — | Metadata tests match workspace members to real packages and lock entries and validate local dependency names/paths and declared targets; compilation verifies supported feature selections. |
| [apps/osu-radio-cli/.gitignore](../../apps/osu-radio-cli/.gitignore) | Focused | — | Real Git consumer excludes this crate target/probe while target-like source names remain visible; no generated files are created. |
| [apps/osu-radio-cli/Cargo.toml](../../apps/osu-radio-cli/Cargo.toml) | Focused | — | Repository metadata checks validate membership, local dependency paths/names, lock entries and declared source/build targets; scoped builds/tests consume the manifest. |
| [apps/osu-radio-qt/.gitignore](../../apps/osu-radio-qt/.gitignore) | Focused | — | Real Git consumer excludes root .qmlls.ini, preserves a nested lookalike and source entries; the existing CXX-Qt generated-path comment is retained. |
| [apps/osu-radio-qt/Cargo.toml](../../apps/osu-radio-qt/Cargo.toml) | Focused | — | Repository metadata checks validate membership, local dependency paths/names, lock entries and declared source/build targets; scoped builds/tests consume the manifest. |
| [apps/osu-radio-qt/qml/components/qmldir](../../apps/osu-radio-qt/qml/components/qmldir) | Focused | — | Local Theme singleton declaration consumed by production component/gallery imports; offline component and asset probes assert successful font loading. |
| [apps/osu-radio-server/Cargo.toml](../../apps/osu-radio-server/Cargo.toml) | Focused | — | Repository metadata checks validate membership, local dependency paths/names, lock entries and declared source/build targets; scoped builds/tests consume the manifest. |
| [crates/osu-radio-client/Cargo.toml](../../crates/osu-radio-client/Cargo.toml) | Focused | — | Repository metadata checks validate membership, local dependency paths/names, lock entries and declared source/build targets; scoped builds/tests consume the manifest. |
| [crates/osu-radio-player/Cargo.toml](../../crates/osu-radio-player/Cargo.toml) | Focused | — | Repository metadata checks validate membership, local dependency paths/names, lock entries and declared source/build targets; scoped builds/tests consume the manifest. |
| [crates/radio-core/.gitignore](../../crates/radio-core/.gitignore) | Focused | — | Real Git consumer excludes this crate target/probe while target-like source names remain visible; no generated files are created. |
| [crates/radio-core/Cargo.toml](../../crates/radio-core/Cargo.toml) | Focused | — | Repository metadata checks validate membership, local dependency paths/names, lock entries and declared source/build targets; scoped builds/tests consume the manifest. |
| [crates/radio-db/Cargo.toml](../../crates/radio-db/Cargo.toml) | Focused | — | Repository metadata checks validate membership, local dependency paths/names, lock entries and declared source/build targets; scoped builds/tests consume the manifest. |
| [crates/radio-db/src/migrations/.diesel_lock](../../crates/radio-db/src/migrations/.diesel_lock) | Focused | — | Zero-byte legacy artifact is explicitly checked as inert/empty; current SeaORM startup/migration consumers do not use a Diesel lock. No executable behavior is invented for it. |
| [crates/radio-scanner/.gitignore](../../crates/radio-scanner/.gitignore) | Focused | — | Real Git consumer excludes this crate target/probe while target-like source names remain visible; no generated files are created. |
| [crates/radio-scanner/Cargo.toml](../../crates/radio-scanner/Cargo.toml) | Focused | — | Repository metadata checks validate membership, local dependency paths/names, lock entries and declared source/build targets; scoped builds/tests consume the manifest. |
| [crates/radio-services/Cargo.toml](../../crates/radio-services/Cargo.toml) | Focused | — | Repository metadata checks validate membership, local dependency paths/names, lock entries and declared source/build targets; scoped builds/tests consume the manifest. |
| [rust-toolchain.toml](../../rust-toolchain.toml) | Focused | — | tomllib verifies exact-version channel form, minimal profile and rustfmt/Clippy components; the CI guard requires repository-based installation in all three jobs, and live rustup resolves the current pin to 1.98.0. |
| [tools/osu-lazer-realm-parser/FodyWeavers.xml](../../tools/osu-lazer-realm-parser/FodyWeavers.xml) | Focused | — | Full producer rebuild runs Fody/Realm and succeeds with the expected empty default-schema warning; producer opens externally shaped fixtures dynamically. |
| [tools/osu-lazer-realm-parser/fixture-generator/fixture-generator.csproj](../../tools/osu-lazer-realm-parser/fixture-generator/fixture-generator.csproj) | Focused | — | Actual isolated Release build/weaving produces genuine Realm fixtures consumed by production helper and Rust assertions; no repository dependency changes. |
| [tools/osu-lazer-realm-parser/osu-lazer-realm-parser.csproj](../../tools/osu-lazer-realm-parser/osu-lazer-realm-parser.csproj) | Focused | — | Full Release rebuild succeeds while excluding fixture-generator sources; actual helper process contracts pass. NuGet audit endpoint and empty dynamic-schema warnings documented. |

## Guidance, assets and fixtures

| File | Audit | Observed lines | Evidence / remaining action |
| --- | --- | --- | --- |
| [.agents/skills/cr/SKILL.md](../../.agents/skills/cr/SKILL.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [.agents/skills/cr/references/rust-review-checklist.md](../../.agents/skills/cr/references/rust-review-checklist.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [.agents/skills/osu-radio-api/SKILL.md](../../.agents/skills/osu-radio-api/SKILL.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [.agents/skills/osu-radio-gui/SKILL.md](../../.agents/skills/osu-radio-gui/SKILL.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [.agents/skills/osu-radio-scanner/SKILL.md](../../.agents/skills/osu-radio-scanner/SKILL.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [AGENTS.md](../../AGENTS.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [CLAUDE.md](../../CLAUDE.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [apps/osu-radio-qt/assets/SOURCES.md](../../apps/osu-radio-qt/assets/SOURCES.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [apps/osu-radio-qt/assets/fonts/Nunito-Variable.ttf](../../apps/osu-radio-qt/assets/fonts/Nunito-Variable.ttf) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/fonts/OFL.txt](../../apps/osu-radio-qt/assets/fonts/OFL.txt) | Focused | — | Full supplied UTF-8 notice content pinned by SHA-256 with normalized newlines; SOURCES.md linkage asserted. This is payload integrity, not provenance certification. |
| [apps/osu-radio-qt/assets/fonts/Poppins-Bold.ttf](../../apps/osu-radio-qt/assets/fonts/Poppins-Bold.ttf) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/fonts/Poppins-Medium.ttf](../../apps/osu-radio-qt/assets/fonts/Poppins-Medium.ttf) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/fonts/Poppins-OFL.txt](../../apps/osu-radio-qt/assets/fonts/Poppins-OFL.txt) | Focused | — | Full supplied UTF-8 notice content pinned by SHA-256 with normalized newlines; SOURCES.md linkage asserted. This is payload integrity, not provenance certification. |
| [apps/osu-radio-qt/assets/fonts/Poppins-Regular.ttf](../../apps/osu-radio-qt/assets/fonts/Poppins-Regular.ttf) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/fonts/Poppins-SemiBold.ttf](../../apps/osu-radio-qt/assets/fonts/Poppins-SemiBold.ttf) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/LUCIDE-LICENSE](../../apps/osu-radio-qt/assets/icons/LUCIDE-LICENSE) | Focused | — | Full supplied UTF-8 notice content pinned by SHA-256 with normalized newlines; SOURCES.md linkage asserted. This is payload integrity, not provenance certification. |
| [apps/osu-radio-qt/assets/icons/REMIX-LICENSE](../../apps/osu-radio-qt/assets/icons/REMIX-LICENSE) | Focused | — | Full supplied UTF-8 notice content pinned by SHA-256 with normalized newlines; SOURCES.md linkage asserted. This is payload integrity, not provenance certification. |
| [apps/osu-radio-qt/assets/icons/add-to-queue.svg](../../apps/osu-radio-qt/assets/icons/add-to-queue.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/arrow-down-s-line.svg](../../apps/osu-radio-qt/assets/icons/arrow-down-s-line.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/arrow-left.svg](../../apps/osu-radio-qt/assets/icons/arrow-left.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/chevron-down.svg](../../apps/osu-radio-qt/assets/icons/chevron-down.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/circle-plus.svg](../../apps/osu-radio-qt/assets/icons/circle-plus.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/copy.svg](../../apps/osu-radio-qt/assets/icons/copy.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/layers.svg](../../apps/osu-radio-qt/assets/icons/layers.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/minus.svg](../../apps/osu-radio-qt/assets/icons/minus.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/music.svg](../../apps/osu-radio-qt/assets/icons/music.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/pause.svg](../../apps/osu-radio-qt/assets/icons/pause.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/pencil-line.svg](../../apps/osu-radio-qt/assets/icons/pencil-line.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/pencil.svg](../../apps/osu-radio-qt/assets/icons/pencil.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/play.svg](../../apps/osu-radio-qt/assets/icons/play.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/playlist-delete.svg](../../apps/osu-radio-qt/assets/icons/playlist-delete.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/playlist-edit.svg](../../apps/osu-radio-qt/assets/icons/playlist-edit.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/plus.svg](../../apps/osu-radio-qt/assets/icons/plus.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/repeat-2.svg](../../apps/osu-radio-qt/assets/icons/repeat-2.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/rotate-cw.svg](../../apps/osu-radio-qt/assets/icons/rotate-cw.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/search-line.svg](../../apps/osu-radio-qt/assets/icons/search-line.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/search.svg](../../apps/osu-radio-qt/assets/icons/search.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/settings-4-line.svg](../../apps/osu-radio-qt/assets/icons/settings-4-line.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/settings.svg](../../apps/osu-radio-qt/assets/icons/settings.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/shuffle.svg](../../apps/osu-radio-qt/assets/icons/shuffle.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/skip-back.svg](../../apps/osu-radio-qt/assets/icons/skip-back.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/skip-forward.svg](../../apps/osu-radio-qt/assets/icons/skip-forward.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/square.svg](../../apps/osu-radio-qt/assets/icons/square.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/volume-2.svg](../../apps/osu-radio-qt/assets/icons/volume-2.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/volume-up-fill.svg](../../apps/osu-radio-qt/assets/icons/volume-up-fill.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/icons/x.svg](../../apps/osu-radio-qt/assets/icons/x.svg) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/logos/lazer.png](../../apps/osu-radio-qt/assets/logos/lazer.png) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/assets/logos/stable.png](../../apps/osu-radio-qt/assets/logos/stable.png) | Focused | — | Loaded from the compiled Qt resource bundle in the offline native-assets probe; image decode/dimensions or font family asserted. Manifest matches all image/font files. |
| [apps/osu-radio-qt/tests/fixtures/__pycache__/server.cpython-314.pyc](../../apps/osu-radio-qt/tests/fixtures/__pycache__/server.cpython-314.pyc) | Focused | — | Retained artifact executes sorting/media/playlist CRUD in its own private HTTP subprocess under matching Python 3.14; header and unchanged bytes checked. This does not claim parity with newer source fixes; execution skips on a different bytecode magic. |
| [apps/osu-radio-server/tests/fixtures/README.md](../../apps/osu-radio-server/tests/fixtures/README.md) | Focused | — | Document structure/fences checked; actual extensionless WAV/MP3/Ogg probing asserts the documented 200–350 ms range. Original generation/provenance is supplied documentation. |
| [apps/osu-radio-server/tests/fixtures/karakara.jpg](../../apps/osu-radio-server/tests/fixtures/karakara.jpg) | Focused | — | Actual JPEG fixture stored by reference and returned byte-for-byte by the cover route, with missing-file/unknown-ID handling; server does not transform the original artwork. |
| [apps/osu-radio-server/tests/fixtures/tone.mp3](../../apps/osu-radio-server/tests/fixtures/tone.mp3) | Focused | — | Actual fixture copied to an extensionless path; server media probe asserts 200–350 ms duration. MP3 also participates in exact bounded HTTP/download-byte assertions. |
| [apps/osu-radio-server/tests/fixtures/tone.ogg](../../apps/osu-radio-server/tests/fixtures/tone.ogg) | Focused | — | Actual fixture copied to an extensionless path; server media probe asserts 200–350 ms duration. MP3 also participates in exact bounded HTTP/download-byte assertions. |
| [apps/osu-radio-server/tests/fixtures/tone.wav](../../apps/osu-radio-server/tests/fixtures/tone.wav) | Focused | — | Actual fixture copied to an extensionless path; server media probe asserts 200–350 ms duration. MP3 also participates in exact bounded HTTP/download-byte assertions. |
| [crates/osu-radio-player/tests/fixtures/README.md](../../crates/osu-radio-player/tests/fixtures/README.md) | Focused | — | Document structure/fences and exact inline tone-name inventory checked; decoder assertions confirm documented channels/rate/duration and controlled-mixer use. FFmpeg generation/provenance is not rerun. |
| [crates/osu-radio-player/tests/fixtures/cbr.mp3](../../crates/osu-radio-player/tests/fixtures/cbr.mp3) | Focused | — | Actual decoder and extensionless-copy consumer checks stereo/44100 Hz/1.4–1.7 s duration and nonzero samples; mixer seek/EOF/replacement cases consume this file without physical output. |
| [crates/osu-radio-player/tests/fixtures/pcm.wav](../../crates/osu-radio-player/tests/fixtures/pcm.wav) | Focused | — | Actual decoder and extensionless-copy consumer checks stereo/44100 Hz/1.4–1.7 s duration and nonzero samples; mixer seek/EOF/replacement cases consume this file without physical output. |
| [crates/osu-radio-player/tests/fixtures/vbr.mp3](../../crates/osu-radio-player/tests/fixtures/vbr.mp3) | Focused | — | Actual decoder and extensionless-copy consumer checks stereo/44100 Hz/1.4–1.7 s duration and nonzero samples; mixer seek/EOF/replacement cases consume this file without physical output. |
| [crates/osu-radio-player/tests/fixtures/vorbis.ogg](../../crates/osu-radio-player/tests/fixtures/vorbis.ogg) | Focused | — | Actual decoder and extensionless-copy consumer checks stereo/44100 Hz/1.4–1.7 s duration and nonzero samples; mixer seek/EOF/replacement cases consume this file without physical output. |
| [docs/agent/architecture.md](../../docs/agent/architecture.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [docs/agent/backend.md](../../docs/agent/backend.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [docs/agent/database.md](../../docs/agent/database.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [docs/agent/development.md](../../docs/agent/development.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [docs/agent/frontend.md](../../docs/agent/frontend.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [docs/agent/index.md](../../docs/agent/index.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [docs/agent/scanner.md](../../docs/agent/scanner.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [docs/agent/search-performance.md](../../docs/agent/search-performance.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [docs/agent/test-coverage.md](../../docs/agent/test-coverage.md) | Focused | — | Every document checked for nonempty UTF-8 text and closed example fences, plus local file/heading links; prose facts/external URLs are not automatically verified. |
| [tools/osu-lazer-realm-parser/README.md](../../tools/osu-lazer-realm-parser/README.md) | Focused | — | Local source/test links checked; documented real-helper CLI, schema, import and fixture claims have matching executed assertions. External/platform setup remains separate. |

| [crates/radio-scanner/tests/test_build_helper.py](../../crates/radio-scanner/tests/test_build_helper.py) | Focused | — | Five subprocess contracts execute the real std-only build script against private fake dotnet; POSIX only. |
| [tests/test_repository.py](../../tests/test_repository.py) | Focused | — | Fifteen stdlib checks audited and passed: Markdown fences/links/attribution/fixtures, manifest/lock/entry points, full inventory, Git ignore/empty legacy state, toolchain, CR/Serena field guards and the scoped CI command matrix/Bash syntax. |

| [crates/radio-db/src/tests/model_projections.rs](../../crates/radio-db/src/tests/model_projections.rs) | Focused | — | Shared explicit-value field assertions for persisted/library/search/playlist projections and rollback; passed inside SQLite and fresh disposable PostgreSQL repository suites. |
