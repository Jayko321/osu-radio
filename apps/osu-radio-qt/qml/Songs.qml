pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import QtQuick.Effects
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
    title: selectedTab === 0 ? "osu! radio — Songs" : "osu! radio — Settings"
    color: Theme.background
    flags: Qt.Window | Qt.FramelessWindowHint
    font.family: Theme.fontFamily

    property int selectedTab: 0
    readonly property alias appBridge: bridge
    AppBridge {
        id: bridge
        objectName: "appBridge"
        Component.onCompleted: connectSession()
        onSelectedArtworkUrlChanged: if (hasSelection && selectedArtworkUrl.length === 0) requestMedia(selectedAudioId)
    }

    component Artwork: Item {
        id: art
        property url source
        property real radius: 8
        Rectangle { anchors.fill: parent; radius: art.radius; color: Theme.surface }
        Image {
            id: image
            anchors.fill: parent
            source: art.source
            cache: false
            fillMode: Image.PreserveAspectCrop
            horizontalAlignment: Image.AlignHCenter
            verticalAlignment: Image.AlignVCenter
            smooth: true
            visible: false
        }
        Rectangle {
            id: mask
            anchors.fill: parent
            radius: art.radius
            color: "white"
            layer.enabled: true
            visible: false
        }
        MultiEffect {
            anchors.fill: parent
            source: image
            maskEnabled: true
            maskSource: mask
            maskThresholdMin: 0.5
            maskSpreadAtMin: 1.0
        }
    }

    WindowBar {
        id: titleBar
        width: parent.width
        window: root
        selectedTab: root.selectedTab
        onTabSelected: index => root.selectedTab = index
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
                    AppIcon { x: 12; anchors.verticalCenter: parent.verticalCenter; name: "search"; color: Theme.muted }
                }
                Row {
                    id: filters
                    x: 20
                    y: search.y + search.height + 16
                    spacing: 10
                    Repeater {
                        model: ["Title", "All musics", "Tags"]
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
                        required property string title
                        required property string artist
                        required property string subtitle
                        required property string durationLabel
                        required property string artworkUrl
                        readonly property bool inViewport: songsPane.visible
                            && y + height >= songList.contentY
                            && y <= songList.contentY + songList.height
                        onInViewportChanged: if (inViewport) bridge.requestMedia(audioId)
                        onArtworkUrlChanged: if (inViewport && artworkUrl.length === 0) bridge.requestMedia(audioId)
                        Component.onCompleted: if (inViewport) bridge.requestMedia(audioId)
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

            Item {
                id: settingsPane
                objectName: "settingsPane"
                anchors.fill: parent
                visible: root.selectedTab === 1
                AppField {
                    id: settingsSearch
                    objectName: "settingsSearch"
                    x: 20
                    y: 32
                    width: parent.width - 40
                    leftPadding: 44
                    placeholderText: "Type to search settings..."
                    Accessible.name: "Search settings"
                    AppIcon { x: 12; anchors.verticalCenter: parent.verticalCenter; name: "search"; color: Theme.muted }
                }
                Basic.ScrollView {
                    id: settingsScroll
                    x: 20
                    y: settingsSearch.y + settingsSearch.height + 32
                    width: parent.width - 40
                    height: parent.height - y - 20
                    contentWidth: availableWidth
                    clip: true
                    Basic.ScrollBar.horizontal.policy: Basic.ScrollBar.AlwaysOff
                    Column {
                        width: settingsScroll.availableWidth
                        spacing: 16
                        Text {
                            text: "General"
                            color: Theme.text
                            font.family: Theme.fontFamily
                            font.pixelSize: 24
                            font.weight: Font.DemiBold
                        }
                        Text {
                            text: "osu! folders"
                            color: Theme.text
                            font.family: Theme.fontFamily
                            font.pixelSize: 16
                        }
                        Row {
                            width: parent.width
                            spacing: 12
                            AppMenu {
                                id: folderMenu
                                objectName: "folderMenu"
                                width: parent.width - addFolder.width - parent.spacing
                                entries: bridge.folders
                                valueRole: "id"
                                currentIndex: entries.findIndex(entry => entry.id === bridge.selectedFolderId)
                                displayText: currentIndex >= 0 ? entries[currentIndex].label : "No osu! folders"
                                enabled: bridge.connected && !bridge.foldersLoading && !bridge.folderBusy && entries.length > 0
                                Accessible.name: "osu! folders"
                                Basic.ToolTip.visible: hovered && currentIndex >= 0
                                onChosen: value => bridge.selectFolder(value)
                            }
                            IconButton {
                                id: addFolder
                                objectName: "addFolder"
                                iconName: "plus"
                                accessibleName: "Add osu! folder"
                                enabled: bridge.connected && !bridge.connecting && !bridge.foldersLoading && !bridge.folderBusy
                                onClicked: bridge.addFolder()
                            }
                        }
                        Text {
                            objectName: "folderStatus"
                            width: parent.width
                            text: bridge.folderMessage
                            visible: text.length > 0
                            color: Theme.text
                            font.family: Theme.fontFamily
                            font.pixelSize: 14
                            wrapMode: Text.Wrap
                        }
                        AppButton {
                            objectName: "retryFolders"
                            text: "Retry"
                            visible: bridge.folderCanRetry
                            enabled: bridge.connected && !bridge.foldersLoading && !bridge.folderBusy
                            onClicked: bridge.retryFolders()
                        }
                    }
                }
            }
        }

        Item {
            id: player
            x: sidebar.width
            width: parent.width - x
            height: parent.height
            readonly property real geometryScale: Math.max(width / 960, 1)
            readonly property real contentWidth: Math.min(650 * geometryScale, 960, width * 0.85)
            readonly property real contentLeft: (width - contentWidth) * 138 / 310
            readonly property real coverSize: Math.min(340 * geometryScale, 640, contentWidth, Math.max(height - 225, 0))

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
                    value: 0
                    enabled: false
                    padding: 0
                    Accessible.name: "Seeking (unavailable)"
                    background: Rectangle { y: 6; width: progress.width; height: 4; radius: 2; color: Theme.muted }
                    handle: Rectangle { x: -8; width: 16; height: 16; radius: 8; color: Theme.accent }
                }
                Text {
                    y: 101
                    text: "00:00"
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
                    IconButton { anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter; iconName: "volume-2"; accessibleName: "Volume (unavailable)"; enabled: false }
                    Row {
                        anchors.centerIn: parent
                        spacing: 28
                        IconButton { anchors.verticalCenter: parent.verticalCenter; iconName: "shuffle"; accessibleName: "Shuffle (unavailable)"; enabled: false }
                        IconButton { anchors.verticalCenter: parent.verticalCenter; iconName: "skip-back"; accessibleName: "Previous track (unavailable)"; enabled: false }
                        IconButton {
                            width: 48
                            height: 48
                            iconName: "play"
                            accessibleName: "Play (unavailable)"
                            enabled: false
                            background: Rectangle { radius: 24; color: Theme.accent }
                        }
                        IconButton { anchors.verticalCenter: parent.verticalCenter; iconName: "skip-forward"; accessibleName: "Next track (unavailable)"; enabled: false }
                        IconButton { anchors.verticalCenter: parent.verticalCenter; iconName: "repeat-2"; accessibleName: "Repeat (unavailable)"; enabled: false }
                    }
                    IconButton { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; iconName: "circle-plus"; accessibleName: "Add to playlist (unavailable)"; enabled: false }
                }
            }
        }
    }
}
