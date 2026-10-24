pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic

Basic.Popup {
    id: root
    property string mode: "none"
    property int speedPercent: 100
    property bool adjustPitch: false
    property bool available: true
    property bool saving: false
    property string errorMessage: ""
    property bool canRetry: false
    property Item previousFocus: null
    signal dtCycleRequested()
    signal htToggleRequested()
    signal speedRequested(int percent)
    signal pitchRequested(bool adjust)
    signal clearRequested()
    signal retryRequested()

    width: 320
    padding: 16
    margins: 12
    modal: true
    dim: false
    focus: true
    closePolicy: Basic.Popup.CloseOnEscape | Basic.Popup.CloseOnPressOutside
    onAboutToShow: previousFocus = root.parent.Window.window.activeFocusItem
    onOpened: {
        if (available) dtButton.forceActiveFocus(Qt.PopupFocusReason);
        else closeButton.forceActiveFocus(Qt.PopupFocusReason);
    }
    onClosed: if (previousFocus) previousFocus.forceActiveFocus(Qt.PopupFocusReason)
    background: Rectangle { radius: 12; color: "#f20d0d0d"; border.color: Theme.border }
    // A control that disables itself (Clear at 1x) drops focus; the scope keeps D and E reachable.
    contentItem: FocusScope {
        implicitWidth: column.implicitWidth
        implicitHeight: column.implicitHeight
        focus: true
        Keys.onPressed: event => {
            if (!root.opened || (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))) return;
            if (event.key === Qt.Key_D || event.key === Qt.Key_E) {
                if (root.available && !event.isAutoRepeat) {
                    if (event.key === Qt.Key_D) root.dtCycleRequested();
                    else root.htToggleRequested();
                }
                event.accepted = true;
            }
        }
        Column {
            id: column
            width: parent.width
            spacing: 12
            Item {
                width: parent.width; height: 32
                Text {
                    anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
                    text: "Mods"; color: Theme.text; font.family: Theme.fontFamily
                    font.pixelSize: 16; font.weight: Font.Medium
                }
                IconButton {
                    id: closeButton
                    objectName: "modsCloseButton"
                    anchors.right: parent.right
                    width: 32; height: 32
                    iconName: "x"; icon.width: 18; icon.height: 18
                    accessibleName: "Close mods"
                    onClicked: root.close()
                }
            }
            Row {
                width: parent.width; spacing: 8
                AppButton {
                    id: dtButton
                    objectName: "modsDtButton"
                    width: (parent.width - parent.spacing) / 2
                    text: root.mode === "dt" && root.adjustPitch ? "NC (D)" : "DT (D)"
                    variant: root.mode === "dt" ? "accent" : "alternate"
                    enabled: root.available
                    Accessible.name: "Cycle Double Time / Nightcore (D)"
                    onClicked: root.dtCycleRequested()
                }
                AppButton {
                    objectName: "modsHtButton"
                    width: (parent.width - parent.spacing) / 2
                    text: "HT (E)"
                    variant: root.mode === "ht" ? "accent" : "alternate"
                    enabled: root.available
                    Accessible.name: "Toggle Half Time (E)"
                    onClicked: root.htToggleRequested()
                }
            }
            Text {
                objectName: "modsSpeedLabel"
                text: "Speed: " + (root.speedPercent / 100).toFixed(2) + "×"
                color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 14
            }
            Basic.Slider {
                id: speedSlider
                objectName: "modsSpeedSlider"
                width: parent.width
                from: root.mode === "dt" ? 101 : root.mode === "ht" ? 50 : 100
                to: root.mode === "dt" ? 200 : root.mode === "ht" ? 99 : 100
                stepSize: 1
                snapMode: Basic.Slider.SnapAlways
                value: root.speedPercent
                enabled: root.available && root.mode !== "none"
                palette.window: Theme.text; palette.dark: Theme.accent; palette.midlight: Theme.muted
                Accessible.name: "Mod speed percent"
                onMoved: root.speedRequested(Math.round(value))
                onPressedChanged: if (!pressed) value = root.speedPercent
                Connections {
                    target: root
                    function onSpeedPercentChanged() { if (!speedSlider.pressed) speedSlider.value = root.speedPercent; }
                }
            }
            AppSwitch {
                objectName: "modsPitchSwitch"
                text: "Adjust pitch"
                checked: root.adjustPitch
                enabled: root.available && root.mode !== "none"
                Accessible.name: text
                onToggled: root.pitchRequested(checked)
            }
            AppButton {
                objectName: "modsClearButton"
                width: parent.width
                text: "Clear mods"
                Accessible.name: text
                enabled: root.available && root.mode !== "none"
                onClicked: root.clearRequested()
            }
            Text {
                objectName: "modsSaveStatus"
                width: parent.width
                visible: root.saving || root.errorMessage.length > 0
                text: root.errorMessage || "Saving…"
                color: root.errorMessage.length > 0 ? Theme.red : Theme.muted
                font.family: Theme.fontFamily; font.pixelSize: 12
                wrapMode: Text.Wrap
            }
            AppButton {
                objectName: "modsRetryButton"
                width: parent.width
                text: "Retry"
                Accessible.name: text
                visible: root.errorMessage.length > 0
                enabled: root.canRetry && !root.saving
                onClicked: root.retryRequested()
            }
        }
    }
}
