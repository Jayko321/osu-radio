pragma ComponentBehavior: Bound
import QtQuick

Item {
    id: root
    property var labels: []
    property int currentIndex: 0
    signal selected(int index)
    implicitHeight: 42
    implicitWidth: row.implicitWidth + 8
    Rectangle {
        anchors.fill: parent
        radius: 12
        color: Theme.surface
    }
    Row {
        id: row
        anchors.centerIn: parent
        spacing: 4
        Repeater {
            model: root.labels
            AppButton {
                id: tab
                required property string modelData
                required property int index
                text: modelData
                height: 34
                variant: root.currentIndex === index ? "light" : "alternate"
                Accessible.role: Accessible.PageTab
                Accessible.description: root.currentIndex === index ? "Selected tab" : "Tab"
                background: Rectangle {
                    radius: 8
                    color: root.currentIndex === tab.index ? Theme.text : "transparent"
                }
                onClicked: root.selected(index)
            }
        }
    }
}
