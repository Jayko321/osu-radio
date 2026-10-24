# Frontend and desktop GUI

Read [the architecture and capability map](index.md) first. Read the approved [architecture](architecture.md) for ownership and decision authority. Child-process hosting is an implementation detail. The confirmed direction is desktop music playback on Linux and Windows, eventually including hosted sources; macOS is unverified and mobile platforms are future goals. Database-backed API contracts and persistence workflows are described in [backend](backend.md) and [database](database.md).

For a GUI task, use the [osu-radio-gui procedure](../../.agents/skills/osu-radio-gui/SKILL.md). Build commands and their limitations belong in [development](development.md); server-side protocol ownership belongs in [backend](backend.md).

## Ownership and change locations

| Concern | Current owner | Where a change belongs |
| --- | --- | --- |
| HTTP transport, independent wire DTOs, errors | [client `api.rs`](../../crates/osu-radio-client/src/api.rs), [client `models.rs`](../../crates/osu-radio-client/src/models.rs) | Reusable frontend/backend communication stays in `osu-radio-client`. DTO details are deliberately excluded from this guide. |
| Session and child lifecycle | [client `session.rs`](../../crates/osu-radio-client/src/session.rs), [client `server.rs`](../../crates/osu-radio-client/src/server.rs) | Change supervision here, coordinating readiness output with the server. |
| Application coordination | [client controller](../../crates/osu-radio-client/src/controller.rs) | One event loop, session, background work and shutdown; coordinate the five client VMs. |
| Toolkit-free state and rules | [client exports](../../crates/osu-radio-client/src/lib.rs) | Library, playlists, settings, player and queue each own their state and handlers. |
| Qt projection and dispatch | [AppBridge](../../apps/osu-radio-qt/src/app_bridge.rs) | QObject lifecycle and delivery to persistent domain adapters on the Qt thread. |
| Panels and controls | [QML](../../apps/osu-radio-qt/qml) | Window composition, local searches, focus, popups and presentation. |
| Artwork | [provider/cache](../../apps/osu-radio-qt/src/artwork.h) | Decode images and retain bounded URLs/QImages in Qt. |
| System media I/O | [MPRIS/SMTC crate](../../crates/osu-radio-media-controls/src/lib.rs) | Local native translation and resources; [client coordination](../../crates/osu-radio-client/src/controller/system_media.rs) retains playback rules. |

The [client manifest](../../crates/osu-radio-client/Cargo.toml) has no GUI toolkit.
Qt depends on the client; views do not read osu! files or persistence directly.
Vizia has been removed. See [architecture](architecture.md) for the complete ownership contract.

## Reusable client behavior

`ApiClient` owns a reusable HTTP client and normalized base URL. Transport failures, response decoding failures, and unsuccessful HTTP statuses remain distinct `ApiError` cases. Status errors use the response's `error` field where it can be decoded, with the HTTP status as fallback. Keep this handling out of views; this guide intentionally does not freeze endpoint payloads or persistence-dependent status contracts.

