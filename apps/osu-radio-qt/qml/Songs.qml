pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import QtQuick.Effects
import QtQuick.Dialogs
import OsuRadio 1.0
import "components"

Basic.ApplicationWindow {
    id: root
    objectName: "songsWindow"
    width: 1440
    height: 952
    minimumWidth: 1024
    minimumHeight: 640
    visible: true
    title: selectedTab === 0 ? "osu! radio — Songs" : selectedTab === 1 ? "osu! radio — Playlists" : "osu! radio — Settings"
    color: Theme.background
    flags: Qt.Window | Qt.FramelessWindowHint
    font.family: Theme.fontFamily

    property int selectedTab: 0
    property var mediaDelegates: []
    function scheduleVisibleMedia() {
        if (!visibleMediaTimer.running) visibleMediaTimer.start();
    }
    function addMediaDelegate(row) {
        mediaDelegates.push(row);
        scheduleVisibleMedia();
    }
    function removeMediaDelegate(row) {
        const index = mediaDelegates.indexOf(row);
        if (index >= 0) mediaDelegates.splice(index, 1);
        scheduleVisibleMedia();
    }
    Timer {
        id: visibleMediaTimer
        interval: 0
        onTriggered: {
            const ids = [];
            for (const row of root.mediaDelegates) {
                if (row && row.inViewport && row.available && row.audioId >= 0) ids.push(row.audioId);
            }
            bridge.setVisibleMedia(ids);
        }
    }
    onSelectedTabChanged: {
        if (selectedTab === 0) bridge.showLibrary();
        else if (selectedTab === 1) bridge.showPlaylists();
        scheduleVisibleMedia();
    }
    readonly property alias appBridge: bridge
    AppBridge {
        id: bridge
        objectName: "appBridge"
        Component.onCompleted: connectSession()
        onSelectedArtworkUrlChanged: if (hasSelection && selectedArtworkUrl.length === 0) requestMedia(selectedAudioId)
        onMediaViewChanged: root.scheduleVisibleMedia()
    }


    Item {
        id: applicationScene
        anchors.fill: parent

        WindowBar {
            id: titleBar
            width: parent.width
            window: root
            selectedTab: root.selectedTab
            queueEnabled: bridge.connected
            queueVisible: queuePanel.visible
            onTabSelected: index => root.selectedTab = index
            onQueueRequested: queuePanel.visible ? queuePanel.close() : queuePanel.open()
        }

        Rectangle {
            id: connectionBanner
            anchors.top: titleBar.bottom
            width: parent.width
            height: visible ? Math.max(connectionText.implicitHeight + 24, 60) : 0
            visible: !bridge.connected
            color: Theme.background
            Text {
                id: connectionText
                objectName: "connectionStatus"
                x: 20
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - connectionRetry.width - 60
                text: bridge.connectionStatus
                color: Theme.text
                font.family: Theme.fontFamily
                font.pixelSize: 14
                wrapMode: Text.Wrap
            }
            AppButton {
                id: connectionRetry
                objectName: "retryConnection"
                anchors.right: parent.right
                anchors.rightMargin: 20
                anchors.verticalCenter: parent.verticalCenter
                text: "Retry"
                visible: !bridge.connecting
                enabled: !bridge.connecting
                onClicked: bridge.connectSession()
            }
        }

        Item {
            id: body
            anchors.top: connectionBanner.bottom
            anchors.bottom: parent.bottom
            width: parent.width
            clip: true

            Item {
                id: backdrop
                anchors.fill: parent
                Image {
                    id: backdropImage
                    anchors.fill: parent
                    source: bridge.selectedArtworkUrl
                    cache: false
                    fillMode: Image.PreserveAspectCrop
                    visible: false
                }
                MultiEffect {
                    anchors.fill: parent
                    source: backdropImage
                    blurEnabled: true
                    blurMax: 20
                    blur: 1.0
                    opacity: 0.06
                }
                Canvas {
                    anchors.fill: parent
                    onWidthChanged: requestPaint()
                    onHeightChanged: requestPaint()
                    onPaint: {
                        const ctx = getContext("2d");
                        ctx.reset();
                        const warm = ctx.createRadialGradient(width * 0.3 + 500, height * -0.1 + 500,
                            0, width * 0.3 + 500, height * -0.1 + 500, 500);
                        warm.addColorStop(0, "rgba(154,39,41,0.5)");
                        warm.addColorStop(0.31, "rgba(154,39,41,0.34)");
                        warm.addColorStop(0.58, "rgba(154,39,41,0.16)");
                        warm.addColorStop(1, "rgba(154,39,41,0)");
                        ctx.fillStyle = warm;
                        ctx.fillRect(0, 0, width, height);
                        const cool = ctx.createRadialGradient(width * -0.18 + 600, height * 0.4 + 450,
                            0, width * -0.18 + 600, height * 0.4 + 450, 600);
                        cool.addColorStop(0, "rgba(75,15,48,0.52)");
                        cool.addColorStop(0.31, "rgba(75,15,48,0.35)");
                        cool.addColorStop(0.58, "rgba(75,15,48,0.17)");
                        cool.addColorStop(1, "rgba(75,15,48,0)");
                        ctx.fillStyle = cool;
                        ctx.fillRect(0, 0, width, height);
                    }
                }
            }

            MaterialSurface {
                id: sidebar
                width: 480
                height: parent.height
                kind: "regular"
                sourceItem: backdrop
                radius: 0
                Rectangle { anchors.fill: parent; color: "transparent"; border.color: Theme.border }
                Item {
                    id: songsPane
                    objectName: "songsPane"
                    anchors.fill: parent
                    visible: root.selectedTab === 0
                    AppField {
                        id: search
                        objectName: "songSearch"
                        x: 20
                        y: 32
                        width: parent.width - 40
                        leftPadding: 44
                        placeholderText: "Type to search songs..."
                        Accessible.name: "Search songs"
                        onTextChanged: bridge.searchLibrary(text)
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
                            currentIndex: bridge.trackSortIndex
                            onChosen: index => bridge.setTrackSort(index)
                            Accessible.name: "Sort tracks"
                            background: Rectangle { radius: 16; color: "transparent"; border.color: Theme.border }
                        }
                        Repeater {
                            model: ["All musics", "Tags"]
                            AppButton {
                                required property string modelData
                                height: 32
                                text: modelData
                                iconName: "chevron-down"
                                enabled: false
                                background: Rectangle { radius: 16; color: "transparent"; border.color: Theme.border }
                                Accessible.name: modelData + " (unavailable)"
                            }
                        }
                    }
                    Text {
                        id: libraryStatus
                        objectName: "libraryStatus"
                        x: 20
                        y: filters.y + filters.height + 20
                        width: parent.width - 40
                        text: bridge.libraryMessage
                        visible: text.length > 0
                        color: Theme.text
                        font.family: Theme.fontFamily
                        font.pixelSize: 14
                        wrapMode: Text.Wrap
                    }
                    ListView {
                        id: songList
                        objectName: "songList"
                        x: 20
                        y: libraryStatus.y + (libraryStatus.visible ? libraryStatus.height : 0) + 12
                        width: parent.width - 40
                        height: Math.max(0, footer.y - y - 12)
                        clip: true
                        spacing: 16
                        model: bridge
                        currentIndex: -1
                        boundsBehavior: Flickable.StopAtBounds
                        Basic.ScrollBar.vertical: Basic.ScrollBar { }
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
                            readonly property bool inViewport: songsPane.visible
                                && y + height >= songList.contentY
                                && y <= songList.contentY + songList.height
                            onAudioIdChanged: root.scheduleVisibleMedia()
                            onInViewportChanged: root.scheduleVisibleMedia()
                            onArtworkUrlChanged: if (inViewport && artworkUrl.length === 0) root.scheduleVisibleMedia()
                            Component.onCompleted: root.addMediaDelegate(card)
                            Component.onDestruction: root.removeMediaDelegate(card)
                            width: songList.width
                            height: 90
                            padding: 0
                            Accessible.name: title + ", " + artist
                            onClicked: bridge.selectTrack(audioId)
                            background: Item {
                                Artwork { anchors.fill: parent; source: card.artworkUrl }
                                Rectangle {
                                    anchors.fill: parent
                                    radius: 8
                                    gradient: Gradient {
                                        orientation: Gradient.Horizontal
                                        GradientStop { position: 0.415; color: "#eb0e0e0e" }
                                        GradientStop { position: 1; color: "transparent" }
                                    }
                                }
                                Rectangle {
                                    anchors.fill: parent
                                    radius: 8
                                    color: "transparent"
                                    border.width: card.activeFocus ? 2 : 1
                                    border.color: bridge.selectedAudioId === card.audioId ? Theme.accent
                                        : (card.hovered || card.activeFocus ? Theme.border : "transparent")
                                }
                            }
                            contentItem: Item {
                                Text {
                                    x: 20
                                    y: 17
                                    width: parent.width - 40
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
                                    width: parent.width - 40
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
                        id: footer
                        objectName: "refreshLibrary"
                        x: 20
                        y: parent.height - height - 20
                        height: 36
                        text: "Refresh library"
                        iconName: "rotate-cw"
                        enabled: bridge.connected && !bridge.libraryLoading
                        onClicked: bridge.refreshLibrary()
                        Accessible.name: "Refresh library"
                    }
                }

                PlaylistPane {
                    id: playlistsPane
                    objectName: "playlistsPane"
                    anchors.fill: parent
                    visible: root.selectedTab === 1
                    playlists: bridge.filteredPlaylists
                    activeId: bridge.activePlaylistId
                    activeName: bridge.activePlaylistName
                    query: bridge.playlistQuery
                    connected: bridge.connected
                    loading: bridge.playlistLoading
                    busy: bridge.playlistBusy
                    coverPreparing: bridge.playlistCoverPreparing
                    message: bridge.playlistMessage
                    editorOpen: bridge.playlistEditorOpen
                    editorId: bridge.playlistEditorId
                    editorName: bridge.playlistEditorName
                    editorArtworkUrl: bridge.playlistEditorArtworkUrl
                    tracksModel: bridge
                    trackCount: bridge.trackCount
                    sortIndex: bridge.trackSortIndex
                    onSearchEdited: query => bridge.searchPlaylists(query)
                    onCreateRequested: bridge.beginPlaylistCreate()
                    onEditRequested: id => bridge.beginPlaylistEdit(id)
                    onSelectRequested: id => bridge.selectPlaylist(id)
                    onQueueRequested: id => bridge.addPlaylistToQueue(id)
                    onDeleteRequested: id => bridge.deletePlaylist(id)
                    onRequestArtwork: id => bridge.requestPlaylistArtwork(id)
                    onNameEdited: name => bridge.setPlaylistEditorName(name)
                    onCoverRequested: bridge.choosePlaylistCover()
                    onCoverResetRequested: bridge.resetPlaylistCover()
                    onSaveRequested: bridge.savePlaylistEditor()
                    onCancelRequested: bridge.cancelPlaylistEditor()
                    onRefreshRequested: bridge.refreshPlaylists()
                    onSortChosen: index => bridge.setTrackSort(index)
                    trackDelegate: PlaylistTrackCard {
                        id: playlistTrack
                        readonly property bool inViewport: playlistsPane.visible && y + height >= playlistsPane.tracksList.contentY
                            && y <= playlistsPane.tracksList.contentY + playlistsPane.tracksList.height
                        onAudioIdChanged: root.scheduleVisibleMedia()
                        onInViewportChanged: root.scheduleVisibleMedia()
                        onArtworkUrlChanged: if (inViewport && artworkUrl.length === 0) root.scheduleVisibleMedia()
                        Component.onCompleted: root.addMediaDelegate(playlistTrack)
                        Component.onDestruction: root.removeMediaDelegate(playlistTrack)
                        width: playlistsPane.tracksList.width
                        selected: bridge.selectedPlaylistItemId === playlistItemId
                        busy: bridge.playlistBusy
                        onClicked: bridge.selectPlaylistItem(playlistItemId)
                        onRemoveRequested: itemId => bridge.removePlaylistItem(itemId)
                    }
                }

                Item {
                    id: settingsPane
                    objectName: "settingsPane"
                    readonly property string fontFamily: Theme.fallbackFont.name || "Nunito"
                    readonly property string query: settingsSearch.text.trim().toLowerCase()
                    readonly property bool foldersMatch: matches(generalSettingsHeader.text, foldersLabel.text)
                    readonly property bool individualVolumeMatches: matches(audioSettingsHeader.text, individualVolumeSwitch.text)
                    readonly property bool globalVolumeMatches: bridge.individualVolumeEnabled && matches(audioSettingsHeader.text, globalVolumeLabel.settingName)
                    readonly property bool hasMatches: foldersMatch || individualVolumeMatches || globalVolumeMatches
                    function matches(section: string, setting: string): bool {
                        return query.length === 0 || section.toLowerCase().includes(query) || setting.toLowerCase().includes(query);
                    }
                    // A narrower result must remain reachable after scrolling a long status.
                    onQueryChanged: Qt.callLater(() => { settingsScroll.contentItem.contentY = 0; })
                    anchors.fill: parent
                    visible: root.selectedTab === 2
                    AppField {
                        id: settingsSearch
                        objectName: "settingsSearch"
                        x: 20
                        y: 22
                        width: parent.width - 40
                        height: 44
                        leftPadding: 16
                        rightPadding: 44
                        font.family: settingsPane.fontFamily
                        placeholderText: "Type to search settings..."
                        Accessible.name: "Search settings"
                        background: Rectangle {
                            radius: 8
                            color: Theme.background
                            border.color: settingsSearch.activeFocus ? Theme.text : settingsSearch.hovered ? Theme.muted : Theme.border
                            border.width: settingsSearch.activeFocus ? 2 : 1
                        }
                        AppIcon {
                            objectName: "settingsSearchIcon"
                            anchors.right: parent.right
                            anchors.rightMargin: 16
                            anchors.verticalCenter: parent.verticalCenter
                            name: "search-line"
                            color: Theme.muted
                        }
                    }
                    Basic.ScrollView {
                        id: settingsScroll
                        objectName: "settingsScroll"
                        x: 20
                        y: settingsSearch.y + settingsSearch.height + 30
                        width: parent.width - 40
                        height: parent.height - y - 20
                        contentWidth: availableWidth
                        contentHeight: settingsContent.implicitHeight
                        clip: true
                        Basic.ScrollBar.horizontal.policy: Basic.ScrollBar.AlwaysOff
                        Column {
                            id: settingsContent
                            objectName: "settingsContent"
                            width: settingsScroll.availableWidth
                            spacing: 40
                            Column {
                                objectName: "generalSettingsSection"
                                visible: settingsPane.foldersMatch
                                width: parent.width
                                spacing: 24
                                Row {
                                    height: 24
                                    spacing: 12
                                    AppIcon { name: "pencil-line" }
                                    Text {
                                        id: generalSettingsHeader
                                        objectName: "generalSettingsHeader"
                                        text: "General"
                                        color: Theme.text
                                        font.family: settingsPane.fontFamily
                                        font.pixelSize: 24
                                        font.weight: Font.Bold
                                        height: parent.height
                                        verticalAlignment: Text.AlignVCenter
                                    }
                                }
                                Column {
                                    width: parent.width
                                    spacing: 8
                                    Text {
                                        id: foldersLabel
                                        text: "osu! folders"
                                        color: Theme.text
                                        font.family: settingsPane.fontFamily
                                        font.pixelSize: 16
                                        height: 20
                                    }
                                    Row {
                                        width: parent.width
                                        spacing: 12
                                        AppMenu {
                                            id: folderMenu
                                            objectName: "folderMenu"
                                            width: parent.width - addFolder.width - parent.spacing
                                            height: 44
                                            leftPadding: 16
                                            font.family: settingsPane.fontFamily
                                            font.pixelSize: 16
                                            entries: bridge.folders
                                            valueRole: "id"
                                            currentIndex: entries.findIndex(entry => entry.id === bridge.selectedFolderId)
                                            displayText: currentIndex >= 0 ? entries[currentIndex].label : "No osu! folders"
                                            enabled: bridge.connected && !bridge.foldersLoading && !bridge.folderBusy && entries.length > 0
                                            Accessible.name: "osu! folders"
                                            Basic.ToolTip.visible: hovered && currentIndex >= 0
                                            onChosen: value => bridge.selectFolder(value)
                                            indicator: AppIcon {
                                                x: folderMenu.width - width - 16
                                                anchors.verticalCenter: parent.verticalCenter
                                                name: "arrow-down-s-line"
                                            }
                                            background: Rectangle {
                                                radius: 8
                                                color: Theme.background
                                                border.width: 1
                                                border.color: folderMenu.hovered ? Theme.muted : Theme.border
                                            }
                                        }
                                        IconButton {
                                            id: addFolder
                                            objectName: "addFolder"
                                            width: 44
                                            height: 44
                                            iconName: "plus"
                                            accessibleName: "Add osu! folder"
                                            enabled: bridge.connected && !bridge.connecting && !bridge.foldersLoading && !bridge.folderBusy
                                            onClicked: { addFolder.forceActiveFocus(); bridge.addFolder(); }
                                            background: Rectangle {
                                                radius: 8
                                                color: addFolder.hovered ? Theme.surface : Theme.background
                                                border.width: 1
                                                border.color: Theme.border
                                            }
                                        }
                                    }
                                    Text {
                                        objectName: "folderStatus"
                                        width: parent.width
                                        text: bridge.folderMessage
                                        visible: text.length > 0
                                        color: Theme.text
                                        font.family: settingsPane.fontFamily
                                        font.pixelSize: 14
                                        wrapMode: Text.Wrap
                                    }
                                    AppButton {
                                        objectName: "retryFolders"
                                        text: "Retry"
                                        font.family: settingsPane.fontFamily
                                        visible: bridge.folderCanRetry
                                        enabled: bridge.connected && !bridge.foldersLoading && !bridge.folderBusy
                                        onClicked: bridge.retryFolders()
                                    }
                                }
                                AppSwitch {
                                    objectName: "unicodeTitlesSwitch"
                                    width: parent.width
                                    text: "Use Unicode track titles"
                                    font.family: settingsPane.fontFamily
                                    font.pixelSize: 16
                                    checked: bridge.useUnicodeTitles
                                    onToggled: bridge.setUseUnicodeTitles(checked)
                                }
                                AppSwitch {
                                    objectName: "unicodeArtistsSwitch"
                                    width: parent.width
                                    text: "Use Unicode artist names"
                                    font.family: settingsPane.fontFamily
                                    font.pixelSize: 16
                                    checked: bridge.useUnicodeArtists
                                    onToggled: bridge.setUseUnicodeArtists(checked)
                                }
                                Text {
                                    objectName: "trackNamePreferencesStatus"
                                    width: parent.width
                                    text: bridge.trackNamePreferencesMessage
                                    visible: text.length > 0
                                    color: Theme.text
                                    font.family: settingsPane.fontFamily
                                    font.pixelSize: 14
                                    wrapMode: Text.Wrap
                                }
                            }
                            Column {
                                objectName: "audioSettingsSection"
                                visible: settingsPane.individualVolumeMatches || settingsPane.globalVolumeMatches
                                width: parent.width
                                spacing: 24
                                Row {
                                    height: 24
                                    spacing: 12
                                    AppIcon { name: "volume-up-fill" }
                                    Text {
                                        id: audioSettingsHeader
                                        objectName: "audioSettingsHeader"
                                        text: "Audio"
                                        color: Theme.text
                                        font.family: settingsPane.fontFamily
                                        font.pixelSize: 24
                                        font.weight: Font.Bold
                                        height: parent.height
                                        verticalAlignment: Text.AlignVCenter
                                    }
                                }
                                Column {
                                    width: parent.width
                                    spacing: 16
                                    AppSwitch {
                                        id: individualVolumeSwitch
                                        objectName: "individualVolumeSwitch"
                                        visible: settingsPane.individualVolumeMatches
                                        width: parent.width
                                        height: 44
                                        text: "Individual track volume"
                                        font.family: settingsPane.fontFamily
                                        font.pixelSize: 16
                                        checked: bridge.individualVolumeEnabled
                                        enabled: bridge.connected && bridge.audioSettingsLoaded
                                        onToggled: bridge.setIndividualVolumeEnabled(checked)
                                    }
                                    Column {
                                        objectName: "globalVolumeSetting"
                                        width: parent.width
                                        spacing: 8
                                        visible: settingsPane.globalVolumeMatches
                                        Text {
                                            id: globalVolumeLabel
                                            readonly property string settingName: "Global volume"
                                            text: settingName + " — " + bridge.globalVolumePercent + "%"
                                            color: Theme.text
                                            font.family: settingsPane.fontFamily
                                            font.pixelSize: 16
                                        }
                                        Basic.Slider {
                                            objectName: "globalVolumeSlider"
                                            palette.window: Theme.text
                                            palette.dark: Theme.accent
                                            palette.midlight: Theme.muted
                                            width: parent.width
                                            from: 0
                                            to: 100
                                            stepSize: 1
                                            value: bridge.globalVolumePercent
                                            enabled: bridge.audioSettingsLoaded
                                            Accessible.name: "Global volume"
                                            onMoved: bridge.setGlobalVolume(Math.round(value))
                                        }
                                    }
                                    Text {
                                        objectName: "audioSettingsMessage"
                                        width: parent.width
                                        text: bridge.audioSettingsMessage
                                        visible: text.length > 0
                                        color: Theme.text
                                        font.family: settingsPane.fontFamily
                                        font.pixelSize: 14
                                        wrapMode: Text.Wrap
                                    }
                                    AppButton {
                                        objectName: "retryAudioSettings"
                                        text: "Retry"
                                        font.family: settingsPane.fontFamily
                                        visible: bridge.audioSettingsCanRetry
                                        enabled: bridge.connected
                                        onClicked: bridge.retryAudioSettings()
                                    }
                                }
                            }
                            Text {
                                objectName: "settingsSearchEmpty"
                                width: parent.width
                                visible: !settingsPane.hasMatches
                                text: "No settings found. Try a different search or clear the search field."
                                color: Theme.muted
                                font.family: settingsPane.fontFamily
                                font.pixelSize: 16
                                wrapMode: Text.Wrap
                            }
                        }
                    }
                }
            }

            Item {
                id: player
                objectName: "playerPane"
                x: sidebar.width
                width: parent.width - x
                height: parent.height
                readonly property real geometryScale: Math.max(width / 960, 1)
                readonly property real contentWidth: Math.min(650 * geometryScale, 960, width * 0.85)
                readonly property real contentLeft: (width - contentWidth) * 138 / 310
                readonly property real coverSize: Math.min(340 * geometryScale, 640, contentWidth, Math.max(height - 225, 0))

                QueuePanel {
                    id: queuePanel
                    objectName: "queuePanel"
                    host: player
                    tracks: bridge.queueTracks
                    loading: bridge.queueLoading
                    message: bridge.queueMessage
                    onOpened: bridge.setQueueVisible(true)
                    onClosed: bridge.setQueueVisible(false)
                    onMediaRequested: id => bridge.requestMedia(id)
                    onMediaDelegateAdded: row => root.addMediaDelegate(row)
                    onMediaDelegateRemoved: row => root.removeMediaDelegate(row)
                    onVisibleMediaChanged: root.scheduleVisibleMedia()
                    onRetryRequested: bridge.setQueueVisible(true)
                }

                Item {
                    x: player.contentLeft
                    width: player.contentWidth
                    height: Math.max(0, player.height - 225)
                    Artwork {
                        objectName: "selectedArtwork"
                        anchors.centerIn: parent
                        width: player.coverSize
                        height: width
                        radius: 12
                        source: bridge.selectedArtworkUrl
                    }
                }
                Item {
                    id: information
                    x: player.contentLeft
                    y: player.height - 225
                    width: player.contentWidth
                    height: 173
                    Text {
                        objectName: "selectedTitle"
                        width: parent.width
                        height: 38
                        text: bridge.hasSelection ? bridge.selectedTitle : "No track selected"
                        color: Theme.text
                        font.family: Theme.fontFamily
                        font.pixelSize: 28
                        font.weight: Font.Bold
                        elide: Text.ElideRight
                    }
                    Text {
                        y: 44
                        width: parent.width
                        height: 27
                        text: bridge.selectedArtist
                        color: Theme.text
                        font.family: Theme.fontFamily
                        font.pixelSize: 20
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }
                    Basic.Slider {
                        id: progress
                        objectName: "playbackProgress"
                        y: 85
                        width: parent.width
                        height: 16
                        from: 0
                        to: Math.max(bridge.playbackDuration, 1)
                        enabled: bridge.canSeek
                        padding: 0
                        property int dragAudioId: -1
                        property int dragItemId: -1
                        Accessible.name: "Playback position"
                        Component.onCompleted: value = bridge.playbackPosition
                        onPressedChanged: {
                            if (pressed) { dragAudioId = bridge.selectedAudioId; dragItemId = bridge.selectedPlaylistItemId; }
                            else {
                                if (enabled && dragAudioId === bridge.selectedAudioId && dragItemId === bridge.selectedPlaylistItemId) bridge.seekPlayback(value);
                                value = bridge.playbackPosition;
                            }
                        }
                        onMoved: if (!pressed) bridge.seekPlayback(value)
                        Connections {
                            target: bridge
                            function onPlaybackPositionChanged() { if (!progress.pressed) progress.value = bridge.playbackPosition; }
                            function onPlaybackDurationChanged() { if (!progress.pressed) progress.value = bridge.playbackPosition; }
                            function onSelectedAudioIdChanged() { if (!progress.pressed) progress.value = bridge.playbackPosition; }
                            function onSelectedPlaylistItemIdChanged() { if (!progress.pressed) progress.value = bridge.playbackPosition; }
                        }
                        background: Rectangle {
                            y: 6
                            width: progress.width
                            height: 4
                            radius: 2
                            color: Theme.muted
                            scale: progress.mirrored ? -1 : 1
                            Rectangle {
                                width: progress.position * parent.width
                                height: parent.height
                                radius: parent.radius
                                color: Theme.accent
                            }
                        }
                        handle: Rectangle { x: progress.visualPosition * (progress.width - width); width: 16; height: 16; radius: 8; color: Theme.accent }
                    }
                    Text {
                        y: 101
                        objectName: "playbackPosition"
                        text: bridge.playbackPositionLabel
                        color: Theme.text
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                    }
                    Text {
                        objectName: "selectedDuration"
                        y: 101
                        anchors.right: parent.right
                        text: bridge.selectedDurationLabel
                        color: Theme.text
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                    }
                    Item {
                        y: 125
                        width: parent.width
                        height: 48
                        IconButton {
                            id: volumeButton
                            objectName: "volumeButton"
                            anchors.left: parent.left
                            anchors.verticalCenter: parent.verticalCenter
                            iconName: "volume-2"
                            accessibleName: "Volume"
                            onClicked: volumePopup.open()
                            Basic.Popup {
                                id: volumePopup
                                objectName: "volumePopup"
                                y: -height - 8
                                width: 180
                                height: contentItem.implicitHeight + topPadding + bottomPadding
                                property int editAudioId: -1
                                onOpened: { editAudioId = bridge.selectedAudioId; volumeSlider.value = bridge.volume; volumeSlider.forceActiveFocus(); }
                                Connections {
                                    target: bridge
                                    function onSelectedAudioIdChanged() { volumePopup.close(); }
                                    function onSelectedPlaylistItemIdChanged() { volumePopup.close(); }
                                    function onIndividualVolumeEnabledChanged() { volumePopup.close(); }
                                }
                                padding: 12
                                focus: true
                                closePolicy: Basic.Popup.CloseOnEscape | Basic.Popup.CloseOnPressOutside
                                onClosed: volumeButton.forceActiveFocus()
                                background: Rectangle { radius: 8; color: "#f20d0d0d"; border.color: Theme.muted }
                                contentItem: Column {
                                    spacing: 4
                                    Text { text: Math.round(bridge.volume * 100) + "%"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 12 }
                                    Basic.Slider {
                                        id: volumeSlider
                                        objectName: "volumeSlider"
                                        palette.window: Theme.text
                                        palette.dark: Theme.accent
                                        palette.midlight: Theme.muted
                                        width: parent.width
                                        from: 0
                                        to: 1
                                        value: bridge.volume
                                        stepSize: 0.01
                                        enabled: bridge.volumeEnabled
                                        Accessible.name: bridge.individualVolumeEnabled ? "Selected track volume" : "Global volume"
                                        Connections {
                                            target: bridge
                                            function onVolumeChanged() { if (!volumeSlider.pressed) volumeSlider.value = bridge.volume; }
                                        }
                                        onPressedChanged: if (!pressed) value = bridge.volume
                                        onMoved: {
                                            if (bridge.individualVolumeEnabled) bridge.changeTrackVolume(volumePopup.editAudioId, Math.round(value * 100));
                                            else bridge.changeVolume(value);
                                        }
                                    }
                                    AppButton {
                                        objectName: "useGlobalVolumeButton"
                                        text: "Use global volume"
                                        width: parent.width
                                        visible: bridge.individualVolumeEnabled
                                        enabled: bridge.volumeEnabled && bridge.volumeHasOverride
                                        onClicked: bridge.useGlobalVolume()
                                    }
                                    Text {
                                        width: parent.width
                                        text: bridge.audioSettingsMessage
                                        visible: bridge.audioSettingsCanRetry
                                        color: Theme.text
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 12
                                        wrapMode: Text.Wrap
                                    }
                                    AppButton {
                                        objectName: "retryVolumeSave"
                                        text: "Retry"
                                        visible: bridge.audioSettingsCanRetry
                                        enabled: bridge.connected
                                        onClicked: bridge.retryAudioSettings()
                                    }
                                }
                            }
                        }
                        Row {
                            anchors.centerIn: parent
                            spacing: 28
                            IconButton { objectName: "shuffleButton"; anchors.verticalCenter: parent.verticalCenter; iconName: "shuffle"; accessibleName: "Shuffle (unavailable)"; enabled: false }
                            IconButton { objectName: "previousTrackButton"; anchors.verticalCenter: parent.verticalCenter; iconName: "skip-back"; accessibleName: "Previous track"; enabled: bridge.connected && bridge.canPrevious; onClicked: bridge.previousTrack() }
                            IconButton {
                                width: 48
                                height: 48
                                objectName: "playPauseButton"
                                iconName: bridge.selectedIsPlaying ? "pause" : "play"
                                accessibleName: bridge.selectedIsPlaying ? "Pause" : "Play"
                                enabled: bridge.hasSelection && bridge.selectedAvailable && bridge.connected && bridge.audioSettingsLoaded
                                onClicked: bridge.togglePlayback()
                                background: Rectangle { radius: 24; color: Theme.accent }
                            }
                            IconButton { objectName: "nextTrackButton"; anchors.verticalCenter: parent.verticalCenter; iconName: "skip-forward"; accessibleName: "Next track"; enabled: bridge.connected && bridge.canNext; onClicked: bridge.nextTrack() }
                            IconButton { objectName: "repeatButton"; anchors.verticalCenter: parent.verticalCenter; iconName: "repeat-2"; accessibleName: "Repeat (unavailable)"; enabled: false }
                        }
                        IconButton { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; iconName: "circle-plus"; objectName: "addToPlaylistButton"; accessibleName: "Add to playlist"; enabled: bridge.canAddPlaylist && bridge.connected; onClicked: bridge.openPlaylistAdd() }
                    }
                    Text {
                        objectName: "playbackMessage"
                        y: 182
                        width: parent.width
                        text: bridge.playbackMessage
                        visible: text.length > 0
                        color: Theme.text
                        font.family: Theme.fontFamily
                        font.pixelSize: 12
                        elide: Text.ElideRight
                        Basic.ToolTip.visible: messageHover.hovered
                        Basic.ToolTip.text: text
                        HoverHandler { id: messageHover }
                    }
                }
            }
        }
    }
    PlaylistsModal {
        id: playlistsDialog
        objectName: "playlistsDialog"
        scene: applicationScene
        playlists: bridge.playlists
        candidates: bridge.playlistCandidates
        adding: bridge.playlistAdding
        busy: bridge.playlistBusy
        loading: bridge.playlistLoading
        message: bridge.playlistMessage
        targetId: bridge.playlistTargetId
        onTargetRequested: id => bridge.choosePlaylistTarget(id)
        onDifficultyRequested: id => bridge.togglePlaylistDifficulty(id)
        onAddRequested: bridge.addPlaylistItems()
        onRefreshRequested: bridge.refreshPlaylists()
        onClosed: if (bridge.playlistOpen) bridge.closePlaylists()
    }
    Connections {
        target: bridge
        function onPlaylistOpenChanged() {
            if (bridge.playlistOpen && bridge.playlistAdding) playlistsDialog.open();
            else playlistsDialog.close();
        }
    }
    FileDialog {
        id: playlistCoverPicker
        objectName: "playlistCoverPicker"
        property double editorEpoch: 0
        title: "Choose playlist cover"
        fileMode: FileDialog.OpenFile
        nameFilters: ["Images (*.png *.jpg *.jpeg)"]
        onAccepted: bridge.completePlaylistCoverPick(editorEpoch, selectedFile.toString())
        onRejected: bridge.completePlaylistCoverPick(editorEpoch, "")
    }
    Connections {
        target: bridge
        function onPlaylistCoverPickerRequested(epoch) {
            playlistCoverPicker.editorEpoch = epoch;
            playlistCoverPicker.open();
        }
    }
    FolderSelectionModal {
        id: folderDialog
        objectName: "folderSelectionDialog"
        scene: applicationScene
        rows: bridge.folderSelectionRows
        applying: bridge.folderSelectionApplying
        discovering: bridge.folderSelectionDiscovering
        picking: bridge.folderSelectionPicking
        message: bridge.folderSelectionMessage
        onToggleRequested: path => bridge.toggleFolderSelection(path)
        onRefreshRequested: path => bridge.refreshFolderSelection(path)
        onRetryCountRequested: path => bridge.retryFolderCount(path)
        onBrowseRequested: bridge.browseFolderSelection()
        onApplyRequested: bridge.applyFolderSelection()
        onClosed: if (bridge.folderSelectionOpen) bridge.closeFolderSelection()
    }
    Connections {
        target: bridge
        function onFolderSelectionOpenChanged() {
            if (bridge.folderSelectionOpen) folderDialog.open();
            else folderDialog.close();
        }
    }
}
