pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import OsuRadio 1.0
import "components"

Item {
    required property AppBridge bridge
    id: settingsPane
    objectName: "settingsPane"
    readonly property string fontFamily: Theme.fallbackFont.name || "Nunito"
    readonly property string query: settingsSearch.text.trim().toLowerCase()
    readonly property bool foldersMatch: matches(generalSettingsHeader.text, foldersLabel.text)
    readonly property bool individualVolumeMatches: matches(audioSettingsHeader.text, individualVolumeSwitch.text)
    readonly property bool globalVolumeMatches: settingsPane.bridge.settings.individualVolumeEnabled && matches(audioSettingsHeader.text, globalVolumeLabel.settingName)
    readonly property bool discordMatches: matches("Integrations", "Discord Rich Presence")
    readonly property bool hasMatches: foldersMatch || individualVolumeMatches || globalVolumeMatches || discordMatches
    function matches(section: string, setting: string): bool {
        return query.length === 0 || section.toLowerCase().includes(query) || setting.toLowerCase().includes(query);
    }
    // A narrower result must remain reachable after scrolling a long status.
    onQueryChanged: Qt.callLater(() => {
        const flickable = settingsScroll.contentItem as Flickable;
        if (flickable) flickable.contentY = 0;
    })
    anchors.fill: parent
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
                            entries: settingsPane.bridge.settings.folders
                            valueRole: "id"
                            currentIndex: entries.findIndex(entry => entry.id === settingsPane.bridge.settings.selectedFolderId)
                            displayText: currentIndex >= 0 ? entries[currentIndex].label : "No osu! folders"
                            enabled: settingsPane.bridge.connected && !settingsPane.bridge.settings.foldersLoading && !settingsPane.bridge.settings.folderBusy && entries.length > 0
                            Accessible.name: "osu! folders"
                            Basic.ToolTip.visible: hovered && currentIndex >= 0
                            onChosen: value => settingsPane.bridge.settings.selectFolder(value)
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
                            enabled: settingsPane.bridge.connected && !settingsPane.bridge.connecting && !settingsPane.bridge.settings.foldersLoading && !settingsPane.bridge.settings.folderBusy
                            onClicked: { addFolder.forceActiveFocus(); settingsPane.bridge.settings.addFolder(); }
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
                        text: settingsPane.bridge.settings.folderMessage
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
                        visible: settingsPane.bridge.settings.folderCanRetry
                        enabled: settingsPane.bridge.connected && !settingsPane.bridge.settings.foldersLoading && !settingsPane.bridge.settings.folderBusy
                        onClicked: settingsPane.bridge.settings.retryFolders()
                    }
                }
                AppSwitch {
                    objectName: "unicodeTitlesSwitch"
                    width: parent.width
                    text: "Use Unicode track titles"
                    font.family: settingsPane.fontFamily
                    font.pixelSize: 16
                    checked: settingsPane.bridge.settings.useUnicodeTitles
                    onToggled: settingsPane.bridge.settings.setUseUnicodeTitles(checked)
                }
                AppSwitch {
                    objectName: "unicodeArtistsSwitch"
                    width: parent.width
                    text: "Use Unicode artist names"
                    font.family: settingsPane.fontFamily
                    font.pixelSize: 16
                    checked: settingsPane.bridge.settings.useUnicodeArtists
                    onToggled: settingsPane.bridge.settings.setUseUnicodeArtists(checked)
                }
                Text {
                    objectName: "trackNamePreferencesStatus"
                    width: parent.width
                    text: settingsPane.bridge.settings.trackNamePreferencesMessage
                    visible: text.length > 0
                    color: Theme.text
                    font.family: settingsPane.fontFamily
                    font.pixelSize: 14
                    wrapMode: Text.Wrap
                }
            }
            Column {
                objectName: "discordSettingsSection"
                visible: settingsPane.discordMatches
                width: parent.width
                spacing: 16
                Text {
                    text: "Integrations"
                    color: Theme.text
                    font.family: settingsPane.fontFamily
                    font.pixelSize: 20
                    font.bold: true
                }
                AppSwitch {
                    objectName: "discordRichPresenceSwitch"
                    width: parent.width
                    text: "Discord Rich Presence"
                    font.family: settingsPane.fontFamily
                    font.pixelSize: 16
                    checked: settingsPane.bridge.settings.discordRichPresence
                    onToggled: settingsPane.bridge.settings.setDiscordRichPresence(checked)
                }
                Text {
                    width: parent.width
                    text: "Show the currently playing track on your Discord profile."
                    color: Theme.muted
                    font.family: settingsPane.fontFamily
                    font.pixelSize: 14
                    wrapMode: Text.Wrap
                }
                Text {
                    objectName: "discordPreferenceStatus"
                    width: parent.width
                    text: settingsPane.bridge.settings.discordPreferenceMessage
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
                        checked: settingsPane.bridge.settings.individualVolumeEnabled
                        enabled: settingsPane.bridge.connected && settingsPane.bridge.settings.audioSettingsLoaded
                        onToggled: settingsPane.bridge.settings.setIndividualVolumeEnabled(checked)
                    }
                    Column {
                        objectName: "globalVolumeSetting"
                        width: parent.width
                        spacing: 8
                        visible: settingsPane.globalVolumeMatches
                        Text {
                            id: globalVolumeLabel
                            readonly property string settingName: "Global volume"
                            text: settingName + " — " + settingsPane.bridge.settings.globalVolumePercent + "%"
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
                            value: settingsPane.bridge.settings.globalVolumePercent
                            enabled: settingsPane.bridge.settings.audioSettingsLoaded
                            Accessible.name: "Global volume"
                            onMoved: settingsPane.bridge.settings.setGlobalVolume(Math.round(value))
                        }
                    }
                    Text {
                        objectName: "audioSettingsMessage"
                        width: parent.width
                        text: settingsPane.bridge.settings.audioSettingsMessage
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
                        visible: settingsPane.bridge.settings.audioSettingsCanRetry
                        enabled: settingsPane.bridge.connected
                        onClicked: settingsPane.bridge.settings.retryAudioSettings()
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
