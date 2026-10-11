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
        if (selectedTab === 0) bridge.playlists.showLibrary();
        else if (selectedTab === 1) bridge.playlists.showPlaylists();
        scheduleVisibleMedia();
    }
    readonly property alias appBridge: bridge
    AppBridge {
        id: bridge
        objectName: "appBridge"
        Component.onCompleted: connectSession()
        onMediaViewChanged: root.scheduleVisibleMedia()
    }


    Connections {
        target: bridge.player
        function onSelectedArtworkUrlChanged() { if (bridge.player.hasSelection && String(bridge.player.selectedArtworkUrl).length === 0) bridge.requestMedia(bridge.player.selectedAudioId); }
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
            queueVisible: player.queue.visible
            onTabSelected: index => root.selectedTab = index
            onQueueRequested: player.queue.visible ? player.queue.close() : player.queue.open()
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
                    source: String(bridge.player.selectedArtworkUrl)
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
                SongsPanel {
                    anchors.fill: parent
                    visible: root.selectedTab === 0
                    bridge: root.appBridge
                    shell: root
                }

                PlaylistsPanel {
                    anchors.fill: parent
                    visible: root.selectedTab === 1
                    bridge: root.appBridge
                    shell: root
                }

                SettingsPanel {
                    anchors.fill: parent
                    visible: root.selectedTab === 2
                    bridge: root.appBridge
                }
            }

            PlayerPanel {
                id: player
                bridge: root.appBridge
                shell: root
                x: sidebar.width
                width: parent.width - x
                height: parent.height
            }
        }
    }
    PlaylistsModal {
        id: playlistsDialog
        objectName: "playlistsDialog"
        scene: applicationScene
        playlists: bridge.playlists.playlists
        candidates: bridge.playlists.playlistCandidates
        adding: bridge.playlists.playlistAdding
        busy: bridge.playlists.playlistBusy
        loading: bridge.playlists.playlistLoading
        message: bridge.playlists.playlistMessage
        targetId: bridge.playlists.playlistTargetId
        onTargetRequested: id => bridge.playlists.choosePlaylistTarget(id)
        onDifficultyRequested: id => bridge.playlists.togglePlaylistDifficulty(id)
        onAddRequested: bridge.playlists.addPlaylistItems()
        onRefreshRequested: bridge.playlists.refreshPlaylists()
        onClosed: if (bridge.playlists.playlistOpen) bridge.playlists.closePlaylists()
    }
    Connections {
        target: bridge.playlists
        function onPlaylistOpenChanged() {
            if (bridge.playlists.playlistOpen && bridge.playlists.playlistAdding) playlistsDialog.open();
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
        onAccepted: bridge.playlists.completePlaylistCoverPick(editorEpoch, selectedFile.toString())
        onRejected: bridge.playlists.completePlaylistCoverPick(editorEpoch, "")
    }
    Connections {
        target: bridge.playlists
        function onPlaylistCoverPickerRequested(epoch) {
            playlistCoverPicker.editorEpoch = epoch;
            playlistCoverPicker.open();
        }
    }
    FolderSelectionModal {
        id: folderDialog
        objectName: "folderSelectionDialog"
        scene: applicationScene
        rows: bridge.settings.folderSelectionRows
        applying: bridge.settings.folderSelectionApplying
        discovering: bridge.settings.folderSelectionDiscovering
        picking: bridge.settings.folderSelectionPicking
        message: bridge.settings.folderSelectionMessage
        onToggleRequested: path => bridge.settings.toggleFolderSelection(path)
        onRefreshRequested: path => bridge.settings.refreshFolderSelection(path)
        onRetryCountRequested: path => bridge.settings.retryFolderCount(path)
        onBrowseRequested: bridge.settings.browseFolderSelection()
        onApplyRequested: bridge.settings.applyFolderSelection()
        onClosed: if (bridge.settings.folderSelectionOpen) bridge.settings.closeFolderSelection()
    }
    Connections {
        target: bridge.settings
        function onFolderSelectionOpenChanged() {
            if (bridge.settings.folderSelectionOpen) folderDialog.open();
            else folderDialog.close();
        }
    }
}
