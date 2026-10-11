pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import OsuRadio 1.0
import "components"

Item {
    required property AppBridge bridge
    required property var shell
    id: songsPane
    objectName: "songsPane"
    anchors.fill: parent
    AppField {
        id: search
        objectName: "songSearch"
        x: 20
        y: 32
        width: parent.width - 40
        leftPadding: 44
        placeholderText: "Type to search songs..."
        Accessible.name: "Search songs"
        onTextChanged: songsPane.bridge.library.searchLibrary(text)
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
            currentIndex: songsPane.bridge.library.trackSortIndex
            onChosen: index => songsPane.bridge.library.setTrackSort(index)
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
        text: songsPane.bridge.library.libraryMessage
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
        height: Math.max(0, libraryFooter.y - y - 12)
        clip: true
        spacing: 16
        model: songsPane.bridge.library.tracks
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
            onAudioIdChanged: songsPane.shell.scheduleVisibleMedia()
            onInViewportChanged: songsPane.shell.scheduleVisibleMedia()
            onArtworkUrlChanged: if (inViewport && artworkUrl.length === 0) songsPane.shell.scheduleVisibleMedia()
            Component.onCompleted: songsPane.shell.addMediaDelegate(card)
            Component.onDestruction: songsPane.shell.removeMediaDelegate(card)
            width: songList.width
            height: 90
            padding: 0
            Accessible.name: title + ", " + artist
            onClicked: songsPane.bridge.library.selectTrack(audioId)
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
                    border.color: songsPane.bridge.library.selectedAudioId === card.audioId ? Theme.accent
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
        id: libraryFooter
        objectName: "refreshLibrary"
        x: 20
        y: parent.height - height - 20
        height: 36
        text: "Refresh library"
        iconName: "rotate-cw"
        enabled: songsPane.bridge.connected && !songsPane.bridge.library.libraryLoading
        onClicked: songsPane.bridge.library.refreshLibrary()
        Accessible.name: "Refresh library"
    }
}
