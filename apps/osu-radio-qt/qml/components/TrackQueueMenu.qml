pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic

AppContextMenu {
    id: contextMenu
    required property Item row
    property bool canPlay: true
    property bool canInsert: true
    property bool canAddToPlaylist: true
    property bool showRemove: false
    property bool canRemove: true
    signal playRequested()
    signal insertRequested(bool next)
    signal addToPlaylistRequested()
    signal removeRequested()
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
    component Divider: Basic.MenuSeparator {
        enabled: false
        padding: 4
        topPadding: 4
        bottomPadding: 4
        contentItem: Rectangle { implicitHeight: 1; color: Theme.border }
    }
    AppMenuItem {
        objectName: "playQueueTrack"
        text: "Play"
        iconName: "play"
        enabled: contextMenu.canPlay
        onTriggered: contextMenu.playRequested()
    }
    AppMenuItem {
        objectName: "appendQueueTrack"
        text: "Add to end"
        iconName: "add-to-queue"
        enabled: contextMenu.canInsert
        onTriggered: contextMenu.insertRequested(false)
    }
    AppMenuItem {
        objectName: "insertQueueTrackNext"
        text: "Play next"
        iconName: "skip-forward"
        enabled: contextMenu.canInsert
        onTriggered: contextMenu.insertRequested(true)
    }
    Divider {}
    AppMenuItem {
        objectName: "addTrackToPlaylist"
        text: "Add to playlist…"
        iconName: "list-plus"
        enabled: contextMenu.canAddToPlaylist
        onTriggered: contextMenu.addToPlaylistRequested()
    }
    Divider {
        visible: contextMenu.showRemove
        height: visible ? implicitHeight : 0
    }
    AppMenuItem {
        objectName: "removePlaylistTrack"
        text: "Remove from playlist"
        iconName: "playlist-delete"
        destructive: true
        visible: contextMenu.showRemove
        height: visible ? implicitHeight : 0
        enabled: visible && contextMenu.canRemove
        onTriggered: contextMenu.removeRequested()
    }
}
