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
    readonly property bool hotkeysBlocked: folderDialog.visible || playlistsDialog.visible || confirmDialog.visible || player.popupOpen
    // Panels ask the scene, which owns the single dialog and toast, so they stay free of both.
    function confirm(title: string, message: string, actionText: string, action: var): void {
        confirmDialog.title = title;
        confirmDialog.message = message;
        confirmDialog.actionText = actionText;
        confirmDialog.action = action;
        confirmDialog.open();
    }
    // Call just before a queue action: the toast shows once the queue operation finishes without an error.
    function expectQueueToast(text: string): void { queueOperation.expect(text); }
    function playlistById(id: int): var {
        return findById(bridge.playlists.playlists, id);
    }
    function findById(list: var, id: int): var { return list.find(entry => entry.id === id); }
    // The playlist is gone from the canonical list once the delete is confirmed by the server.
    function deletePlaylist(id: int): void {
        const playlist = playlistById(id);
        confirm("Delete “" + (playlist ? playlist.name : "this playlist") + "”?",
            "The playlist and its order will be deleted. Tracks stay in your library.", "Delete", () => {
                playlistOperation.expect("Playlist deleted", () => !playlistById(id));
                bridge.playlists.deletePlaylist(id);
            });
    }
    // The search field of the active tab; null where the tab has none (an open playlist, for one).
    function activeSearch(): Item {
        const field = selectedTab === 0 ? songsPanel.searchField
            : selectedTab === 1 ? playlistsPanel.searchField : settingsPanel.searchField;
        return field && field.visible ? field : null;
    }
    function seekBy(seconds: real): void {
        const state = bridge.player;
        if (!state.canSeek) return;
        state.seekPlayback(Math.max(0, Math.min(state.playbackDuration, state.playbackPosition + seconds)));
    }
    // Runs only for keys no focused control consumed, so text fields, sliders and buttons keep their own.
    function handleHotkey(key: int, modifiers: int): bool {
        if (hotkeysBlocked || (modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))) return false;
        const state = bridge.player;
        if (key === Qt.Key_Escape) {
            if (!activeFocusItem || activeFocusItem !== activeSearch()) return false;
            applicationScene.forceActiveFocus();
        } else if (key === Qt.Key_Space) {
            if (state.canPlay && bridge.connected && bridge.settings.audioSettingsLoaded) state.togglePlayback();
        } else if (key === Qt.Key_M) {
            state.toggleMute();
        } else if (key === Qt.Key_Left || key === Qt.Key_Right) {
            seekBy(key === Qt.Key_Left ? -5 : 5);
        } else return false;
        return true;
    }
    // F1 only opens the menu; D and E belong to the open popup.
    Shortcut {
        sequence: "F1"
        enabled: !folderDialog.visible && !playlistsDialog.visible && !confirmDialog.visible
        onActivated: player.openMods()
    }
    Shortcut {
        sequence: "Ctrl+F"
        enabled: !root.hotkeysBlocked
        onActivated: { const field = root.activeSearch(); if (field) field.forceActiveFocus(Qt.ShortcutFocusReason); }
    }
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
        // Holds focus when no control does, so Space and arrows work until a field or button takes it.
        focus: true
        Keys.onPressed: event => { event.accepted = root.handleHotkey(event.key, event.modifiers); }

        WindowBar {
            id: titleBar
            objectName: "titleBar"
            width: parent.width
            window: root
            selectedTab: root.selectedTab
            queueEnabled: bridge.connected
            queueVisible: player.queue.visible
            onTabSelected: index => root.selectedTab = index
            onQueueRequested: player.queue.visible ? player.queue.close() : player.queue.open()
        }

        // Overlay: it never takes part in the layout, so the window does not shift when it appears.
        Item {
            id: connectionBanner
            readonly property bool shown: !bridge.connected
            anchors.top: titleBar.bottom
            width: parent.width
            height: Math.max(connectionText.implicitHeight + 24, 60)
            z: 10
            clip: true
            visible: shown || bannerContent.opacity > 0

            Rectangle {
                id: bannerContent
                width: parent.width
                height: parent.height
                y: connectionBanner.shown ? 0 : -height
                opacity: connectionBanner.shown ? 1 : 0
                color: "#f81c1416"
                Behavior on y { NumberAnimation { duration: Theme.ms(Theme.motionBase); easing.type: Theme.easing } }
                Behavior on opacity { NumberAnimation { duration: Theme.ms(Theme.motionBase); easing.type: Theme.easing } }
                Rectangle { width: 4; height: parent.height; color: Theme.red }
                Rectangle { width: parent.width; height: 1; anchors.bottom: parent.bottom; color: Theme.border }
                Canvas {
                    id: connectionSpinner
                    objectName: "connectionSpinner"
                    x: 24
                    anchors.verticalCenter: parent.verticalCenter
                    width: 14
                    height: 14
                    visible: bridge.connecting
                    onPaint: {
                        const ctx = getContext("2d");
                        ctx.reset();
                        ctx.lineWidth = 2;
                        ctx.strokeStyle = Theme.muted;
                        ctx.beginPath();
                        ctx.arc(7, 7, 6, 0, Math.PI * 2);
                        ctx.stroke();
                        ctx.strokeStyle = Theme.text;
                        ctx.beginPath();
                        ctx.arc(7, 7, 6, -Math.PI / 2, 0);
                        ctx.stroke();
                    }
                    RotationAnimator on rotation {
                        from: 0
                        to: 360
                        duration: 800
                        loops: Animation.Infinite
                        running: connectionSpinner.visible && connectionBanner.visible
                    }
                }
                Text {
                    id: connectionText
                    objectName: "connectionStatus"
                    x: connectionSpinner.visible ? 48 : 24
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - x - (connectionRetry.visible ? connectionRetry.width + 20 : 0) - 20
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
        }

        Item {
            id: body
            objectName: "body"
            anchors.top: titleBar.bottom
            anchors.bottom: parent.bottom
            width: parent.width
            clip: true

            Item {
                id: backdrop
                anchors.fill: parent
                CrossfadeSource {
                    id: backdropFade
                    duration: Theme.ms(400)
                    source: String(bridge.player.selectedArtworkUrl)
                }
                Item {
                    id: backdropArt
                    anchors.fill: parent
                    visible: false
                    layer.enabled: true
                    Image {
                        anchors.fill: parent
                        source: backdropFade.previous
                        cache: false
                        fillMode: Image.PreserveAspectCrop
                    }
                    Image {
                        id: backdropImage
                        anchors.fill: parent
                        source: backdropFade.source
                        opacity: backdropFade.progress
                        cache: false
                        fillMode: Image.PreserveAspectCrop
                    }
                }
                MultiEffect {
                    anchors.fill: parent
                    source: backdropArt
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
                    id: songsPanel
                    anchors.fill: parent
                    visible: root.selectedTab === 0
                    NumberAnimation on opacity { from: 0; to: 1; duration: Theme.ms(Theme.motionFast); easing.type: Theme.easing; running: songsPanel.visible }
                    bridge: root.appBridge
                    shell: root
                }

                PlaylistsPanel {
                    id: playlistsPanel
                    anchors.fill: parent
                    visible: root.selectedTab === 1
                    NumberAnimation on opacity { from: 0; to: 1; duration: Theme.ms(Theme.motionFast); easing.type: Theme.easing; running: playlistsPanel.visible }
                    bridge: root.appBridge
                    shell: root
                }

                SettingsPanel {
                    id: settingsPanel
                    anchors.fill: parent
                    visible: root.selectedTab === 2
                    NumberAnimation on opacity { from: 0; to: 1; duration: Theme.ms(Theme.motionFast); easing.type: Theme.easing; running: settingsPanel.visible }
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
    ConfirmDialog {
        id: confirmDialog
        objectName: "confirmDialog"
        property var action: null
        scene: applicationScene
        onConfirmed: if (action) action()
    }
    Toast {
        id: toast
        objectName: "toast"
        anchorItem: player
    }
    OperationWatcher {
        id: queueOperation
        busy: bridge.queue.queueBusy
        message: bridge.queue.queueMessage
        onSucceeded: text => toast.show(text)
    }
    OperationWatcher {
        id: playlistOperation
        busy: bridge.playlists.playlistBusy
        message: bridge.playlists.playlistMessage
        onSucceeded: text => toast.show(text)
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
