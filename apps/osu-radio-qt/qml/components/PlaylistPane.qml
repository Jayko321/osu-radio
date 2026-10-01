pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import QtQuick.Layouts

Item {
    id: root
    property var playlists: []
    property var tracksModel
    property Component trackDelegate
    property int activeId: -1
    property string activeName: ""
    property string query: ""
    property int trackCount: 0
    property int sortIndex: 0
    property bool busy: false
    property bool coverPreparing: false
    property bool loading: false
    property bool connected: true
    property string message: ""
    property bool editorOpen: false
    property int editorId: -1
    property string editorName: ""
    property url editorArtworkUrl
    readonly property alias tracksList: tracks
    signal searchEdited(string query)
    signal createRequested()
    signal editRequested(int id)
    signal selectRequested(int id)
    signal queueRequested(int id)
    signal deleteRequested(int id)
    signal requestArtwork(int id)
    signal nameEdited(string name)
    signal coverRequested()
    signal coverResetRequested()
    signal saveRequested()
    signal cancelRequested()
    signal refreshRequested()
    signal sortChosen(int index)

    ColumnLayout {
        id: controls
        x: 20
        y: 28
        width: root.width - 40
        spacing: 16
        RowLayout {
            Layout.fillWidth: true
            visible: root.activeId < 0
            AppField {
                id: search
                objectName: "playlistSearch"
                Layout.fillWidth: true
                leftPadding: 44
                placeholderText: "Type to search playlists..."
                text: root.query
                Accessible.name: "Search playlists"
                onTextEdited: root.searchEdited(text)
                AppIcon { x: 12; anchors.verticalCenter: parent.verticalCenter; name: "search"; color: Theme.muted }
            }
            IconButton {
                objectName: "createPlaylistButton"
                iconName: "plus"
                accessibleName: "New playlist"
                enabled: root.connected && !root.busy
                onClicked: root.createRequested()
            }
        }
        RowLayout {
            Layout.fillWidth: true
            visible: root.activeId >= 0
            IconButton {
                objectName: "playlistBackButton"
                iconName: "arrow-left"
                accessibleName: "Back to playlists"
                enabled: !root.busy
                onClicked: root.selectRequested(-1)
            }
            Text {
                objectName: "activePlaylistTitle"
                Layout.fillWidth: true
                text: root.activeName
                color: Theme.text
                font.family: Theme.fontFamily
                font.pixelSize: 24
                font.weight: Font.Bold
                elide: Text.ElideRight
            }
            IconButton {
                objectName: "editPlaylistButton"
                iconName: "pencil"
                accessibleName: "Edit playlist"
                enabled: root.connected && !root.busy
                onClicked: root.editRequested(root.activeId)
            }
        }
        PlaylistEditor {
            objectName: "playlistEditor"
            Layout.fillWidth: true
            visible: root.editorOpen
            name: root.editorName
            artworkUrl: root.editorArtworkUrl
            creating: root.editorId < 0
            busy: root.busy
            preparing: root.coverPreparing
            onNameEdited: name => root.nameEdited(name)
            onCoverRequested: root.coverRequested()
            onCoverResetRequested: root.coverResetRequested()
            onSaveRequested: root.saveRequested()
            onCancelRequested: root.cancelRequested()
        }
        AppMenu {
            objectName: "playlistTrackSortMenu"
            Layout.preferredWidth: 172
            Layout.preferredHeight: 32
            visible: root.activeId >= 0
            entries: [
                {index: 0, label: "Title A–Z"},
                {index: 1, label: "Artist A–Z"},
                {index: 2, label: "Recently played"}
            ]
            currentIndex: root.sortIndex
            onChosen: index => root.sortChosen(index)
            Accessible.name: "Sort playlist tracks"
            background: Rectangle { radius: 16; color: "transparent"; border.color: Theme.border }
        }
        Text {
            objectName: "playlistStatus"
            Layout.fillWidth: true
            text: root.loading ? "Loading playlists…" : root.message
            visible: text.length > 0
            color: Theme.text
            font.family: Theme.fontFamily
            font.pixelSize: 14
            wrapMode: Text.Wrap
        }
    }
    ListView {
        id: lists
        objectName: "playlistList"
        x: 20
        y: controls.y + controls.height + 20
        width: root.width - 40
        height: Math.max(0, refresh.y - y - 16)
        visible: root.activeId < 0
        model: root.playlists.length
        clip: true
        spacing: 12
        boundsBehavior: Flickable.StopAtBounds
        Basic.ScrollBar.vertical: Basic.ScrollBar { }
        delegate: Basic.Button {
            id: card
            required property int index
            readonly property var modelData: root.playlists[index] || ({ id: -1, name: "", itemCount: 0, artworkUrl: "" })
            readonly property int playlistId: modelData.id
            readonly property bool inViewport: playlistId >= 0 && root.visible && lists.visible && y + height >= lists.contentY && y <= lists.contentY + lists.height
            onInViewportChanged: if (inViewport) root.requestArtwork(modelData.id)
            onPlaylistIdChanged: if (inViewport) root.requestArtwork(playlistId)
            onModelDataChanged: if (inViewport && !modelData.artworkUrl) root.requestArtwork(modelData.id)
            Component.onCompleted: if (inViewport) root.requestArtwork(modelData.id)
            objectName: "playlistCard"
            enabled: root.connected && !root.busy
            width: lists.width
            height: 96
            padding: 12
            Accessible.name: "Open " + modelData.name + ", " + modelData.itemCount + " tracks"
            onClicked: root.selectRequested(modelData.id)
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
                    actions.popup(card, Qt.point(0, card.height), actions.itemAt(0));
                    event.accepted = true;
                }
            }
            background: Rectangle { radius: 12; color: card.hovered ? Theme.surface : "#200e0e0e"; border.color: card.activeFocus ? Theme.text : Theme.border }
            contentItem: RowLayout {
                spacing: 16
                Artwork {
                    Layout.preferredWidth: 72
                    Layout.preferredHeight: 72
                    radius: 8
                    source: card.modelData.artworkUrl || ""
                    AppIcon { anchors.centerIn: parent; name: "music"; color: Theme.muted; visible: !card.modelData.artworkUrl }
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4
                    Text { Layout.fillWidth: true; text: card.modelData.name; elide: Text.ElideRight; color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 19; font.weight: Font.DemiBold }
                    Text { text: card.modelData.itemCount + (card.modelData.itemCount === 1 ? " track" : " tracks"); color: Theme.muted; font.family: Theme.fontFamily; font.pixelSize: 13 }
                }
                IconButton {
                    id: actionsButton
                    objectName: "playlistActionsButton"
                    text: "⋮"
                    display: Basic.AbstractButton.TextOnly
                    font.pixelSize: 26
                    accessibleName: "Actions for " + card.modelData.name
                    enabled: !root.busy
                    onClicked: { forceActiveFocus(); actions.popup(actionsButton, Qt.point(0, height), actions.itemAt(0)); }
                    Basic.Menu {
                        id: actions
                        objectName: "playlistActionsMenu"
                        width: 236
                        padding: 6
                        background: Rectangle {
                            color: "#161616"
                            radius: 12
                            border.color: "#333333"
                        }
                        component ActionItem: Basic.MenuItem {
                            id: action
                            required property string iconName
                            property color foreground: Theme.text
                            implicitHeight: 36
                            leftPadding: 16
                            rightPadding: 16
                            font.family: Theme.fontFamily
                            font.pixelSize: 16
                            enabled: root.connected && !root.busy
                            contentItem: Item {
                                opacity: action.enabled ? 1 : 0.4
                                Text {
                                    anchors.left: parent.left
                                    anchors.right: actionIcon.left
                                    anchors.rightMargin: 12
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: action.text
                                    font: action.font
                                    color: action.foreground
                                    elide: Text.ElideRight
                                }
                                AppIcon {
                                    id: actionIcon
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    name: action.iconName
                                    color: action.foreground
                                    size: 16
                                }
                            }
                            background: Rectangle {
                                radius: 6
                                color: action.highlighted ? "#14f2f4fc" : "transparent"
                            }
                        }
                        property Item previousFocus: null
                        onAboutToShow: previousFocus = actionsButton.activeFocus ? actionsButton : card
                        onOpened: {
                            for (let i = 0; i < count; ++i) {
                                if (itemAt(i).enabled) { currentIndex = i; itemAt(i).forceActiveFocus(); break; }
                            }
                        }
                        onClosed: if (previousFocus) previousFocus.forceActiveFocus()
                        ActionItem {
                            text: "Add to queue"
                            iconName: "add-to-queue"
                            enabled: root.connected && !root.busy && card.modelData.itemCount > 0
                            onTriggered: root.queueRequested(card.modelData.id)
                        }
                        ActionItem { text: "Edit"; iconName: "playlist-edit"; onTriggered: root.editRequested(card.modelData.id) }
                        ActionItem { text: "Delete"; iconName: "playlist-delete"; foreground: Theme.red; onTriggered: root.deleteRequested(card.modelData.id) }
                    }
                }
            }
        }
        Text {
            anchors.centerIn: parent
            width: parent.width
            visible: lists.count === 0 && !root.loading
            text: root.query.length > 0 ? "No matching playlists" : "Create your first playlist with +"
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.Wrap
            color: Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: 14
        }
    }
    ListView {
        id: tracks
        objectName: "playlistTrackList"
        x: 20
        y: controls.y + controls.height + 20
        width: root.width - 40
        height: Math.max(0, root.height - y - 20)
        visible: root.activeId >= 0
        clip: true
        spacing: 12
        model: root.tracksModel
        delegate: root.trackDelegate
        currentIndex: -1
        boundsBehavior: Flickable.StopAtBounds
        Basic.ScrollBar.vertical: Basic.ScrollBar { }
        Text {
            anchors.centerIn: parent
            visible: root.trackCount === 0 && root.message.length === 0
            text: "This playlist is empty"
            color: Theme.muted
            font.family: Theme.fontFamily
            font.pixelSize: 14
        }
    }
    AppButton {
        id: refresh
        objectName: "refreshPlaylists"
        x: 20
        y: root.height - height - 20
        height: 36
        visible: root.activeId < 0
        text: "Refresh playlists"
        iconName: "rotate-cw"
        enabled: root.connected && !root.loading && !root.busy
        onClicked: root.refreshRequested()
    }
}
