# Contributing

Report bugs and propose features through GitHub Issues. Include the application
version, operating system and steps to reproduce a bug. Remove private paths,
credentials and personal data from logs before sharing them.

Fork the repository, create a task branch from `dev`, and open your pull request
against `dev`. Explain the change and how you checked it; add screenshots when
useful for interface changes. The maintainer reviews contributions. A successful
Linux CI and the maintainer's approval are required before merging external PRs.
Ordinary PRs use squash merge.

Use Conventional Commits for new commit messages and squash titles, for example
`fix: correct installer path` or `feat: add playlist search`. This is a convention,
not an additional automated check.

See the [development guide](docs/agent/development.md) for prerequisites, build
commands and the scoped verification matrix. Do not use Cargo `--all-features`:
SQLite and PostgreSQL backends require separate configurations. Start with
[.env.example](.env.example) for environment settings; preserve your local `.env`.
Run checks relevant to your change, `cargo fmt --check`, and the documented scoped
Clippy commands. Linux PR CI also builds the Qt application; interface tests and
Windows builds are performed before releases.

Keep component boundaries described in the [project map](docs/agent/index.md).
Discuss changes to architecture, dependencies, protocols or storage before
implementing them. Avoid unrelated changes and preserve read-only access to osu!
source libraries. Changes are contributed under the existing MIT license.

`main` contains prepared releases and is the default branch. Development happens
in `dev`; releases merge its complete tested state into `main` with a merge
commit. Urgent release fixes must also reach `dev`. GitHub Releases are the
changelog. Alpha releases are marked as pre-releases; new tags use SemVer with a
`v` prefix. The maintainer can push directly, including force-push, to both branches.
