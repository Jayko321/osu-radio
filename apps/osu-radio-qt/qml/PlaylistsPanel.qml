pragma ComponentBehavior: Bound
import QtQuick
import OsuRadio 1.0
import "components"

PlaylistPane {
    required property AppBridge bridge
    required property var shell
    id: playlistsPane
    objectName: "playlistsPane"
    anchors.fill: parent
    playlists: playlistsPane.bridge.playlists.filteredPlaylists
    activeId: playlistsPane.bridge.playlists.activePlaylistId
    activeName: playlistsPane.bridge.playlists.activePlaylistName
    query: playlistsPane.bridge.playlists.playlistQuery
    connected: playlistsPane.bridge.connected
    loading: playlistsPane.bridge.playlists.playlistLoading
    busy: playlistsPane.bridge.playlists.playlistBusy
    queueBusy: playlistsPane.bridge.queue.queueBusy
    coverPreparing: playlistsPane.bridge.playlists.playlistCoverPreparing
    message: playlistsPane.bridge.playlists.playlistMessage
    editorOpen: playlistsPane.bridge.playlists.playlistEditorOpen
    editorId: playlistsPane.bridge.playlists.playlistEditorId
    editorName: playlistsPane.bridge.playlists.playlistEditorName
    editorArtworkUrl: String(playlistsPane.bridge.playlists.playlistEditorArtworkUrl)
    tracksModel: playlistsPane.bridge.playlists.tracks
    trackCount: playlistsPane.bridge.playlists.tracks.trackCount
    sortIndex: playlistsPane.bridge.playlists.trackSortIndex
    onSearchEdited: query => playlistsPane.bridge.playlists.searchPlaylists(query)
    onCreateRequested: playlistsPane.bridge.playlists.beginPlaylistCreate()
    onEditRequested: id => playlistsPane.bridge.playlists.beginPlaylistEdit(id)
    onSelectRequested: id => playlistsPane.bridge.playlists.selectPlaylist(id)
    onQueueRequested: id => { playlistsPane.shell.expectQueueToast("Playlist added to queue"); playlistsPane.bridge.playlists.addPlaylistToQueue(id); }
    onPlayRequested: id => playlistsPane.bridge.playlists.playPlaylistFrom(id)
    onDeleteRequested: id => playlistsPane.shell.deletePlaylist(id)
    onRequestArtwork: id => playlistsPane.bridge.playlists.requestPlaylistArtwork(id)
    onNameEdited: name => playlistsPane.bridge.playlists.setPlaylistEditorName(name)
    onCoverRequested: playlistsPane.bridge.playlists.choosePlaylistCover()
    onCoverResetRequested: playlistsPane.bridge.playlists.resetPlaylistCover()
    onSaveRequested: playlistsPane.bridge.playlists.savePlaylistEditor()
    onCancelRequested: playlistsPane.bridge.playlists.cancelPlaylistEditor()
    onRefreshRequested: playlistsPane.bridge.playlists.refreshPlaylists()
    onSortChosen: index => playlistsPane.bridge.playlists.setTrackSort(index)
    trackDelegate: PlaylistTrackCard {
        id: playlistTrack
        readonly property bool inViewport: playlistsPane.visible && y + height >= playlistsPane.tracksList.contentY
            && y <= playlistsPane.tracksList.contentY + playlistsPane.tracksList.height
        onAudioIdChanged: playlistsPane.shell.scheduleVisibleMedia()
        onInViewportChanged: playlistsPane.shell.scheduleVisibleMedia()
        onArtworkUrlChanged: if (inViewport && artworkUrl.length === 0) playlistsPane.shell.scheduleVisibleMedia()
        Component.onCompleted: playlistsPane.shell.addMediaDelegate(playlistTrack)
        Component.onDestruction: playlistsPane.shell.removeMediaDelegate(playlistTrack)
        width: playlistsPane.tracksList.width
        selected: playlistsPane.bridge.playlists.selectedPlaylistItemId === playlistItemId
        busy: playlistsPane.bridge.playlists.playlistBusy
        canInsert: playlistsPane.bridge.connected && !playlistsPane.bridge.queue.queueBusy
        canPlay: playlistsPane.bridge.connected && playlistsPane.bridge.settings.audioSettingsLoaded
        canAddToPlaylist: playlistsPane.bridge.connected
        onInsertRequested: next => {
            playlistsPane.shell.expectQueueToast(next ? "Will play next" : "Added to end of queue");
            playlistsPane.bridge.playlists.insertPlaylistItem(playlistItemId, next);
        }
        onPlayRequested: {
            const player = playlistsPane.bridge.player;
            const selected = playlistsPane.bridge.playlists.selectedPlaylistItemId === playlistItemId;
            playlistsPane.bridge.playlists.selectPlaylistItem(playlistItemId);
            // Open question: a non-selected row that is already the playing item would be paused here.
            if (!selected || !player.selectedIsPlaying)
                player.togglePlayback();
        }
        onAddToPlaylistRequested: {
            playlistsPane.bridge.playlists.selectPlaylistItem(playlistItemId);
            playlistsPane.bridge.playlists.openPlaylistAdd();
        }
        onClicked: playlistsPane.bridge.playlists.selectPlaylistItem(playlistItemId)
        onRemoveRequested: itemId => playlistsPane.bridge.playlists.removePlaylistItem(itemId)
    }
}
