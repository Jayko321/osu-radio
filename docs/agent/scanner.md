# Scanner and import boundary

Read [the project map](index.md) first. This guide describes implementation checked against the source; commands and prerequisites are in [development](development.md). Use the [scanner skill](../../.agents/skills/osu-radio-scanner/SKILL.md) for the change procedure.

The scanner discovers installations and returns imported domain values. It does not choose what the application retains, modify osu! data, copy audio, or own playback. Database internals and persistence workflows belong to [database](database.md); scanner contracts do not decide persistence.

The confirmed product direction is desktop music playback from local osu! installations and eventually hosted sources. Hosted providers and their import representation remain undecided. Linux and Windows are current platforms; macOS discovery branches exist but are unverified, and mobile support is future work.

## Ownership and entry points

| Change | Start here | Boundary |
| --- | --- | --- |
| Shared source identity and imported values | [`radio-core/src/lib.rs`](../../crates/radio-core/src/lib.rs), [`import_types.rs`](../../crates/radio-core/src/import_types.rs) | Domain data and pure lookup only; no filesystem operations or persistence. |
| Discovery policy, streaming, filtering, limits | [`discovery/mod.rs`](../../crates/radio-scanner/src/discovery/mod.rs) | `find_osu_markers_with` is the synchronous callback core; `discover` adapts it for async callers. |
| Known installation locations and relocation | [`discovery/known.rs`](../../crates/radio-scanner/src/discovery/known.rs) | OS-specific candidate generation and `storage.ini` reading. |
| Walking and pruning | [`discovery/sweep.rs`](../../crates/radio-scanner/src/discovery/sweep.rs) | Marker names, OS scan roots, directory exclusion policy. |
| Source dispatch | [`radio-scanner/src/lib.rs`](../../crates/radio-scanner/src/lib.rs) | `get_beatmap_sets` selects the source reader or returns `UnsupportedSourceError`. |
| Helper invocation and cleanup | [`lazer/scanner.rs`](../../crates/radio-scanner/src/lazer/scanner.rs) | Process lifetime, pipes, parsing loop, diagnostic propagation. |
| Helper output mapping | [`lazer/types.rs`](../../crates/radio-scanner/src/lazer/types.rs) | Private wire records map into public core values. |
| Realm extraction | [`Program.cs`](../../tools/osu-lazer-realm-parser/Program.cs) | Dynamic, read-only Realm access and NDJSON production. |
| Helper build integration | [`build.rs`](../../crates/radio-scanner/build.rs), [helper project](../../tools/osu-lazer-realm-parser/osu-lazer-realm-parser.csproj) | Build-time executable selection; helper targets .NET 8. |
| CLI presentation and selection | [`scan.rs`](../../apps/osu-radio-cli/src/commands/scan.rs), [`import.rs`](../../apps/osu-radio-cli/src/commands/import.rs), [`helpers.rs`](../../apps/osu-radio-cli/src/commands/helpers.rs), [`types.rs`](../../apps/osu-radio-cli/src/types.rs) | Keep reusable discovery and import logic in the crates. |

Discovery is also used for backend folder inspection in [installation service](../../crates/radio-services/src/osu_installation.rs). Changes to candidate selection can affect that caller even without importing anything. Its database-dependent behavior is outside this guide.

## Domain and audio references

`OsuKind` currently has `Stable` and `Lazer`; `as_str` and `FromStr` use lowercase `stable`/`lazer` and reject other strings. `OsuMarker` carries the kind, marker file path, and its parent root path.

`ImportedBeatmapSet` contains `source`, optional online ID/hash, named file usages, and all of its `ImportedBeatmap` values. A beatmap holds optional difficulty name, BPM, hash, and metadata. `BeatmapMetadata` contains title/artist variants, optional author, source/tags, user tags, preview time, and audio/background filenames. `RealmNamedFileUsage`, `RealmFile`, and `RealmUser` are the current names in the shared domain; despite these names, the crate performs no Realm I/O. Do not put GUI styling or a hosted-provider decision into these types incidentally.

