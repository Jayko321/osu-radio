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
    Rectangle {
        id: indicator
        objectName: "tabsIndicator"
        readonly property Item target: repeater.count > root.currentIndex ? repeater.itemAt(root.currentIndex) : null
        visible: target !== null
        x: target ? row.x + target.x : 0
        y: target ? row.y + target.y : 0
        width: target ? target.width : 0
        height: target ? target.height : 0
        radius: 8
        color: Theme.text
        Behavior on x { NumberAnimation { duration: Theme.ms(180); easing.type: Theme.easing } }
        Behavior on width { NumberAnimation { duration: Theme.ms(180); easing.type: Theme.easing } }
    }
    Row {
        id: row
        anchors.centerIn: parent
        spacing: 4
        Repeater {
            id: repeater
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
                background: Item {}
                onClicked: root.selected(index)
            }
        }
    }
}
