# Search and compact Songs API measurements — 2026-09-19

Implemented on top of the initial dirty working tree. No new migration, persistent search index or visual change was added. The existing `/api/beatmap-sets` response remains equivalent; the shared GUI controller now uses `/api/tracks` and retains every related difficulty.

## Baseline and method

- Initial Git HEAD: `6a269db9ea080e3e2025ed809032282a4c16d073` plus the saved working tree, including untracked source.
- Source snapshot SHA-256: `9d51e917ccee09dc4f4f683ed8e7e2b6c79f881f5b61a59c126e5338868cecba`.
- SQLite backup: 13,003 sets, 53,217 beatmaps, 37,332 metadata rows, 12,120 audio sources, 53,526 tags and 362,525 set-tag links. Created with SQLite backup API, then reused for every stage; the application database was not modified.
- Local baseline source, dirty-tree patch, binaries, backup, raw response bodies and logs are under `/tmp/osu-search-baseline`. These local artifacts may disappear when `/tmp` is cleared; no library payloads are checked into Git.
- Linux 7.2.5, Ryzen 5 5600X (6 cores/12 threads), Rust 1.98.1. Default Cargo debug and release profiles; SQLite server with docs enabled.
- One warmup and five measured samples per case; tables show medians in milliseconds. No numeric timing threshold was added to CI.
- HTTP is loopback time through receiving the full body, excluding JSON decode. Timings fluctuate on this shared workstation; per-stage HTTP samples are retained in local JSON files. Component phases and end-to-end controller measurements are separate runs, so their medians should not be added as if they were one trace.
- End-to-end measurements exclude child startup, media I/O and GUI painting. The new case executes the controller request task; baseline replays the original HTTP/mapper path into controller completion. Both send the normal controller notifications.
- Search phases include SQL execution, ORM decoding/materialization and transaction startup in “read”; “match” includes Rust normalization/grouping/filtering; “load” includes selected full rows and their materialization. Empty queries keep the original aggregate path. Stage 3 uses the stage 2 search path, followed by grouping and compact serialization.

## HTTP latency after each stage

Stage 0 is the initial implementation; stage 1 filters tags in SQL; stage 2 separates projections/matching from full loading; stage 3 returns compact tracks. The first three use the legacy endpoint.

| Profile | Query | Stage 0 | Stage 1 | Stage 2 | Stage 3 |
| --- | --- | ---: | ---: | ---: | ---: |
| debug | `rock` | 2770.3 | 1335.0 | 1244.9 | 1121.5 |
| debug | `rock hard` | 2914.6 | 1397.7 | 1185.9 | 1060.9 |
| debug | `星` | 2341.6 | 1170.2 | 688.2 | 674.7 |
| debug | `does-not-exist-zzzz` | 2252.7 | 978.8 | 613.7 | 613.8 |
| debug | (empty) | 1441.6 | 1445.5 | 1501.6 | 1309.1 |
| release | `rock` | 762.3 | 432.8 | 331.2 | 315.0 |
| release | `rock hard` | 723.5 | 432.3 | 334.0 | 308.3 |
| release | `星` | 666.8 | 392.5 | 201.9 | 208.7 |
| release | `does-not-exist-zzzz` | 674.5 | 317.5 | 192.6 | 193.0 |
| release | (empty) | 325.7 | 325.7 | 359.4 | 365.4 |

## Response size and equivalence

| Query | Legacy bytes | Compact bytes | Reduction | Tracks |
| --- | ---: | ---: | ---: | ---: |
| `rock` | 4,995,626 | 2,727,142 | 45.4% | 3,803 |
| `rock hard` | 4,258,171 | 2,318,526 | 45.6% | 2,787 |
| `星` | 524,358 | 277,422 | 47.1% | 363 |
| `does-not-exist-zzzz` | 2 | 2 | 0.0% | 0 |
| (empty) | 15,626,232 | 8,541,859 | 45.3% | 12,120 |

Each stage 1/2 HTTP response was compared as complete parsed JSON against baseline: IDs, order and fields, not just counts. Service phase measurements also compare complete aggregates. Server serialization measurements compare full client tracks, including all difficulties. Captured client measurements independently compare displayed fields, IDs, order and cover choice against the original mapper preserved in the benchmark.

