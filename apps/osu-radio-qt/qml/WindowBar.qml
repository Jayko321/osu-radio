pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import "components"

Rectangle {
    id: bar
    required property var window
    property bool gallery: false
    property int selectedTab: 0
    property bool queueEnabled: false
    property bool queueVisible: false
    signal tabSelected(int index)
    signal queueRequested()
    readonly property bool maximized: window.visibility === Window.Maximized
    height: 50
    color: Theme.background
    z: 10

    function toggleMaximized() {
        if (maximized)
            window.showNormal();
        else
            window.showMaximized();
    }

    // This area is behind controls: only unoccupied title-bar space starts a move.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        onPressed: bar.window.startSystemMove()
        onDoubleClicked: bar.toggleMaximized()
    }

    Row {
        x: 20
        anchors.verticalCenter: parent.verticalCenter
        spacing: 4
        AppButton {
            height: 34
            objectName: "songsTabButton"
            text: bar.gallery ? "Components" : "Songs"
            iconName: "music"
            variant: bar.selectedTab === 0 ? "light" : "link"
            foreground: bar.selectedTab === 0 ? Theme.background : Theme.text
            onClicked: bar.tabSelected(0)
            font.pixelSize: 16
            font.weight: Font.Bold
            Accessible.name: text
        }
        AppButton {
            height: 34
            objectName: "playlistsTabButton"
            text: "Playlists"
            iconName: "layers"
            variant: bar.selectedTab === 1 ? "light" : "link"
            foreground: bar.selectedTab === 1 ? Theme.background : Theme.text
            onClicked: bar.tabSelected(1)
            font.pixelSize: 16
            font.weight: Font.Bold
            Accessible.name: "Playlists"
        }
        AppButton {
            height: 34
            objectName: "settingsButton"
            text: "Settings"
            iconName: "settings-4-line"
            variant: bar.selectedTab === 2 ? "light" : "link"
            foreground: bar.selectedTab === 2 ? Theme.background : Theme.text
            enabled: !bar.gallery
            onClicked: bar.tabSelected(2)
            font.pixelSize: 16
            font.weight: Font.Bold
            Accessible.name: "Settings"
        }
    }

    Row {
        anchors.right: parent.right
        height: parent.height
        spacing: 0
        IconButton {
            objectName: "queueButton"
            width: 46
            height: bar.height
            visible: !bar.gallery
            enabled: bar.queueEnabled
            iconName: "layers"
            foreground: bar.queueVisible ? "#c142b0" : Theme.text
            accessibleName: bar.queueVisible ? "Hide queue" : "Show queue"
            onClicked: bar.queueRequested()
        }
        Basic.Button {
            id: minimize
            width: 46
            height: 50
            Accessible.name: "Minimize"
            contentItem: AppIcon { name: "minus"; size: 16; opacity: minimize.hovered ? 1 : 0.62 }
            background: Rectangle { color: minimize.hovered ? "#14f2f4fc" : "transparent" }
            onClicked: bar.window.showMinimized()
        }
        Basic.Button {
            id: maximize
            width: 46
            height: 50
            Accessible.name: bar.maximized ? "Restore" : "Maximize"
            contentItem: AppIcon { name: bar.maximized ? "copy" : "square"; size: 16; opacity: maximize.hovered ? 1 : 0.62 }
            background: Rectangle { color: maximize.hovered ? "#14f2f4fc" : "transparent" }
            onClicked: bar.toggleMaximized()
        }
        Basic.Button {
            id: close
            width: 46
            height: 50
            Accessible.name: "Close window"
            contentItem: AppIcon { name: "x"; size: 16; opacity: close.hovered ? 1 : 0.62 }
            background: Rectangle { color: close.hovered ? Theme.red : "transparent" }
            onClicked: bar.window.close()
        }
    }

    // Eight narrow handles belong to the complete window, not just its title bar.
    // Native operations preserve window-manager snapping and platform resize rules.
    Item {
        id: resizeHandles
        parent: bar.window.contentItem
        anchors.fill: parent
        z: 1000
        visible: !bar.maximized && bar.window.visibility !== Window.FullScreen
        Repeater {
            model: [Qt.TopEdge, Qt.BottomEdge, Qt.LeftEdge, Qt.RightEdge,
                Qt.TopEdge | Qt.LeftEdge, Qt.TopEdge | Qt.RightEdge,
                Qt.BottomEdge | Qt.LeftEdge, Qt.BottomEdge | Qt.RightEdge]
            MouseArea {
                required property int modelData
                readonly property bool edgeLeft: (modelData & Qt.LeftEdge) !== 0
                readonly property bool edgeRight: (modelData & Qt.RightEdge) !== 0
                readonly property bool edgeTop: (modelData & Qt.TopEdge) !== 0
                readonly property bool edgeBottom: (modelData & Qt.BottomEdge) !== 0
                readonly property bool corner: (edgeLeft || edgeRight) && (edgeTop || edgeBottom)
                width: edgeLeft || edgeRight ? 6 : resizeHandles.width - 12
                height: edgeTop || edgeBottom ? 6 : resizeHandles.height - 12
                x: edgeRight ? resizeHandles.width - width : (edgeLeft ? 0 : 6)
                y: edgeBottom ? resizeHandles.height - height : (edgeTop ? 0 : 6)
                cursorShape: corner ? ((edgeLeft && edgeTop) || (edgeRight && edgeBottom) ? Qt.SizeFDiagCursor : Qt.SizeBDiagCursor)
                    : (edgeLeft || edgeRight ? Qt.SizeHorCursor : Qt.SizeVerCursor)
                acceptedButtons: Qt.LeftButton
                onPressed: bar.window.startSystemResize(modelData)
            }
        }
    }
}
