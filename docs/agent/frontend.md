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
Qt's title-bar layers button, immediately before the window controls, opens
[`QueuePanel.qml`](../../apps/osu-radio-qt/qml/components/QueuePanel.qml) over the
player with a dark gradient and rounded artwork cards showing title, artist and
duration. The virtualized `ListView` includes history, current and upcoming rows,
checking/unavailable status and retained unavailable captions. Cards are 82px with
14px spacing, a 96px cache buffer and no delegate reuse pool. Opening positions
once on current (or first upcoming/end of history) after loading, without selecting
or playing. Later role changes preserve scrolling and decoded artwork URLs.
Close, Escape and outside click dismiss it and restore opener focus.

The queue adapter exposes `tracks`, `startIndex`, `queueLoading`, `queueBusy`, `queueMessage`,
`setQueueVisible(bool)`, `playEntry(QString)`, `removeEntry(QString)`,
`moveEntry(QString, QVariant)`, `clearUpcoming()`, `clearAll()` and
`setVisibleQueueMedia(QVariantList)`. Entry IDs and anchors are decimal strings,
including values above JS integer precision; a null anchor means end. Rust rejects
invalid IDs and non-string anchors before dispatching existing QueueAction.
Existing roles 256–263 stay unchanged; queue adds 264 `entryId` (string),
265 `isCurrent`, 266 `isPast` and 267 `availability` (checking/available/unavailable).
`entryIdAt(index)` resolves keyboard/drop anchors. Queue reconciliation uses
entry identity and standard row insert/remove/move notifications; role updates
use `dataChanged`. Retained artwork is keyed by entry identity and cover source.
Library and playlist instances retain their existing behavior.

Play (including checking), remove, both clears and Ctrl+Up/Down dispatch existing
client operations. The separate drag handle shows an insertion line and scrolls
at viewport edges; only drop sends one ID/anchor command. Server confirmation
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
Qt's [progress slider](../../apps/osu-radio-qt/qml/PlayerPanel.qml) fills the elapsed
portion with `Theme.accent`, following its position during playback and seeking.
Shutdown cancels the event stream and downloads, joins the worker, releases its
files, then ends the backend session.

Coverage lives in [worker tests](../../crates/osu-radio-client/src/playback.rs),
[queue VM tests](../../crates/osu-radio-client/src/view_models/queue/tests.rs),
[stream tests](../../crates/osu-radio-client/src/api/playback_tests.rs), and the
[Qt probe](../../apps/osu-radio-qt/tests/AdapterProbe.qml). Galleries remain offline.
Shuffle, repeat and output-device selection remain deferred.
Physical output, desktop slider interaction and Windows require separate manual
checks.

The real worker-thread test uses a fake engine to assert download generations,
stale-file deletion, ordered Started/Finished feedback, periodic Playing updates,
source replacement, joined shutdown and temporary-file cleanup. Core tests also
reject nonfinite/out-of-range assignment volume without replacing the current
source, mode, token or volume. These checks open no audio device.

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
later enable/reconnect starts from the newest measured position.
Playing uses Listening (2), title/details and artist/state. Unicode preferences
apply to current-track metadata. Text uses existing fallbacks and truncates safely
to 128 UTF-8 bytes. Paused retains title plus a static `Paused • mm:ss • artist`
label with no timestamps; resume/seek rebuild start/end timestamps. Discord
controls progress rendering and the app name; unknown/zero duration omits timers,
known duration uses second-resolution timestamps. No paths or database IDs are
sent in Activity.

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
logged to stderr without dumping payloads. No track covers or external uploads
are required. Linux scans
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

## User playlists

Adding a new osu! source imports Stable/Lazer collections into this same tab,
including empty collections and unavailable difficulties. “Обновить импорт” in
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
shows each concrete difficulty separately with background artwork and a white
selected border with blue glow. Click selects only; existing Play launches the
selected item. Unavailable entries stay selectable/removable with Play disabled.
Right click, Menu or Shift+F10 opens removal. Display sorting retains selected
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
Playlist removal remains a distinct action in the same menu.

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