## HTTP through controller completion

These are measured end-to-end medians, including Rust decoding/mapping and controller replacement. Debounce is actually awaited, not added arithmetically.

| Profile | Query | Original, no debounce | New, no debounce | Original, 200 ms debounce | New, 200 ms debounce |
| --- | --- | ---: | ---: | ---: | ---: |
| debug | `rock` | 2575.5 | 1115.0 | 2756.9 | 1301.5 |
| debug | `rock hard` | 2520.7 | 1098.5 | 2695.2 | 1277.6 |
| debug | `星` | 2250.4 | 648.4 | 2494.6 | 861.8 |
| debug | `does-not-exist-zzzz` | 2124.3 | 578.5 | 2348.9 | 774.6 |
| debug | (empty) | 1720.4 | 1389.0 | 1940.5 | 1558.5 |
| release | `rock` | 757.5 | 303.0 | 877.2 | 513.4 |
| release | `rock hard` | 693.9 | 303.6 | 920.7 | 498.9 |
| release | `星` | 667.6 | 186.2 | 933.1 | 387.9 |
| release | `does-not-exist-zzzz` | 675.0 | 168.1 | 814.1 | 366.4 |
| release | (empty) | 354.5 | 341.7 | 582.0 | 552.5 |

## SQL/ORM, matching and selected-result loading

| Profile | Query | Stage | Read | Match | Full result load |
| --- | --- | --- | ---: | ---: | ---: |
| debug | `rock` | 0 | 2142.917 | 268.112 | 0.000 |
| debug | `rock` | 1 | 991.758 | 132.211 | 0.000 |
| debug | `rock` | 2 | 503.970 | 180.867 | 325.147 |
| debug | `rock hard` | 0 | 2090.667 | 338.539 | 0.000 |
| debug | `rock hard` | 1 | 985.579 | 167.212 | 0.000 |
| debug | `rock hard` | 2 | 523.181 | 202.734 | 289.083 |
| debug | `星` | 0 | 2089.198 | 276.877 | 0.000 |
| debug | `星` | 1 | 952.220 | 129.577 | 0.000 |
| debug | `星` | 2 | 476.345 | 175.511 | 32.024 |
| debug | `does-not-exist-zzzz` | 0 | 2075.011 | 185.440 | 0.000 |
| debug | `does-not-exist-zzzz` | 1 | 862.132 | 92.084 | 0.000 |
| debug | `does-not-exist-zzzz` | 2 | 477.182 | 139.593 | 0.002 |
| debug | (empty) | 0 | 848.658 | 0.000 | 0.000 |
| debug | (empty) | 1 | 860.945 | 0.000 | 0.000 |
| debug | (empty) | 2 | 847.086 | 0.000 | 0.000 |
| release | `rock` | 0 | 739.076 | 79.215 | 0.000 |
| release | `rock` | 1 | 344.744 | 45.972 | 0.000 |
| release | `rock` | 2 | 153.832 | 50.948 | 106.098 |
| release | `rock hard` | 0 | 634.497 | 87.466 | 0.000 |
| release | `rock hard` | 1 | 359.321 | 52.916 | 0.000 |
| release | `rock hard` | 2 | 156.642 | 56.471 | 95.966 |
| release | `星` | 0 | 628.499 | 83.983 | 0.000 |
| release | `星` | 1 | 340.754 | 48.417 | 0.000 |
| release | `星` | 2 | 143.339 | 48.895 | 12.677 |
| release | `does-not-exist-zzzz` | 0 | 642.596 | 56.147 | 0.000 |
| release | `does-not-exist-zzzz` | 1 | 239.782 | 42.463 | 0.000 |
| release | `does-not-exist-zzzz` | 2 | 144.312 | 44.925 | 0.000 |
| release | (empty) | 0 | 240.650 | 0.000 | 0.000 |
| release | (empty) | 1 | 235.050 | 0.000 | 0.000 |
| release | (empty) | 2 | 237.323 | 0.000 | 0.000 |

For stages 0/1, the initial read already materializes the full library, so the separate load column is zero. No-match stage 2 skips full loading. Reads are not SQL-engine-only timings.

