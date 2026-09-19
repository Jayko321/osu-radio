pragma ComponentBehavior: Bound
import QtQuick

QtObject {
    id: probe
    // Context properties are installed by the native test harness after the real root loads.
    // qmllint disable unqualified
    readonly property var window: probeWindow
    readonly property bool gallery: probeGallery
    readonly property string testCase: probeCase
    // qmllint enable unqualified
    readonly property var bridge: gallery || !window ? null : window.appBridge
    property int stage: 0
    property int notifications: 0
    property int rowChanges: 0
    property int resets: 0
    property int retainedResets: 0
    property bool passed: false
    readonly property var mock: gallery && window ? window.galleryStore.bridge : null
    readonly property Connections mockConnection: Connections {
        target: probe.mock
        function onStateJsonChanged() { probe.notifications++; }
    }
    readonly property Connections modelConnection: Connections {
        target: probe.bridge
        ignoreUnknownSignals: true
        function onDataChanged() { probe.rowChanges++; }
        function onModelReset() { probe.resets++; }
    }
    function check(condition: bool, message: string): void {
        if (!condition) throw new Error("Adapter probe: " + message);
    }
    function finish(): void {
        passed = true;
        console.info("Adapter probe passed: " + testCase);
        window.close();
        Qt.quit();
    }
    function findChild(item: var, name: string): var {
        if (item.objectName === name) return item;
        const children = item.children || [];
        for (let i = 0; i < children.length; ++i) {
            const found = findChild(children[i], name);
            if (found) return found;
        }
        return null;
    }
    function galleryProbe(): void {
        const original = JSON.parse(mock.stateJson);
        mock.dispatch("galleryDisabled", "true");
        mock.dispatch("press", "");
        mock.dispatch("cycleFilter", "");
        mock.dispatch("field", "blocked");
        check(notifications === 1 && JSON.parse(mock.stateJson).gallery.presses === 0, "disabled suppression");
        mock.dispatch("galleryDisabled", "false");
        mock.dispatch("press", "");
        check(notifications === 3 && JSON.parse(mock.stateJson).gallery.presses === 1, "reenable notification");
        mock.dispatch("unknown", "");
        check(notifications === 3, "unknown actions must not notify");
        check(original.gallery.presses === 0, "gallery starts clean");
        const last = original.gallery.menu_items.length - 1;
        mock.dispatch("menuQuery", original.gallery.menu_items[last]);
        const menu = findChild(window.contentItem, "searchableMenu");
        check(menu.entries.length === 1 && menu.entries[0].index === last, "gallery search retains original menu index");
        menu.choose(0);
        check(JSON.parse(mock.stateJson).gallery.menu_selected === last, "gallery menu value role uses original index");
        check(window.objectName === "galleryWindow", "standalone gallery root");
        finish();
    }
    function searchProbe(): void {
        if (!bridge.connected || bridge.libraryLoading) return;
        const search = findChild(window.contentItem, "songSearch");
        switch (stage) {
        case 0:
            if (bridge.trackCount !== 3) return;
            bridge.selectTrack(42);
            search.text = "r";
            search.text = "ro";
            search.text = "roc hard";
            stage = 1;
            break;
        case 1:
            check(bridge.trackCount === 1 && bridge.selectedAudioId === 42, "search preserves selected audio ID");
            check(bridge.selectedSubtitle === "Fixture artist | Hard", "filtered split label remains stable");
            search.text = "missing";
            stage = 2;
            break;
        case 2:
            check(bridge.trackCount === 0 && bridge.libraryMessage === "Nothing found.", "separate search empty state");
            search.text = "retry";
            stage = 3;
            break;
        case 3:
            check(bridge.libraryMessage.includes("fixture search"), "search error visible");
            bridge.refreshLibrary();
            stage = 4;
            break;
        case 4:
            check(bridge.trackCount === 1 && bridge.selectedAudioId === 42, "refresh retries current query");
            search.text = "";
            stage = 5;
            break;
        case 5:
            check(bridge.trackCount === 3 && bridge.selectedAudioId === 42, "clear restores library and selection");
            finish();
            break;
        }
    }
    function step(): void {
        if (passed || gallery) return;
        if (testCase === "search") { searchProbe(); return; }
        if (testCase === "requests") {
            if (!bridge.connected || bridge.folders.length !== 2) return;
            check(bridge.libraryLoading, "library HTTP request remains pending at close");
            finish();
            return;
        }
        if (testCase === "empty") {
            if (!bridge.connected || bridge.libraryLoading || bridge.foldersLoading) return;
            check(bridge.trackCount === 0 && !bridge.hasSelection, "empty real library");
            check(bridge.selectedAudioId === -1 && bridge.selectedArtworkUrl === "", "explicit empty selection");
            check(bridge.folders.length === 0 && bridge.selectedFolderId === -1, "empty real folders");
            check(bridge.libraryMessage.length > 0, "empty library explanation");
            finish();
            return;
        }
        switch (stage) {
        case 0:
            if (bridge.connecting || bridge.connectionStatus.length === 0) return;
            check(!bridge.connected && !bridge.folderBusy, "startup failure state");
            stage = 1;
            bridge.connectSession();
            break;
        case 1:
            if (!bridge.connected || bridge.libraryLoading || bridge.foldersLoading) return;
            if (bridge.folders.length !== 2 || !bridge.libraryMessage.includes("fixture library")) return;
            check(bridge.trackCount === 0 && !bridge.hasSelection, "library failure independent from folders");
            check(bridge.selectedFolderId === 31, "first folder by ID");
            check(findChild(window.contentItem, "addFolder").enabled, "Add enabled after independent folder load");
            check(bridge.folders[1].id === 52 && bridge.folders[1].label === "lazer - /fixtures/52", "typed folder entries");
            stage = 2;
            bridge.refreshLibrary();
            break;
        case 2:
            if (bridge.libraryLoading || bridge.trackCount !== 3 || bridge.selectedDurationLabel !== "02:05" || bridge.selectedArtworkUrl === "") return;
            const list = findChild(window.contentItem, "songList");
            const card = list.itemAtIndex(0);
            if (!card) return;
            check(card.audioId === 7 && card.title === "Track 7" && card.durationLabel === "02:05", "typed live model roles");
            check(card.subtitle === "Fixture artist | Hard", "shared subtitle role");
            check(bridge.selectedAudioId === 7 && bridge.selectedTitle === "Track 7", "first track uses database ID");
            check(rowChanges > 0, "media uses targeted row notifications");
            retainedResets = resets;
            stage = 3;
            bridge.selectTrack(42);
            break;
        case 3:
            if (bridge.selectedAudioId !== 42 || bridge.selectedDurationLabel !== "02:05" || bridge.selectedArtworkUrl === "") return;
            check(bridge.selectedTitle === "Track 42" && bridge.selectedSubtitle === "Fixture artist | Hard", "shared selection formatting");
            check(resets === retainedResets, "selection and media must not replace the model");
            findChild(window.contentItem, "settingsSearch").text = "settings query";
            window.selectedTab = 1;
            check(findChild(window.contentItem, "selectedTitle").text === "Track 42", "persistent player follows selection");
            check(findChild(window.contentItem, "songSearch").text === "", "search fields are independent");
            check(bridge.selectedAudioId === 42, "settings preserves player selection");
            findChild(window.contentItem, "folderMenu").choose(1);
            check(bridge.selectedAudioId === 42, "folder selection does not filter songs");
            stage = 4;
            bridge.refreshFolders();
            break;
        case 4:
            if (bridge.foldersLoading || !bridge.folderMessage.includes("fixture folders")) return;
            check(bridge.folders.length === 2 && bridge.selectedFolderId === 52, "failed folder refresh retains rows and ID");
            check(bridge.folderCanRetry && !bridge.folderBusy, "folder failure offers retry");
            stage = 5;
            bridge.retryFolders();
            break;
        case 5:
            if (bridge.foldersLoading || bridge.folderCanRetry) return;
            check(bridge.selectedFolderId === 52 && bridge.folders.length === 2, "folder retry retains selected ID");
            stage = 6;
            bridge.refreshLibrary();
            break;
        case 6:
            if (bridge.libraryLoading || !bridge.libraryMessage.includes("fixture library")) return;
            check(bridge.trackCount === 3 && bridge.selectedAudioId === 42, "failed refresh retains rows and selection");
            check(resets === retainedResets, "failed refresh must not reset model");
            stage = 7;
            bridge.refreshLibrary();
            break;
        case 7:
            if (bridge.libraryLoading || bridge.trackCount !== 1) return;
            check(bridge.selectedAudioId === 103 && bridge.selectedTitle === "Track 103", "shrinking refresh selects first remaining ID");
            stage = 8;
            bridge.refreshLibrary();
            break;
        case 8:
            if (bridge.libraryLoading || bridge.trackCount !== 0) return;
            check(!bridge.hasSelection && bridge.selectedAudioId === -1, "empty refresh clears selection");
            check(bridge.selectedArtworkUrl === "" && bridge.selectedDurationLabel === "--:--", "empty player clears media");
            finish();
            break;
        }
    }
    readonly property Timer poll: Timer {
        interval: 10
        repeat: true
        running: !probe.passed && !probe.gallery
        onTriggered: probe.step()
    }
    Component.onCompleted: {
        if (gallery) galleryProbe();
    }
}
