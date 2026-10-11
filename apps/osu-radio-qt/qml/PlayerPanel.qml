pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import OsuRadio 1.0
import "components"

Item {
    required property AppBridge bridge
    required property var shell
    id: player
    readonly property alias queue: queuePanel
    Connections {
        target: player.bridge.player
        function onSelectedPlaylistItemIdChanged() { volumePopup.close(); if (!progress.pressed) progress.value = player.bridge.player.playbackPosition; }
    }
    objectName: "playerPane"
    readonly property real geometryScale: Math.max(width / 960, 1)
    readonly property real contentWidth: Math.min(650 * geometryScale, 960, width * 0.85)
    readonly property real contentLeft: (width - contentWidth) * 138 / 310
    readonly property real coverSize: Math.min(340 * geometryScale, 640, contentWidth, Math.max(height - 225, 0))

    QueuePanel {
        id: queuePanel
        objectName: "queuePanel"
        host: player
        tracks: player.bridge.queue.queueTracks
        loading: player.bridge.queue.queueLoading
        message: player.bridge.queue.queueMessage
        onOpened: player.bridge.queue.setQueueVisible(true)
        onClosed: player.bridge.queue.setQueueVisible(false)
        onMediaRequested: id => player.bridge.requestMedia(id)
        onMediaDelegateAdded: row => player.shell.addMediaDelegate(row)
        onMediaDelegateRemoved: row => player.shell.removeMediaDelegate(row)
        onVisibleMediaChanged: player.shell.scheduleVisibleMedia()
        onRetryRequested: player.bridge.queue.setQueueVisible(true)
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
            source: String(player.bridge.player.selectedArtworkUrl)
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
            text: player.bridge.player.hasSelection ? player.bridge.player.selectedTitle : "No track selected"
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
            text: player.bridge.player.selectedArtist
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
            to: Math.max(player.bridge.player.playbackDuration, 1)
            enabled: player.bridge.player.canSeek
            padding: 0
            property int dragAudioId: -1
            property int dragItemId: -1
            Accessible.name: "Playback position"
            Component.onCompleted: value = player.bridge.player.playbackPosition
            onPressedChanged: {
                if (pressed) { dragAudioId = player.bridge.player.selectedAudioId; dragItemId = player.bridge.player.selectedPlaylistItemId; }
                else {
                    if (enabled && dragAudioId === player.bridge.player.selectedAudioId && dragItemId === player.bridge.player.selectedPlaylistItemId) player.bridge.player.seekPlayback(value);
                    value = player.bridge.player.playbackPosition;
                }
            }
            onMoved: if (!pressed) player.bridge.player.seekPlayback(value)
            Connections {
                target: player.bridge.player
                function onPlaybackPositionChanged() { if (!progress.pressed) progress.value = player.bridge.player.playbackPosition; }
                function onPlaybackDurationChanged() { if (!progress.pressed) progress.value = player.bridge.player.playbackPosition; }
                function onSelectedAudioIdChanged() { if (!progress.pressed) progress.value = player.bridge.player.playbackPosition; }
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
                                    radius: parent.radius
                    color: Theme.accent
                }
            }
            handle: Rectangle { x: progress.visualPosition * (progress.width - width); width: 16; height: 16; radius: 8; color: Theme.accent }
        }
        Text {
            y: 101
            objectName: "playbackPosition"
            text: player.bridge.player.playbackPositionLabel
            color: Theme.text
            font.family: Theme.fontFamily
            font.pixelSize: 12
        }
        Text {
            objectName: "selectedDuration"
            y: 101
            anchors.right: parent.right
            text: player.bridge.player.selectedDurationLabel
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
                    onOpened: { editAudioId = player.bridge.player.selectedAudioId; volumeSlider.value = player.bridge.settings.volume; volumeSlider.forceActiveFocus(); }
                    Connections {
                        target: player.bridge.player
                        function onSelectedAudioIdChanged() { volumePopup.close(); }
                    }
                    Connections {
                        target: player.bridge.settings
                        function onIndividualVolumeEnabledChanged() { volumePopup.close(); }
                    }
                    padding: 12
                    focus: true
                    closePolicy: Basic.Popup.CloseOnEscape | Basic.Popup.CloseOnPressOutside
                    onClosed: volumeButton.forceActiveFocus()
                    background: Rectangle { radius: 8; color: "#f20d0d0d"; border.color: Theme.muted }
                    contentItem: Column {
                        spacing: 4
                        Text { text: Math.round(player.bridge.settings.volume * 100) + "%"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 12 }
                        Basic.Slider {
                            id: volumeSlider
                            objectName: "volumeSlider"
                            palette.window: Theme.text
                            palette.dark: Theme.accent
                            palette.midlight: Theme.muted
                            width: parent.width
                            from: 0
                            to: 1
                            value: Number(player.bridge.settings.volume)
                            stepSize: 0.01
                            enabled: player.bridge.settings.volumeEnabled
                            Accessible.name: player.bridge.settings.individualVolumeEnabled ? "Selected track volume" : "Global volume"
                            Connections {
                                target: player.bridge.settings
                                function onVolumeChanged() { if (!volumeSlider.pressed) volumeSlider.value = player.bridge.settings.volume; }
                            }
                            onPressedChanged: if (!pressed) value = player.bridge.settings.volume
                            onMoved: {
                                if (player.bridge.settings.individualVolumeEnabled) player.bridge.settings.changeTrackVolume(volumePopup.editAudioId, Math.round(value * 100));
                                else player.bridge.settings.changeVolume(value);
                            }
                        }
                        AppButton {
                            objectName: "useGlobalVolumeButton"
                            text: "Use global volume"
                            width: parent.width
                            visible: player.bridge.settings.individualVolumeEnabled
                            enabled: player.bridge.settings.volumeEnabled && player.bridge.settings.volumeHasOverride
                            onClicked: player.bridge.settings.useGlobalVolume()
                        }
                        Text {
                            width: parent.width
                            text: player.bridge.settings.audioSettingsMessage
                            visible: player.bridge.settings.audioSettingsCanRetry
                            color: Theme.text
                            font.family: Theme.fontFamily
                            font.pixelSize: 12
                            wrapMode: Text.Wrap
                        }
                        AppButton {
                            objectName: "retryVolumeSave"
                            text: "Retry"
                            visible: player.bridge.settings.audioSettingsCanRetry
                            enabled: player.bridge.connected
                            onClicked: player.bridge.settings.retryAudioSettings()
                        }
                    }
                }
            }
            Row {
                anchors.centerIn: parent
                spacing: 28
                IconButton { objectName: "shuffleButton"; anchors.verticalCenter: parent.verticalCenter; iconName: "shuffle"; accessibleName: "Shuffle (unavailable)"; enabled: false }
                IconButton { objectName: "previousTrackButton"; anchors.verticalCenter: parent.verticalCenter; iconName: "skip-back"; accessibleName: "Previous track"; enabled: player.bridge.connected && player.bridge.player.canPrevious; onClicked: player.bridge.player.previousTrack() }
                IconButton {
                    width: 48
                    height: 48
                    objectName: "playPauseButton"
                    iconName: player.bridge.player.selectedIsPlaying ? "pause" : "play"
                    accessibleName: player.bridge.player.selectedIsPlaying ? "Pause" : "Play"
                    enabled: player.bridge.player.canPlay && player.bridge.connected && player.bridge.settings.audioSettingsLoaded
                    onClicked: player.bridge.player.togglePlayback()
                    background: Rectangle { radius: 24; color: Theme.accent }
                }
                IconButton { objectName: "nextTrackButton"; anchors.verticalCenter: parent.verticalCenter; iconName: "skip-forward"; accessibleName: "Next track"; enabled: player.bridge.connected && player.bridge.player.canNext; onClicked: player.bridge.player.nextTrack() }
                IconButton { objectName: "repeatButton"; anchors.verticalCenter: parent.verticalCenter; iconName: "repeat-2"; accessibleName: "Repeat (unavailable)"; enabled: false }
            }
            IconButton { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; iconName: "circle-plus"; objectName: "addToPlaylistButton"; accessibleName: "Add to playlist"; enabled: player.bridge.player.canAddPlaylist && player.bridge.connected; onClicked: player.bridge.playlists.openPlaylistAdd() }
        }
        Text {
            objectName: "playbackMessage"
            y: 182
            width: parent.width
            text: player.bridge.player.playbackMessage
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
