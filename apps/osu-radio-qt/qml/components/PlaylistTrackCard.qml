pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic

Basic.Button {
    id: card
    required property int audioId
    required property int playlistItemId
    required property bool available
    required property string title
    required property string artist
    required property string subtitle
    required property string durationLabel
    required property string artworkUrl
    property bool selected: false
    property bool busy: false
    property bool canInsert: true
    property bool canPlay: true
    property bool canAddToPlaylist: true
    signal playRequested()
    signal insertRequested(bool next)
    signal addToPlaylistRequested()
    signal removeRequested(int itemId)
    height: 76
    padding: 12
    leftPadding: 16
    rightPadding: 16
    Accessible.name: title + ", " + subtitle
    Keys.onPressed: event => itemMenu.handleKey(event)
    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: itemMenu.openAt(point.position)
    }
    TrackQueueMenu {
        id: itemMenu
        objectName: "playlistItemMenu"
        row: card
        canPlay: card.canPlay && card.available && !card.busy
        canInsert: card.canInsert && !card.busy
        canAddToPlaylist: card.canAddToPlaylist && card.available && !card.busy
        showRemove: true
        canRemove: !card.busy
        onPlayRequested: card.playRequested()
        onInsertRequested: next => card.insertRequested(next)
        onAddToPlaylistRequested: card.addToPlaylistRequested()
        onRemoveRequested: card.removeRequested(card.playlistItemId)
    }
    background: Item {
        Artwork { anchors.fill: parent; placeholder: false; source: card.artworkUrl; imageOpacity: card.available ? 1 : 0.5 }
        Rectangle {
            anchors.fill: parent
            radius: 8
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0; color: card.selected ? "#f0241c38" : "#ed0e0e0e" }
                GradientStop { position: 1; color: "#660e0e0e" }
            }
        }
        Rectangle {
            anchors.fill: parent
            radius: 8
            color: "transparent"
            border.width: card.activeFocus ? 2 : 1
            border.color: card.selected ? Theme.accent : card.activeFocus || card.hovered ? Theme.border : Qt.alpha(Theme.border, 0)
            Behavior on border.color { ColorAnimation { duration: Theme.ms(Theme.motionFast); easing.type: Theme.easing } }
        }
    }
    contentItem: Item {
        Text {
            id: duration
            anchors.right: parent.right
            y: subtitleText.y
            text: card.durationLabel
            color: card.available ? Theme.text : Theme.muted
            opacity: card.available ? 0.8 : 1
            font.family: Theme.fontFamily
            font.pixelSize: 14
            font.features: { "tnum": 1 }
        }
        Text {
            width: parent.width
            text: card.title
            color: Theme.text
            font.family: Theme.fontFamily
            font.pixelSize: 19
            font.weight: Font.Bold
            elide: Text.ElideRight
        }
        Text {
            id: subtitleText
            y: 24
            width: parent.width - duration.width - 12
            text: card.subtitle
            color: card.available ? Theme.text : Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: 13
            elide: Text.ElideRight
        }
    }
}
