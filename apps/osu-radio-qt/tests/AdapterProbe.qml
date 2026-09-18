import QtQuick
import OsuRadio 1.0

QtObject {
    id: probe
    property int notifications: 0
    readonly property MockBridge bridge: MockBridge {}
    readonly property Connections connection: Connections {
        target: probe.bridge
        function onStateJsonChanged() { probe.notifications++; }
    }
    function check(condition: bool, message: string): void {
        if (!condition) throw new Error("Adapter probe: " + message);
    }
    Component.onCompleted: {
        const original = JSON.parse(bridge.stateJson);
        check(original.selected === 0 && original.tracks.length === 4, "initial sample");
        bridge.dispatch("selectTrack", "2");
        check(notifications === 1 && JSON.parse(bridge.stateJson).selected === 2, "selection notification");
        bridge.dispatch("selectTrack", "2");
        bridge.dispatch("selectTrack", "100");
        bridge.dispatch("selectTrack", "-1");
        bridge.dispatch("unknown", "");
        check(notifications === 1, "invalid and repeated actions must not notify");
        bridge.dispatch("search", "no match");
        const searched = JSON.parse(bridge.stateJson);
        check(notifications === 2 && searched.search === "no match", "search notification");
        check(JSON.stringify(searched.tracks) === JSON.stringify(original.tracks), "search preserves tracks");
        bridge.dispatch("galleryDisabled", "true");
        bridge.dispatch("press", "");
        bridge.dispatch("cycleFilter", "");
        bridge.dispatch("field", "blocked");
        check(notifications === 3 && JSON.parse(bridge.stateJson).gallery.presses === 0, "disabled suppression");
        bridge.dispatch("galleryDisabled", "false");
        bridge.dispatch("press", "");
        check(notifications === 5 && JSON.parse(bridge.stateJson).gallery.presses === 1, "reenable notification");
        console.info("Adapter probe passed");
    }
}
