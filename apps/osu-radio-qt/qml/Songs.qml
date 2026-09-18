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
    title: "osu! radio — Songs"
    color: Theme.background
    flags: Qt.Window | Qt.FramelessWindowHint
    font.family: Theme.fontFamily

    Store { id: store }
    readonly property var selectedTrack: store.state.tracks[store.state.selected]
    readonly property var coverNames: ["karakara.jpg", "alice.jpg", "rabbit.jpg", "bbbb.png"]
    readonly property var cardTints: ["#eb04061a", "#eb1a1804", "#d11a0415", "#eb1a0404"]
    function coverFor(index: int): string { return "qrc:/assets/covers/" + coverNames[index]; }
    function duration(seconds: int): string {
        return Math.floor(seconds / 60).toString().padStart(2, "0") + ":"
            + (seconds % 60).toString().padStart(2, "0");
    }

    component Artwork: Item {
        id: art
        property url source
        property real radius: 8
        Image {
            id: image
            anchors.fill: parent
            source: art.source
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

    WindowBar { id: titleBar; width: parent.width; window: root }

    Item {
        id: body
        anchors.top: titleBar.bottom
        anchors.bottom: parent.bottom
        width: parent.width
        clip: true

        Item {
            id: backdrop
            anchors.fill: parent
            Image {
                id: backdropImage
                anchors.fill: parent
                source: root.coverFor(store.state.selected)
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
            AppField {
                id: search
                objectName: "songSearch"
                x: 20
                y: 32
                width: parent.width - 40
                leftPadding: 44
                placeholderText: "Type to search songs..."
                text: store.state.search
                onTextEdited: store.send("search", text)
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
            ListView {
                id: songList
                objectName: "songList"
                x: 20
                y: filters.y + filters.height + 32
                width: parent.width - 40
                height: footer.y - y - 12
                clip: true
                spacing: 16
                model: store.state.tracks
                currentIndex: store.state.selected
                boundsBehavior: Flickable.StopAtBounds
                Basic.ScrollBar.vertical: Basic.ScrollBar { }
                delegate: Basic.Button {
                    id: card
                    required property var modelData
                    required property int index
                    width: songList.width
                    height: 90
                    padding: 0
                    Accessible.name: modelData.title + ", " + modelData.artist
                    onClicked: store.send("selectTrack", index)
                    background: Item {
                        Artwork { anchors.fill: parent; source: root.coverFor(card.index) }
                        Rectangle {
                            anchors.fill: parent
                            radius: 8
                            gradient: Gradient {
                                orientation: Gradient.Horizontal
                                GradientStop { position: card.index === 2 ? 0 : 0.415; color: root.cardTints[card.index] }
                                GradientStop { position: 1; color: "transparent" }
                            }
                        }
                        Rectangle {
                            anchors.fill: parent
                            radius: 8
                            color: "transparent"
                            border.width: card.activeFocus ? 2 : 1
                            border.color: store.state.selected === card.index ? Theme.accent
                                : (card.hovered || card.activeFocus ? Theme.border : "transparent")
                        }
                    }
                    contentItem: Item {
                        Text {
                            x: 20
                            y: 17
                            width: parent.width - 40
                            height: 30
                            text: card.modelData.title
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
                            text: card.modelData.subtitle.length > 0
                                ? card.modelData.artist + " | " + card.modelData.subtitle : card.modelData.artist
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
                x: 20
                y: parent.height - height - 20
                height: 36
                text: "Refresh library"
                iconName: "rotate-cw"
                enabled: false
                Accessible.name: "Refresh library (unavailable)"
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
                    source: root.coverFor(store.state.selected)
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
                    text: root.selectedTrack.title
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
                    text: root.selectedTrack.artist
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
                    text: root.duration(root.selectedTrack.duration_seconds)
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
