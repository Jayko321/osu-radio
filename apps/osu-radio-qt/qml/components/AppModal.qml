pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic
import QtQuick.Layouts
import QtQuick.Effects

Basic.Popup {
    id: root
    parent: Basic.Overlay.overlay
    anchors.centerIn: parent
    property Item scene: null
    property string title: ""
    property bool dismissible: true
    property Item initialFocus: closeButton
    property Item previousFocus: null
    property real panelWidth: 740
    property real panelHeight: 620
    default property alias body: bodyContainer.data
    property alias footer: footerContainer.data
    width: Math.min(panelWidth, parent ? parent.width - 48 : panelWidth)
    height: Math.min(panelHeight, parent ? parent.height - 48 : panelHeight)
    padding: 28
    modal: true
    dim: true
    focus: true
    closePolicy: dismissible ? Basic.Popup.CloseOnEscape | Basic.Popup.CloseOnPressOutside : Basic.Popup.NoAutoClose
    onAboutToShow: previousFocus = scene && scene.Window.window ? scene.Window.window.activeFocusItem : null
    onOpened: if (initialFocus) initialFocus.forceActiveFocus(Qt.PopupFocusReason)
    onClosed: if (previousFocus) previousFocus.forceActiveFocus(Qt.PopupFocusReason)
    background: Rectangle { color: "#f218181b"; radius: 12; border.color: Theme.border; border.width: 1 }
    Basic.Overlay.modal: Item {
        ShaderEffectSource {
            id: capture
            sourceItem: root.visible ? root.scene : null
            visible: false
            live: root.visible
        }
        MultiEffect {
            anchors.fill: parent
            source: root.visible ? capture : null
            visible: root.visible && root.scene !== null
            blurEnabled: root.visible
            blurMax: 40
            blur: 1
            autoPaddingEnabled: false
        }
        Rectangle { anchors.fill: parent; color: "#66000000" }
    }
    contentItem: ColumnLayout {
        spacing: 24
        RowLayout {
            Layout.fillWidth: true
            Text {
                text: root.title
                Layout.fillWidth: true
                color: Theme.text
                font.family: Theme.fontFamily
                font.pixelSize: 24
                font.weight: Font.DemiBold
                wrapMode: Text.Wrap
            }
            IconButton {
                id: closeButton
                objectName: "modalClose"
                iconName: "x"
                accessibleName: "Close dialog"
                enabled: root.dismissible
                onClicked: root.close()
            }
        }
        Item { id: bodyContainer; Layout.fillWidth: true; Layout.fillHeight: true }
        Item { id: footerContainer; Layout.fillWidth: true; implicitHeight: childrenRect.height }
    }
}
