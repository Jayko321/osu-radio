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
    function openMods(): void {
        if (!modsPopup.opened) modsPopup.open();
    }
    Connections {
        target: player.bridge.player
        function onSelectedPlaylistItemIdChanged() { volumePopup.close(); }
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
        tracks: player.bridge.queue.tracks
        startIndex: player.bridge.queue.startIndex
        loading: player.bridge.queue.queueLoading
        message: player.bridge.queue.queueMessage
        onOpened: player.bridge.queue.setQueueVisible(true)
        onClosed: player.bridge.queue.setQueueVisible(false)
        onPlayRequested: id => player.bridge.queue.playEntry(id)
        onRemoveRequested: id => player.bridge.queue.removeEntry(id)
        onMoveRequested: (id, before) => player.bridge.queue.moveEntry(id, before)
        onClearUpcomingRequested: player.bridge.queue.clearUpcoming()
        onClearAllRequested: player.bridge.queue.clearAll()
        onVisibleEntriesChanged: ids => player.bridge.queue.setVisibleQueueMedia(ids)
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
        PlaybackProgress {
            id: progress
            objectName: "playbackProgress"
            y: 85
            width: parent.width
            height: 16
            enabled: player.bridge.player.canSeek
            playbackPosition: player.bridge.player.playbackPosition
            playbackDuration: player.bridge.player.playbackDuration
            scaleKey: [player.bridge.player.selectedAudioId, player.bridge.player.selectedPlaylistItemId,
                player.bridge.player.launchToken, player.bridge.player.playbackSpeedPercent].join(":")
            onSeekRequested: seconds => player.bridge.player.seekPlayback(seconds)
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
            IconButton {
                id: modsButton
                objectName: "modsButton"
                anchors.left: volumeButton.right
                anchors.verticalCenter: parent.verticalCenter
                iconName: "gauge"
                accessibleName: "Mods (F1)"
                onClicked: player.openMods()
                AudioModsPopup {
                    id: modsPopup
                    objectName: "modsPopup"
                    y: -height - 8
                    mode: player.bridge.settings.modsMode
                    speedPercent: player.bridge.settings.modsSpeedPercent
                    adjustPitch: player.bridge.settings.modsAdjustPitch
                    available: player.bridge.connected && player.bridge.settings.modsAvailable
                    saving: player.bridge.settings.modsSaving
                    errorMessage: player.bridge.settings.modsMessage
                    canRetry: player.bridge.connected
                    onAboutToShow: player.bridge.settings.beginModsEdit()
                    // Mods that finish loading under an open popup have no addressee captured yet.
                    onAvailableChanged: if (opened && available) player.bridge.settings.beginModsEdit()
                    onClosed: player.bridge.settings.finishModsEdit()
                    onDtCycleRequested: player.bridge.settings.cycleDtMod()
                    onHtToggleRequested: player.bridge.settings.toggleHtMod()
                    onSpeedRequested: percent => player.bridge.settings.setModSpeed(percent)
                    onPitchRequested: adjust => player.bridge.settings.setModAdjustPitch(adjust)
                    onClearRequested: player.bridge.settings.clearMods()
                    onRetryRequested: player.bridge.settings.retryMods()
                    Connections {
                        target: player.bridge.player
                        function onSelectedAudioIdChanged() { modsPopup.close(); }
                        function onSelectedPlaylistItemIdChanged() { modsPopup.close(); }
                    }
                    Connections {
                        target: player.bridge.settings
                        function onIndividualModsEnabledChanged() { modsPopup.close(); }
                    }
                }
            }
            Row {
                anchors.centerIn: parent
                // Leaves room for the volume and mods buttons at the minimum window width.
                spacing: Math.max(8, Math.min(28, (player.contentWidth - 390) / 4))
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