The C# helper constructs each file reference as `<realm-parent>/files/<first hash character>/<first two hash characters>/<hash>`. A missing or shorter-than-two-character hash gives no resolved path. This is path construction, not an existence or readability check.

`ImportedBeatmapSet::resolved_audio_path` matches `metadata.audio_file` against a named file usage's `filename` and returns its `file.resolved_path`. The match is exact and the first matching usage wins; absent metadata, audio filename, file, or path yields `None`. `resolved_background_path` performs the same exact first-match lookup for `metadata.background_file`. Both reuse a private named-file resolver. These pure lookups do not inspect the disk. The CLI's `resolved` audio status therefore means a reference was supplied, not that playback succeeded. Preserve references to the original source files; copying audio is not scanner work.

## Discovery tiers and constraints

Markers are exact filenames: `client.realm` for lazer and `osu!.db` for stable. Discovery checks marker presence, not whether the installation can be successfully imported.

| Highest tier | Work performed |
| --- | --- |
| `Known` | Probe known candidate paths and follow lazer relocation configuration; no recursive sweep. |
| `Shallow` | Run known probes, then walk roots with `shallow_max_depth` (default `4`). |
| `Full` | Run known probes, shallow sweep, then an unbounded sweep; this is the default. |

`DiscoveryOptions::roots` replaces OS defaults when nonempty. Explicit roots probe the root itself and its `osu`/`osu!` children before any sweep. Empty roots select OS candidates and sweep mounted drives on Windows, or `/` elsewhere, plus the home directory. Tests must always supply temporary explicit roots; a default `Full` run is OS-wide.

Shallow sweeps retain every distinct root because their depth limit is relative to each root. In the unbounded tier, a parent walk skips separately listed subtree roots, which receive their own walks. This avoids scanning the home tree twice while preserving explicit roots beneath pruned directories and roots reached through symlinks. Root boundary comparisons use the supplied paths; canonical marker deduplication still handles alternate path spellings.

Known candidates cover Windows environment-based installation directories and drive-root/`Games` locations; Linux XDG/default data, Flatpak, osu-wine, and the default Wine prefix; and the macOS Application Support lazer directory. Read the platform branches in `known.rs` when changing exact candidates. The old claim that `Known` reads no directories at all is too strong: default Linux Wine candidate generation enumerates `~/.wine/drive_c/users`. Explicit-root known probes do not use that enumeration.

Lazer candidates inspect `storage.ini` files up to 64 KiB, parse a nonempty case-insensitive `FullPath` key, and probe the resulting directory for `client.realm`. Read/parse failures are ignored. A relocation target can lie outside an explicit root: roots constrain candidate generation and traversal, not containment of relocation references. Values are passed through as paths; they are not rebased against the configuration directory.

`kind` filters markers before emission and before counting toward `limit`. `limit` is an optional `NonZeroUsize`; it counts unique accepted markers, not directories visited. The collector deduplicates canonical marker paths when possible while preserving readable paths in emitted values. Callback `ControlFlow::Break` and an exhausted limit set a shared stop flag under the callback mutex. Waiting workers recheck that flag after acquiring the mutex, so neither condition permits further callbacks. Because sweep workers run in parallel, discovery order is not a stable selection contract.

The sweep includes hidden entries and disables all standard ignore filters, including `.ignore`, `.gitignore`, Git global ignores, `.git/info/exclude`, and parent ignore lookup. It does not follow directory symlinks. It ignores individual traversal errors and prunes selected system, cache, trash, dependency, and version-control directories. Pruning is deliberately conservative: secondary drives, hidden osu! directories, and plausible locations such as `Program Files/osu!` must remain discoverable. Exact exclusions live in `sweep.rs` and its platform-specific tests.

## Streaming and cancellation

