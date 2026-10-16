pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic

Basic.Menu {
    id: contextMenu
    required property Item row
    property bool canInsert: true
    property bool showRemove: false
    property bool canRemove: true
    signal insertRequested(bool next)
    signal removeRequested()
    width: 252
    padding: 6
    function openAt(position: point): void {
        row.forceActiveFocus();
        popup(row, position);
    }
    function handleKey(event: KeyEvent): void {
        if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
            openAt(Qt.point(0, row.height));
            event.accepted = true;
        }
    }
    onOpened: {
        for (let i = 0; i < count; ++i) {
            if (itemAt(i).visible && itemAt(i).enabled) {
                currentIndex = i;
                itemAt(i).forceActiveFocus();
                break;
            }
        }
    }
    onClosed: row.forceActiveFocus()
    background: Rectangle { color: "#161616"; radius: 12; border.color: "#333333" }
    component ActionItem: Basic.MenuItem {
        id: action
        required property string iconName
        implicitHeight: 36
        leftPadding: 16
        rightPadding: 16
        font.family: Theme.fontFamily
        font.pixelSize: 16
        Accessible.name: text
        contentItem: Item {
            opacity: action.enabled ? 1 : 0.4
            Text {
                anchors.left: parent.left
                anchors.right: actionIcon.left
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                text: action.text
                font: action.font
                color: Theme.text
                elide: Text.ElideRight
            }
            AppIcon {
                id: actionIcon
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                name: action.iconName
                size: 16
            }
        }
        background: Rectangle { radius: 6; color: action.highlighted ? "#14f2f4fc" : "transparent" }
    }
    ActionItem {
        objectName: "appendQueueTrack"
        text: "Add to end"
        iconName: "add-to-queue"
        enabled: contextMenu.canInsert
        onTriggered: contextMenu.insertRequested(false)
    }
    ActionItem {
        objectName: "insertQueueTrackNext"
        text: "Play next"
        iconName: "skip-forward"
        enabled: contextMenu.canInsert
        onTriggered: contextMenu.insertRequested(true)
    }
    ActionItem {
        objectName: "removePlaylistTrack"
        text: "Remove from playlist"
        iconName: "playlist-delete"
        visible: contextMenu.showRemove
        height: visible ? implicitHeight : 0
        enabled: visible && contextMenu.canRemove
        onTriggered: contextMenu.removeRequested()
    }
}
