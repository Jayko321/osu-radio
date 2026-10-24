import QtQuick

// Turns "busy went true, then false without an error" into a success signal, for adapters that expose
// only busy and message. Call expect() right before the action. The intent is dropped if busy never
// rises (the action was rejected or another one was already running), so a later unrelated operation
// cannot claim it. Busy and message change in separate notifications, so the verdict waits one turn.
QtObject {
    id: root
    property bool busy: false
    property string message: ""
    property string pending: ""
    property bool started: false
    property var accept: null
    signal succeeded(string text)
    function expect(text: string, check): void {
        pending = text;
        accept = check === undefined ? null : check;
        started = false;
        expiry.restart();
    }
    function settle(): void {
        if (!pending.length || busy) return;
        const text = pending;
        pending = "";
        if (accept ? accept() : message.length === 0) succeeded(text);
    }
    onBusyChanged: {
        if (!pending.length) return;
        if (busy) { started = true; expiry.stop(); }
        else if (started) { started = false; Qt.callLater(settle); }
    }
    readonly property Timer expiryTimer: Timer {
        id: expiry
        interval: 1500
        onTriggered: if (!root.started) root.pending = ""
    }
}
