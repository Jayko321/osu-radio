pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import QtQuick.Layouts

AppModal {
    id: root
    title: "Add to playlist"
    property var playlists: []
    property var candidates: []
    property bool adding: true
    property bool busy: false
    property bool loading: false
    property string message: ""
    property int targetId: -1
    signal targetRequested(int id)
    signal difficultyRequested(int id)
    signal addRequested()
    signal refreshRequested()
    dismissible: !busy
    initialFocus: targetMenu

    ColumnLayout {
        anchors.fill: parent
        spacing: 16
        Text {
            text: "Choose a playlist"
            color: Theme.text
            font.family: Theme.fontFamily
        }
        AppMenu {
            id: targetMenu
            objectName: "playlistTargetMenu"
            Layout.fillWidth: true
            entries: root.playlists.map(playlist => ({id: playlist.id, label: playlist.name}))
            valueRole: "id"
            currentIndex: entries.findIndex(entry => entry.id === root.targetId)
            displayText: currentIndex >= 0 ? entries[currentIndex].label : "No playlists yet"
            enabled: !root.busy && entries.length > 0
            Accessible.name: "Target playlist"
            onChosen: id => root.targetRequested(id)
        }
        Text {
            Layout.fillWidth: true
            text: root.loading ? "Loading playlists…" : root.message
            visible: text.length > 0
            color: Theme.text
            font.family: Theme.fontFamily
            wrapMode: Text.Wrap
        }
        Text {
            text: "Choose difficulties"
            color: Theme.text
            font.family: Theme.fontFamily
        }
        Basic.ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true
            ColumnLayout {
                width: parent.width
                spacing: 8
                Repeater {
                    model: root.candidates
                    Basic.CheckBox {
                        id: candidate
                        required property var modelData
                        Layout.fillWidth: true
                        text: modelData.name
                        checked: modelData.checked
                        enabled: !root.busy
                        Accessible.name: "Add difficulty " + text
                        onToggled: root.difficultyRequested(modelData.id)
                        contentItem: Text { text: candidate.text; leftPadding: candidate.indicator.width + candidate.spacing; color: Theme.text; font.family: Theme.fontFamily; verticalAlignment: Text.AlignVCenter; wrapMode: Text.Wrap }
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
            text: "Add"
            variant: "accent"
            enabled: !root.busy && root.targetId >= 0 && root.candidates.some(candidate => candidate.checked)
            onClicked: root.addRequested()
        }
    }
}
