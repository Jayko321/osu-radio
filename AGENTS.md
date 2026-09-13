# osu! radio: shared agent guidance

Build a desktop music player for local osu! installations, with hosted sources intended later. Linux and Windows are current targets; macOS is unverified, Android and iOS are future goals. Hosting and provider choices remain open.

`radio-services` is the shared persisted-model entry point for server and CLI.
`radio-db` uses SeaORM 2 repositories and a fresh versioned schema. Read the
[database guide](docs/agent/database.md) for ownership, immutable metadata identity,
snapshot replacement, and explicit reset behavior. Legacy databases require a
new database or an explicitly requested reset; ordinary startup never resets.

## Ownership and constraints

| Component | Owns |
| --- | --- |
| `crates/radio-core` | Source-neutral domain/import types; no I/O or persistence. |
| `crates/radio-scanner` | Discovery and source readers; returns data, preserves read-only access to osu! sources. |
| `tools/osu-lazer-realm-parser` | Realm extraction; stdout is NDJSON, diagnostics go to stderr. |
| `apps/osu-radio-cli` | Thin development harness; reusable behavior belongs in crates. |
| `apps/osu-radio-server` | Backend startup, HTTP transport and services; backend owns OS access and eventual audio serving. |
| `crates/radio-services` | Shared model services and transaction orchestration for server and CLI. |
| `crates/radio-db` | Persistence boundary; concrete repositories own SQL and constraints; opaque transactions bind repositories. |
| `crates/osu-radio-client` | Toolkit-free frontend access, session/supervision and reusable view models. |
| `apps/osu-radio-gui-vizia` | Views, signals, events, styles and assets. |

Preserve domain/backend/client/GUI boundaries. Vizia and a child-process server are today's implementation, not permanent requirements for future platforms. The GUI is currently a prototype: track selection changes presentation, not audio playback. Keep UI-only assets and classes out of client view models.

Preserve unrelated working-tree changes. Use explicit roots in discovery tests and manual checks; unscoped discovery can walk every mounted root. Scanner import reads source data and returns it; it must not copy audio or decide persistence. Do not use Cargo `--all-features`: the workspace contains incompatible database backend features. Use the scoped [verification matrix](docs/agent/development.md#verification-matrix).

## Read for the task

Start with [the guide index](docs/agent/index.md) for reading order, architecture, capability status and deferred decisions.

| Task | Guide and procedure |
| --- | --- |
| Domain types, discovery, import mapping, Realm helper | [Scanner guide](docs/agent/scanner.md), [osu-radio-scanner skill](.agents/skills/osu-radio-scanner/SKILL.md) |
| GUI layout, interactions, styles, assets | [Frontend guide](docs/agent/frontend.md), [osu-radio-gui skill](.agents/skills/osu-radio-gui/SKILL.md) |
| Reusable client state or server supervision | [Frontend guide](docs/agent/frontend.md), [backend guide](docs/agent/backend.md) |
| Database repositories, persistence, server/client API contracts | [Database guide](docs/agent/database.md), [backend guide](docs/agent/backend.md), [API skill](.agents/skills/osu-radio-api/SKILL.md) |
| Backend startup, service boundaries, errors, optional API docs | [Backend guide](docs/agent/backend.md) |
| Setup, environment, commands, testing, troubleshooting | [Development guide](docs/agent/development.md) |
| Findings-only pre-commit review | [cr skill](.agents/skills/cr/SKILL.md) and its linked checklist |

For library/framework/SDK/API/CLI/cloud-service guidance, fetch current documentation with Context7, including familiar technologies. Resolve with `npx ctx7@latest library <name> "<specific question>"` before `docs`, unless a library ID was supplied. Follow the [Context7 procedure](docs/agent/development.md#context7-and-source-evidence), including running requests outside the default sandbox. Repository source establishes current behavior; upstream documentation establishes API advice. Source-only review, business-logic debugging and refactoring do not require an upstream lookup by themselves.

## Maintain the guidance

- Update the affected guide with behavior changes, linking current source/tests and stating verification limits. Update routing here only when ownership, constraints or reading paths change; ordinary edits do not require rewriting all guides.
- Keep technical facts in `docs/agent/`, task procedures in `.agents/skills/`, and Claude-only notes in `CLAUDE.md`. Link shared procedures instead of duplicating them.
- Label implementation, placeholders, confirmed direction and deferred decisions separately. Do not turn an implementation detail or a proposed improvement into a product requirement.
- Check changed links, paths, symbols, flags and examples. Record executed checks separately from suggested commands. `docs/agent/` is versioned; review reports elsewhere under `docs/` remain ignored.
- Keep database contracts in the database guide and API procedures in the shared API skill.
