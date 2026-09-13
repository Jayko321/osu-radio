# Claude Code guidance

@AGENTS.md

The import above is the shared project contract. Use its task links to read only the relevant guides. Database internals and persistence workflows are documented in [database](docs/agent/database.md); HTTP contracts are in [backend](docs/agent/backend.md).

## Shared procedures in Claude

Read and follow these repository-local procedures for matching tasks; their bodies are shared with Codex:

- GUI layout, interactions, styles or assets: [osu-radio-gui](.agents/skills/osu-radio-gui/SKILL.md).
- Discovery, import mapping or Realm helper changes: [osu-radio-scanner](.agents/skills/osu-radio-scanner/SKILL.md).
- Database repositories and server/client API changes: [osu-radio-api](.agents/skills/osu-radio-api/SKILL.md).
- Findings-only pre-commit review: [cr](.agents/skills/cr/SKILL.md), including its linked Rust checklist and report format.

These files live under `.agents/skills/`; this repository does not provide Claude slash-command wrappers for them. Follow the linked files directly when a matching procedure is requested or needed. Do not duplicate them under `.claude/` just to maintain a second copy.

Serena has project configuration in `.serena/project.yml`. If its symbol tools are available, use them for definitions and callers; targeted `rg` and source reads also work. Do not assume a configured Context7 subagent exists: use the shared [Context7 procedure](docs/agent/development.md#context7-and-source-evidence). Match commands to the actual host shell rather than assuming Windows or PowerShell.
