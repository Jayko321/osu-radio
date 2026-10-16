pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls.Basic as Basic

Basic.Popup {
    id: root
    required property Item host
    required property var tracks
    property int startIndex: 0
    property bool loading: false
    property string message: ""
    property Item previousFocus: null
    property bool positionPending: false
    property var mediaDelegates: []
    // Counters are also used by the offscreen scalability probe.
    property int createdDelegates: 0
    property int liveDelegates: 0
    property string dragId: ""
    property bool dragging: false
    property int insertionIndex: -1
    property point pointerPosition: Qt.point(0, 0)
    signal playRequested(string id)
    signal removeRequested(string id)
    signal moveRequested(string id, var beforeEntryId)
    signal clearUpcomingRequested()
    signal clearAllRequested()
    signal visibleEntriesChanged(var ids)
    signal retryRequested()
    function scheduleMedia(): void { mediaTimer.restart(); }
    function reportMedia(): void {
        const ids = [];
        if (visible && !positionPending) for (const row of mediaDelegates) if (row.inViewport) ids.push(row.entryId);
        visibleEntriesChanged(ids);
    }
    function positionOnce(): void {
        if (!opened || loading || !positionPending) return;
        list.forceLayout();
        positionPending = false;
        list.currentIndex = list.count > 0 ? Math.min(startIndex, list.count - 1) : -1;
        list.positionViewAtIndex(list.currentIndex, ListView.Beginning);
        scheduleMedia();
    }
    function cancelDrag(): void {
        dragId = ""; dragging = false; insertionIndex = -1;
    }
    function updateDrop(point: point): void {
        pointerPosition = point;
        insertionIndex = Math.max(0, Math.min(list.count,
            Math.floor((point.y + list.contentY - list.originY + 48) / 96)));
    }
    function dropDrag(): void {
        if (dragging && insertionIndex >= 0)
            moveRequested(dragId, insertionIndex < list.count ? tracks.entryIdAt(insertionIndex) : null);
        cancelDrag();
    }
    function focusRow(index: int): void {
        list.currentIndex = Math.max(0, Math.min(index, list.count - 1));
        list.positionViewAtIndex(list.currentIndex, ListView.Contain);
        if (list.currentItem) list.currentItem.forceActiveFocus(Qt.TabFocusReason);
    }
    onLoadingChanged: Qt.callLater(positionOnce)
    onStartIndexChanged: Qt.callLater(positionOnce)
    parent: host
    x: host.width - width
    width: Math.min(430, host.width)
    height: host.height
    padding: 16
    modal: true
    dim: true
    focus: true
    closePolicy: Basic.Popup.CloseOnEscape | Basic.Popup.CloseOnPressOutside
    onAboutToShow: { previousFocus = host.Window.window.activeFocusItem; positionPending = true; }
    onOpened: { closeButton.forceActiveFocus(Qt.PopupFocusReason); Qt.callLater(positionOnce); }
    onClosed: {
        cancelDrag(); mediaTimer.stop(); visibleEntriesChanged([]);
        if (previousFocus) previousFocus.forceActiveFocus(Qt.PopupFocusReason);
    }
    background: null
    Basic.Overlay.modal: Item {
        Rectangle {
            readonly property point origin: root.host.mapToItem(parent, 0, 0)
            x: origin.x; y: origin.y
            width: root.host.width; height: root.height
            gradient: Gradient {
                GradientStop { position: 0; color: "#f5000000" }
                GradientStop { position: 1; color: "#cc0e0e0e" }
            }
        }
    }
    Timer { id: mediaTimer; interval: 0; onTriggered: root.reportMedia() }
    Timer {
        interval: 30; repeat: true; running: root.dragging
        onTriggered: {
            const step = root.pointerPosition.y < 32 ? -18 : root.pointerPosition.y > list.height - 32 ? 18 : 0;
            if (step) {
                list.contentY = Math.max(list.originY, Math.min(list.contentY + step, list.originY + Math.max(0, list.contentHeight - list.height)));
                root.updateDrop(root.pointerPosition);
            }
        }
    }
    Connections {
        target: root.tracks
        ignoreUnknownSignals: true
        function onRowsInserted(): void { root.scheduleMedia(); Qt.callLater(root.positionOnce); }
        function onRowsRemoved(): void { root.scheduleMedia(); }
        function onRowsMoved(): void { root.scheduleMedia(); }
        function onDataChanged(): void { root.scheduleMedia(); }
    }
    contentItem: Item {
        Keys.onEscapePressed: event => { if (root.dragId.length) { root.cancelDrag(); event.accepted = true; } else root.close(); }
        Item {
            id: header
            width: parent.width; height: 32
            Text {
                anchors.left: parent.left; anchors.right: closeButton.left
                anchors.verticalCenter: parent.verticalCenter
                text: "Queue"; color: Theme.text; font.family: Theme.fontFamily
                font.pixelSize: 14; font.weight: Font.Medium; elide: Text.ElideRight
            }
            IconButton {
                id: closeButton
                objectName: "queueCloseButton"
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                width: 32; height: 32; iconName: "x"; icon.width: 18; icon.height: 18
                foreground: "#b4b4b4"; accessibleName: "Close queue"
                onClicked: root.close()
            }
        }
        Row {
            id: actions
            anchors.top: header.bottom; anchors.topMargin: 8; spacing: 8
            AppButton { objectName: "queueClearUpcoming"; text: "Clear upcoming"; onClicked: root.clearUpcomingRequested() }
            AppButton { objectName: "queueClearAll"; text: "Clear all"; onClicked: root.clearAllRequested() }
        }
        Column {
            id: status
            anchors.top: actions.bottom; anchors.topMargin: 8; width: parent.width; spacing: 8
            Text {
                objectName: "queueStatus"; width: parent.width
                visible: root.message.length > 0 || list.count === 0
                text: root.message || (root.loading ? "Loading queue…" : "Queue is empty")
                color: Theme.muted; font.family: Theme.fontFamily; font.pixelSize: 14; wrapMode: Text.Wrap
            }
            AppButton { objectName: "queueRetryButton"; visible: root.message.length > 0; enabled: !root.loading; text: "Retry"; onClicked: root.retryRequested() }
        }
        ListView {
            id: list
            objectName: "queueScroll"
            anchors.top: status.bottom; anchors.topMargin: 14; anchors.bottom: parent.bottom
            width: parent.width; clip: true; spacing: 14
            model: root.tracks
            cacheBuffer: 96
            reuseItems: false
            currentIndex: -1
            keyNavigationEnabled: false
            boundsBehavior: Flickable.StopAtBounds
            Basic.ScrollBar.vertical: Basic.ScrollBar {}
            onContentYChanged: root.scheduleMedia()
            onHeightChanged: root.scheduleMedia()
            delegate: Basic.ItemDelegate {
                id: card
                required property int index
                required property var model
                readonly property string entryId: model.entryId
                readonly property int audioId: model.audioId
                readonly property string artworkUrl: model.artworkUrl
                readonly property string durationLabel: model.durationLabel
                readonly property bool inViewport: root.visible && y + height > list.contentY && y < list.contentY + list.height
                objectName: "queueTrackCard"
                width: list.width; height: 82
                activeFocusOnTab: true
                function reveal(): void {
                    list.currentIndex = index;
                    list.positionViewAtIndex(index, ListView.Contain);
                }
                onActiveFocusChanged: if (activeFocus) reveal()
                onInViewportChanged: root.scheduleMedia()
                onEntryIdChanged: root.scheduleMedia()
                Component.onCompleted: {
                    root.mediaDelegates.push(card); root.createdDelegates++; root.liveDelegates++; root.scheduleMedia();
                }
                Component.onDestruction: {
                    const i = root.mediaDelegates.indexOf(card);
                    if (i >= 0) root.mediaDelegates.splice(i, 1);
                    root.liveDelegates--; root.scheduleMedia();
                }
                Accessible.name: model.title + ", " + model.artist + ", " + durationLabel + ", " + (model.isCurrent ? "Current, " : model.isPast ? "Past, " : "Upcoming, ") + model.availability
                onClicked: if (model.availability !== "unavailable") root.playRequested(entryId)
                Keys.onPressed: event => {
                    if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
                        const up = event.key === Qt.Key_Up;
                        if (event.modifiers & Qt.ControlModifier) {
                            if (up && index > 0) root.moveRequested(entryId, root.tracks.entryIdAt(index - 1));
                            else if (!up && index < list.count - 1) root.moveRequested(entryId, index + 2 < list.count ? root.tracks.entryIdAt(index + 2) : null);
                        } else root.focusRow(index + (up ? -1 : 1));
                        event.accepted = true;
                    } else if (event.key === Qt.Key_Delete) {
                        root.removeRequested(entryId); event.accepted = true;
                    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                        if (model.availability !== "unavailable") root.playRequested(entryId);
                        event.accepted = true;
                    }
                }
                background: Item {
                    Artwork { anchors.fill: parent; source: card.artworkUrl }
                    Rectangle {
                        anchors.fill: parent; radius: 8
                        border.width: card.activeFocus || card.model.isCurrent ? 2 : 0
                        border.color: Theme.accent
                        gradient: Gradient {
                            orientation: Gradient.Horizontal
                            GradientStop { position: 0; color: "#d9000000" }
                            GradientStop { position: 1; color: "#50000000" }
                        }
                    }
                }
                contentItem: Item {
                    Column {
                        x: 8; anchors.verticalCenter: parent.verticalCenter
                        width: Math.max(0, parent.width - 124); spacing: 2
                        Text {
                            width: parent.width; text: card.model.title
                            color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 20
                            font.weight: Font.Bold; elide: Text.ElideRight
                        }
                        Text {
                            width: parent.width
                            text: (card.model.isCurrent ? "Current · " : card.model.isPast ? "Past · " : "") + card.model.artist + " // " + card.durationLabel
                            color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 14; elide: Text.ElideRight
                        }
                        Text {
                            visible: card.model.availability !== "available"
                            text: card.model.availability === "checking" ? "Checking…" : "Unavailable"
                            color: Theme.muted; font.family: Theme.fontFamily; font.pixelSize: 12
                        }
                    }
                    IconButton {
                        objectName: "queuePlayEntry"
                        anchors.right: removeButton.left; anchors.verticalCenter: parent.verticalCenter
                        width: 32; height: 40; iconName: "play"; accessibleName: "Play " + card.model.title
                        enabled: card.model.availability !== "unavailable"
                        onActiveFocusChanged: if (activeFocus) card.reveal()
                        onClicked: root.playRequested(card.entryId)
                    }
                    IconButton {
                        id: removeButton
                        objectName: "queueRemoveEntry"
                        anchors.right: handle.left; anchors.verticalCenter: parent.verticalCenter
                        width: 32; height: 40; iconName: "x"; accessibleName: "Remove " + card.model.title
                        onActiveFocusChanged: if (activeFocus) card.reveal()
                        onClicked: root.removeRequested(card.entryId)
                    }
                    Basic.Button {
                        id: handle
                        objectName: "queueDragHandle"
                        anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                        width: 32; height: 40; text: "≡"
                        Accessible.name: "Move " + card.model.title + "; Control Up or Down"
                        onActiveFocusChanged: if (activeFocus) card.reveal()
                        MouseArea {
                            anchors.fill: parent; preventStealing: true
                            property point pressedAt: Qt.point(0, 0)
                            onPressed: mouse => { list.currentIndex = card.index; pressedAt = Qt.point(mouse.x, mouse.y); root.dragId = card.entryId; }
                            onPositionChanged: mouse => {
                                if (!pressed || root.dragId !== card.entryId) return;
                                if (Math.abs(mouse.y - pressedAt.y) + Math.abs(mouse.x - pressedAt.x) > 8) root.dragging = true;
                                if (root.dragging) root.updateDrop(mapToItem(list, mouse.x, mouse.y));
                            }
                            onReleased: root.dropDrag()
                            onCanceled: root.cancelDrag()
                        }
                    }
                }
            }
            Rectangle {
                objectName: "queueInsertionIndicator"
                parent: list.contentItem
                visible: root.dragging && root.insertionIndex >= 0
                x: 0; y: list.originY + Math.max(0, Math.min(root.insertionIndex * 96 - 7, list.contentHeight - 3))
                width: list.width; height: 3; color: Theme.accent; z: 10
            }
        }
    }
}
