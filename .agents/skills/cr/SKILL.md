---
name: cr
description: Strict Rust pre-commit code review for repo-local /cr or $cr requests. Use when the user asks Codex to review staged and unstaged changes before commit, produce a timestamped Markdown findings report under docs, check staged and unstaged diffs plus compile-relevant untracked files, and look for panics, resource leaks, async problems, error handling, data loss, race conditions, unsafe code, CLI UX regressions, test gaps, performance pitfalls, and security or privacy issues.
---

# CR

Run a strict Rust pre-commit review of this repository's changed code and write the findings to a timestamped Markdown file in `docs/`.

## Rules

- Review both staged and unstaged changes.
- Inspect untracked files when they look needed for compilation, tests, generated modules, Cargo configuration, fixtures, or docs referenced by changed code.
- Do not modify reviewed source, manifests, lockfiles, generated source, or formatting. The only authored repository write is the final review report in `docs/`; ignored build artifacts from the checks below are allowed.
- Do not run commands that rewrite files, such as `cargo fmt`, `cargo fix`, `cargo clippy --fix`, or code generators.
- Prefer read-only checks. Commands may write ignored build artifacts under `target/` and the Realm helper's `bin/` and `obj/` directories, but must not update source or `Cargo.lock`; use `--locked` for Cargo checks that resolve dependencies.
- Treat failed validation commands as review findings when the failure is caused by the changes or by missing compile-relevant untracked files.
- Output findings only in the report. Do not include a commit-readiness verdict, praise, broad summary, or unrelated cleanup suggestions.

## Workflow

1. Read [AGENTS.md](../../../AGENTS.md) before reviewing.
2. Gather repository state:
   - `git status --short`
   - `git diff --cached --name-status`
   - `git diff --name-status`
   - `git ls-files --others --exclude-standard`
3. Gather diffs:
   - staged: `git diff --cached --`
   - unstaged: `git diff --`
   - if staged and unstaged edit the same file, consider the combined final file state as well as the staged commit content.
4. Inspect changed files and any relevant surrounding code. Use `rg` and targeted file reads instead of broad exploration.
5. For Rust changes, read [the Rust review checklist](references/rust-review-checklist.md) and apply it to the diffs.
6. Run targeted read-only checks when useful, selecting the smallest check from the shared [development verification matrix](../../../docs/agent/development.md#verification-matrix). Use `--locked` for Cargo checks that resolve dependencies. Do not use `--all-features`; the workspace contains incompatible backend features. Compile both server documentation feature states for affected server changes. Do not run broad discovery, real-source imports, or database workflows as routine review checks. Read the affected [component guide](../../../docs/agent/index.md) for source-specific constraints; database contracts are documented in the database guide.
7. Create a report path using local date and time: `docs/code-review-YYYY-MM-DD-HHMMSS.md`.
8. Write the report as Markdown findings only.

## Report Format

Use this structure:

```markdown
# Code Review Findings

- [P1] `path/to/file.rs:42` Short title
  Explain the concrete bug or risk, why it matters, and what scenario triggers it. Mention whether it appears in staged, unstaged, or untracked compile-relevant files.

- [P2] `path/to/file.rs:87` Short title
  Explain the issue with enough detail that the author can fix it without re-running the whole review.
```

Priority meanings:

- `P0`: blocks release or can cause data loss, security exposure, or reliably broken builds.
- `P1`: likely correctness bug, panic, deadlock, async hang, race, or serious CLI regression.
- `P2`: meaningful maintainability, test, performance, error handling, or edge-case risk.
- `P3`: minor but actionable issue worth fixing before commit.

If no actionable findings exist, write exactly:

```markdown
# Code Review Findings

No findings.
```

After writing the report, respond with only the created report path and any validation commands that could not be run.
