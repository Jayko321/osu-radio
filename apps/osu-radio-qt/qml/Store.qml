import QtQuick
import OsuRadio 1.0

QtObject {
    id: root
    readonly property MockBridge bridge: MockBridge {}
    readonly property var state: JSON.parse(bridge.stateJson)
    function send(action: string, value: var): void {
        bridge.dispatch(action, value === undefined ? "" : String(value))
    }
}
