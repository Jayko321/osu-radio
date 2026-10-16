# Approved architecture

This document records the user's approved architecture and why each boundary
exists. [Frontend](frontend.md), [backend](backend.md) and [database](database.md)
describe the implementation and contracts; [development](development.md) lists
checks and their limits. Approval is distinct from source confirmation or a
passing test.

## Decision authority

Architectural decisions belong to the user. Changing crate boundaries, state
ownership, dependencies, coordination, protocols, lifecycle or storage requires
the user's decision. Agents implement approved decisions and raise unexpected
architectural questions in the primary session before changing those decisions.
Routine implementation and fixes within these boundaries do not require a new
architecture approval. Neither an agent proposal nor today's implementation
creates a future product requirement.

## Approved state owners

| Owner in `osu-radio-client` | Responsibility | Reason and limit |
| --- | --- | --- |
| `LibraryViewModel` | Songs, search, sorting, loading and selected library track | Library results and selection survive playlist navigation independently. Selection alone never starts audio. |
| `PlaylistsViewModel` | Playlist list/detail, query, selected item, editor and item operations | A concrete playlist item has its own identity even when several difficulties share audio. Rows carry item ID and track together. |
| `SettingsViewModel` | Preferences, audio volume, registered folders and staged add/refresh/remove workflow | State/rules are reusable and work before a server connects; storage and native pickers are separate adapters. |
| `PlayerViewModel` | Selected-track presentation, playback state, control availability and Play/Pause/Resume rules | Selected, current and loading tracks remain distinguishable. Entry IDs/tokens identify launches; waiting gates local audio and recovered pause preserves independent selection. |
| `QueueViewModel` | Full queue rows, history/current boundary, loading/operation errors and open-panel refresh | The client projects the queue; the server owns persisted queue order and assignments. |

The VMs and their domain support modules live in `src/view_models/`;
`src/controller/` holds coordination and execution infrastructure.
Each VM owns its data and handlers. Shared sorting and title/artist preferences
are applied consistently through explicit inputs; they do not create a second
owner of a list. Interactions use explicit inputs, results and coordinator calls.
A file split alone does not establish ownership. Domain handlers must not remain
scattered `impl Controller` implementations beside nominal VM structs.

One `AppController`, one event loop and the public `spawn/send/shutdown` facade
remain. The controller owns session lifecycle, background work, coordination and
shutdown. It routes library and playlist updates separately. Position updates do
not resend collections. Async work retains request epochs, revision/token checks,
retry lanes, cancellation and acknowledged shutdown.

## Approved media and playback boundaries

The common client infrastructure owns media scheduling: visible, selected and
queue demand, four concurrent pipelines, priorities, generations, cancellation
and delivery acknowledgements. HTTP bytes are limited to 16 MiB per encoded cover
and the existing 30-second request timeout. Decode/cache installation or stale
result discard acknowledges delivery before a slot is released.

Qt owns image decoding, URLs and one bounded decoded cache shared across panels,
playlist artwork and editor drafts. The existing 64 MiB limit, proportional
1280px image bound, selected-cover protection and immutable version/epoch keys
remain. QML disables its additional provider cache. Keeping these facilities out
of toolkit-free VMs avoids making Qt resources part of reusable state.

Queue mutations join the existing `PendingPlayback` lane; additions are never
retried automatically. Queue viewport demand carries entry IDs, resolves through
the current Queue VM and shares audio/cache work with the other panels. The
Qt panel uses a separate TrackListModel for the complete queue, including history.
Entry IDs cross Qt as strings. The [T07 report](../impl/queue-rework/T07.md) records
client contracts and the [T08 report](../impl/queue-rework/T08.md) records model/panel
evidence; the [T09 report](../impl/queue-rework/T09.md) records insertion menus.
The [T10 report](../impl/queue-rework/T10.md) separates real-backend automated
probes from native platform and physical-audio acceptance.

The server owns persistence, source-file access, original audio serving and the
permanent queue. The client owns temporary audio downloads and the local playback
worker. `osu-radio-player` accepts local files and owns synchronous decoding,
seek, output and volume; it has no HTTP, database or GUI dependency. No server
HTTP contract or schema change is part of this refactor.

