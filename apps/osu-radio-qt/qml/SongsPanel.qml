pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import OsuRadio 1.0
import "components"

Item {
    required property AppBridge bridge
    required property var shell
    id: songsPane
    objectName: "songsPane"
    readonly property alias searchField: search
    anchors.fill: parent
    AppField {
        id: search
        objectName: "songSearch"
        x: 20
        y: 32
        width: parent.width - 40
        leftPadding: 44
        clearable: true
        placeholderText: "Type to search songs..."
        Accessible.name: "Search songs"
        onTextChanged: songsPane.bridge.library.searchLibrary(text)
        AppIcon { x: 12; anchors.verticalCenter: parent.verticalCenter; name: "search"; color: Theme.muted }
    }
    Row {
        id: filters
        x: 20
        y: search.y + search.height + 16
        spacing: 10
        AppMenu {
            objectName: "trackSortMenu"
            width: 172
            height: 32
            entries: [
                {index: 0, label: "Title A–Z"},
                {index: 1, label: "Artist A–Z"},
                {index: 2, label: "Recently played"}
            ]
            currentIndex: songsPane.bridge.library.trackSortIndex
            onChosen: index => songsPane.bridge.library.setTrackSort(index)
            Accessible.name: "Sort tracks"
            background: Rectangle { radius: 16; color: "transparent"; border.color: Theme.border }
        }
    }
    Text {
        id: libraryStatus
        objectName: "libraryStatus"
        x: 20
        y: filters.y + filters.height + 20
        width: parent.width - 40
        text: songsPane.bridge.library.libraryMessage
        visible: text.length > 0 && !songsPane.showEmpty
        color: Theme.text
        font.family: Theme.fontFamily
        font.pixelSize: 14
        wrapMode: Text.Wrap
    }
    // The client reports an empty result only through these two texts; any other text is an error,
    // which stays in libraryStatus. Revisit if the client texts change.
    readonly property bool searchEmpty: songsPane.bridge.library.libraryMessage === "Nothing found."
    readonly property bool showEmpty: songList.count === 0 && !songsPane.bridge.library.libraryLoading
        && songsPane.bridge.connected
        && (searchEmpty || String(songsPane.bridge.library.libraryMessage).startsWith("No songs yet"))
    readonly property bool showSkeleton: songList.count === 0 && songsPane.bridge.library.libraryLoading
    Column {
        objectName: "songSkeleton"
        x: songList.x
        y: songList.y
        width: songList.width
        spacing: songList.spacing
        visible: songsPane.showSkeleton
        SequentialAnimation on opacity {
            running: songsPane.showSkeleton && Theme.motion
            loops: Animation.Infinite
            NumberAnimation { to: 0.45; duration: Theme.ms(600); easing.type: Easing.InOutSine }
            NumberAnimation { to: 1; duration: Theme.ms(600); easing.type: Easing.InOutSine }
        }
        Repeater {
            model: 4
            Rectangle { width: songList.width; height: 90; radius: 8; color: Theme.surface }
        }
    }
    Item {
        objectName: "songsEmptyState"
        x: songList.x
        y: songList.y
        width: songList.width
        height: songList.height
        visible: songsPane.showEmpty
        Column {
            anchors.centerIn: parent
            width: parent.width
            spacing: 10
            AppIcon {
                anchors.horizontalCenter: parent.horizontalCenter
                visible: songsPane.searchEmpty
                name: "search"
                size: 40
                color: Theme.muted
            }
            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: songsPane.searchEmpty ? "Nothing found" : "No songs yet"
                color: Theme.text
                font.family: Theme.fontFamily
                font.pixelSize: 16
                font.weight: Font.DemiBold
            }
            Text {
                visible: songsPane.searchEmpty
                width: parent.width
                text: "No songs match “" + search.text.trim() + "”."
                horizontalAlignment: Text.AlignHCenter
                elide: Text.ElideRight
                color: Theme.muted
                font.family: Theme.fontFamily
                font.pixelSize: 14
            }
            AppButton {
                anchors.horizontalCenter: parent.horizontalCenter
                objectName: songsPane.searchEmpty ? "songsClearSearch" : "songsAddFolder"
                text: songsPane.searchEmpty ? "Clear search" : "Add an osu! folder"
                onClicked: {
                    if (songsPane.searchEmpty) { search.clear(); search.forceActiveFocus(); }
                    else songsPane.shell.selectedTab = 2;
                }
            }
        }
    }
    ListView {
        id: songList
        objectName: "songList"
        x: 20
        y: libraryStatus.y + (libraryStatus.visible ? libraryStatus.height : 0) + 12
        width: parent.width - 40
        height: Math.max(0, libraryFooter.y - y - 12)
        clip: true
        spacing: 16
        model: songsPane.bridge.library.tracks
        currentIndex: -1
        boundsBehavior: Flickable.StopAtBounds
        Basic.ScrollBar.vertical: AppScrollBar {}
        delegate: Basic.Button {
            id: card
            required property int audioId
            required property int playlistItemId
            required property bool available
            required property string title
            required property string artist
            required property string subtitle
            required property string durationLabel
            required property string artworkUrl
            readonly property bool selectedTrack: songsPane.bridge.library.selectedAudioId === audioId
            readonly property bool inViewport: songsPane.visible
                && y + height >= songList.contentY
                && y <= songList.contentY + songList.height
            onAudioIdChanged: songsPane.shell.scheduleVisibleMedia()
            onInViewportChanged: songsPane.shell.scheduleVisibleMedia()
            onArtworkUrlChanged: if (inViewport && artworkUrl.length === 0) songsPane.shell.scheduleVisibleMedia()
            Component.onCompleted: songsPane.shell.addMediaDelegate(card)
            Component.onDestruction: songsPane.shell.removeMediaDelegate(card)
            width: songList.width
            height: 90
            padding: 0
            Accessible.name: title + ", " + artist
            onClicked: songsPane.bridge.library.selectTrack(audioId)
            Keys.onPressed: event => queueMenu.handleKey(event)
            TapHandler {
                acceptedButtons: Qt.RightButton
                onTapped: queueMenu.openAt(point.position)
            }
            TrackQueueMenu {
                id: queueMenu
                objectName: "songQueueMenu"
                row: card
                canInsert: songsPane.bridge.connected && !songsPane.bridge.queue.queueBusy
                    && !songsPane.bridge.playlists.playlistBusy
                canPlay: card.available && songsPane.bridge.connected && songsPane.bridge.settings.audioSettingsLoaded
                canAddToPlaylist: card.available && songsPane.bridge.connected && !songsPane.bridge.playlists.playlistBusy
                onInsertRequested: next => {
                    songsPane.shell.expectQueueToast(next ? "Will play next" : "Added to end of queue");
                    songsPane.bridge.library.insertTrack(card.audioId, next);
                }
                onPlayRequested: {
                    const player = songsPane.bridge.player;
                    const selected = songsPane.bridge.library.selectedAudioId === card.audioId;
                    songsPane.bridge.library.selectTrack(card.audioId);
                    // Commands run in order, so togglePlayback sees the new selection. A toggle on the
                    // playing current track would pause it, so only a non-current or paused one is toggled.
                    if (player.currentAudioId !== card.audioId || (selected && !player.selectedIsPlaying))
                        player.togglePlayback();
                }
                onAddToPlaylistRequested: {
                    songsPane.bridge.library.selectTrack(card.audioId);
                    songsPane.bridge.playlists.openPlaylistAdd();
                }
            }
            background: Item {
                Artwork { anchors.fill: parent; source: card.artworkUrl }
                Rectangle {
                    anchors.fill: parent
                    radius: 8
                    gradient: Gradient {
                        orientation: Gradient.Horizontal
                        GradientStop { position: 0.415; color: card.selectedTrack ? "#f0241c38" : "#eb0e0e0e" }
                        GradientStop { position: 1; color: "transparent" }
                    }
                }
                Rectangle {
                    anchors.fill: parent
                    radius: 8
                    color: "transparent"
                    border.width: card.activeFocus ? 2 : 1
                    border.color: card.selectedTrack ? Theme.accent
                        : (card.hovered || card.activeFocus ? Theme.border : Qt.alpha(Theme.border, 0))
                    Behavior on border.color { ColorAnimation { duration: Theme.ms(Theme.motionFast); easing.type: Theme.easing } }
                }
            }
            contentItem: Item {
                Text {
                    id: duration
                    x: parent.width - width - 18
                    y: 20
                    text: card.durationLabel
                    color: Theme.text
                    opacity: 0.8
                    font.family: Theme.fontFamily
                    font.pixelSize: 14
                    font.features: { "tnum": 1 }
                }
                Text {
                    x: 20
                    y: 17
                    width: parent.width - 58 - duration.width
                    height: 30
                    text: card.title
                    color: Theme.text
                    font.family: Theme.fontFamily
                    font.pixelSize: 22
                    font.weight: Font.Bold
                    elide: Text.ElideRight
                }
                Text {
                    x: 20
                    y: 49
                    width: parent.width - 58 - duration.width
                    height: 22
                    text: card.subtitle
                    color: Theme.text
                    font.family: Theme.fontFamily
                    font.pixelSize: 16
                    font.weight: Font.Medium
                    elide: Text.ElideRight
                }
            }
        }
    }
    AppButton {
        id: libraryFooter
        objectName: "refreshLibrary"
        x: 20
        y: parent.height - height - 20
        height: 36
        text: "Refresh library"
        iconName: "rotate-cw"
        enabled: songsPane.bridge.connected && !songsPane.bridge.library.libraryLoading
        onClicked: songsPane.bridge.library.refreshLibrary()
        Accessible.name: "Refresh library"
    }
}