`find_osu_markers_with` blocks its calling thread and invokes a callback for each accepted marker. `find_osu_markers` collects that callback output. Async callers use `discover`, which launches the same core with `spawn_blocking` and sends markers through a bounded channel of capacity `16`; `Discovery::next` receives one marker and `collect` drains them all.

Dropping `Discovery` sets the shared worker stop flag and closes the receiver, releasing a blocked `blocking_send`. Walk workers check the flag on every visited entry, including entries in marker-free trees; cancellation no longer waits for another marker. An already stopped run skips known-candidate enumeration too. In-flight filesystem operations and already-started known-candidate enumeration are not interrupted, and the spawn task is not joined by the `Discovery` value.

## Supported imports and CLI behavior

`get_beatmap_sets` currently imports only lazer; stable discovery returns useful markers but selecting one for import produces the typed `UnsupportedSourceError`. The CLI maps that error into an explanation that stable import is not yet supported. Adding stable import would require a reader and dispatch change, not merely changing discovery.

`import_from_lazer_realm` selects the configured helper; `import_from_lazer_realm_with_helper` accepts an executable explicitly and is the process-test seam. Both return a fully materialized `Vec<ImportedBeatmapSet>`. Line-by-line decoding does not make the public import result streaming, and a CLI output limit does not reduce helper extraction or memory use.

The CLI's `scan --root` constrains discovery, resolving a relative root against the working directory. `--source` becomes the walk's kind filter; `--first` and `--limit` stop marker discovery and conflict with each other. A scan limit of zero is rejected. Text output renders arrivals as they are received; JSON output collects first.

For inspection, prefer `import --marker <explicit-client.realm>`: it bypasses discovery. The source is inferred from the filename unless `--source` is supplied; marker construction itself does not verify the file. Without a marker, selection runs `Full` discovery using the supplied root/filter. It collects at most two markers when no index is supplied to detect ambiguity; selecting by index reruns discovery, whose ordering can vary. `import --limit` limits printed **beatmaps**, defaults to the value in [`consts.rs`](../../apps/osu-radio-cli/src/consts.rs), and may show only part of a set in the CLI JSON preview. It does not truncate the imported values returned by the scanner. See [scoped examples](development.md) before running commands.

## C# helper protocol and failure handling

Normal invocation is an executable plus one `client.realm` argument. `Program.cs` opens its absolute path with `IsDynamic = true` and `IsReadOnly = true`, suppresses Realm logging, and requires a `BeatmapSet` object type. Missing optional members generally become `null` or empty collections. Numeric helpers tolerate missing/incompatible values and reject integers outside `i32` range; this does not mean every possible Realm/schema failure is swallowed.

Normal stdout contains one JSON object per beatmap set, one object per line, with `source: "Lazer"` and snake_case keys. `beatmaps` is required by the Rust parser; missing `files` and `user_tags` default to empty collections. The nested metadata, author, and file records map explicitly through private Rust wire types into the domain. Keep the producer in `Program.cs`, parser/mapping in `lazer/types.rs`, domain fields in `import_types.rs`, and representative parser input in [`lazer/tests.rs`](../../crates/radio-scanner/src/lazer/tests.rs) synchronized when the protocol changes.

`--schema <path>` is a separate diagnostic mode that emits schema records; its stdout is **not** import input. Do not mix schema records, progress messages, or blank lines into normal stdout: every line is parsed as a beatmap-set record. Diagnostics belong on stderr. Helper exit codes are `0` for success, `2` for usage, `3` for a missing Realm file, and `4` for an exception during reading/export.

The Rust wrapper pipes both outputs and drains stderr concurrently while decoding stdout. Stderr is currently buffered without a size cap. Spawn failures include the helper and Realm paths. A stdout read or parse failure kills and waits for the child, aborts the stderr task, and returns the read/parse error; it does not return partial imports. After EOF it waits for process success, joining the stderr task. A nonzero exit reports status and trimmed stderr; successful stderr is forwarded to the parent stderr. Forwarding failures also return an error. The child has `kill_on_drop(true)` as a cancellation safeguard.