`Session::start` starts `EmbeddedServer`, then creates an `ApiClient` at the reported URL. `Session` exposes `api`, `base_url`, and asynchronous `shutdown`. It does not itself perform an HTTP readiness request. After `Connected`, the GUI loads library and folders independently, each with loading, empty and error states. Songs have a “Refresh library” button in the bottom-left footer, outside the scrolling list; it also retries failures and is disabled while loading. Status messages remain above the list and are hidden when empty. Settings show Retry only after a failure.

The current [supervisor](../../crates/osu-radio-client/src/server.rs) has these behaviors:

- `ServerOptions` accepts `binary`, `working_directory`, `address`, and `startup_timeout`. Defaults are no binary/directory override, `127.0.0.1:0`, and 30 seconds. The address is configurable; the GUI uses those defaults, letting the OS select a port.
- Binary resolution checks the explicit option, `OSU_RADIO_SERVER_BIN`, a sibling of the current executable, then the executable name on `PATH`. The filename uses the platform's executable suffix. Build the server before launching the GUI; its Cargo dependency on the client does not build the server executable.
- The child receives `OSU_RADIO_SERVER_ADDRESS` from the options. It inherits the working directory unless overridden. Server configuration accepts a missing `.env` when the process environment supplies the database URL; existing malformed/unreadable files remain errors. See [backend configuration](backend.md) and the [server config source](../../apps/osu-radio-server/src/config.rs).
- Stdin is closed. Stdout and stderr are piped and drained, with `[server]` prefixes. Stderr is forwarded while stdout is inspected for readiness; stdout continues forwarding afterward. Leaving either pipe unread can block the child.
- Readiness depends on a stdout line containing `listening on ` followed by an `http://` or `https://` URL. The server prints its bound address, so the URL contains the actual port rather than `0`. This is a protocol between [server `main.rs`](../../apps/osu-radio-server/src/main.rs) and `parse_ready_line`, not optional diagnostic wording.
- Early stdout EOF waits for child exit within the startup deadline; a still-running child is killed and reaped on timeout. Read failure, early exit and timeout have separate errors. Cancellation during startup also awaits termination. Post-start stdout/stderr read loops stop on read errors; there is no automatic restart or ongoing health monitor.
- `shutdown` takes the child once and awaits its termination; repeated shutdown is harmless. `kill_on_drop(true)` and `EmbeddedServer::drop` provide fallback termination, but `Drop` does not await it. A hard kill or crash of the parent cannot be assumed to run destructors or terminate the child.

Readiness parsing has focused unit tests in [server.rs](../../crates/osu-radio-client/src/server.rs). The [ignored embedded-server integration test](../../crates/osu-radio-client/tests/embedded_server.rs) starts a real server and performs database-backed requests; it uses a disposable SQLite database and refuses configured database environment variables. Its prerequisites and scope are recorded in [development](development.md).

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
The Settings panel uses the bundled Nunito font, 20px side margins, a search icon
on the right, 24px section icons and 44px dark fields with 8px corners.
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
button, “Обновить импорт”, to stage a source refresh. Removal and refresh replace each
other; pressing the same action again cancels it. Pending actions have outlines. Counts animate
Computing every 400 ms, include zero, and expose a separate count-error retry.
Need help opens https://www.google.com through Qt.openUrlExternally.

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

[`build.rs`](../../apps/osu-radio-qt/build.rs) embeds QML and assets; resource
loading does not depend on the working directory. Server configuration still does.
Asset origins/licenses are in [`SOURCES.md`](../../apps/osu-radio-qt/assets/SOURCES.md).
Software-rendered integration probes verify state transitions, not GPU blur,
native picker interaction, desktop layout or Windows behavior. See the manual
acceptance matrix in [development](development.md#qt-frontend).

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
