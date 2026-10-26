pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import OsuRadio 1.0
import "components"

Item {
    required property AppBridge bridge
    id: settingsPane
    objectName: "settingsPane"
    readonly property alias searchField: settingsSearch
    readonly property string fontFamily: Theme.fontFamily
    readonly property string query: settingsSearch.text.trim().toLowerCase()
    readonly property bool foldersMatch: matches(generalSettingsHeader.text, foldersLabel.text)
    readonly property bool individualVolumeMatches: matches(audioSettingsHeader.text, individualVolumeSwitch.text)
    readonly property bool globalVolumeMatches: settingsPane.bridge.settings.individualVolumeEnabled && matches(audioSettingsHeader.text, globalVolumeLabel.settingName)
    readonly property bool individualModsMatches: matches(audioSettingsHeader.text, individualModsSwitch.text)
    readonly property bool trayMatches: matches("Window", "Close button hides to tray")
    readonly property bool discordMatches: matches("Integrations", "Discord Rich Presence")
    readonly property bool versionMatches: matches(aboutSettingsHeader.text, aboutVersion.settingName)
    readonly property bool licenseMatches: matches(aboutSettingsHeader.text, aboutLicense.settingName)
    readonly property bool projectMatches: matches(aboutSettingsHeader.text, aboutProjectLabel.text)
    readonly property bool hasMatches: foldersMatch || individualVolumeMatches || globalVolumeMatches || individualModsMatches || trayMatches || discordMatches || versionMatches || licenseMatches || projectMatches
    function matches(section: string, setting: string): bool {
        return query.length === 0 || section.toLowerCase().includes(query) || setting.toLowerCase().includes(query);
    }
    // A narrower result must remain reachable after scrolling a long status.
    onQueryChanged: Qt.callLater(() => {
        const flickable = settingsScroll.contentItem as Flickable;
        if (flickable) flickable.contentY = 0;
    })
    anchors.fill: parent
    component SectionHeader: Row {
        property string text
        property string iconName
        height: 24
        spacing: 12
        AppIcon { name: parent.iconName }
        Text {
            text: parent.text
            color: Theme.text
            font.family: settingsPane.fontFamily
            font.pixelSize: 18
            font.weight: Font.Bold
            height: parent.height
            verticalAlignment: Text.AlignVCenter
        }
    }
    component SectionCard: Rectangle {
        default property alias content: body.data
        height: body.implicitHeight + 32
        radius: 12
        color: Qt.rgba(1, 1, 1, 0.03)
        border.width: 1
        border.color: Theme.border
        Column {
            id: body
            x: 16
            y: 16
            width: parent.width - 32
            spacing: 14
        }
    }
    // Explanations are not search keys; indent 49 aligns a hint with its switch label.
    component Hint: Text {
        color: Theme.muted
        font.family: settingsPane.fontFamily
        font.pixelSize: 13
        wrapMode: Text.Wrap
    }
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
        Basic.ScrollBar.vertical: AppScrollBar {}
        Basic.ScrollBar.horizontal.policy: Basic.ScrollBar.AlwaysOff
        Column {
            id: settingsContent
            objectName: "settingsContent"
            width: settingsScroll.availableWidth
            spacing: 28
            Column {
                objectName: "generalSettingsSection"
                visible: settingsPane.foldersMatch
                width: parent.width
                spacing: 12
                SectionHeader {
                    id: generalSettingsHeader
                    objectName: "generalSettingsHeader"
                    text: "General"
                    iconName: "settings"
                }
                SectionCard {
                    width: parent.width
                    Column {
                        width: parent.width
                        spacing: 8
                        Column {
                            width: parent.width
                            spacing: 2
                            Text {
                                id: foldersLabel
                                text: "osu! folders"
                                color: Theme.text
                                font.family: settingsPane.fontFamily
                                font.pixelSize: 16
                                height: 20
                            }
                            Hint {
                                width: parent.width
                                text: "Where your beatmaps are imported from."
                            }
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
                                showToolTip: currentIndex >= 0
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
            }
            Column {
                objectName: "audioSettingsSection"
                visible: settingsPane.individualVolumeMatches || settingsPane.globalVolumeMatches || settingsPane.individualModsMatches
                width: parent.width
                spacing: 12
                SectionHeader {
                    id: audioSettingsHeader
                    objectName: "audioSettingsHeader"
                    text: "Audio"
                    iconName: "volume-up-fill"
                }
                SectionCard {
                    width: parent.width
                    Column {
                        visible: settingsPane.individualVolumeMatches
                        width: parent.width
                        spacing: 2
                        AppSwitch {
                            id: individualVolumeSwitch
                            objectName: "individualVolumeSwitch"
                            visible: settingsPane.individualVolumeMatches
                            width: parent.width
                            text: "Individual track volume"
                            font.family: settingsPane.fontFamily
                            font.pixelSize: 16
                            checked: settingsPane.bridge.settings.individualVolumeEnabled
                            enabled: settingsPane.bridge.connected && settingsPane.bridge.settings.audioSettingsLoaded
                            onToggled: settingsPane.bridge.settings.setIndividualVolumeEnabled(checked)
                        }
                        Hint {
                            width: parent.width
                            leftPadding: 49
                            text: "Each track remembers its own volume."
                        }
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
                    Column {
                        visible: settingsPane.individualModsMatches
                        width: parent.width
                        spacing: 2
                        AppSwitch {
                            id: individualModsSwitch
                            objectName: "individualModsSwitch"
                            visible: settingsPane.individualModsMatches
                            width: parent.width
                            text: "Individual track mods"
                            font.family: settingsPane.fontFamily
                            font.pixelSize: 16
                            checked: settingsPane.bridge.settings.individualModsEnabled
                            enabled: settingsPane.bridge.connected && settingsPane.bridge.settings.modsLoaded
                            onToggled: settingsPane.bridge.settings.setIndividualModsEnabled(checked)
                        }
                        Hint {
                            width: parent.width
                            leftPadding: 49
                            text: "Each track remembers its own speed."
                        }
                    }
                    Text {
                        objectName: "modsSettingsMessage"
                        width: parent.width
                        text: settingsPane.bridge.settings.modsMessage || (settingsPane.bridge.settings.modsSaving ? "Saving mod settings…" : "")
                        visible: settingsPane.individualModsMatches && text.length > 0
                        color: Theme.text
                        font.family: settingsPane.fontFamily
                        font.pixelSize: 14
                        wrapMode: Text.Wrap
                    }
                    AppButton {
                        objectName: "retryMods"
                        text: "Retry"
                        font.family: settingsPane.fontFamily
                        visible: settingsPane.individualModsMatches && settingsPane.bridge.settings.modsMessage.length > 0
                        enabled: settingsPane.bridge.connected
                        onClicked: settingsPane.bridge.settings.retryMods()
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
            Column {
                objectName: "traySettingsSection"
                visible: settingsPane.trayMatches
                width: parent.width
                spacing: 12
                SectionHeader {
                    text: "Window"
                    iconName: "layers"
                }
                SectionCard {
                    width: parent.width
                    AppSwitch {
                        objectName: "closeToTraySwitch"
                        width: parent.width
                        text: "Close button hides to tray"
                        font.family: settingsPane.fontFamily
                        font.pixelSize: 16
                        checked: settingsPane.bridge.settings.closeToTray
                        onToggled: settingsPane.bridge.settings.setCloseToTray(checked)
                    }
                    Text {
                        width: parent.width
                        text: "Keep playing in the system tray when the window is closed. Quit from the tray menu."
                        color: Theme.muted
                        font.family: settingsPane.fontFamily
                        font.pixelSize: 14
                        wrapMode: Text.Wrap
                    }
                    Text {
                        objectName: "trayPreferenceStatus"
                        width: parent.width
                        text: settingsPane.bridge.settings.trayPreferenceMessage
                        visible: text.length > 0
                        color: Theme.text
                        font.family: settingsPane.fontFamily
                        font.pixelSize: 14
                        wrapMode: Text.Wrap
                    }
                }
            }
            Column {
                objectName: "discordSettingsSection"
                visible: settingsPane.discordMatches
                width: parent.width
                spacing: 12
                SectionHeader {
                    text: "Integrations"
                    iconName: "plug"
                }
                SectionCard {
                    width: parent.width
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
            }
            Column {
                objectName: "aboutSettingsSection"
                visible: settingsPane.versionMatches || settingsPane.licenseMatches || settingsPane.projectMatches
                width: parent.width
                spacing: 12
                SectionHeader {
                    id: aboutSettingsHeader
                    objectName: "aboutSettingsHeader"
                    text: "About"
                    iconName: "info"
                }
                SectionCard {
                    width: parent.width
                    Text {
                        id: aboutVersion
                        objectName: "aboutVersion"
                        readonly property string settingName: "Version"
                        visible: settingsPane.versionMatches
                        width: parent.width
                        text: settingName + " — " + Qt.application.version
                        color: Theme.text
                        font.family: settingsPane.fontFamily
                        font.pixelSize: 16
                        wrapMode: Text.Wrap
                    }
                    Text {
                        id: aboutLicense
                        objectName: "aboutLicense"
                        readonly property string settingName: "License"
                        visible: settingsPane.licenseMatches
                        width: parent.width
                        text: settingName + " — MIT"
                        color: Theme.text
                        font.family: settingsPane.fontFamily
                        font.pixelSize: 16
                        wrapMode: Text.Wrap
                    }
                    Column {
                        objectName: "aboutProjectSetting"
                        visible: settingsPane.projectMatches
                        width: parent.width
                        spacing: 8
                        Text {
                            id: aboutProjectLabel
                            text: "Project"
                            color: Theme.text
                            font.family: settingsPane.fontFamily
                            font.pixelSize: 16
                        }
                        AppButton {
                            objectName: "aboutProjectLink"
                            text: "https://github.com/Jayko321/osu-radio"
                            variant: "link"
                            width: parent.width
                            font.family: settingsPane.fontFamily
                            Accessible.name: aboutProjectLabel.text + ": " + text
                            onClicked: Qt.openUrlExternally(text)
                        }
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
