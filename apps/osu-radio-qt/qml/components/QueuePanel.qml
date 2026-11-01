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
    property real insertionY: 0
    property int upcomingCount: 0
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
    // Section labels make row positions irregular, so the drop point comes from live delegate geometry:
    // the insertion index is the first row whose midpoint lies below the pointer.
    function updateDrop(point: point): void {
        pointerPosition = point;
        const y = point.y + list.contentY;
        let at = list.count;
        for (const row of mediaDelegates)
            if (row.index >= 0 && row.index < at && row.y + row.height / 2 > y) at = row.index;
        insertionIndex = at;
        const anchor = list.itemAtIndex(Math.min(at, list.count - 1));
        if (anchor) insertionY = at < list.count ? anchor.y - 7 : anchor.y + anchor.height + 7;
    }
    // Rows are ordered history, current, upcoming, so the first "upcoming" row is found by bisection.
    // 268 is the TrackListModel `queueSection` role.
    function refreshUpcoming(): void {
        const model = tracks;
        let low = 0, high = model ? model.rowCount(model.index(-1, -1)) : 0;
        const count = high;
        while (low < high) {
            const mid = (low + high) >> 1;
            if (model.data(model.index(mid, 0), 268) === "upcoming") high = mid; else low = mid + 1;
        }
        upcomingCount = count - low;
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
    onTracksChanged: refreshUpcoming()
    Component.onCompleted: refreshUpcoming()
    onLoadingChanged: Qt.callLater(positionOnce)
    onStartIndexChanged: Qt.callLater(positionOnce)
    parent: host
    // Offset from the right edge, animated by the enter/exit transitions.
    readonly property real scrollPosition: list.contentY
    function restoreScroll(position: real): void {
        positionPending = false;
        list.forceLayout();
        list.contentY = Math.max(list.originY, Math.min(position, list.originY + Math.max(0, list.contentHeight - list.height)));
        scheduleMedia();
    }
    property real slide: 0
    x: host.width - width + slide
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
    enter: Transition {
        ParallelAnimation {
            NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Theme.ms(Theme.motionPanel); easing.type: Theme.easing }
            NumberAnimation { property: "slide"; from: 24; to: 0; duration: Theme.ms(Theme.motionPanel); easing.type: Theme.easing }
        }
    }
    exit: Transition {
        ParallelAnimation {
            NumberAnimation { property: "opacity"; to: 0; duration: Theme.ms(Theme.motionBase); easing.type: Theme.easing }
            NumberAnimation { property: "slide"; to: 24; duration: Theme.ms(Theme.motionBase); easing.type: Theme.easing }
        }
    }
    Basic.Overlay.modal: Item {
        // Qt owns the overlay item's own opacity, so the shade fades through this child, following the panel.
        Item {
            objectName: "queueDimmerFade"
            anchors.fill: parent
            opacity: root.opacity
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
        function onRowsInserted(): void { root.refreshUpcoming(); root.scheduleMedia(); Qt.callLater(root.positionOnce); }
        function onRowsRemoved(): void { root.refreshUpcoming(); root.scheduleMedia(); }
        function onRowsMoved(): void { root.refreshUpcoming(); root.scheduleMedia(); }
        function onModelReset(): void { root.refreshUpcoming(); }
        function onDataChanged(): void { root.refreshUpcoming(); root.scheduleMedia(); }
    }
    contentItem: Item {
        Keys.onEscapePressed: event => { if (root.dragId.length) { root.cancelDrag(); event.accepted = true; } else root.close(); }
        Item {
            id: header
            width: parent.width; height: 32
            Text {
                id: title
                anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
                text: "Queue"; color: Theme.text; font.family: Theme.fontFamily
                font.pixelSize: 18; font.weight: Font.Bold
            }
            Text {
                objectName: "queueLeft"
                anchors.left: title.right; anchors.leftMargin: 6; anchors.baseline: title.baseline
                visible: list.count > 0
                text: "· " + root.upcomingCount + " left"; color: Theme.muted
                font.family: Theme.fontFamily; font.pixelSize: 14
            }
            Row {
                anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                IconButton {
                    objectName: "queueClearUpcoming"
                    width: 32; height: 32; iconName: "list-x"; icon.width: 18; icon.height: 18
                    foreground: "#b4b4b4"; accessibleName: "Clear upcoming"
                    onClicked: root.clearUpcomingRequested()
                }
                IconButton {
                    objectName: "queueClearAll"
                    width: 32; height: 32; iconName: "trash"; icon.width: 18; icon.height: 18
                    foreground: "#b4b4b4"; accessibleName: "Clear all"
                    onClicked: root.clearAllRequested()
                }
                IconButton {
                    id: closeButton
                    objectName: "queueCloseButton"
                    width: 32; height: 32; iconName: "x"; icon.width: 18; icon.height: 18
                    foreground: "#b4b4b4"; accessibleName: "Close queue"
                    onClicked: root.close()
                }
            }
        }
        Column {
            id: status
            anchors.top: header.bottom; anchors.topMargin: 8; width: parent.width; spacing: 8
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
            Basic.ScrollBar.vertical: AppScrollBar {}
            onContentYChanged: root.scheduleMedia()
            onHeightChanged: root.scheduleMedia()
            section.property: "queueSection"
            section.delegate: Item {
                required property string section
                objectName: "queueSectionLabel"
                width: list.width; height: 28
                Text {
                    anchors.left: parent.left; anchors.leftMargin: 2
                    anchors.bottom: parent.bottom; anchors.bottomMargin: 2
                    text: parent.section === "history" ? "HISTORY" : parent.section === "current" ? "NOW PLAYING" : "UP NEXT"
                    color: Theme.muted; font.family: Theme.fontFamily
                    font.pixelSize: 11; font.weight: Font.Medium; font.letterSpacing: 0.8
                }
            }
            add: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Theme.ms(Theme.motionBase); easing.type: Theme.easing } }
            remove: Transition { NumberAnimation { property: "opacity"; to: 0; duration: Theme.ms(Theme.motionBase); easing.type: Theme.easing } }
            displaced: Transition { NumberAnimation { property: "y"; duration: Theme.ms(Theme.motionBase); easing.type: Theme.easing } }
            delegate: Basic.ItemDelegate {
                id: card
                required property int index
                required property var model
                readonly property string entryId: model.entryId
                readonly property int audioId: model.audioId
                readonly property string artworkUrl: model.artworkUrl
                readonly property string durationLabel: model.durationLabel
                readonly property bool inViewport: root.visible && y + height > list.contentY && y < list.contentY + list.height
                // Actions show on hover or when focus is inside the row; keyboard focus keeps them reachable while hidden.
                readonly property bool actionsShown: rowHover.hovered || activeFocus || playButton.activeFocus
                    || removeButton.activeFocus || handle.activeFocus || root.dragId === entryId
                readonly property real dim: model.isPast ? 0.55 : 1
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
                HoverHandler { id: rowHover }
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
                    opacity: card.dim
                    Artwork { anchors.fill: parent; placeholder: false; source: card.artworkUrl }
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
                    opacity: card.dim
                    Column {
                        x: 8; anchors.verticalCenter: parent.verticalCenter
                        width: Math.max(0, parent.width - 124); spacing: 2
                        Text {
                            width: parent.width; text: card.model.title
                            color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 20
                            font.weight: Font.Bold; elide: Text.ElideRight
                        }
                        Text {
                            width: parent.width; text: card.model.artist
                            color: Theme.text; font.family: Theme.fontFamily; font.pixelSize: 14; elide: Text.ElideRight
                        }
                        Text {
                            visible: card.model.availability !== "available"
                            text: card.model.availability === "checking" ? "Checking…" : "Unavailable"
                            color: Theme.muted; font.family: Theme.fontFamily; font.pixelSize: 12
                        }
                    }
                    Text {
                        objectName: "queueDuration"
                        anchors.right: parent.right; anchors.rightMargin: 12; anchors.verticalCenter: parent.verticalCenter
                        text: card.durationLabel; color: Theme.muted; font.family: Theme.fontFamily; font.pixelSize: 14
                        opacity: card.actionsShown ? 0 : 1
                        Behavior on opacity { NumberAnimation { duration: Theme.ms(Theme.motionFast); easing.type: Theme.easing } }
                    }
                    Row {
                        id: actionRow
                        anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                        opacity: card.actionsShown ? 1 : 0
                        Behavior on opacity { NumberAnimation { duration: Theme.ms(Theme.motionFast); easing.type: Theme.easing } }
                        IconButton {
                            id: playButton
                            objectName: "queuePlayEntry"
                            width: 32; height: 40; iconName: "play"; accessibleName: "Play " + card.model.title
                            enabled: card.model.availability !== "unavailable"
                            onActiveFocusChanged: if (activeFocus) card.reveal()
                            onClicked: root.playRequested(card.entryId)
                        }
                        IconButton {
                            id: removeButton
                            objectName: "queueRemoveEntry"
                            width: 32; height: 40; iconName: "x"; accessibleName: "Remove " + card.model.title
                            onActiveFocusChanged: if (activeFocus) card.reveal()
                            onClicked: root.removeRequested(card.entryId)
                        }
                        IconButton {
                            id: handle
                            objectName: "queueDragHandle"
                            width: 32; height: 40; iconName: "grip-vertical"
                            accessibleName: "Move " + card.model.title + "; Control Up or Down"
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
                    // Hidden actions take no pointer input: a press there acts like a press on the row.
                    MouseArea {
                        objectName: "queueHiddenActionGuard"
                        anchors.fill: actionRow; enabled: !card.actionsShown
                        onClicked: card.clicked()
                    }
                }
            }
            Rectangle {
                objectName: "queueInsertionIndicator"
                parent: list.contentItem
                visible: root.dragging && root.insertionIndex >= 0
                x: 0; y: root.insertionY
                width: list.width; height: 3; color: Theme.accent; z: 10
            }
        }
    }
}
