import QtQuick
import QtQuick.Controls.Basic as Basic

Basic.TextField {
    id: control
    // Opt-in: a trailing Clear button that shows while the field has text.
    property bool clearable: false
    signal cleared()
    implicitWidth: 240
    implicitHeight: 40
    leftPadding: 12
    rightPadding: clearable ? 44 : 12
    topPadding: 6
    bottomPadding: 6
    font.family: Theme.fontFamily
    font.pixelSize: 16
    color: Theme.text
    placeholderTextColor: enabled ? Theme.muted : Theme.text
    selectionColor: "#40eff1f5"
    selectedTextColor: Theme.text
    selectByMouse: true
    opacity: enabled ? 1 : 0.4
    IconButton {
        objectName: "clearSearchButton"
        anchors.right: parent.right
        anchors.rightMargin: 6
        anchors.verticalCenter: parent.verticalCenter
        width: 28
        height: 28
        icon.width: 16
        icon.height: 16
        iconName: "x"
        accessibleName: "Clear search"
        visible: control.clearable && control.text.length > 0
        // TextField.clear() emits textChanged but not textEdited.
        onClicked: { control.clear(); control.forceActiveFocus(); control.cleared(); }
    }
    background: Rectangle {
        radius: 8
        color: Theme.surface
        border.color: control.hovered ? Theme.muted : Theme.border
        border.width: 1
        Rectangle {
            anchors.fill: parent
            anchors.margins: -4
            radius: 11
            color: "transparent"
            border.color: Theme.text
            border.width: 2
            visible: control.enabled && control.activeFocus
        }
    }
}
