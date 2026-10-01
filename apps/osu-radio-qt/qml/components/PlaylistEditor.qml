pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import QtQuick.Layouts

Rectangle {
    id: root
    property string name: ""
    property url artworkUrl
    property bool creating: true
    property bool busy: false
    property bool preparing: false
    signal nameEdited(string name)
    signal coverRequested()
    signal coverResetRequested()
    signal saveRequested()
    signal cancelRequested()
    implicitHeight: editor.implicitHeight + 32
    color: "#260e0e0e"
    radius: 12
    border.color: Theme.border
    onVisibleChanged: if (visible) nameField.forceActiveFocus()

    ColumnLayout {
        id: editor
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: 16
        spacing: 12
        RowLayout {
            Layout.fillWidth: true
            Text {
                Layout.fillWidth: true
                text: root.creating ? "New playlist" : "Edit playlist"
                color: Theme.text
                font.family: Theme.fontFamily
                font.pixelSize: 18
                font.weight: Font.DemiBold
            }
            IconButton {
                objectName: "closePlaylistEditor"
                Layout.preferredWidth: 28
                Layout.preferredHeight: 28
                iconName: "x"
                accessibleName: "Close playlist editor"
                enabled: !root.busy
                onClicked: root.cancelRequested()
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 12
            Basic.Button {
                id: cover
                objectName: "choosePlaylistCover"
                Layout.preferredWidth: 80
                Layout.preferredHeight: 80
                padding: 0
                enabled: !root.busy
                Accessible.name: "Choose playlist cover"
                onClicked: root.coverRequested()
                background: Item {
                    Artwork { anchors.fill: parent; source: root.artworkUrl }
                    Rectangle { anchors.fill: parent; color: "transparent"; radius: 8; border.color: cover.activeFocus ? Theme.text : Theme.border }
                }
                contentItem: Item {
                    AppIcon { anchors.centerIn: parent; name: "plus"; visible: root.artworkUrl.toString().length === 0 }
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 8
                AppField {
                    id: nameField
                    objectName: "playlistEditorName"
                    Layout.fillWidth: true
                    placeholderText: "Playlist name"
                    text: root.name
                    enabled: !root.busy
                    Accessible.name: "Playlist name"
                    onTextEdited: root.nameEdited(text)
                    onAccepted: if (save.enabled) root.saveRequested()
                }
                AppButton {
                    objectName: "resetPlaylistCover"
                    text: "Use automatic cover"
                    variant: "link"
                    Layout.preferredHeight: 28
                    enabled: !root.busy
                    onClicked: root.coverResetRequested()
                }
            }
        }
        Text {
            objectName: "playlistCoverPreparationStatus"
            Layout.fillWidth: true
            visible: root.preparing
            text: "Preparing cover…"
            color: Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: 13
        }
        AppButton {
            id: save
            objectName: "savePlaylistEditor"
            Layout.fillWidth: true
            text: root.busy ? "Saving…" : root.creating ? "Create" : "Save"
            variant: "light"
            enabled: !root.busy && !root.preparing && root.name.trim().length > 0
            onClicked: root.saveRequested()
        }
    }
}
