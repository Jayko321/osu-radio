pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import QtQuick.Layouts

AppModal {
    id: root
    title: "Select osu! folders"
    property var rows: []
    property bool applying: false
    property bool discovering: false
    property bool picking: false
    property string message: ""
    property int computingFrame: 0
    readonly property bool hasPending: rows.some(row => row.action.length > 0)
    signal toggleRequested(string markerPath)
    signal refreshRequested(string markerPath)
    signal retryCountRequested(string markerPath)
    signal browseRequested()
    signal applyRequested()
    dismissible: !applying
    initialFocus: browseButton
    Timer {
        interval: 400
        repeat: true
        running: root.visible && root.rows.some(row => row.countPending)
        onTriggered: root.computingFrame = (root.computingFrame + 1) % 4
    }
    ColumnLayout {
        anchors.fill: parent
        spacing: 12
        Text {
            Layout.fillWidth: true
            text: root.discovering ? "Discovering osu! installations…" : "Select the installations to add, refresh or remove."
            color: Theme.text
            font.family: Theme.fontFamily
            font.pixelSize: 16
            wrapMode: Text.Wrap
        }
        Basic.ScrollView {
            id: scroll
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true
            Basic.ScrollBar.vertical: AppScrollBar {}
            Basic.ScrollBar.horizontal.policy: Basic.ScrollBar.AlwaysOff
            Column {
                width: scroll.availableWidth
                spacing: 8
                Repeater {
                    model: root.rows
                    Rectangle {
                        id: row
                        required property var modelData
                        required property int index
                        objectName: "folderSelectionRow" + index
                        width: parent ? parent.width : 0
                        height: Math.max(94, rowContent.implicitHeight + 32)
                        radius: 8
                        color: modelData.registered ? "#303faf7c" : "#80333333"
                        border.width: modelData.action.length > 0 ? 2 : 1
                        border.color: modelData.action === "remove" ? Theme.red : modelData.action === "add" || modelData.action === "refresh" ? Theme.accent : Theme.border
                        RowLayout {
                            id: rowContent
                            anchors.fill: parent
                            anchors.margins: 16
                            spacing: 12
                            Image {
                                source: "qrc:/assets/logos/" + (row.modelData.kind === "stable" ? "stable" : "lazer") + ".png"
                                Layout.preferredWidth: 54
                                Layout.preferredHeight: 54
                                fillMode: Image.PreserveAspectFit
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 6
                                Text {
                                    id: pathText
                                    Layout.fillWidth: true
                                    text: row.modelData.path
                                    color: Theme.text
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 14
                                    font.weight: Font.Medium
                                    elide: Text.ElideMiddle
                                    AppToolTip { visible: pathHover.hovered; text: pathText.text }
                                    HoverHandler { id: pathHover }
                                }
                                RowLayout {
                                    spacing: 8
                                    Rectangle {
                                        implicitWidth: 72
                                        implicitHeight: 24
                                        radius: 12
                                        color: row.modelData.kind === "stable" ? "#f573ad" : "#39d5d5"
                                        Text {
                                            anchors.centerIn: parent
                                            text: row.modelData.kind.toUpperCase()
                                            color: Theme.text
                                            font.family: Theme.fontFamily
                                            font.pixelSize: 11
                                            font.weight: Font.DemiBold
                                        }
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        text: row.modelData.countPending ? "Computing" + ".".repeat(root.computingFrame)
                                            : row.modelData.countError.length > 0 ? "Count unavailable" : row.modelData.count + " Beatmaps"
                                        color: Theme.muted
                                        font.family: Theme.fontFamily
                                        font.pixelSize: 12
                                        elide: Text.ElideRight
                                        AppToolTip { visible: countHover.hovered && row.modelData.countError.length > 0; text: row.modelData.countError }
                                        HoverHandler { id: countHover }
                                    }
                                    IconButton {
                                        iconName: "rotate-cw"
                                        accessibleName: "Retry beatmap count"
                                        implicitWidth: 28
                                        implicitHeight: 28
                                        visible: row.modelData.countError.length > 0
                                        enabled: !root.applying
                                        onClicked: root.retryCountRequested(row.modelData.markerPath)
                                    }
                                    // The count text shrinks first, so the capsule never pushes the path or badge out.
                                    Rectangle {
                                        id: status
                                        objectName: "folderStatus" + row.index
                                        readonly property bool removing: row.modelData.action === "remove"
                                        visible: row.modelData.action.length > 0
                                        Layout.preferredWidth: statusRow.implicitWidth + 20
                                        implicitHeight: 24
                                        radius: 12
                                        color: removing ? Qt.alpha(Theme.red, 0.25) : Theme.accentSoft
                                        Row {
                                            id: statusRow
                                            anchors.centerIn: parent
                                            spacing: 4
                                            AppIcon {
                                                anchors.verticalCenter: parent.verticalCenter
                                                size: 14
                                                name: row.modelData.action === "remove" ? "minus" : row.modelData.action === "refresh" ? "rotate-cw" : "check"
                                                color: status.removing ? Theme.red : Theme.accent
                                            }
                                            Text {
                                                objectName: "folderStatusText" + row.index
                                                anchors.verticalCenter: parent.verticalCenter
                                                text: row.modelData.action === "remove" ? "Will be removed"
                                                    : row.modelData.action === "refresh" ? "Will be refreshed" : "Will be added"
                                                color: status.removing ? Theme.red : Theme.accent
                                                font.family: Theme.fontFamily
                                                font.pixelSize: 11
                                                font.weight: Font.DemiBold
                                            }
                                        }
                                    }
                                }
                                Text {
                                    Layout.fillWidth: true
                                    visible: text.length > 0
                                    text: row.modelData.error
                                    color: Theme.red
                                    font.family: Theme.fontFamily
                                    font.pixelSize: 12
                                    wrapMode: Text.Wrap
                                }
                            }
                            IconButton {
                                objectName: "folderRefresh" + row.index
                                visible: row.modelData.registered
                                iconName: "rotate-cw"
                                accessibleName: "Refresh import"
                                checkable: true
                                checked: row.modelData.action === "refresh"
                                enabled: !root.applying
                                onClicked: root.refreshRequested(row.modelData.markerPath)
                                background: Rectangle { radius: 8; color: row.modelData.action === "refresh" ? Theme.accentSoft : "transparent" }
                            }
                            IconButton {
                                objectName: "folderToggle" + row.index
                                iconName: row.modelData.registered ? "minus" : "plus"
                                accessibleName: row.modelData.registered ? "Remove folder" : "Add folder"
                                checkable: true
                                checked: row.modelData.action === "add" || row.modelData.action === "remove"
                                enabled: !root.applying
                                onClicked: root.toggleRequested(row.modelData.markerPath)
                                background: Rectangle { radius: 8; color: row.modelData.action === "add" || row.modelData.action === "remove" ? Theme.accentSoft : "transparent" }
                            }
                        }
                    }
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            AppButton {
                id: browseButton
                objectName: "browseFolders"
                text: root.picking ? "Choosing…" : "Select a Different Folder"
                enabled: !root.applying && !root.picking
                onClicked: root.browseRequested()
            }
        }
        Text {
            Layout.fillWidth: true
            visible: text.length > 0
            text: root.message
            color: Theme.text
            font.family: Theme.fontFamily
            font.pixelSize: 12
            wrapMode: Text.Wrap
        }
    }
    footer: AppButton {
        objectName: "applyFolders"
        anchors.right: parent.right
        variant: "accent"
        text: root.applying ? "Applying…" : "Apply"
        enabled: !root.applying && !root.picking && root.hasPending
        onClicked: root.applyRequested()
    }
}
