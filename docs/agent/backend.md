# Backend guidance

Read [the index](index.md) for product direction and [development](development.md)
for checks. This guide describes the current server boundary and hosting protocol.
`radio-db` is being rewritten: its internals, database-backed endpoint contracts,
and persistence workflows are deliberately deferred. Existing routes and DTOs are
implementation evidence, not a settled API specification.

## Ownership and change locations

| Concern | Owner and source | Where to change |
| --- | --- | --- |
| Environment, process exit, listener | [`config.rs`](../../apps/osu-radio-server/src/config.rs), [`main.rs`](../../apps/osu-radio-server/src/main.rs) | Configuration and server startup belong here. |
| HTTP routing and response shape | [`routes/`](../../apps/osu-radio-server/src/routes/mod.rs) | Handlers call services and map results into route DTOs. Keep both feature variants synchronized. |
| Backend use cases | [`services/`](../../apps/osu-radio-server/src/services/mod.rs) | Compose backend operations here; do not move use cases into handlers or a GUI. |
| Request failures | [`error.rs`](../../apps/osu-radio-server/src/error.rs) | Use `ApiError` for HTTP error translation and safe diagnostics. |
| OpenAPI and Scalar | [`docs.rs`](../../apps/osu-radio-server/src/docs.rs), [`Cargo.toml`](../../apps/osu-radio-server/Cargo.toml) | Documentation metadata and its optional dependencies belong here. |
| Frontend access and process ownership | Client [`api.rs`](../../crates/osu-radio-client/src/api.rs), [`server.rs`](../../crates/osu-radio-client/src/server.rs), [`session.rs`](../../crates/osu-radio-client/src/session.rs) | Keep reusable HTTP calls and supervision in the toolkit-free client. |

The confirmed architectural direction keeps OS access, source reading, and future
audio serving behind the backend boundary. Discovery and source parsing remain
reusable scanner work; the server composes them for backend use cases. Domain
types remain free of I/O. Database queries belong to the persistence component,
whose design is outside these guides.

The GUI calls `osu-radio-client`, not server internals, scanner functions, or
local osu! files. `ApiClient` has its own wire DTOs; the client also owns reusable
view models. See [frontend](frontend.md) for their responsibilities. The current
server does not yet implement music streaming or playback; those are product
goals. Neither hosted-source providers nor remote hosting are selected.

## Current startup configuration

`ServerConfig::from_env` calls `dotenvy::dotenv()` and propagates its error before
reading configuration. A discoverable, loadable `.env` is therefore required by
today's implementation even if all configuration variables already exist in the
process environment. Merely setting environment variables is not a workaround
for `Failed to load .env`.

`SQLITE_DATABASE_URL` is currently a startup prerequisite; its value and the
database setup workflow are deferred. `OSU_RADIO_SERVER_ADDRESS` defaults to
`127.0.0.1:3000` when its environment lookup fails. An invalid address or a bind
failure propagates with context and exits unsuccessfully; it does not silently
select another port. Keep the loopback default unless a task explicitly changes
the hosting model.

`serve` prepares application state, binds the TCP listener, reads the actual
bound address, prints its startup messages, then enters `axum::serve`. This is
the current order in source, not a separate HTTP health-check protocol.

## Embedded readiness and lifecycle

The current desktop frontend launches a child server. This is today's deployment
choice, not a permanent requirement for Windows, Linux, or future mobile apps.

`EmbeddedServer::start` uses `ServerOptions::address`, whose default is
`127.0.0.1:0`, as the child's `OSU_RADIO_SERVER_ADDRESS`. Port zero requests an
OS-assigned port. The option can be overridden; the supervisor does not always
force port zero. `ServerOptions::working_directory` changes the child's working
directory when present; otherwise it inherits the parent directory. This affects
whether the server finds `.env`.

The server emits:

```text
osu-radio server listening on http://127.0.0.1:<bound-port>
```

The client's `parse_ready_line` finds the substring `listening on `, trims the
remainder, and accepts a remainder starting with `http://` or `https://`.
`read_ready_line` waits for this output, with a default 30-second timeout. A
wording change that removes the marker, or text appended to the address, changes
the protocol. Update producer, parser, and tests together. Readiness means the
bound address has been announced; it does not mean the client has completed an
HTTP health probe.

Both child output pipes are consumed: stderr is forwarded immediately and stdout
is read through readiness and afterward. Logs receive a `[server]` prefix in the
parent. Keep draining both pipes so a full buffer cannot stall the child.
Premature exit, output-read failure, spawn failure, and timeout are exposed as
`ServerError` variants. `Session::start` pairs this server with an `ApiClient`
pointing at the reported URL.

`shutdown` kills and awaits the owned child; repeated shutdown is harmless after
the child has been taken. `Drop` requests termination, and Tokio's child has
`kill_on_drop(true)`. These are normal-lifecycle safeguards, not protection
against a hard-killed parent: abrupt GUI termination can leave an orphan. See
[frontend](frontend.md) for window closure and runtime ownership.

The executable resolution order is an explicit `ServerOptions::binary`, then
`OSU_RADIO_SERVER_BIN`, then a server executable beside the running executable,
then the bare platform executable name resolved through `PATH`. The name uses
the platform's executable suffix. Building the GUI does not build this child
binary; see [development](development.md).

## Error boundary

`ApiError::bad_request` and `ApiError::not_found` carry intentional client-facing
messages and status codes. Other errors converted through `From<E>` become 500
responses containing the fixed message `The server failed to handle the
request.` The underlying error chain is logged to stderr, not returned in the
response. Preserve this separation; do not expose internal paths or storage
diagnostics through unexpected-error messages.

Startup failures print their contextual error chain and return a failing exit
code. The frontend receives supervisor/client errors through the reusable client
boundary and decides how to display them.

## Optional API documentation

The default `docs` feature enables `utoipa`, `utoipa-axum`, and `utoipa-scalar`,
mounts Scalar at `/docs`, and prints its URL. The dependencies are optional;
`--no-default-features` removes this documentation code.

`routes::router` has two mutually exclusive implementations: an ordinary axum
router without `docs`, and an `OpenApiRouter` with it. A route change must update
both. Handlers and DTOs use `cfg_attr(feature = "docs", ...)` for OpenAPI
attributes and schema derives. Keep documentation-only field annotations gated
as well. Check both feature states using the commands in
[development](development.md); compiling only one cannot verify the other.

## Evidence and verification limits

Client [`server.rs` tests](../../crates/osu-radio-client/src/server.rs) include
`the_bound_address_is_read_back_from_the_startup_line` and
`other_startup_lines_are_not_mistaken_for_the_address`. They exercise parsing
without launching a server.

The ignored integration test
[`the_embedded_server_answers_on_the_port_it_reports`](../../crates/osu-radio-client/tests/embedded_server.rs)
launches a built server and also exercises database-backed requests. It is not a
pure supervision test and is deferred during this documentation effort. Server
route/service tests use [`test_support.rs`](../../apps/osu-radio-server/src/test_support.rs),
but their database-dependent behavior is outside this guide.

All behavior above is source-confirmed. Recommended checks and actual validation
results must be reported separately; a source review or compile check is not a
successful server launch or end-to-end playback test.
