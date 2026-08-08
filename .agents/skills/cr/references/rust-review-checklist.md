# Rust Review Checklist

Apply this checklist only to changed code, affected surrounding code, and compile-relevant untracked files.

## Panics and aborts

- Flag new or newly reachable `unwrap`, `expect`, `panic`, `todo`, `unimplemented`, unchecked indexing, `slice[..]` assumptions, integer overflow risks, and `assert` used for runtime input validation.
- Accept panics in tests, narrow internal invariants, or one-off development harnesses only when the invariant is local and documented by nearby control flow.

## Resource and memory behavior

- In safe Rust, look for resource leaks and unbounded growth rather than classic memory leaks: leaked tasks, leaked file handles, unbounded caches, unbounded channels, growing `Vec` or `HashMap` state, repeated full-file reads, or reference cycles with `Rc` or `Arc`.
- Flag `mem::forget`, `Box::leak`, `Arc` cycles, detached background tasks, and handles that are never awaited, aborted, closed, or joined.
- Check streaming or scanner code for accidental retention of all beatmaps, all audio metadata, or large buffers when an iterator or incremental path is expected.

## Async and concurrency

- Flag blocking filesystem, process, mutex, or CPU-heavy work inside async tasks when it can stall the Tokio runtime. Prefer existing async APIs or `spawn_blocking` for real blocking work.
- Check `tokio::spawn` lifetimes, dropped `JoinHandle`s, cancellation behavior, and error propagation.
- Check lock ordering, holding a mutex across `.await`, and mixing sync mutexes with async workflows.
- Check channels and tasks for hangs when senders or receivers are dropped, backpressure is missing, or loops ignore cancellation.

## Error handling

- Prefer specific error enum variants over stringly typed errors for expected failures.
- Flag lost source errors, broad catch-all mapping that prevents callers from reacting, and CLI output that hides actionable failures.
- Ensure external source discovery and scanner code reports missing installs, malformed data, permission failures, and unsupported formats distinctly when practical.

## Data loss and state

- Flag deletes, overwrites, moves, cache writes, or generated output that can affect user data or osu! folders.
- Confirm scanner/source code reads external sources and returns discovered data without deciding long-term app storage.
- Prefer local path audio references for current behavior; treat remote URL storage as future-facing unless explicitly requested.

## Unsafe, FFI, and vendored code

- Flag any new `unsafe` unless the safety invariants are local, necessary, and documented.
- Do not recommend edits under `crates/radio-scanner/vendor/realm-db-reader` unless the user explicitly asked to change vendored code.

## CLI and UX regressions

- Keep CLI code thin. Flag reusable behavior implemented only in `apps/osu-radio-cli` when it belongs in a crate.
- Flag changed command output or argument behavior that could confuse the development harness without a clear reason.

## Tests and verification

- Expect focused unit tests for pure parsing, helpers, and domain behavior.
- Expect fixture-based tests for scanner/import behavior when real file shapes matter.
- Flag missing regression tests for bugs, parsing edge cases, source discovery, async cancellation, and cross-crate API behavior.

## Performance

- Flag repeated full directory scans, repeated database reads, avoidable cloning of large metadata, unnecessary allocation in tight scanner loops, and synchronous process/file work on async paths.
- Check that expensive checks are scoped or cached when they may run during normal CLI or future server workflows.

## Security and privacy

- Flag logging or reporting full sensitive local paths when avoidable, leaking tokens or environment values, shell command construction from untrusted data, path traversal, and following symlinks in ways that escape intended source roots.
- Treat osu! install paths and user library details as local private data; avoid unnecessary persistence or network exposure.
