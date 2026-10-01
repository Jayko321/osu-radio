pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import QtQuick.Layouts

AppModal {
    id: root
    title: adding ? "В плейлист" : "Playlists"
    property var playlists: []
    property var candidates: []
    property bool adding: false
    property bool busy: false
    property bool loading: false
    property string message: ""
    property int targetId: -1
    property int editingId: -1
    signal createRequested(string name)
    signal renameRequested(int id, string name)
    signal deleteRequested(int id)
    signal selectRequested(int id)
    signal targetRequested(int id)
    signal difficultyRequested(int id)
    signal addRequested()
    signal refreshRequested()
    dismissible: !busy
    initialFocus: nameField
    onOpened: { editingId = -1; nameField.text = ""; }

    ColumnLayout {
        anchors.fill: parent
        spacing: 12
        RowLayout {
            Layout.fillWidth: true
            AppField {
                id: nameField
                objectName: "playlistNameField"
                Layout.fillWidth: true
                placeholderText: root.editingId >= 0 ? "New playlist name" : "My playlist"
                Accessible.name: "Playlist name"
                enabled: !root.busy
                onAccepted: if (saveName.enabled) saveName.clicked()
            }
            AppButton {
                id: saveName
                objectName: "savePlaylistName"
                text: root.editingId >= 0 ? "Rename" : "Create"
                enabled: !root.busy && nameField.text.trim().length > 0
                onClicked: {
                    if (root.editingId >= 0) root.renameRequested(root.editingId, nameField.text);
                    else root.createRequested(nameField.text);
                }
            }
            AppButton {
                text: "New"
                visible: root.editingId >= 0
                enabled: !root.busy
                onClicked: { root.editingId = -1; nameField.text = ""; nameField.forceActiveFocus(); }
            }
        }
        Text {
            Layout.fillWidth: true
            text: root.loading ? "Loading playlists…" : root.message
            visible: text.length > 0
            color: Theme.text
            font.family: Theme.fontFamily
            wrapMode: Text.Wrap
        }
        Basic.ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true
            ColumnLayout {
                width: parent.width
                spacing: 8
                Text {
                    text: "No playlists yet. Create one above."
                    visible: root.playlists.length === 0 && !root.loading
                    color: Theme.muted
                    font.family: Theme.fontFamily
                }
                Repeater {
                    model: root.playlists
                    RowLayout {
                        id: row
                        required property var modelData
                        Layout.fillWidth: true
                        AppButton {
                            Layout.fillWidth: true
                            text: row.modelData.name
                            variant: root.adding && root.targetId === row.modelData.id ? "accent" : "link"
                            enabled: !root.busy
                            Accessible.name: root.adding ? "Choose " + text : "Open " + text
                            onClicked: {
                                if (root.adding) root.targetRequested(row.modelData.id);
                                else root.selectRequested(row.modelData.id);
                            }
                        }
                        AppButton {
                            text: "Rename"
                            enabled: !root.busy
                            onClicked: { root.editingId = row.modelData.id; nameField.text = row.modelData.name; nameField.forceActiveFocus(); }
                        }
                        IconButton {
                            iconName: "minus"
                            accessibleName: "Delete " + row.modelData.name
                            enabled: !root.busy
                            onClicked: root.deleteRequested(row.modelData.id)
                        }
                    }
                }
                Text {
                    text: "Choose difficulties"
                    visible: root.adding
                    color: Theme.text
                    font.family: Theme.fontFamily
                }
                Repeater {
                    model: root.adding ? root.candidates : []
                    Basic.CheckBox {
                        id: candidate
                        required property var modelData
                        text: modelData.name
                        checked: modelData.checked
                        enabled: !root.busy
                        Accessible.name: "Add difficulty " + text
                        onToggled: root.difficultyRequested(modelData.id)
                        contentItem: Text { text: candidate.text; leftPadding: candidate.indicator.width + candidate.spacing; color: Theme.text; font.family: Theme.fontFamily; verticalAlignment: Text.AlignVCenter }
                    }
                }
            }
        }
    }
    footer: Row {
        anchors.right: parent.right
        spacing: 12
        AppButton { text: "Refresh"; enabled: !root.busy && !root.loading; onClicked: root.refreshRequested() }
        AppButton {
            objectName: "confirmPlaylistAdd"
            text: "Добавить"
            visible: root.adding
            variant: "accent"
            enabled: !root.busy && root.targetId >= 0 && root.candidates.some(candidate => candidate.checked)
            onClicked: root.addRequested()
        }
    }
}
