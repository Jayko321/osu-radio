pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic

Basic.ComboBox {
    id: control
    property var entries: []
    property bool searchable: false
    property string query: ""
    signal queryEdited(string value)
    signal chosen(int index)
    property int focusedRow: 0
    model: entries
    textRole: "label"
    valueRole: "index"
    implicitWidth: 320
    implicitHeight: 40
    leftPadding: 12
    rightPadding: 40
    font.family: Theme.fontFamily
    font.pixelSize: 14
    hoverEnabled: true
    opacity: enabled ? 1 : 0.4

    function choose(row: int): void {
        if (enabled && row >= 0 && row < entries.length) {
            chosen(entries[row].index);
            popup.close();
        }
    }
    function move(delta: int): void {
        if (entries.length > 0) {
            focusedRow = (focusedRow + delta + entries.length) % entries.length;
            options.positionViewAtIndex(focusedRow, ListView.Contain);
        }
    }
    onEntriesChanged: focusedRow = 0
    onActivated: index => choose(index)
    onEnabledChanged: if (!enabled) popup.close()
    contentItem: Text {
        text: control.displayText
        color: Theme.text
        font: control.font
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
    }
    indicator: AppIcon {
        x: control.width - width - 12
        y: (control.height - height) / 2
        name: "chevron-down"
        size: 20
    }
    background: Rectangle {
        radius: 8
        color: Theme.surface
        border.width: 1
        border.color: control.hovered ? Theme.muted : Theme.border
    }
    Basic.ToolTip.visible: hovered && displayText.length > 45
    Basic.ToolTip.text: displayText
    Basic.ToolTip.delay: 700

    popup: Basic.Popup {
        id: menuPopup
        y: control.height + 6
        width: control.width
        padding: 8
        topMargin: 8
        bottomMargin: 8
        modal: true
        focus: true
        closePolicy: Basic.Popup.CloseOnEscape | Basic.Popup.CloseOnPressOutside
        onOpened: {
            control.focusedRow = Math.max(0, control.currentIndex);
            if (control.searchable) search.forceActiveFocus();
            else options.forceActiveFocus();
        }
        onClosed: control.forceActiveFocus()
        background: Rectangle {
            color: "#f20d0d0d"
            border.color: Theme.border
            radius: 12
        }
        contentItem: Column {
            spacing: control.searchable ? 8 : 0
            AppField {
                id: search
                width: parent.width
                visible: control.searchable
                height: visible ? 40 : 0
                placeholderText: "Search..."
                text: control.query
                Accessible.name: "Search menu options"
                onTextEdited: control.queryEdited(text)
                Keys.onDownPressed: {
                    options.forceActiveFocus();
                    control.focusedRow = 0;
                }
                Keys.onReturnPressed: control.choose(control.focusedRow)
                Keys.onEnterPressed: control.choose(control.focusedRow)
            }
            ListView {
                id: options
                width: parent.width
                height: Math.min(280, Math.max(40, contentHeight))
                clip: true
                model: control.entries
                currentIndex: control.focusedRow
                spacing: 4
                activeFocusOnTab: true
                keyNavigationEnabled: false
                Basic.ScrollBar.vertical: Basic.ScrollBar {}
                Keys.onDownPressed: control.move(1)
                Keys.onUpPressed: control.move(-1)
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Home) {
                        control.focusedRow = 0;
                        options.positionViewAtBeginning();
                        event.accepted = true;
                    } else if (event.key === Qt.Key_End) {
                        control.focusedRow = options.count - 1;
                        options.positionViewAtEnd();
                        event.accepted = true;
                    }
                }
                Keys.onReturnPressed: control.choose(control.focusedRow)
                Keys.onEnterPressed: control.choose(control.focusedRow)
                Keys.onSpacePressed: control.choose(control.focusedRow)
                delegate: Basic.ItemDelegate {
                    id: option
                    required property var modelData
                    required property int index
                    width: options.width
                    implicitHeight: Math.max(40, optionLabel.implicitHeight + 16)
                    padding: 8
                    text: modelData.label
                    hoverEnabled: true
                    // The list owns keyboard traversal; options retain native mouse activation.
                    focusPolicy: Qt.NoFocus
                    contentItem: Text {
                        id: optionLabel
                        text: option.text
                        color: Theme.text
                        font.family: Theme.fontFamily
                        font.pixelSize: 14
                        wrapMode: Text.Wrap
                    }
                    background: Rectangle {
                        radius: 8
                        color: option.index === control.currentIndex ? Theme.accent
                            : option.hovered || (options.activeFocus && option.index === control.focusedRow)
                            ? Theme.surface : "transparent"
                    }
                    onClicked: control.choose(index)
                }
                Text {
                    anchors.centerIn: parent
                    visible: options.count === 0
                    text: "No matching options"
                    color: Theme.muted
                    font.family: Theme.fontFamily
                    font.pixelSize: 14
                }
            }
        }
    }
}
