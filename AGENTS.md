# AGENTS.md

## Project

- Rust workspace for `osu-radio`.
- Goal: listen to osu! beatmap audio outside of the game.
- Long-term shape: a backend server does the heavy lifting, and a frontend UI talks to that server.
- Workspace members:
  - `apps/osu-radio-cli`: CLI executable.
  - `crates/radio-core`: shared domain types.
  - `crates/radio-scanner`: osu! install discovery, external source reading, and beatmap scanning.
- Supporting code:
  - `tools/osu-lazer-realm-parser`: C# helper for osu!lazer Realm parsing.

## Change Boundaries

- Prefer small, scoped changes that follow the existing crate layout.
- Do not add new workspace crates unless explicitly asked or unless the existing crates cannot reasonably own the behavior.
- Prefer adding modules inside existing crates over creating new crates.
- Keep `crates/radio-core` for clean domain types only.
- Put external source discovery and reading in `crates/radio-scanner`.
- `crates/radio-scanner` should read from external sources and hand discovered data to callers; it should not decide what the app stores.
- Keep CLI-only argument handling, output, and command wiring in `apps/osu-radio-cli`.
- Treat `crates/radio-scanner/vendor/realm-db-reader` as vendored third-party code. Do not edit it unless explicitly asked.

## Architecture Direction

- A future server app, likely `apps/osu-radio-server`, is expected to own backend/server behavior when that work becomes concrete.
- The server decides what metadata/audio references are stored and serves audio to the frontend.
- The frontend should not read the OS, osu! folders, or beatmap files directly. It should talk to the server.
- Scanner/source code may eventually support multiple sources, such as local stable, local lazer, remote mirrors, or official osu! API metadata.
- For now, prefer storing or passing audio source references as local paths instead of copying audio into app-owned storage.
- Keep remote URL audio sources as a future possibility, not a default assumption.

## Agent Workflow

- Before implementing, inspect the relevant files first. Avoid broad repo-wide exploration unless the task is unclear.
- Prefer existing local patterns over new abstractions.
- Prefer plain modules, functions, and data types until duplication or complexity makes an abstraction clearly useful.
- Prefer duplicated simple code over premature abstraction.
- Add an abstraction when the project has multiple real implementations or paths that need the same interface. For example, scanner/import paths that support both stable and lazer may justify a shared abstraction.
- Avoid unrelated refactors, formatting churn, dependency changes, or public API changes unless required by the task.
- Do not add dependencies just for convenience. Use the standard library or existing workspace dependencies unless a new dependency clearly pays for itself.
- If a request is ambiguous, make the smallest reasonable assumption and continue; ask only when the choice would change the design or risk data loss.
- For feature work, implement the narrowest behavior that satisfies the stated acceptance criteria.
- For bug fixes, prefer a focused regression test when practical.

## Error Handling

- Prefer custom error enums over `anyhow`.
- Keep errors specific enough for callers to react to expected failure modes.
- Avoid stringly typed errors when a small enum variant would be clearer.
- Use broad boxed errors only at temporary boundaries or when explicitly asked.

## Async Policy

- Async APIs across crate boundaries are allowed and preferred for scanner/import flows that may perform I/O.
- Prefer async where real filesystem, database, or external process work is involved.
- Avoid making purely CPU-bound or simple data transformation APIs async.

## Testing Policy

- Prefer small unit tests for pure parsing, helpers, and domain behavior.
- Use focused fixture-based tests for scanner/import behavior when real file shapes matter.
- Keep integration tests for important end-to-end flows only.
- Avoid large fixture setups unless the behavior cannot be tested clearly with smaller inputs.

## CLI Policy

- The CLI exists mainly as a development harness for testing library code without the future UI.
- Keep the CLI boring and thin.
- Put reusable behavior in crates, not in the CLI.

## Verification

- Use targeted checks first:
  - `cargo test -p radio-core`
  - `cargo test -p radio-scanner`
  - `cargo test -p osu-radio-cli`
- Use workspace checks when the change crosses crates or public APIs:
  - `cargo test`
  - `cargo clippy --workspace --all-targets --all-features`
- Run `cargo fmt` after Rust edits.
- If a check is skipped, report why.

## Useful Commands

- Build workspace: `cargo build`
- Run CLI: `cargo run -p osu-radio-cli`
- Test workspace: `cargo test`
- Format Rust code: `cargo fmt`
- Lint Rust code: `cargo clippy --workspace --all-targets --all-features`

## Project Notes

- Rust edition is `2024`.
- Tokio is the workspace async runtime.
- Current scanner API returns placeholder `Vec<String>` data in places; avoid assuming the domain model is final.