[`build.rs`](../../crates/radio-scanner/build.rs) normally runs `dotnet build --configuration Release` into Cargo's output directory and embeds the resulting executable path. `OSU_LAZER_REALM_PARSER_PATH` bypasses that build when set at build time and overrides the selected path when set at runtime. It must identify an executable, not a bare DLL. These are current desktop build mechanics, not a mobile-hosting commitment; see [development prerequisites](development.md).

## Verification evidence and gaps

These are source-linked checks to select for a change, not a claim that they were run while writing this guide. Record actual command results separately.

| Concern | Existing tests / evidence |
| --- | --- |
| Core source names | [`radio-core/src/lib.rs`](../../crates/radio-core/src/lib.rs): `osu_kind_round_trips_through_its_stored_string`, `unknown_osu_kind_is_rejected`. |
| Roots, limits, source filter, tier depth, deduplication, async parity | [`discovery/mod.rs`](../../crates/radio-scanner/src/discovery/mod.rs): `finds_markers_under_explicit_root`, `stops_after_limit`, `filters_by_kind_during_the_walk`, `known_depth_finds_candidates_without_walking`, `shallow_depth_skips_deeply_nested_markers`, `does_not_emit_duplicates_across_tiers`, `discover_stream_yields_the_same_markers_as_the_sync_core`. |
| Parallel stopping and stream cancellation | [`discovery/mod.rs`](../../crates/radio-scanner/src/discovery/mod.rs): `parallel_sweep_stops_after_limit_or_callback_break`, `dropping_discovery_sets_worker_stop_flag_without_markers`. The drop test checks the shared flag deterministically, not shutdown latency. |
| Overlapping roots and Git ignore isolation | [`discovery/sweep.rs`](../../crates/radio-scanner/src/discovery/sweep.rs): `full_sweep_visits_overlapping_roots_once_in_either_order`, `shallow_sweep_preserves_depth_relative_to_each_root`, `full_sweep_preserves_explicit_roots_inside_pruned_directories`, `git_exclude_rules_do_not_hide_markers`, `git_global_ignores_do_not_hide_markers`. Visits are counted before marker deduplication; global Git configuration is isolated in a child test process. |
| Relocation parsing and absent candidates | [`discovery/known.rs`](../../crates/radio-scanner/src/discovery/known.rs): `parses_the_full_path_key`, `rejects_missing_malformed_and_commented_entries`, `follows_a_relocated_lazer_data_directory`, `ignores_known_candidates_that_do_not_exist`. |
| Pruning and marker names | Platform-gated tests in [`discovery/sweep.rs`](../../crates/radio-scanner/src/discovery/sweep.rs), including `recognizes_marker_file_names`, `keeps_unix_plausible_osu_locations`, and `keeps_windows_plausible_osu_locations`. |
| Unsupported stable import | [`radio-scanner/src/lib.rs`](../../crates/radio-scanner/src/lib.rs): `reports_unsupported_scanner_sources`. |
| NDJSON mapping and wrong record shape | [`lazer/tests.rs`](../../crates/radio-scanner/src/lazer/tests.rs): `parses_lazer_beatmap_set_json_into_core_type`, `rejects_beatmap_first_json`. |
| Child process success and failure diagnostics | Unix-only fake-helper tests in [`lazer/tests.rs`](../../crates/radio-scanner/src/lazer/tests.rs): `accepts_a_fake_ndjson_helper`, `includes_helper_stderr_in_failure`. |

Existing tests do not establish a discovery shutdown-latency bound, malformed-output child cleanup, Windows helper execution, or live Realm compatibility. There is no separate C# test project. Passing Rust fake-helper tests does not prove a real osu! library imports correctly. Use the [verification matrix](development.md) and add a focused regression check for behavior changed; do not run broad filesystem discovery or a real import as an automatic documentation check.
