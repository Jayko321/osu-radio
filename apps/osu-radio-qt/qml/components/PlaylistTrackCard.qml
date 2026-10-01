pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import QtQuick.Effects

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
    signal removeRequested(int itemId)
    height: 76
    padding: 12
    leftPadding: 16
    rightPadding: 16
    Accessible.name: title + ", " + subtitle
    Keys.onPressed: event => {
        if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
            itemMenu.popup(card, Qt.point(0, card.height), itemMenu.itemAt(0));
            event.accepted = true;
        }
    }
    TapHandler {
        acceptedButtons: Qt.RightButton
        onTapped: { card.forceActiveFocus(); itemMenu.popup(card, point.position, itemMenu.itemAt(0)); }
    }
    Basic.Menu {
        id: itemMenu
        objectName: "playlistItemMenu"
        onOpened: { currentIndex = 0; itemAt(0).forceActiveFocus(); }
        onClosed: card.forceActiveFocus()
        Basic.MenuItem {
            objectName: "removePlaylistTrack"
            text: "Remove from playlist"
            enabled: !card.busy
            onTriggered: card.removeRequested(card.playlistItemId)
        }
    }
    background: Item {
        Artwork { anchors.fill: parent; source: card.artworkUrl; imageOpacity: card.available ? 1 : 0.5 }
        Rectangle {
            anchors.fill: parent
            radius: 8
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0; color: "#ed0e0e0e" }
                GradientStop { position: 1; color: "#660e0e0e" }
            }
        }
        MultiEffect {
            anchors.fill: selectedFrame
            source: selectedFrame
            visible: card.selected
            shadowEnabled: true
            shadowColor: "#69c9ff"
            shadowBlur: 0.7
            shadowOpacity: 0.8
            shadowHorizontalOffset: 0
            shadowVerticalOffset: 0
            autoPaddingEnabled: true
        }
        Rectangle {
            id: selectedFrame
            anchors.fill: parent
            radius: 8
            color: "transparent"
            border.width: card.selected || card.activeFocus ? 2 : 1
            border.color: card.selected ? "white" : card.activeFocus || card.hovered ? Theme.border : "transparent"
        }
    }
    contentItem: Column {
        spacing: 2
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
            width: parent.width
            text: card.subtitle
            color: card.available ? Theme.text : Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: 13
            elide: Text.ElideRight
        }
    }
}