## Serialization and client CPU work

| Profile | Query | Legacy DTO + serialize | Compact serialize | Legacy decode | Compact decode | Legacy map | Compact map | Legacy controller | Compact controller |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| debug | `rock` | 158.796 | 81.935 | 81.750 | 42.674 | 10.251 | 1.382 | 1.237 | 2.407 |
| debug | `rock hard` | 132.378 | 69.074 | 70.043 | 36.147 | 8.025 | 1.029 | 0.917 | 1.994 |
| debug | `星` | 17.350 | 8.706 | 8.340 | 4.271 | 0.856 | 0.134 | 0.124 | 0.232 |
| debug | `does-not-exist-zzzz` | 0.008 | 0.004 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| debug | (empty) | 485.272 | 259.374 | 257.426 | 134.117 | 32.896 | 4.435 | 4.306 | 8.108 |
| release | `rock` | 8.291 | 3.332 | 11.338 | 4.609 | 1.983 | 0.329 | 0.380 | 0.994 |
| release | `rock hard` | 6.204 | 2.583 | 9.237 | 3.821 | 1.376 | 0.240 | 0.281 | 0.810 |
| release | `星` | 0.686 | 0.393 | 1.120 | 0.484 | 0.120 | 0.032 | 0.039 | 0.096 |
| release | `does-not-exist-zzzz` | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |
| release | (empty) | 35.956 | 17.822 | 33.150 | 14.268 | 7.024 | 1.031 | 1.340 | 3.374 |

The legacy serialization column includes moving aggregate rows into DTOs; compact grouping/DTO construction is in the service path, before the compact serialization timer. Controller replacement is slightly more expensive because `Track` now retains/clones the complete difficulty list; decode and mapping shrink substantially. Empty searches still load the entire aggregate and therefore do not guarantee a lower HTTP time despite a smaller payload.

## Verification actually executed

- SQLite repository contracts: 5 passed; service tests: 9 passed. PostgreSQL repository contracts: 4 passed; shared service contract passed on separate disposable `radio_db_test_*` databases in a temporary cluster. The cluster was stopped afterward.
- Both server routers (docs enabled and disabled): 16 tests passed per configuration. New HTTP tests compare compact and legacy tracks, nullable/Unicode fallback, representative metadata, cover references, all difficulties, multi-audio subtitles and literal query decoding.
- Client tests passed with and without `mock` (24/30 non-benchmark tests); controller cancellation, 200 ms debounce, stale success/error suppression, refresh/retry, selection and media scheduling retained.
- Vizia: 17 tests passed. Qt: 5 unit tests and 5 default integration/offscreen tests passed, including live search/retry/clear, selection, offline gallery and child cleanup. The separately ignored real-backend offscreen test passed on its disposable SQLite database.
- Release checks passed for both GUIs. Scoped Clippy with `-D warnings` passed for repositories/services/client/server, SQLite and PostgreSQL routers with/without docs, client mock mode and both GUIs. Native Qt headers emitted existing C++ warnings; Rust checks passed.
- Search contracts cover literal `%`, `_`, quotes, backslash and regex-looking text; words distributed across difficulties/sets sharing audio; no cross-audio leakage; 70 unique words; repeated normalized words; more than 1,000 audio IDs across three batches; and snapshot consistency during a concurrent committed replacement.
- Formatting, diff whitespace and edited guide links checked. Existing migrations and GUI source edits were compared against the initial snapshot and remain byte-for-byte unchanged.
- Visual validation remains with the user. These checks do not establish Windows, physical desktop rendering or audio playback correctness. PostgreSQL correctness was exercised, but the real-library performance tables use SQLite. The 1k/10k/50k synthetic benchmark was updated but was not executed for this report.

## Reproduction

See [development measurement commands](development.md#search-measurements), the [HTTP capture script](../../tools/search-benchmark/benchmark_http.py), [service phase benchmarks](../../crates/radio-services/src/beatmap_set/benchmarks.rs), [serialization benchmark](../../apps/osu-radio-server/src/routes/tracks.rs) and [client/controller benchmarks](../../crates/osu-radio-client/src/controller/benchmarks.rs). Saved baseline binaries are required for original end-to-end latency.
