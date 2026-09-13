---
name: osu-radio-api
description: Modify shared radio-services workflows, radio-db repositories, or osu-radio server/client API contracts. Use for database-backed behavior and HTTP integration; not scanner mapping or GUI-only presentation.
---

# Database and API work

Read [database](../../../docs/agent/database.md) for schema and repository contracts,
[backend](../../../docs/agent/backend.md) for HTTP behavior and hosting, and
[development](../../../docs/agent/development.md#repository-and-backend-checks) for
backend-specific verification. Read client source when changing a wire contract.

- Trace the public route or CLI call through its service and repository before
  editing. Server and CLI use `radio-services::Services` exclusively for persisted
  models. Keep database/transaction handles and ORM types out of the service API;
  services privately use concrete repositories and opaque `radio-db::Transaction`.
- Aggregate queries belong to the repository representing the result. Compose
  model services using transaction-bound repositories for snapshot replacement
  and installation deletion. Every changed model passes through its service;
  preserve the singleton writer lock, shared cleanup and outer-only commit. Inner
  operations must neither open transactions nor fall back to a pool. Repositories
  retain SQL, cascades and immutable metadata constraints.
- Read the complete scanner output before replacement. Preserve rollback of old
  snapshot, shared rows and timestamp together, including when the new snapshot is
  empty. Persistence never copies or deletes audio files.
- Treat metadata encoding as an identity contract. A change to included fields,
  ordering or normalization needs an explicit version decision and updated fixture;
  never silently update existing hash-keyed content.
- Preserve HTTP field names/statuses, duplicate registration responses, omitted
  versus null patch semantics and the safe unexpected-error response. Check both
  optional documentation router variants and corresponding client DTOs.
- Use versioned migrations for schema changes. Normal startup migrates; reset is
  explicit and limited to application-owned tables. Never run repository contracts
  against the configured application database. PostgreSQL contracts require a fresh
  disposable `radio_db_test_*` database via `RADIO_DB_TEST_POSTGRES_URL`.
- Select SQLite and PostgreSQL separately; never use `--all-features`. Run the
  affected service, repository, route, CLI or client tests and scoped Clippy. Report
  runtime checks separately from compile-only coverage and actual GUI/source tests.
- Update the affected technical guide and source/test links. Fetch current upstream
  API guidance using the shared Context7 procedure when library-specific advice is
  needed; repository source establishes local behavior.