The server's availability worker is owned by `serve`, starts after state recovery
and listener preparation, and uses existing queue services for selection and guarded
result commits. `AppState` retains only a wakeup signal, not its lifecycle handle.
Its four blocking file operations run outside write transactions; checked entries
are not reopened on startup. Awaited shutdown stops selection/application, drains
started I/O and joins the supervisor. Drop requests the same stop without awaiting.
The existing child kill protocol remains; hard process termination cannot acknowledge
worker shutdown. See [backend lifecycle](backend.md#queue-availability-worker) and
[worker tests](../../apps/osu-radio-server/src/queue_check/tests.rs). This implements
the approved [T05](../impl/queue-rework/T05.md) without adding another queue owner.

## Approved Qt presentation

`AppBridge` is an ordinary QObject for connection, lifecycle and update delivery.
It exposes readonly `library`, `playlists`, `settings`, `player` and `queue`
references. Those separate Qt VM adapters belong to the bridge, live for the
window lifetime and mutate properties/models only on the Qt thread. Queued
callbacks safely handle QObject destruction.

One `TrackListModel` implementation has distinct library, playlist and queue instances.
Playlist roles derive item identity from the row itself, never another list's
index. `Main.qml` composes separate Songs, Playlists, Settings and Player panels.
Existing playlist components and `QueuePanel` stay reusable. QML owns appearance,
geometry, keyboard focus, popups and local Settings filtering; playback command
selection belongs to the shared client.

The offline gallery keeps mock data and shared visual components. It starts no
session, backend, discovery or audio device and performs no native preference
storage. Vizia code/resources/workspace dependencies are removed; fixtures needed
by the server remain under the server's ownership.

## Temporary settings storage

**QSettings is temporary storage.** Qt continues reading/writing the existing
native keys with current defaults, immediate session display and error handling:
`display/use_unicode_titles`, `display/use_unicode_artists` and
`integrations/discord_rich_presence`. The organization/application remain
`osu-radio/osu-radio-qt`, UserScope/NativeFormat, without fallbacks. A failed write
retains the session choice; load failures report errors and preserve established
default behavior. Preferences are available without a running server.

The common Settings VM owns values and settings rules. Qt owns native reads,
writes, paths/format and the translation of storage failures. Audio preferences
retain their existing backend storage/retry workflow. This split avoids treating
a temporary toolkit store as the long-term settings architecture.

## Implementation evidence

| Implementation | Source |
| --- | --- |
| Public command facade and single event loop | [controller.rs](../../crates/osu-radio-client/src/controller.rs) |
| Cross-domain coordination | [coordinator.rs](../../crates/osu-radio-client/src/controller/coordinator.rs) |
| VM request execution and ordered coordinator results | [workflow.rs](../../crates/osu-radio-client/src/controller/workflow.rs) |
| Library owner | [library.rs](../../crates/osu-radio-client/src/view_models/library.rs) |
| Playlist owner and atomic item/track rows | [playlists.rs](../../crates/osu-radio-client/src/view_models/playlists.rs) |
| Settings owner; audio/folder workflow data | [settings.rs](../../crates/osu-radio-client/src/view_models/settings.rs), [volume.rs](../../crates/osu-radio-client/src/view_models/volume.rs), [folders.rs](../../crates/osu-radio-client/src/view_models/folders.rs) |
| Player owner and primary command rules | [player.rs](../../crates/osu-radio-client/src/view_models/player.rs) |
| Full queue projection and operation owner | [queue.rs](../../crates/osu-radio-client/src/view_models/queue.rs) |
| Common media scheduler | [media.rs](../../crates/osu-radio-client/src/controller/media.rs) |
| QObject delivery/lifetime | [app_bridge.rs](../../apps/osu-radio-qt/src/app_bridge.rs) |
| Domain Qt adapters and reusable track model | [qt_view_models.rs](../../apps/osu-radio-qt/src/qt_view_models.rs) |
| Window composition | [Main.qml](../../apps/osu-radio-qt/qml/Main.qml) |
| Independent visual panels | [Songs](../../apps/osu-radio-qt/qml/SongsPanel.qml), [Playlists](../../apps/osu-radio-qt/qml/PlaylistsPanel.qml), [Settings](../../apps/osu-radio-qt/qml/SettingsPanel.qml), [Player](../../apps/osu-radio-qt/qml/PlayerPanel.qml) |

The source map is maintained in [frontend](frontend.md). Source access and queue
contracts are in [backend](backend.md#database-backed-http-contracts).
Executed verification and limits are recorded in the
[development guide](development.md#view-model-refactor-verification). Source
ownership and passing offscreen probes do not prove physical sound or native
Windows interaction.

## Deferred decisions

Future settings storage and migration, hosted-source providers/hosting, mobile
implementation, output-device selection, shuffle and repeat remain undecided.
Qt and the supervised child server are current implementation choices, not
requirements for every future platform. Linux and Windows are current targets;
macOS is unverified, Android/iOS are future goals. No dependency upgrade or new
provider is selected by this refactor.
