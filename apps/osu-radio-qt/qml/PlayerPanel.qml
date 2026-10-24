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
    readonly property bool popupOpen: volumePopup.opened || modsPopup.opened || queuePanel.visible
    readonly property int volumePercent: Math.round(Number(bridge.settings.volume) * 100)
    // Mute is session player state; the saved volume (volumePercent, the slider) is never touched by it.
    readonly property bool muted: bridge.player.muted
    readonly property bool modsActive: bridge.settings.modsAvailable && bridge.settings.modsMode !== "none"
    // Same commands as the slider; individual mode edits the selected track only.
    function stepVolume(delta: int): void {
        const settings = bridge.settings;
        const percent = Math.max(0, Math.min(100, volumePercent + delta));
        if (!settings.volumeEnabled || percent === volumePercent) return;
        if (settings.individualVolumeEnabled) settings.changeTrackVolume(bridge.player.selectedAudioId, percent);
        else settings.changeVolume(percent / 100);
    }
    // A wheel notch is 120 units; touchpads send small deltas, so they accumulate into whole 5% steps.
    property real wheelRemainder: 0
    function wheelVolume(angleDelta: real): void {
        wheelRemainder += angleDelta;
        const notches = Math.trunc(wheelRemainder / 120);
        if (notches === 0) return;
        wheelRemainder -= notches * 120;
        stepVolume(notches * 5);
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
        onClearUpcomingRequested: { player.shell.expectQueueToast("Upcoming cleared"); player.bridge.queue.clearUpcoming(); }
        onClearAllRequested: player.shell.confirm("Clear the whole queue?",
            "This removes every track from the queue, including the current track and the history, and stops playback.", "Clear all", () => {
                player.shell.expectQueueToast("Queue cleared");
                player.bridge.queue.clearAll();
            })
        onVisibleEntriesChanged: ids => player.bridge.queue.setVisibleQueueMedia(ids)
        onRetryRequested: player.bridge.queue.setQueueVisible(true)
    }

    Item {
        x: player.contentLeft
        width: player.contentWidth
        height: Math.max(0, player.height - 225)
        CrossfadeSource {
            id: coverFade
            source: String(player.bridge.player.selectedArtworkUrl)
        }
        Artwork {
            objectName: "previousArtwork"
            anchors.centerIn: parent
            width: player.coverSize
            height: width
            radius: 12
            placeholder: false
            revealDuration: 0
            source: coverFade.previous
        }
        Artwork {
            objectName: "selectedArtwork"
            anchors.centerIn: parent
            width: player.coverSize
            height: width
            radius: 12
            opacity: coverFade.progress
            source: coverFade.source
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
            text: player.bridge.player.hasSelection ? player.bridge.player.selectedArtist : "Pick a song to start"
            color: player.bridge.player.hasSelection ? Theme.text : Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: 20
            font.weight: Font.DemiBold
            elide: Text.ElideRight
        }
        PlaybackProgress {
            id: progress
            objectName: "playbackProgress"
            y: 81
            width: parent.width
            height: 24
            enabled: player.bridge.player.canSeek
            playbackPosition: player.bridge.player.playbackPosition
            playbackDuration: player.bridge.player.playbackDuration
            scaleKey: [player.bridge.player.selectedAudioId, player.bridge.player.selectedPlaylistItemId,
                player.bridge.player.launchToken, player.bridge.player.playbackSpeedPercent].join(":")
            onSeekRequested: seconds => player.bridge.player.seekPlayback(seconds)
        }
        Text {
            y: 107
            objectName: "playbackPosition"
            text: player.bridge.player.playbackPositionLabel
            color: Theme.text
            font.family: Theme.fontFamily
            font.pixelSize: 12
        }
        Text {
            objectName: "selectedDuration"
            y: 107
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
                iconName: player.muted || player.volumePercent === 0 ? "volume-x" : player.volumePercent < 40 ? "volume-1" : "volume-2"
                accessibleName: "Volume"
                onClicked: volumePopup.open()
                WheelHandler { onWheel: event => player.wheelVolume(event.angleDelta.y) }
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
                    transformOrigin: Basic.Popup.BottomLeft
                    enter: PopupEnter {}
                    exit: PopupExit {}
                    focus: true
                    closePolicy: Basic.Popup.CloseOnEscape | Basic.Popup.CloseOnPressOutside
                    onClosed: volumeButton.forceActiveFocus()
                    background: PopupSurface { radius: 8; border.color: Theme.muted }
                    contentItem: Column {
                        spacing: 4
                        WheelHandler { onWheel: event => player.wheelVolume(event.angleDelta.y) }
                        Text {
                            objectName: "volumeScope"
                            text: player.bridge.settings.individualVolumeEnabled ? "This track" : "All tracks"
                            color: Theme.muted; font.family: Theme.fontFamily; font.pixelSize: 12
                        }
                        Text { objectName: "volumePercent"; text: (player.muted ? 0 : player.volumePercent) + "%"; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 12 }
                        Item {
                            width: parent.width
                            height: muteButton.height
                            IconButton {
                                id: muteButton
                                objectName: "muteButton"
                                anchors.left: parent.left
                                anchors.verticalCenter: parent.verticalCenter
                                iconName: "volume-x"
                                foreground: player.muted ? Theme.accent : Theme.text
                                accessibleName: "Mute (M)"
                                Accessible.name: "Mute"
                                onClicked: player.bridge.player.toggleMute()
                            }
                            Basic.Slider {
                                id: volumeSlider
                                objectName: "volumeSlider"
                                palette.window: Theme.text
                                palette.dark: Theme.accent
                                palette.midlight: Theme.muted
                                anchors.left: muteButton.right
                                anchors.leftMargin: 4
                                anchors.right: parent.right
                                anchors.verticalCenter: parent.verticalCenter
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
                foreground: player.modsActive ? Theme.accent : Theme.text
                accessibleName: player.modsActive ? "Mods (F1): " + modsBadge.label : "Mods (F1)"
                onClicked: player.openMods()
                Rectangle {
                    id: modsBadge
                    objectName: "modsBadge"
                    readonly property string label: (player.bridge.settings.modsMode === "dt"
                        ? (player.bridge.settings.modsAdjustPitch ? "NC" : "DT") : "HT")
                        + " " + player.bridge.settings.modsSpeedPercent / 100 + "×"
                    visible: player.modsActive
                    x: parent.width - width + 2
                    y: -2
                    width: badgeText.implicitWidth + 8
                    height: 14
                    radius: 7
                    color: Theme.accent
                    Text {
                        id: badgeText
                        anchors.centerIn: parent
                        text: player.bridge.settings.modsSpeedPercent / 100 + "×"
                        color: Theme.text
                        font.family: Theme.fontFamily
                        font.pixelSize: 10
                        font.weight: Font.DemiBold
                    }
                }
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
            id: playbackMessage
            objectName: "playbackMessage"
            y: 182
            width: parent.width
            text: player.bridge.player.playbackMessage
            visible: text.length > 0
            color: Theme.text
            font.family: Theme.fontFamily
            font.pixelSize: 12
            elide: Text.ElideRight
            AppToolTip { visible: messageHover.hovered; text: playbackMessage.text }
            HoverHandler { id: messageHover }
        }
    }
}