The [HTTP boundary contracts](../../crates/osu-radio-client/src/api/contract_tests.rs)
assert every public operation's method, path/query and request payload, preserve
HTTP/decode/stream/transport/file error distinctions, and check cover/NDJSON/audio
size boundaries with local socket fixtures. They use private files and synthetic
responses; physical output and native Windows remain separate. Executed results
and unforced failure limits are in the [coverage worklist](test-coverage.md#client-http-boundaries-and-lifecycle-2026-10-15).

`Loading<T>` represents `Idle`, `Pending`, `Ready(T)`, and `Failed(String)` and offers `is_pending`, `value`, `error`, and `from_result`. `describe` walks an error's source chain into a single display message, suppressing cause text already present. These helpers are available to frontends; the current GUI uses a status signal rather than `Loading<T>` for connection status. Unit tests are colocated in [client `lib.rs`](../../crates/osu-radio-client/src/lib.rs).

Songs loads `/api/tracks?q=…` through `ApiClient::search_tracks` and the shared
`AppController`. The server groups globally by audio ID, preserving legacy
set/audio order; `Track::from(LibraryTrack)` formats the ready rows.
`library_tracks` remains available for legacy endpoint equivalence checks.
The lowest beatmap ID supplies representative metadata. `Track` retains the raw ordinary
and Unicode title/artist variants and a separate subtitle suffix. Independent
`TrackNamePreferences` choose ordinary text by default or Unicode when enabled;
blank/missing preferred text falls back to the other variant, then the existing
unknown labels. `SetTrackNamePreferences` immediately reformats library, search,
playlist (including unavailable), current-player and pending-queue rows. It uses
`LibraryTracksReordered` / `PlaylistTracksReordered` and preserves selection, artwork, duration, media generation and
playback; incoming responses use the current pair. During playlist detail loading,
`TrackSelected` updates the retained caption while selection stays unavailable.
Display sorting follows the new captions; queue order and saved playlist order stay
unchanged. Server search continues matching both ordinary and Unicode metadata.
Checks live in [preferences tests](../../crates/osu-radio-client/src/view_models/preferences/tests.rs),
[playlist tests](../../crates/osu-radio-client/src/view_models/playlists/tests.rs) and
[queue tests](../../crates/osu-radio-client/src/view_models/queue/tests.rs). The first associated beatmap with a stored
cover reference supplies the cover ID. For sets with multiple distinct audio rows, unique
difficulty names are joined in the subtitle (`Artist | Easy, Hard`); shared-audio sets do
not add difficulty notation. `Track` retains every related difficulty (IDs, nullable name, original set
multiplicity), plus title, artist, subtitle and optional `Duration`, with no toolkit assets. Unknown duration is `--:--`. Selection survives a
refresh by audio ID, falling back to the first remaining row. Tests live in
[track.rs](../../crates/osu-radio-client/src/view_models/track.rs).

The [per-file coverage worklist](test-coverage.md#client-dtos-and-presentation-state-2026-10-15)
records executed DTO volume/null/default/token checks and library/queue request
cancellation, stale-result and selection assertions. The full client suites run
with `mock` off/on; the separate embedded-server test uses a private SQLite and
checks folder persistence through restart. These checks do not establish physical
audio or native Windows behavior.

The [player/settings/playlist owner checks](test-coverage.md#client-player-settings-and-playlist-owners-2026-10-15)
exercise token/item-bound seek effects, local presentation while disconnected,
folder alias preview cancellation and chooser epochs, and retained offline
playlist drafts/list/mutation state. These reusable VM tests use synthetic state
and fake workers/sessions; live Qt interactions and physical sound are separate.

## Track sorting and listening dates

The shared [sorting keys](../../crates/osu-radio-client/src/view_models/sorting.rs)
defaults to `TrackSort::TitleAsc`; `SetTrackSort` changes
the mode for the current session, including search results, refreshes and open
playlists. `ArtistAsc` compares artist then title; `RecentlyPlayed` compares
`last_played_at_ms` descending with unheard rows last, then title and artist.
Alphabetical keys use the displayed fallback strings and Unicode `to_lowercase()`,
with audio IDs (library) or item IDs (playlist) as the final tie-breaker.
This is deterministic string ordering, without locale collation.

Playlist items and presentation rows are reordered together. Display sorting
does not write playlist order or alter the server's queue snapshot. The dedicated
`LibraryTracksReordered` / `PlaylistTracksReordered` update preserves selection, duration and artwork caches, media
generation and pending jobs. Qt's existing `AppMenu` selects the mode through its
typed library adapter.

Only a successful engine start sends the token-bound `started` callback through
the existing retry lane. Loading, decoding or device failure records no listening
date; pause/resume of an already started launch records no new date. Updated
assignment dates are merged into every known row sharing that audio ID and
immediately reorder recent results. Late list responses retain newer known dates.
The [database history contract](database.md#listening-history) defines retention
across removal, reimport and restart.

## Local audio playback

[`osu-radio-player`](../../crates/osu-radio-player/src/lib.rs) wraps Rodio 0.22.2
with `playback`, `mp3`, `vorbis` and `wav` features (defaults disabled). Its synchronous `Player` accepts local
paths and exposes load/play/pause/stop/seek/volume/snapshot. It has no HTTP, IDs,
database or toolkit dependencies. Content sniffing supports extensionless lazer
files. Loading prepares a paused source and preserves the previous source on
open/decode failure. Stop resets position; Play after EOF restarts; seek clamps to
known duration and preserves Playing/Paused, restoring EOF in pause. Volume is a
finite linear coefficient in `0..=1`, initially 1, retained for the session.
Tempo and pitch come from one `PlaybackParameters` pair; see
[Adjustable speed and pitch](#adjustable-speed-and-pitch).

Supported audio is MP3, Ogg/Vorbis and PCM WAV, matching the `.mp3`, `.ogg` and
`.wav` audio extensions in [osu! source](https://github.com/ppy/osu/blob/master/osu.Game/Utils/SupportedExtensions.cs)
(checked 2026-09-22). The [osu! audio guide](https://osu.ppy.sh/wiki/en/Guides/Compressing_files)
identifies Vorbis as the OGG codec. Extensionless decoder and seek coverage lives
in [player tests](../../crates/osu-radio-player/src/tests.rs); adding decoders
requires no HTTP or GUI changes because the Qt frontend use this shared engine.

The shared [controller](../../crates/osu-radio-client/src/controller.rs) serializes
Play/Pause/Stop/Next/Previous commands through the server queue API. The server
persists queue order, history and current position; the client receives only the
current [assignment](../../crates/osu-radio-client/src/models.rs), including
metadata, duration, transition availability, `revision` and `playback_token`.
The [player owner](../../crates/osu-radio-client/src/view_models/player.rs) applies
strictly increasing revisions and reads a snapshot before each reconnect to the
NDJSON event stream. Events use a separate one-hour deadline and blank-line
heartbeats. Disconnecting the stream leaves current local playback running;
reconnecting with the same entry ID/token preserves position. A new entry or token
starts a new launch even for an identical audio ID.

The T06 transport now exposes the full queue through `QueueState.entries`, with
`current_entry_id`, `history_len`, persisted `waiting_for_check`, mode and counters.
Client [DTOs](../../crates/osu-radio-client/src/models.rs) use an independent
`QueueEntryId` with positive i64 validation and decimal-string serde, including the
maximum BIGINT. Qt/QML bindings carry that string without converting through JavaScript Number. `PlaybackAssignment` receives nullable entry ID and
availability plus waiting in both REST and NDJSON.

[ApiClient](../../crates/osu-radio-client/src/api.rs) provides `insert_queue(items,
placement)`, `append_playlist_to_queue`, `play_queue_entry`, `move_queue_entry(id,
before_entry_id)`, `remove_queue_entry` and `clear_upcoming_queue`. `append_queue`
wraps typed audio insertion at end; `queue` reads the full list and `clear_queue`
clears everything. A null move anchor means end. Unknown references are 404;
Play of an existing unavailable entry is 400. New operations return assignments.
[HTTP fixture tests](../../crates/osu-radio-client/src/api/contract_tests.rs) assert
methods, paths and bodies, including maximum string IDs and explicit null anchors.

The [Queue VM](../../crates/osu-radio-client/src/view_models/queue.rs) now owns
`QueueView.entries: Vec<QueueRow>`, including history, duplicates and nullable
sources. Rows retain entry ID, position, optional audio/item ID, availability and
`Track` presentation with saved ordinary/Unicode captions and difficulty suffix.
The view carries current ID, history boundary, mode, waiting, revision/token,
`loading`, operation `busy`, load `message` and independent `operation_error`.
A failed GET or operation preserves the confirmed list and local playback.
`AppCommand::Queue(QueueAction)` supports Refresh, Play/Remove by entry ID,
Move with a nullable anchor, ClearUpcoming, ClearAll, and typed Insert at End/Next.
All mutations share `PendingPlayback` with transport/callback/playlist commands;
only token-bound callbacks retry. Whole-playlist append calls the server directly.
Success applies the assignment and refreshes an open panel, including a no-op;
closed panels do no GET, and epochs/cancellation/revisions reject stale replies.

`SetVisibleQueueMedia(Vec<(QueueEntryId, bool)>)` replaces queue viewport demand;
the boolean means artwork missing. The scheduler resolves IDs against the current
Queue VM, merges duplicate audio IDs with library/selection demand and retains
four pipelines and the shared cache. Closing drops queue demand. Loading a full
history alone requests no media. Qt's full queue uses a separate `TrackListModel`
instance and reports only viewport entry IDs. Track insertion menus use the same queue lane (T09).
The [T07 tests](../../crates/osu-radio-client/src/view_models/queue/t07_tests.rs)
cover mixed HTTP order, errors/no-ops/close, recovery, source rebinding, and visible
media demand with 5000 past rows. The Qt [fixture](../../apps/osu-radio-qt/tests/fixtures/server.py)
uses the T06 wire shape; fixture/probe evidence is separate from physical sound.

A dedicated client worker owns the engine and opens the device only when an
assignment starts playing. Restored paused queues, normal startup and offline
galleries require no audio device. [`ApiClient`](../../crates/osu-radio-client/src/api.rs)
downloads audio by ID into a temporary file in chunks, with a 256 MiB limit and
30-second timeout. The worker retains the current file and one pending download;
changing the entry ID or token cancels and invalidates old downloads. A cached identical ID
can restart without downloading again. Finished/download/decode callbacks carry
the token of their launch, and the server ignores stale or repeated callbacks.
`waiting_for_check` travels through Player presentation, Playback and worker Assign.
Waiting does not download audio, open an engine or send Started/Failed; Play/Pause
retain server intention. Releasing waiting starts only Playing; Paused waits for
explicit Play. Unchanged entry/token and cleared waiting preserve engine, position
and seek across list edits and availability updates. Playback uses the assignment's
source; a reimported queue-row binding cannot replace the existing launch's source,
listening date or volume. The first restored Paused assignment leaves library and
playlist selection independent and opens no audio device.
Source errors advance the queue; an output-device failure pauses it and retains
its position for explicit Play. Source and device callback retries pass through the same serialized command
lane and token check. Device failures use `pause` with an optional playback token,
so a delayed pause cannot affect a different current track.

The Qt frontend keep the selected-track panel. A normal Songs click only selects;
explicit Play of another selection inserts and starts it in the server queue.
Queue transitions select the current track and carry its title, cover and duration
even outside search results. Stop preserves an independently selected row. The
current-track metadata is kept separately from the visible library. Qt's existing
Next/Previous buttons use server availability and preserve pause; Previous on the
first item restarts it. Seeking remains local and checks the launch token so a
delayed same-ID seek cannot affect a duplicate queue item. Volume preferences
are persisted through the API and applied by the local worker.
Position updates arrive every 100 ms while playing and state changes immediately.
Qt's title-bar `list-music` button (accent while open; no count badge, because a closed queue is not loaded), immediately before the window controls, opens
[`QueuePanel.qml`](../../apps/osu-radio-qt/qml/components/QueuePanel.qml) over the
player with a dark gradient and rounded artwork cards showing title, artist and
duration. The virtualized `ListView` includes history, current and upcoming rows
under History / Now playing / Up next labels (`ListView.section` on the `queueSection`
role), checking/unavailable status and retained unavailable captions. History rows are
dimmed to 55%. The header reads "Queue · N left" (upcoming rows, found by bisecting the
`queueSection` role) with `list-x` Clear upcoming, `trash` Clear all and close icons.
Row actions (Play, Remove, `grip-vertical` handle) show on hover or focus inside the row,
otherwise the duration shows; hidden actions ignore the pointer but keep keyboard focus.
Add/remove/displaced list transitions use `motionBase`. Cards are 82px with
14px spacing, a 96px cache buffer and no delegate reuse pool. Opening positions
once on current (or first upcoming/end of history) after loading, without selecting
or playing. Later role changes preserve scrolling and decoded artwork URLs.
Close, Escape and outside click dismiss it and restore opener focus.

Control motion (all durations via `Theme.ms()`, curve `Theme.easing`): `AppButton` and
`IconButton` fade opacity (`motionFast`) and scale to 0.97 while pressed (60 ms); `AppSwitch`
slides its thumb (140 ms) and fades the track (120 ms); the selected tab is a light pill that
slides under the buttons in `WindowBar` and `AppTabs` (180 ms; tab buttons keep their
`variant`, `objectName` and size and have an empty background); a panel shown by a tab
switch fades in over `motionFast`; selected-card frames in `SongsPanel` and
`PlaylistTrackCard` fade their colour. Play/pause icons do not cross-fade. No focus rings.

Tooltips are `AppToolTip` children of the hovered item (`AppToolTip { visible: hovered; text: ... }`;
700 ms delay, `Theme.popup` surface, fades over `motionQuick`, closes with its owner) and every
vertical scroll bar is `AppScrollBar` (6 px overlay handle, 8 px on hover, no track, `AsNeeded`);
no production code uses the `Basic.ToolTip.*` attached properties or a bare `Basic.ScrollBar`.
`AppMenu.showToolTip` gates its full-text tooltip.

`Theme` holds the shared tokens: `hover`, `popup`, `modal`, `accentSoft` (pressed/selected tint),
durations `motionQuick` 80, `motionFast` 120, `motionBase` 160, `motionPanel` 200, `motionSlow` 250 ms
and `easing` (OutCubic). Every duration goes through `Theme.ms()`; the internal `Theme.motion`
flag (no UI setting, probes only) zeroes them. Cards, badges and gradient stops keep a few
literal colours on purpose: the Stable/Lazer badge colours, folder-row backgrounds and card
gradients. The connection banner in `Main.qml` is an overlay under the title bar (it never
shifts the layout): while `bridge.connected` is false it slides in with the status text, a spinner
during `connecting` and a Retry button otherwise.

The scene (`Main.qml`, and `Gallery.qml` with the mock store) owns one
[`ConfirmDialog`](../../apps/osu-radio-qt/qml/components/ConfirmDialog.qml) and one
[`Toast`](../../apps/osu-radio-qt/qml/components/Toast.qml); panels only emit signals or call
`shell.confirm(...)`, `shell.expectQueueToast(text)` and `shell.deletePlaylist(id)`. Clear all and
playlist Delete ask first (Cancel holds the initial focus; Escape and an outside click dismiss
without acting); Clear upcoming acts at once. The toast is a non-modal overlay popup (so the queue
dimmer does not cover it), shown 1000 ms, restarted by a new call, with no buttons or Undo. Success
toasts (“Added to end of queue”, “Will play next”, “Playlist added to queue”, “Upcoming
cleared”, “Queue cleared”, “Playlist deleted”) are inferred in QML by
[`OperationWatcher`](../../apps/osu-radio-qt/qml/components/OperationWatcher.qml): `queueBusy` or
`playlistBusy` must rise, then fall without an error message (a deferred check, because busy and
message are separate notifications). An action rejected before it became busy, or issued while another
was already busy, shows no toast; errors never toast. Playlist delete succeeds when the id has left
`playlists`. The mock store applies actions synchronously, so the gallery reads the result at once.

The queue adapter exposes `tracks`, `startIndex`, `queueLoading`, `queueBusy`, `queueMessage`,
`setQueueVisible(bool)`, `playEntry(QString)`, `removeEntry(QString)`,
`moveEntry(QString, QVariant)`, `clearUpcoming()`, `clearAll()` and
`setVisibleQueueMedia(QVariantList)`. Entry IDs and anchors are decimal strings,
including values above JS integer precision; a null anchor means end. Rust rejects
invalid IDs and non-string anchors before dispatching existing QueueAction.
Existing roles 256–263 stay unchanged; queue adds 264 `entryId` (string),
265 `isCurrent`, 266 `isPast`, 267 `availability` (checking/available/unavailable) and
268 `queueSection` (`history`/`current`/`upcoming`, empty outside the queue).
`entryIdAt(index)` resolves keyboard/drop anchors. Queue reconciliation uses
entry identity and standard row insert/remove/move notifications; role updates
use `dataChanged`. Retained artwork is keyed by entry identity and cover source.
Library and playlist instances retain their existing behavior.

Play (including checking), remove, both clears and Ctrl+Up/Down dispatch existing
client operations. The separate drag handle shows an insertion line and scrolls
at viewport edges; the drop index comes from live delegate midpoints, not a fixed row step; only drop sends one ID/anchor command. Server confirmation
updates order, and cancellation or rejection keeps the confirmed list. Arrow
navigation changes focus; Enter/Space plays and Delete removes. Child actions do
not activate the card. QML sends only viewport entry IDs on scroll/model/delegate
changes; closing sends an empty demand snapshot. Duplicate audio shares the
existing bounded media scheduler/cache. The gallery connects the same panel and
model to MockQueue through MockBridge/Store, with an explicit error example,
without a session, database, file checking or audio device.

The controller loads `/api/queue` only while open, refreshes on committed revisions
and rejects stale replies. Queue media remains independent of Songs search.
Loading, empty and retryable error states are explicit. Qt model/HTTP/offscreen
checks and measured virtualization limits are recorded in the
[T08 report](../impl/queue-rework/T08.md#отчёт-о-выполнении); native Linux/Windows
input, GPU rendering and physical audio remain T10 acceptance.
Qt's [progress slider](../../apps/osu-radio-qt/qml/components/PlaybackProgress.qml) fills the elapsed
portion with `Theme.accent`, following its position during playback and seeking. Its 24 px
seek zone has a 4 px track (6 px on hover) and shows an `m:ss` bubble at the pointer, or at the
handle while dragging. A replaced cover and backdrop cross-fade through
[`CrossfadeSource`](../../apps/osu-radio-qt/qml/components/CrossfadeSource.qml); a change to or
from an empty source swaps at once.
Shutdown cancels the event stream and downloads, joins the worker, releases its
files, then ends the backend session.

Coverage lives in [worker tests](../../crates/osu-radio-client/src/playback.rs),
[queue VM tests](../../crates/osu-radio-client/src/view_models/queue/tests.rs),
[stream tests](../../crates/osu-radio-client/src/api/playback_tests.rs), and the
[Qt probe](../../apps/osu-radio-qt/tests/AdapterProbe.qml). Galleries remain offline.
Output-device selection remains deferred; shuffle and repeat are described in
[their section](#shuffle-repeat-and-previous).
Physical output, desktop slider interaction and Windows require separate manual
checks.

The real worker-thread test uses a fake engine to assert download generations,
stale-file deletion, ordered Started/Finished feedback, periodic Playing updates,
source replacement, joined shutdown and temporary-file cleanup. Core tests also
reject nonfinite/out-of-range assignment volume without replacing the current
source, mode, token or volume. These checks open no audio device.

### Shuffle, repeat and Previous

`PlaybackAssignment` carries `shuffle` and `repeat` ([`RepeatMode`](../../crates/osu-radio-client/src/models.rs),
`#[serde(default)]`), and `PlaybackCommand::SetShuffle/SetRepeat` send the server commands.
`AppCommand::ToggleShuffle` and `CycleRepeat` compute the target from the last
assignment (Off → All → One → Off) in the
[Player VM](../../crates/osu-radio-client/src/view_models/player.rs); with no
assignment nothing is queued, and offline `queue_playback` reports the error.
`PlayerPresentation` exposes `shuffle`, `repeat` and `can_change_modes` (connected
and an assignment exists). The server owns reordering and repeat transitions; the
client only displays the answer.

One rule, `PlayerViewModel::previous`, serves the button and the system Previous: if
the current launch is sounding in the worker and the shown position (speed-scaled, as
in the panel) is strictly over 3 s, it seeks to 0 with the current token and sends no
server command (pause is kept, no `Started`); otherwise it sends Previous. Server
`can_previous` is false on the first entry, so `PlayerViewModel::previous_restarts`
also enables the button (`PlayerPresentation.can_previous`) and the system
`capabilities.previous` while that local restart applies: on the first entry Previous is
available only when the sounding track is past 3 s, and disabled again at 3 s or less
(the user chose this, 2026-10-24; the queue does not move either way). Covered by
`previous_stays_available_on_the_first_entry_while_a_local_restart_applies`.

Qt: the player panel's `shuffleButton` and `repeatButton` use the shared
[`ShuffleModeButton`](../../apps/osu-radio-qt/qml/components/ShuffleModeButton.qml) and
[`RepeatModeButton`](../../apps/osu-radio-qt/qml/components/RepeatModeButton.qml)
(icons `shuffle`, `repeat-2` for Off/All, Lucide `repeat-1` for One; accent when on;
names "Shuffle: off/on", "Repeat: off/all/one"), enabled while `connected &&
canChangeModes`. The offline gallery shows the same components against the mock
(`mock::Action::ToggleShuffle/CycleRepeat`); the mock stores the modes but does not
reorder entries. Checks: Player VM tests in `controller/system_media/tests.rs`,
`view_model_tests.rs`, `models.rs` and `mock/queue.rs`; Qt adapter probes `playback`
and `gallery-queue` in [AdapterProbe.qml](../../apps/osu-radio-qt/tests/AdapterProbe.qml).

## Discord Rich Presence

The shared client [Discord service](../../crates/osu-radio-client/src/discord.rs)
uses `discord-rich-presence` 1.1.0 and public Application ID
`1558624797142028359`. Qt exposes **Settings → Integrations → Discord Rich
Presence**, searchable by section/name, default off. Its existing native
`QSettings` store persists `integrations/discord_rich_presence`; read failures
leave RPC off, write failures keep the session choice with an inline message.
The offline component gallery does not read/write settings or start RPC.
All local preferences load together so a malformed settings file resets both
Unicode choices and RPC safely; each toggle still writes only its own key.
Other desktop adapters can send `AppCommand::SetDiscordRichPresence(bool)`;
no Discord logic, database settings or HTTP routes are duplicated in UI/server.

Only accepted token-bound local worker updates publish presence. Loading,
empty/ended/stopped sources and local playback errors clear it; library selection
and server transport errors do not invent Playing or replace a locally playing
track. Real decoder duration and successful local seek serials anchor timestamps.
A coalesced watch snapshot deduplicates token/source/state/seek/display metadata/
duration changes; ordinary 100 ms position ticks never send IPC updates.
Those ticks refresh the local watch value without notifying the worker, so a
later enable/reconnect starts from the newest measured position. Position and
duration are converted to real time by the applied speed; a speed change alone
republishes, including while paused.
Playing uses Listening (2), title/details and artist/state. Unicode preferences
apply to current-track metadata. Text uses existing fallbacks and truncates safely
to 128 UTF-8 bytes. Paused retains title plus a static `Paused • mm:ss • artist`
label with no timestamps; resume/seek rebuild start/end timestamps. Discord
controls progress rendering and the app name; unknown/zero duration omits timers,
known duration uses second-resolution timestamps. No paths or database IDs are
sent in Activity; the only extra datum is that public cover URL.

The library supplies Activity serialization and outgoing JSON framing.
[The socket adapter](../../crates/osu-radio-client/src/discord/transport.rs)
implements its `DiscordIpc` hooks on one lazily started blocking worker, sharing
the existing Tokio runtime. Incoming frames, including READY, are capped at
64 KiB before allocating the body. Handshake, writes and completion of a started
frame have two-second I/O deadlines. Connected idle waits for an actual first
byte using cancellation-safe `read`, with no idle deadline; readiness alone is
not evidence that a new frame has arrived. That byte is retained until the rest
of the frame is read and discarded on close. PING is echoed byte-for-byte as
PONG, including empty/binary payloads; incoming PONG is ignored. EOF, incomplete,
invalid JSON, oversized and invalid-opcode frames drop IPC.

Every transport failure schedules exponential retry at 1, 2, 4, …, 30 seconds.
READY does not reset that delay; a matching successful Activity/clear ACK does.
Updates during retry replace one watch value. Reconnect reads the newest local
snapshot and projects its position from the monotonic observation anchor.
Disabled RPC drops IPC and parks the already-created worker for reuse; default
startup creates no worker. Disable/shutdown interrupts handshake, started reads
and reconnect waits. Controller shutdown attempts clear with a bounded write
deadline, drops IPC and awaits the worker before the runtime. Music, GUI and
server workflows never await Discord operations.

Activity/clear responses are correlated with the latest request's nonce and
Activity key; stale ERROR/ACK cannot change the current Activity or artwork
policy. The optional image key is `osu_radio`. A matching Activity ERROR allows
one retry without artwork on the same connection. Only successful ACK of that
retry disables artwork for the rest of the worker's process lifetime. A second
rejection suppresses that key until it changes or IPC reconnects; clear ERROR
never triggers artwork fallback. Transport stages and ERROR code/message are
logged to stderr without dumping payloads. No uploads are performed. When a
difficulty of the current track carries a public osu! set ID
(`beatmap_set_online_id`), the large image is
`https://assets.ppy.sh/beatmaps/{id}/covers/list@2x.jpg`, fetched by Discord;
otherwise it stays `osu_radio`. Local-only sets have no such ID. Linux scans
`XDG_RUNTIME_DIR`/temporary directories, IPC indices 0–9, Discord/Vesktop Flatpak
and Snap layouts, with Snap runtime-parent support. Windows uses Tokio named
pipes, indices 0–9. Compatible clients must expose Discord RPC; sandbox permissions
can still prevent access. Linux/Windows implementation and cross-compilation are
separate from actual native Discord/Vesktop runtime acceptance.

Manual setup and acceptance:

1. Open Discord Developer Portal → application `1558624797142028359` → General
   Information; verify its name is **osu! radio** and save if changed. RPC uses the portal name.
2. Optionally open Rich Presence → Art Assets, upload your osu-radio logo, name
   the asset **osu_radio**, save and allow Discord's asset propagation time.
   The checkout only bundles stable/lazer source logos, so use your project logo
   rather than treating either source logo as the osu-radio application logo.
3. Enable activity sharing in Discord and RPC in osu-radio Settings. Play a track,
   pause/resume, seek both directions, next/previous and stop; check profile details
   against local audio. Keep the status visible for at least one minute. Change
   Unicode preferences and test one-character titles/artists and long artists
   while playing; paused time must remain visible.
4. Launch osu-radio before Discord; start/close/restart Discord during playback;
   toggle RPC and exit during reconnect. Verify no stale activity or audio interruption.
   Repeat with native Discord and an RPC-enabled Vesktop, including relevant
   Flatpak/Snap permissions, on Linux; repeat with native Discord on Windows.

Automated coverage: [Activity/lifecycle mock tests](../../crates/osu-radio-client/src/discord/tests.rs),
[real IPC worker regressions](../../crates/osu-radio-client/src/discord/tests/ipc.rs)
cover READY/ACK silence, fragmented/bounded frames, cancellation, raw PING/PONG,
nonce-correlated fallback, stale replies, clear errors and reconnect backoff.
The [transport tests](../../crates/osu-radio-client/src/discord/transport.rs)
also launch child probes with cleared, private runtime/temporary-directory
environments for Unix discovery (including compatible-client subpaths, index 9,
Snap's runtime parent and absence). They exercise the real byte-read path without
searching the user's Discord sockets. Disconnected operations, nonce wraparound,
restored cancellation after a failed clear, extreme timestamp bounds, watch
notifications/coalescing and service teardown are asserted directly. IPC fixture
helpers validate handshake version, PID and nonce fields as well as reply shapes.
Other coverage includes controller stale-token/selection tests,
successful seek serial tests and the Qt `discord-*` persistence/search probes.
Real Discord appearance, artwork propagation, sandbox permissions, physical
sound and native Windows GUI/runtime remain manual checks.

## Individual track volume

The shared [Settings VM](../../crates/osu-radio-client/src/view_models/settings.rs)
loads audio settings before sending playback commands or assignments to the audio
worker. Loading failures expose retry and block playback. Preferences are separate
from engine snapshots; worker notifications never overwrite the selected slider.
Every assignment carries its calculated volume before any load/play operation,
including queue transitions, cached restarts and resumed playback.

Qt Settings groups audio preferences, status and Retry under **Audio**. It uses
the existing `AppSwitch` for **Individual track volume**
and shows **Global volume** when enabled. The player popup edits the selected
audio's absolute percentage, or the global percentage while disabled. Tracks
without an override inherit global volume. Global 20%, track 10%, then global
40% leaves that track at 10%. **Use global volume** removes the override. Disabling
uses global volume for every track and retains overrides for re-enabling.
Selecting B while A plays edits B without changing A's volume. Shared audio in
different playlist rows has one setting. Unavailable/no selection disables the
individual slider; selection/mode changes close the popup and end editing.

The volume button icon follows the level (`volume-x` at 0%, `volume-1` below 40%,
else `volume-2`) and the popup labels its scope ("This track" / "All tracks"). The
mouse wheel over the button or the open popup changes volume by 5% per 120 wheel
units (touchpad deltas accumulate) through the slider's commands. The modes button
turns accent and shows a speed badge while a mod is active and available; its
tooltip names the mode ("Mods (F1): DT 1.5×").

Mute is session-only player state, not a volume: `PlayerViewModel.muted`
(`PlayerPresentation.muted`, Qt `muted` / `toggleMute()`), set by
`AppCommand::SetMuted`. It is never saved, sends no server request and does not touch
global or per-track volume. The playback worker takes `Command::SetMuted` and gives the
engine `0.0` (on assignment, `SetVolume` and engine creation) while `snapshot.volume`
keeps the real level; unmuting restores it. Muting never starts the worker or opens an
audio device (a worker created later is told first). It survives track and queue
changes; the popup's mute button (left of the slider), key **M**, and a volume change
(`SetVolume`, or `SetTrackVolume` with a value: slider or wheel) end it. While muted the
button shows `volume-x` and the popup reads 0% with the slider keeping the saved
value. MPRIS/SMTC and Discord do not see mute. The offline gallery has no player
surface, so the mock has no mute.

Global keys live in [Main.qml](../../apps/osu-radio-qt/qml/Main.qml). Space (play/pause
under the `playPauseButton` conditions), M (mute) and ←/→ (seek 5 s, only with `canSeek`) are
handled by `Keys.onPressed` on the scene, so they run only for keys that no focused
field, button or slider consumed; they are off while a dialog, the mods or volume popup
or the queue is open. The scene takes focus at start. Search fields take focus only by
click or Ctrl+F (the active tab's field; nothing in an open playlist); Escape in that field
returns focus to the scene. Probe `hotkeys` covers these; seeking itself is not observable
offscreen.

Edits apply immediately. One serialized save lane coalesces each target's fast
changes for 200 ms; selection changes finish pending editing. Responses only
acknowledge saves, so delayed reads/responses cannot undo local edits. Save errors
expose retry in Settings and the popup. Normal shutdown awaits pending writes
before terminating the backend; a failed final write is reported without an
unbounded retry loop. All persistence stays in the application's database.
Qt is the desktop GUI; the removed Vizia frontend has no compatibility path.

[Client checks](../../crates/osu-radio-client/src/view_models/volume/tests.rs),
[worker checks](../../crates/osu-radio-client/src/playback.rs) and the
[Qt probe](../../apps/osu-radio-qt/tests/AdapterProbe.qml) cover absolute values,
selection/current independence, reset, mode retention, 0/100 bounds, loading/save
errors, debounce, stale responses, first-play volume and shutdown flush. Manual
slider interaction and physical output on Linux/Windows remain separate checks.

## Adjustable speed and pitch

Implementation of DT/HT/NC from the [specification](../specs/adjustable-pitch-speed.md)
(R1–R8) and [plan](../impl/adjustable-pitch-speed.md). Automated evidence is listed
at the end; listening, native input and Windows acceptance are not claimed here.

**Profile and storage.** A profile is `mode: none|dt|ht`, integer `speed_percent`
and `adjust_pitch` (none = 100/false, DT 101–200, HT 50–99). NC is DT with
`adjust_pitch`; no separate NC profile exists. The server stores the global set and
mode switch in `user_data` and individual profiles by audio source identity in
`audio_mods`; see [database](database.md#audio-mods) and the
[HTTP contract](backend.md#audio-mods-http-contract). Library, playlist, queue and
assignment DTOs carry nullable `mods` — the stored individual profile, never the
global set. Absence means 1× in individual mode; the global set is not inherited.

**Client state.** The [mods module](../../crates/osu-radio-client/src/view_models/mods.rs)
extends `SettingsViewModel` (no sixth owner). `Editor` holds the D/E rules (D: DT →
NC → none, new DT 150% without pitch; E: HT 75% toggle; DT ↔ NC keeps speed),
bounds, local overrides and an `epoch`/`ModsEditTarget` that rejects late edits for
a previous target. `ModsWork` is a save lane separate from volume: 200 ms
coalescing per target, Retry, acknowledgements that never overwrite a newer local
value and a bounded flush on shutdown. Commands are `AppCommand::Mods(ModsAction)`,
state arrives as `AppUpdate::AudioMods(AudioModsState)`. The offline
[mock](../../crates/osu-radio-client/src/mock/mods.rs) reuses `Editor`.

**Playback gate and assignment.** One gate, `SettingsViewModel::playback_ready()`,
requires both volume and mods loads; a failed mods load keeps it closed until Retry.
The last assignment is held and released from either load response. `Assign`
carries `PlaybackParameters` computed from the saved launch source's profile
(`assignment_mods`), applied before the first `play`; the worker validates the
finite 0.5–2.0 pair before changing anything. An edit of the sounding launch
re-sends the same entry/token, updating the engine in place without reload,
position, pause or token change. Editing selected B while A sounds changes only
B's stored profile, projection and displayed duration; it neither starts B nor
touches the queue revision/token.

**Engine.** [Player](../../crates/osu-radio-player/src/lib.rs) takes
`PlaybackParameters { speed, adjust_pitch }` (`set_parameters`, `Snapshot.parameters`).
[stream.rs](../../crates/osu-radio-player/src/stream.rs) leaves 1× untouched, uses
Rodio's speed change when pitch follows speed and Signalsmith Stretch
(`signalsmith-stretch =0.1.3`, MIT) when pitch is preserved, in 512-frame blocks with
a bounded lookahead, no new thread and a single end-of-stream after the tail.
Snapshot position/duration are **source** coordinates; parameter changes apply at
the next block boundary. See the [native build notes](development.md#signalsmith-native-build).

**Time scale.** `PlayerPresentation` divides source position/duration by the actual
speed (4:00 at 1.5× shows 2:40); `selected_duration` uses the selected track's own
profile and metadata is untouched. `AppCommand::Seek` takes UI time and
`seek_selected` multiplies by the actual speed once; system media seek stays in
source time. Qt's [`PlaybackProgress`](../../apps/osu-radio-qt/qml/components/PlaybackProgress.qml)
cancels a drag that survives a launch, token, speed or selection change.

**Qt.** The popup ([AudioModsPopup](../../apps/osu-radio-qt/qml/components/AudioModsPopup.qml))
sits beside the volume button in [PlayerPanel](../../apps/osu-radio-qt/qml/PlayerPanel.qml);
F1 is a `Shortcut` in [Main.qml](../../apps/osu-radio-qt/qml/Main.qml), D/E work only
inside the open popup, mouse and keys emit the same intents. `beginModsEdit` captures
the target in Rust and selection/mode changes close the popup. Settings has an
independent **Individual track mods** switch (searchable) with status and Retry.
The gallery uses the shared mock and stays offline.

**Integrations.** MPRIS/SMTC receive source coordinates plus the worker's applied
speed as `rate` (MPRIS `Rate`, 0.5–2); only a confirmed matching worker reports a
non-1.0 rate. Discord converts position/duration once in `Presence::from_playback`
and republishes on a speed change, including while paused. OS-side mod control
is not provided.

**Automated evidence** (Linux only; see the
[T11 report](../impl/adjustable-pitch-speed/T11.md#отчёт-о-выполнении) for commands
and results): player signal/frequency/duration, EOF and live-edit tests;
[client mods tests](../../crates/osu-radio-client/src/view_models/mods/tests.rs),
[controller HTTP test](../../crates/osu-radio-client/src/controller/mods_tests.rs),
worker gate/parameter tests in `playback.rs`; Qt popup, progress, gallery and live
probes in [AdapterProbe.qml](../../apps/osu-radio-qt/tests/AdapterProbe.qml). The
opt-in [real-backend probe](../../crates/osu-radio-client/src/playback/tests/real_backend.rs)
`real_server_mods_follow_each_launch_across_restart_reimport_and_live_edits` drives a
real server and SQLite with the worker core and a **fake engine**: mode toggle,
A/B edits, restart, remove/reimport, explicit Play and automatic next, asserting the
pair seen at each first `play`. It does not exercise the real DSP, the controller's
presentation/system snapshot or Qt; those are covered separately with fakes. Physical
sound, audible artifacts, native F1/D/E, live drag, MPRIS/SMTC panels, real Discord
and Windows remain manual (T12).

## User playlists

Adding a new osu! source imports Stable/Lazer collections into this same tab,
including empty collections and unavailable difficulties. “Refresh import” in
the folder modal rereads a registered source and updates each linked playlist's
name and membership, replacing manual membership edits while preserving custom
covers, surviving item IDs and order. Disappeared collections and playlists from
removed sources remain visible; missing beatmaps retain their last known captions.
The backend owns collection identity and hash matching; see the
[database contract](database.md#user-playlists).

The shared [`PlaylistsViewModel`](../../crates/osu-radio-client/src/view_models/playlists.rs)
owns navigation, independent playlist query, HTTP requests, inline editor drafts,
selected item IDs and request epochs. Songs always displays the library. The
Playlists tab beside Songs restores its last list/detail and selected difficulty
for this session; the back arrow returns to the list. Settings is the third
navigation tab after Songs and Playlists.
The player stays on the right with the existing window and sidebar dimensions.

[`PlaylistPane.qml`](../../apps/osu-radio-qt/qml/components/PlaylistPane.qml) shares
its list, editor and track cards with the offline gallery. List cards show square
artwork, names and counts including unavailable entries. Their rounded dark menu
contains **Add to queue**, **Edit**, **Delete**, in that order, with trailing SVG
icons and red Delete text. Add to queue calls `append_playlist_to_queue`
directly through the existing serialized lane, retaining saved order, duplicate
difficulties, unavailable items and playlist-item identity without a client GET.
It preserves active playback or pause; an empty/exhausted queue stays stopped.
An open playlist shows a header under the back/title row: 72×72 cover, track count,
**Add to queue** (same gating as the card menu) and accent **Play**
(`playRequested(id)` → `playPlaylistFrom`; enabled when connected, idle and non-empty).
Unavailable entries are skipped by the backend; a wholly unavailable playlist stays stopped.
The [offline queue mock](../../crates/osu-radio-client/src/mock/queue.rs) provides
history, distinct entry IDs, duplicates, checking/unavailable rows, all QueueAction
mutations, whole-playlist append, and memory-only check completion. Invalid actions
preserve confirmed data and expose an operation error. The gallery snapshot exposes
queue state without opening a Session, checking files or creating an audio device.
`+` opens the inline new-playlist card; the detail pencil
opens the same name/cover editor. Success closes it; failure retains the draft.
[`PlaylistEditor.qml`](../../apps/osu-radio-qt/qml/components/PlaylistEditor.qml)
uses a full-width Create/Save button. The editor requests its saved cover when
opened; detail navigation preserves the decoded-image cache.
If creation succeeded before a cover upload failed, retry updates the saved ID
instead of creating another playlist. Playlist search uses the shared filtered
projection; the add dialog keeps the complete list of targets.

[`PlaylistTrackCard.qml`](../../apps/osu-radio-qt/qml/components/PlaylistTrackCard.qml)
shows each concrete difficulty separately with background artwork, its duration
and the same accent selected frame and tinted backdrop as the Songs card. Click selects only; existing Play launches the
selected item. Unavailable entries stay selectable/removable with Play disabled.
Right click, Menu or Shift+F10 opens the track menu (see Track insertion menus). Display sorting retains selected
item identity without changing insertion order or the queue snapshot. Pause,
resume and seeking match `current_playlist_item_id`, including shared audio.

[`PlaylistsModal.qml`](../../apps/osu-radio-qt/qml/components/PlaylistsModal.qml)
is only the add-difficulties dialog. If library search omits the selected song,
the client loads its complete difficulty group without changing that search.
Late openings/results are ignored; failed mutations retain selections for retry.
The former playlist-management entrance and modal are removed.

Qt's native FileDialog selects local PNG/JPEG up to 16 MiB and 16 megapixels.
[`artwork.h`](../../apps/osu-radio-qt/src/artwork.h) applies image orientation,
center-crops and prepares a 512×512 PNG; reusable upload/download behavior stays
in the client. Picker epochs and latest-choice checks reject stale results.
The shared 64 MiB decoded-image cache has separate playlist/version and draft
keys; QML uses `cache: false`. Custom images persist in the database independently
of the selected file; reset uses artwork from the first item in insertion order,
then the neutral placeholder if unavailable. Summary loading excludes PNG bytes.
Use automatic cover previews that fallback immediately in the editor while the
saved custom cover stays intact until Save; closing the editor discards the reset.

Persistence and HTTP contracts are specified in [database](database.md#user-playlists)
and [backend](backend.md#user-playlists). Coverage is in
[client tests](../../crates/osu-radio-client/src/view_models/playlists/tests.rs),
[queue append tests](../../crates/osu-radio-client/src/view_models/queue/tests.rs),
[native Qt checks](../../apps/osu-radio-qt/src/native_tests.h) and
[Qt probes](../../apps/osu-radio-qt/tests/launch_smoke.rs). Desktop input, GPU
rendering, native picker appearance, physical audio and Windows remain separate
acceptance checks. Manual cropping and playlist track reordering remain deferred.

### Track insertion menus

[`TrackQueueMenu.qml`](../../apps/osu-radio-qt/qml/components/TrackQueueMenu.qml)
is shared by Songs (library/search), playlist track cards and offline gallery examples.
Right click, Menu and Shift+F10 open it relative to the source row; Escape or
activation returns focus to that row. “Add to end” and “Play next” change only
insertion placement, without selecting the row, starting audio or resuming a pause.
The menu order is Play, Add to end, Play next, a divider and Add to playlist…;
playlist track cards add a second divider and a red Remove from playlist. Play and
Add to playlist… select the row first, then call `togglePlayback()` /
`openPlaylistAdd()`; commands run in order in one controller loop, so they see the
new selection. Library Play skips the toggle when the row is already the playing
current track (a toggle would pause it); a playlist row that is the playing item
but not selected would still be paused (open question, no adapter change made).
Right click on a playlist list card opens the same actions menu as its “⋮” icon button. It is built on
`AppContextMenu`/`AppMenuItem`, as is the playlist card actions menu; popup
backgrounds (menus, dropdown lists, volume and Mods popups) share `PopupSurface`
(translucent surface with a soft shadow). `AppMenu` dropdowns are modal but do not
dim the window.

The Library Qt adapter's `insertTrack(id, next)` sends `QueueItemRef::Audio`;
the Playlists adapter's `insertPlaylistItem(itemId, next)` sends the active
playlist ID and concrete item ID, preserving shared-audio difficulty identity and
unavailable entries. Both use `QueueAction::Insert` in the existing client lane.
Disconnected, `queueBusy` or `playlistBusy` disables insertion; whole-playlist
append also observes queue busy and still calls the server without fetching items.
`queueMessage` exposes operation errors; confirmed rows update only from server
snapshots. Store JSON-encodes structured insertion payloads, and MockBridge
validates placement, item type and positive i32 IDs before its memory-only action.

[T09](../impl/queue-rework/T09.md) records the Qt keyboard/pointer/HTTP probes,
client fake-engine tests and limits. These checks do not establish native desktop
input, Windows, physical audio or real-database end-to-end acceptance (T10).

## Session and server supervision

`Session::start` starts `EmbeddedServer` by default, then creates an `ApiClient` at the reported URL. With `ServerOptions::remote_url`, it connects to that existing server without resolving a local executable or reading local database configuration. Remote URLs must use HTTP(S), have a host, and contain no credentials, query or fragment; a reverse-proxy path prefix is allowed. A read of `GET /api/queue` must succeed within `startup_timeout` before `Connected`; local child startup still uses stdout readiness without an HTTP probe. Cancellation drops the remote request. `Session` exposes `api`, `base_url`, and asynchronous `shutdown`; shutdown/drop only stop an owned child, never an external server. After `Connected`, the GUI loads library and folders independently, each with loading, empty and error states. Songs have a “Refresh library” button in the bottom-left footer, outside the scrolling list; it also retries failures and is disabled while loading. Status messages remain above the list and are hidden when empty; an empty result (client text `Nothing found.` or `No songs yet…`, connected, not loading) instead shows a centered empty state with Clear search / Add an osu! folder, and loading with no rows shows four pulsing skeleton cards, so any other text is treated as an error and stays in the status line. Settings show Retry only after a failure.

Qt reads the optional `OSU_RADIO_SERVER_URL` process environment variable in
[AppBridge](../../apps/osu-radio-qt/src/app_bridge.rs). An empty/invalid value is
a connection error, with no fallback to a local child. Without the variable,
local startup is unchanged; the gallery remains offline. The remote server owns
its library paths and shared queue/settings/playlists. Folder discovery acts on
the server; the native folder chooser still selects paths on the client computer,
so its result is usable remotely only when the same path exists on the server.
There is no authentication or per-client queue isolation. Use a trusted LAN/VPN;
the server is not configured for public Internet exposure.

The current [supervisor](../../crates/osu-radio-client/src/server.rs) has these behaviors:

- `ServerOptions` accepts `remote_url`, `binary`, `working_directory`, `address`, and `startup_timeout`. Defaults are no remote URL/binary/directory override, `127.0.0.1:0`, and 30 seconds. The address configures a local child; Qt uses its default, letting the OS select a port. With `remote_url`, child-process options are unused.
- Binary resolution checks the explicit option, `OSU_RADIO_SERVER_BIN`, a sibling of the current executable, then the executable name on `PATH`. The filename uses the platform's executable suffix. Build the server before launching the GUI; its Cargo dependency on the client does not build the server executable.
- The child receives `OSU_RADIO_SERVER_ADDRESS` from the options and, on Linux, `OSU_RADIO_SKIP_DOTENV=1`. It inherits the working directory unless overridden. The Linux child ignores `.env` in cwd and ancestors; SQLite defaults to the user XDG data directory without a URL. Explicit process URLs retain priority. Standalone servers retain dotenv loading and fatal malformed/unreadable-file errors unless the switch is exactly `1`. See [backend configuration](backend.md) and the [server config source](../../apps/osu-radio-server/src/config.rs).
- Stdin is closed. Stdout and stderr are piped and drained, with `[server]` prefixes. Stderr is forwarded while stdout is inspected for readiness; stdout continues forwarding afterward. Leaving either pipe unread can block the child.
- Readiness depends on a stdout line containing `listening on ` followed by an `http://` or `https://` URL. The server prints its bound address, so the URL contains the actual port rather than `0`. This is a protocol between [server `main.rs`](../../apps/osu-radio-server/src/main.rs) and `parse_ready_line`, not optional diagnostic wording.
- Early stdout EOF waits for child exit within the startup deadline; a still-running child is killed and reaped on timeout. Read failure, early exit and timeout have separate errors. Cancellation during startup also awaits termination. Post-start stdout/stderr read loops stop on read errors; there is no automatic restart or ongoing health monitor.
- `shutdown` takes the child once and awaits its termination; repeated shutdown is harmless. `kill_on_drop(true)` and `EmbeddedServer::drop` provide fallback termination, but `Drop` does not await it. A hard kill or crash of the parent cannot be assumed to run destructors or terminate the child.

Readiness parsing has focused unit tests in [server.rs](../../crates/osu-radio-client/src/server.rs). The [ignored embedded-server integration test](../../crates/osu-radio-client/tests/embedded_server.rs) starts a real server and performs database-backed requests; it clears inherited configuration in a subprocess, uses private HOME/XDG directories (an explicit private URL on other platforms), and checks restart persistence plus ignored valid/malformed/unreadable dotenv files on Linux. Its prerequisites and scope are recorded in [development](development.md).

[Session tests](../../crates/osu-radio-client/src/session.rs) cover remote HTTP
readiness, bad URL/status/JSON, retry, timeout/cancellation and server survival
after shutdown/drop. The `external_server_connects_without_a_local_binary_and_survives_qt_shutdown`
[Qt probe](../../apps/osu-radio-qt/tests/launch_smoke.rs) uses a separate HTTP fixture
and an unavailable local executable, checks library search and verifies that the
external server still responds after Qt exits. These are loopback/offscreen checks;
two-machine LAN, physical audio and Windows acceptance remain separate.

## GUI state and event flow

The [launcher](../../apps/osu-radio-qt/src/main.rs) selects live or offline gallery
mode before constructing the runtime. Live mode retains the runtime/controller
through awaited shutdown and child reaping. Gallery mode has no session or device.
The [runtime unit contract](../../apps/osu-radio-qt/src/runtime.rs) verifies that
completed tracked work is pruned, the shared context is cleared during shutdown,
retained controllers stop, pending tasks are cancelled and already-started
blocking work completes before shutdown returns, without opening a backend/device.

`AppController::spawn/send/shutdown` remains the command facade. Its one event
loop coordinates VM inputs and asynchronous results. Library and playlist list
updates have separate destinations; playlist rows carry their own item ID with
track metadata. Selection remains independent of playback. Position ticks update
the player projection without replacing track collections.

Library search retains 200 ms debounce, cancellation and late-response rejection.
Refresh repeats the current query; failed refresh retains rows/selection. The
library and playlist queries, lists and selections survive tab changes independently.
Qt Settings search stays local and filters setting/section names, not source paths.

The shared media scheduler consumes visible, selected and queue demands. Four
pipelines remain occupied through HTTP completion and decoding acknowledgement;
stale deliveries still acknowledge their tickets. Generations reject late replies.
Duration results and unavailable-cover metadata survive navigation. New sessions,
explicit library refresh and import refresh invalidate artwork; tab/search/playlist
navigation retains it. Encoded covers stay bounded to 16 MiB and 30 seconds.
Qt owns the shared 64 MiB decoded cache and scales images proportionally to 1280px.
QML provider images use `cache: false`; evicted images may reload without repeating
duration requests. Shutdown cancels requests and awaits workers and the child server.

The [controller checks](../../crates/osu-radio-client/src/controller.rs) and
[coordination tests](../../crates/osu-radio-client/src/controller/view_model_tests.rs)
assert generation wraparound with acknowledged HTTP cancellation and retained
decoder slots, unavailable-cover/unknown-duration caching, duplicate-row duration
updates and identical metadata lookup precedence in the controller and media
input. Fake-engine tests exercise stale/current audio-file ownership, offline
download errors, gated cancellation and public seek dispatch. Process fixtures
assert one pending connection, retained search debounce and reaping a session
whose startup completion arrives during shutdown. These checks do not open an
audio device.

The [client benchmark fixture](../../crates/osu-radio-client/src/controller/benchmarks.rs)
is an ordinary Unix test that drives both opt-in benchmarks in isolated child
processes. It compares captured legacy/compact payloads for five synthetic
queries and serves them through a private HTTP child with no database or media
access. It asserts representative metadata, empty results, duplicate audio and
all 120 requests across both endpoint/debounce modes. Standalone benchmark entry
points remain opt-in; fixture timings are correctness checks, not performance data.

## Implemented interactions and placeholders

| Surface | Behavior | Owner |
| --- | --- | --- |
| Songs / Playlists / Settings | Independent retained lists/searches/selections; player stays visible. | Main and domain panels |
| Track cards | Click selects; explicit Play starts. Context menus insert at end/next without selecting or starting; unavailable playlist items remain insertable/removable. | Library/Playlists/Queue VM |
| Transport | Client chooses Play/Pause/Resume using selected/current item identity and token. | Player VM and coordinator |
| Progress | Selected current launch may seek; dragging is protected from position ticks. | Player adapter/panel |
| Settings | Local preferences work without server; audio writes retry; staged folder actions preserve partial success. | Settings VM and Qt native storage |
| Window | Native Qt move/resize/minimize/maximize/restore/close; observes actual visibility. | WindowBar |
| Queue | Full virtualized history/current/upcoming model, entry-ID Play/remove/move and both clears; viewport media demand. | Queue VM / QueuePanel |

Shuffle, repeat, hosted providers and output-device selection remain deferred.
Offscreen probes do not establish physical audio, GPU rendering or native Windows behavior.

## Qt frontend

[osu-radio-qt](../../apps/osu-radio-qt/Cargo.toml) is a Qt 6.8+ / CXX-Qt 0.10
frontend using the shared client controller and five domain adapters. Default launch starts a live
session; build the server and provide its `.env` as described in
[development](development.md#qt-frontend). `--help` and invalid arguments are
handled before GUI initialization. `--component-gallery` remains standalone and
never creates a runtime, session, audio device or database. Playback and seeking
use the shared controller; filter chips remain unavailable. Playlists use the
shared workflow described above. Qt folder additions import their source when
Apply succeeds.
The engine selects Basic before loading QML, matching the explicit Basic imports
and keeping fallback file-dialog controls independent of KDE Breeze overrides.
The offline `native-assets` [probe](../../apps/osu-radio-qt/tests/VisualProbe.qml)
loads every bundled SVG/PNG/TTF through Qt, checks image readiness/dimensions and
the Poppins/Nunito font families. The
[tool tests](../../apps/osu-radio-qt/tests/test_tools.py) compare its manifest with
the asset directory so added resources require a decode assertion. This checks
resource packaging and decoding, not desktop/GPU appearance.

The offline `controls`, `controls-popups`, `controls-window` and `controls-panels`
[adapter probes](../../apps/osu-radio-qt/tests/AdapterProbe.qml) instantiate the
production components in the gallery window. They assert keyboard/pointer
activation, disabled input suppression, Unicode field clearing, tab/filter
selection, menu value roles and navigation, modal dismissal rules and restored
focus. Composed checks cover blank/preparing/busy editor gates, marker/difficulty
IDs, empty/error queue states and delegate cleanup. The folder-row width binding
tolerates its parent disappearing during dialog teardown; the new scenario
rejects the QML diagnostic that previously occurred after a successful close.
A window double checks tab/queue/window calls and all resize-edge flags.
The probes also check Theme font loading, artwork clearing, material coordinate
updates and Store payload conversion. These are Qt software input/property
checks; blur, masking and appearance still need desktop/GPU acceptance. Native
[artwork checks](../../apps/osu-radio-qt/src/native_tests.h) separately assert
cache keys/epochs and image read/format/decode/size failures without opening an
audio device or application database.

Each domain adapter exposes typed properties. Library and playlist contents use
separate instances of the same `TrackListModel` with
`audioId`, `title`, `artist`, `subtitle`, `durationLabel`, `artworkUrl`,
`playlistItemId` and `available` roles.
Media arrival changes individual rows; selection uses database/item IDs, with `-1`
representing no selected ID at the QML boundary. The small folder collection uses
typed `{id, label}` entries. Asynchronous controller updates are queued to the
QObject thread and safely rejected after object destruction. Qt image URLs and
decoded QImages remain GUI-owned. Provider images use `cache: false` in QML so
Qt does not add an unbounded cache above the application's bounded cache.
Qt restores cached URLs when replacing rows and skips resetting identical rows,
including their playlist item IDs. Model replacement preserves the player and
background URL until the final selection arrives. The cache protects that
selection's cover from eviction. Eviction leaves already displayed card URLs
intact; re-entering rows check cache membership and reload as needed. Provider
URLs include an installation version so a reloaded cover updates QML after
eviction or a previous provider error. A resident immutable key accepts earlier
installation URLs in the same cache epoch; stale epochs and missing keys fail.
Duplicate playlist-cover deliveries reuse a resident immutable key. The adapter
stores new playlist state before exposing properties that can request artwork.
Playlist-summary delegates bind to collection indices with empty fallback rows
while shrinking. Queue delegates bind to TrackListModel roles; entry-ID reconciliation
updates media and order without resetting the list.

[`Main.qml`](../../apps/osu-radio-qt/qml/Main.qml) composes separate Songs, Playlists,
Settings and Player panels. Each panel uses its corresponding Qt VM; the shell
owns tabs and the persistent window composition.
It retains the 1024×640 minimum, 50px title bar, 480px sidebar, 90px cards and
adaptive player geometry. Artwork uses centered cropping. Library refresh remains
outside the scrolling list. Settings use folder IDs, wrapped options and a full
label tooltip. The **General** section holds the folder menu, `+`, and independent
**Use Unicode track titles** / **Use Unicode artist names** switches. Both default
to off and remain available without a server. The Qt adapter reads native
temporary `QSettings` once before connecting (`UserScope`, `NativeFormat`,
`osu-radio/osu-radio-qt`, with fallback settings disabled), using
`display/use_unicode_titles` and `display/use_unicode_artists`. Each toggle
displays the choice immediately, sends it to the shared Settings VM, and writes
only the changed key with `sync()` and a status check. Read failures use both
defaults; write failures retain session choices. General shows English errors.
Reconnect retains current choices; the offline gallery does not read or save them.
Linux persistence, keyboard activation, rapid toggles, and read/write errors are
covered by the isolated `XDG_CONFIG_HOME`
[offscreen probe](../../apps/osu-radio-qt/tests/launch_smoke.rs); Windows native
storage still requires separate validation. **Audio**
holds the individual-volume switch, conditional global slider, status and Retry.
The Settings panel uses the app font (Poppins), 20px side margins, a search icon
on the right and 44px dark fields with 8px corners. Sections appear in the order
General, Audio, Integrations, About; each has a 24px icon with an 18px bold title
and its rows sit in a bordered 12px-radius card. Hints under osu! folders and the
two Audio switches are not search keys.
Settings search trims surrounding whitespace and matches section titles and
current setting names by case-insensitive substring. A section-title match shows
all its currently available settings; a setting-name match shows only matching
rows under their section header. Folder/audio status and Retry stay with the
matching setting's section. Search does not match folder paths, status messages,
or unavailable preferences, and never reveals the conditional global slider while
individual volume is off. Hidden rows/sections leave no layout gaps; changing the
query resets vertical scrolling to the top. A query with no matches shows
`No settings found.` with guidance to change or clear the search. This is local
presentation state and sends no library search command. The `settings-search`
case in [`AdapterProbe.qml`](../../apps/osu-radio-qt/tests/AdapterProbe.qml) covers
keyboard editing/clearing, title/name matches, conditional rows, empty states,
scroll extent/reset and retained state across navigation.
**About** uses the same Settings layout and search, with **Version**, **License**
(`MIT`) and **Project** (`https://github.com/Jayko321/osu-radio`). Matching About
shows all three rows; matching a field name shows its row. Values and the URL
are not search keys. The version binds to `Qt.application.version`:
[`main.rs`](../../apps/osu-radio-qt/src/main.rs) sets it from `CARGO_PKG_VERSION`
through the existing CXX-Qt `QGuiApplication::set_application_version` before
loading QML. Both Qt and server manifests use `0.1.0-alpha`. No VM, protocol or
QSettings key/location changes are involved; the gallery still uses no native
preference storage or backend. The probe checks exact About values and the live
metadata binding. [Release T06](../impl/arch-linux-release/T06.md) additionally
verified installed About/search/values on real KDE Wayland and the Project
action's correct native OpenURI request. Actual browser rendering and physical
keyboard navigation remain unverified; native AT-SPI actions are distinct from
keyboard input. That partial check also verified real lazer import/Play, an
active stereo stream to Xonar, persisted Unicode preferences, default/absolute
XDG paths and library/settings retention across package upgrade/reinstall.
Audible output, stable and menu acceptance remain open; the native KDE picker
opened but its portal process crashed during AT-SPI interaction. See T06 for
the shortened real-library fixture, desktop audio setup and verification limits.
All panes stay constructed,
retaining search text, selection and settings while the right-hand player remains
visible. Output-device and other placeholder preferences remain deferred.
Add is disabled during connection and folder loading; it opens the
folder selection modal. Retry reloads a failed folder collection. Selection updates
the card, player and backdrop together.

[`Artwork.qml`](../../apps/osu-radio-qt/qml/components/Artwork.qml) owns the
centered crop and rounded mask shared by the player, Songs cards, playlist covers,
playlist track cards and inline cover editor. Playlist context menus use explicit
card/button coordinates, including Menu/Shift+F10, and restore their opener's
focus. They do not position keyboard menus at the desktop cursor.
The volume popup receives keyboard focus, closes on Escape and restores the
volume button; its opaque dark surface and explicit slider palette keep the
value and handle readable over artwork.

[`AppModal.qml`](../../apps/osu-radio-qt/qml/components/AppModal.qml) wraps Qt's native
modal Popup with a centered, window-constrained panel, reusable body/footer, close
control, Tab containment, Escape/backdrop dismissal and focus restoration. Both
application roots capture an explicit scene container that excludes the overlay.
ShaderEffectSource/MultiEffect blur that scene while the popup is visible; capture
and blur are disabled while closed. The gallery add-difficulties dialog uses the same modal.

[`FolderSelectionModal.qml`](../../apps/osu-radio-qt/qml/components/FolderSelectionModal.qml)
uses a 740x620 panel constrained to the window, a scrolling list and fixed Apply
footer. Rows show bundled Figma Stable/Lazer logos, paths, source badges and
individual beatmap/difficulty counts. Registered rows are green; plus/minus buttons
stage or undo additions/removals; registered rows also have an accessible rotate
button, “Refresh import”, to stage a source refresh. Removal and refresh replace each
other; pressing the same action again cancels it. Pending actions have outlines. Counts animate
Computing every 400 ms, include zero, and expose a separate count-error retry.
A staged add/remove/refresh also shows a status capsule beside the count (“Will be added”,
“Will be removed”, “Will be refreshed”, with check/minus/rotate-cw icons); the count text
shrinks first on narrow panels. There is no help link until a real address exists.

The toolkit-free [`SettingsViewModel`](../../crates/osu-radio-client/src/view_models/settings.rs)
seeds registered rows on each opening, starts Full backend discovery, merges candidates
incrementally by resolved marker and registered ID, and runs at most two preview
requests concurrently. Native directory selection starts scoped Known discovery
and stages a unique new installation; ambiguous results require individual toggles.
Choosing a directory never saves it. Closing aborts discovery and client preview
requests, discards pending actions, and invalidates late network/picker results.
Counts do not gate Apply.

Apply snapshots actions, imports additions and refreshes registered sources before deleting removals, and continues
sequentially after individual errors. Success clears each action immediately; failed
rows retain their actions/errors for retry. Successful changes refresh folders,
the current library query, the playlist list and any open playlist independently.
Playlist epochs immediately discard older list/detail results. A concurrent playlist
edit defers playlist reload until it completes, including failure; failed library
loading does not suppress that reload. Complete success closes the modal; partial
failure keeps it open. Editing and dismissal are disabled during Apply.
[Folder checks](../../crates/osu-radio-client/src/view_models/folders_tests.rs) and
[playlist checks](../../crates/osu-radio-client/src/view_models/playlists/tests.rs)
cover staging, Apply with library failure, stale responses and deferred refresh
after failed playlist edits. The Qt offscreen folder probe activates refresh with
Space through the live adapter and fixture API; desktop mouse input and Windows
validation remain separate.

[`WindowBar.qml`](../../apps/osu-radio-qt/qml/WindowBar.qml) uses Qt window APIs
for drag/resize/minimize/maximize/restore/close and observes actual window
visibility. Qt 6.8 is the minimum for QML system move/resize methods.
Inactive navigation tabs use the standard white text/icon foreground; selected
tabs use dark text/icons on the white background. Settings retains tab index 2
and the `settingsButton` object name; it is visible but disabled in the offline
gallery.
Qt mouse-wheel scrolling uses twice the platform's default line count in both
live and gallery modes; touch/drag scrolling retains its native behavior.

The opt-in client [`mock` module](../../crates/osu-radio-client/src/mock.rs),
[`MockBridge`](../../apps/osu-radio-qt/src/bridge.rs) and
[`Store.qml`](../../apps/osu-radio-qt/qml/Store.qml) serve the standalone gallery.
Its bounded JSON snapshot is not used for the production library. Shared
[`components`](../../apps/osu-radio-qt/qml/components) preserve native control
keyboard behavior, popup/modal focus handling and disabled demonstrations.
The menu's configurable value role uses folder IDs in Settings and original
indices in the gallery. Gallery playlist actions modify only in-memory mock data.
Its folder examples exercise registered/new rows, zero and failed counts,
Computing, staged actions and partial-success retry without filesystem or backend
access.

The [mock reducer tests](../../crates/osu-radio-client/src/mock.rs) assert staged
folder actions, partial-success retry, browse/reset and disabled-action isolation.
The [mock playlist tests](../../crates/osu-radio-client/src/mock/playlists.rs)
assert editor epochs, cover size/errors/cancellation, revision-specific lookup and
cleanup, serialized draft exclusion, sort/selection independence from saved order
and explicit offline playback errors. They use only memory and inert cover bytes.

[`build.rs`](../../apps/osu-radio-qt/build.rs) embeds QML and assets, converting
discovered paths to forward slashes for QML module entries and resource aliases
on Windows. Resource loading does not depend on the working directory.
The gallery and embedded-asset probes in
[`launch_smoke.rs`](../../apps/osu-radio-qt/tests/launch_smoke.rs) check component
resolution and image/font decoding, separately from native desktop interactions.

The [Arch kit](../../packaging/arch/README.md) installs Qt/server side by side in
`/usr/bin`, with `osu-radio` pointing to `osu-radio-qt` and a desktop entry
launching `/usr/bin/osu-radio` without terminal, icon or working-directory override.
Linux embedded startup skips dotenv and uses the server's XDG SQLite fallback;
standalone startup retains dotenv search as described in [backend](backend.md).
KDE menu/picker/audio acceptance remains separate from recipe validation.
Asset origins/licenses are in [`SOURCES.md`](../../apps/osu-radio-qt/assets/SOURCES.md).
Software-rendered integration probes verify state transitions, not GPU blur,
native picker interaction, desktop layout or Windows behavior. See the manual
acceptance matrix in [development](development.md#qt-frontend).

## System media controls

Live Qt automatically connects local MPRIS on Linux or SMTC on Windows after the
main window exists. There is no toggle or backend selector. Both local-child and
explicit remote sessions use the client's local playback state. The offline
gallery never connects. See the approved [boundary](architecture.md#approved-system-media-boundary).

The [public client facade](../../crates/osu-radio-client/src/controller.rs) exposes
`connect_system_media`, `publish_system_media` and awaited `disconnect_system_media`.
Connecting starts work; only `Status::Connected` acknowledges native registration.
The [coordinator](../../crates/osu-radio-client/src/controller/system_media.rs)
receives ordered native events in its existing loop and publishes the latest
`Snapshot` without waiting for native I/O. The
[Player VM](../../crates/osu-radio-client/src/view_models/player.rs) builds metadata,
capabilities, status and position from the current assignment and matching worker,
using original optional title/artist fields with current Unicode preferences.
Loading/error/waiting does not borrow the previous worker's Playing state or
position. Selected B never changes the published A or redirects system commands.
Play uses the queue (`audio_source_id: None`); unavailable actions are ignored.
Position/duration stay in source time; `rate` is the sounding launch's applied speed
(see [Adjustable speed and pitch](#adjustable-speed-and-pitch)).

Seek events retain `LaunchKey { entry_id, playback_token }` from callback receipt.
Client and worker check the current launch; relative seek beyond the known end
uses the existing Next lane with `expected_token` rechecked before HTTP dispatch.
MPRIS also rejects mismatched TrackId in SetPosition. SMTC has no track identity
in its callback, so OS delivery after a switch is outside that protection.
MPRIS Volume uses existing Settings persistence/debounce, addressing current A in
individual mode even when B is selected. Windows mixer volume is independent of
the application's internal slider; SMTC never sends SetVolume.

The [shared media scheduler](../../crates/osu-radio-client/src/controller/media.rs)
adds current-cover demand even when A is absent from visible rows. One bounded
HTTP request can serve Qt and the system consumer. Qt still acknowledges its own
decode/install; a system-only result needs no Qt acknowledgement. Launch, cover
and generation reject stale bytes before publication. The
[artwork store](../../crates/osu-radio-media-controls/src/artwork.rs) owns the current
temporary encoded file. MPRIS publishes an escaped file URI; SMTC creates a native
stream. Changing identity, missing metadata or shutdown clears old fields/files.
An unavailable cover preserves transport and other metadata.

[MPRIS](../../crates/osu-radio-media-controls/src/linux/mod.rs) uses the existing
Tokio runtime and a unique `org.mpris.MediaPlayer2.osu_radio.instance…` name;
TrackList, playlists and OpenUri are not exported. `Shuffle` and `LoopStatus` (None/Track/Playlist) read the published snapshot and their setters queue `Event::SetShuffle/SetRepeat`, failing with `NotSupported` unless `capabilities.set_modes`; changes are announced in PropertiesChanged. Position ticks
do not emit PropertiesChanged; confirmed `seek_serial` changes emit Seeked.
[SMTC](../../crates/osu-radio-media-controls/src/windows/mod.rs) validates a live,
same-process top-level HWND and keeps all WinRT operations on an MTA worker.
Callbacks enqueue events; native I/O stays outside the controller loop. SMTC publishes
`ShuffleEnabled`/`AutoRepeatMode` with every snapshot and turns
`ShuffleEnabledChangeRequested`/`AutoRepeatModeChangeRequested` into the same events
(the Windows code is unbuilt on Linux and unverified).
The client maps `RepeatMode` ↔ `media_controls::Repeat` in one place (`models.rs`):
`system_snapshot()` takes modes from the last assignment and sets `set_modes` with the
other capabilities (ready and an assignment), and `system_event()` turns the events into
the button commands only when `set_modes` holds.

[Qt lifecycle](../../apps/osu-radio-qt/src/runtime.h) keeps the window alive while
[runtime detach](../../apps/osu-radio-qt/src/runtime.rs) awaits pending attach and
native shutdown. Close is retried after acknowledgement; event-loop exit and
failed launch pump Qt until detach completes before engine/window destruction.
Final controller shutdown repeats cleanup safely. Fatal native failure reports
`System media controls:`, disables only this integration until application restart
and does not reconnect automatically. GUI, HTTP and local audio continue.

[Automated/native evidence and remaining acceptance](development.md#system-media-controls-verification)
distinguish Windows HWND publication from a real panel, media keys and physical
audio. `playerctl shuffle/loop` and the SMTC widget are manual checks. Linux runtime execution remains unverified in the current Windows checkout.

## Queue-rework integration verification

The [T10 report](../impl/queue-rework/T10.md#отчёт-о-выполнении) records exact
commands and results. [launch_smoke](../../apps/osu-radio-qt/tests/launch_smoke.rs)
adds `real_backend_queue_survives_restart_and_preserves_exact_entry_actions`:
two Qt launches use the same private SQLite through a real embedded server,
exercise recovered pause/history, exact entry IDs above 2^53, independent
duplicates, move/remove, search Next, whole-playlist append and both clears.
The fixture verifies retained library/playlists/dates/volume after shutdown.

The [companion client test](../../crates/osu-radio-client/src/playback/tests/real_backend.rs)
uses the existing private fake engine with real HTTP downloads, imports and checks.
It tests seek/engine continuity, accepted Started, retained launch source on
reimport, late callbacks, detached playlist snapshots, restart without an engine
and silent append/source restoration. It drives the worker core directly;
controller/NDJSON sequencing remains covered by the existing VM/route tests.
Qt playing through a physical device and native Linux/Windows drag, focus, hit
testing and scaling remain separate user acceptance.

Shuffle/repeat add to both probes without new infrastructure. The client
`real_server_modes_reorder_only_the_future_repeat_and_survive_restart` follows six
entries through a real server and the fake engine: shuffle keeps history, current and
token and reorders only the rest; Next follows the new order; turning it off restores
the original order; Repeat One restarts the same entry with a new token and the
listening date stays unchanged; Repeat All wraps from the last entry on Finished and
Next; modes survive a server restart and Clear. The Qt `queue-real` case clicks the
real buttons on an empty queue, and the fixture's `verify` step checks that a new
server still reports shuffle on and repeat `all`. The 3-second Previous rule is not in
these probes: it lives in the Player VM, which this worker-core harness does not drive,
and is covered by the controller tests with a fake worker.
