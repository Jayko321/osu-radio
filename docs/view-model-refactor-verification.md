# View model refactor verification

Environment: Linux, Rust 1.98.0, Qt 6.12.0. The root matrix uses the Rust-bundled LLD
(`RUSTFLAGS=-C link-arg=-fuse-ld=lld`), `CXXFLAGS=-g0`, no incremental compilation,
and one test codegen unit. Qt probes run sequentially with offscreen/software
rendering. Local HTTP/IPC fixtures run outside the restricted sandbox. No user's
database or osu! installation is used.

## Baseline

The original working tree (including pre-existing Discord/documentation edits)
was saved to `/tmp/osu-vm-original` before implementation. Baseline tests run on
that immutable snapshot, so concurrent implementation cannot change their input.
Initial sandbox client failures were `PermissionDenied: Operation not permitted`
when binding isolated local sockets; those runs do not establish regressions.

| Check | Result |
| --- | --- |
| Client library, mock off | 122 passed; two existing benchmarks ignored |
| Client library, mock on | 131 passed; two existing benchmarks ignored |
| Player | Six passed |
| Server with docs, SQLite | 44 passed; two ignored; startup integration passed |
| Server without docs, SQLite | 39 passed; two ignored; startup integration passed |
| Qt, sequential ordinary probes | Five unit tests and 17 ordinary integration probes passed; two real-backend probes ignored |
| Workspace formatting | Passed on original snapshot |

## Integration

| Check on final source | Result |
| --- | --- |
| Client library, mock off/on, sequential | 128/137 passed; two existing timing benchmarks ignored per configuration |
| Player | Six passed |
| Server with docs, SQLite | 44 passed; two ignored; startup integration passed |
| Server without docs, SQLite | 39 passed; two ignored; startup integration passed |
| Qt, final sequential run | Five unit tests and 17 ordinary integration probes passed |
| Explicit Qt real-backend probes | Both passed on disposable SQLite |
| Client embedded-server integration | Passed on temporary SQLite; restart preserves registration and removing it leaves the source marker intact |
| QML lint | Zero warnings, strict `-W 0`, all production QML and probes included |
| Scoped all-target Clippy | Client mock off/on, player, Qt and server docs on/off passed with `-D warnings` |
| Formatting / whitespace | Workspace fmt check and diff check passed |

The root matrix logs are `/tmp/osu-vm-final/check-*.log` and `results.tsv`.
Initial Qt Clippy style/FFI diagnostics were fixed; the successful retry is
`check-11-retry.log`. Final client suites include all subsequent reviewer fixes:
`/tmp/client-final-off.log` and `/tmp/client-final-mock.log`. They use
`CARGO_TARGET_DIR=/tmp/osu-vm-client-target`, `--locked --offline --lib`,
`--no-default-features` or `--features mock`, and `--test-threads=1`.

Executed commands in the root matrix:

```sh
cargo test -p osu-radio-client --lib --locked -- --test-threads=1
cargo test -p osu-radio-client --features mock --lib --locked -- --test-threads=1
cargo test -p osu-radio-player --locked
cargo test -p osu-radio-server --locked -- --test-threads=1
cargo test -p osu-radio-server --no-default-features --features sqlite --locked -- --test-threads=1
cargo test -p osu-radio-qt --locked -j1 -- --test-threads=1
cargo build -p osu-radio-server -p osu-radio-qt --locked -j1
cargo test -p osu-radio-qt --locked -j1 --test launch_smoke real_backend_ -- --ignored --test-threads=1
bash apps/osu-radio-qt/scripts/lint-qml.sh
cargo clippy -p osu-radio-client -p osu-radio-player --all-targets --locked -- -D warnings
cargo clippy -p osu-radio-client --features mock -p osu-radio-qt --all-targets --locked -j1 -- -D warnings
cargo clippy -p osu-radio-server --all-targets --locked -- -D warnings
cargo clippy -p osu-radio-server --no-default-features --features sqlite --all-targets --locked -- -D warnings
cargo fmt --all --check
git diff --check
env -u SQLITE_DATABASE_URL -u POSTGRES_DATABASE_URL cargo test -p osu-radio-client --locked --test embedded_server -- --ignored --test-threads=1
```

Qt was rerun after its final source freeze; all ordinary probes and the native
cache/teardown fixture passed. The initial default-parallel client attempt hit
`ETXTBSY` while executing temporary shell fixtures; final sequential runs are
clean. No production workaround was added for that fixture/environment race.

## Independent review

The same reviewer assessed the complete integration, architecture correspondence,
server boundaries and all returned fixes. No actionable finding remains.
Four regressions were fixed and rechecked: settings-mode signals close the volume
popup and restore focus; VisualProbe uses the separate adapters/models; true
cache invalidation clears the selected cover even when metadata is identical and
decoding fails; worker startup failure remains visible in PlayerPresentation and
retains the token-bound pause. Selection priority, playback history and shutdown
volume mutations were moved into their respective owners. Clippy cleanup was
also reviewed for unchanged defaults, notifications and owned row copies.

## Integrity and scope

- Pre-existing Discord source/tests/transport and untracked IPC tests were
  retained byte-for-byte during the refactor.
- The server's Vizia JPEG fixture moved under `apps/osu-radio-server/tests/fixtures`;
  SHA-256 remains `8d9a121a33accf7989c6ac4d48a6241ffb11e725ffbb1faa324885bcf299a02b`.
- Cargo.lock removal preserves every remaining package name/version/source;
  no dependency version upgrades were introduced.
- Architectural authority, approved owners and temporary QSettings storage are
  documented in [architecture](agent/architecture.md).
- Final integrity checks verified the fixture hash, remaining lock checksums,
  unchanged Discord guide sections and 425 local documentation links/anchors.

Physical sound, desktop/GPU/pointer/native picker behavior and Windows execution
are not established by these checks. PostgreSQL execution and real game-library
imports are outside this refactor's automated acceptance scope.
