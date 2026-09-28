pragma ComponentBehavior: Bound
import QtQuick
import QtTest

QtObject {
    id: probe
    // Context properties are installed by the native test harness after the real root loads.
    // qmllint disable unqualified
    readonly property var window: probeWindow
    readonly property bool gallery: probeGallery
    readonly property string testCase: probeCase
    readonly property string screenshotPath: probeScreenshotPath
    // qmllint enable unqualified
    readonly property var bridge: gallery || !window ? null : window.appBridge
    property int stage: 0
    onStageChanged: if (testCase === "playback") console.info("Playback probe stage", stage)
    property int notifications: 0
    property int rowChanges: 0
    property int resets: 0
    property int retainedResets: 0
    property bool passed: false
    readonly property TestCase input: TestCase { parent: probe.window ? probe.window.contentItem : null; when: false }
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
        if (!item) return null;
        if (item.objectName === name) return item;
        // Popups are QObject data, and their content is reparented into the overlay.
        if (item.contentItem && item.contentItem !== item) {
            const content = findChild(item.contentItem, name);
            if (content) return content;
        }
        const children = item.data && typeof item.data !== "function" ? item.data : (item.children || []);
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
        const opener = findChild(window.contentItem, "openModal");
        opener.forceActiveFocus();
        opener.clicked();
        stage = 10;
    }
    function contained(content: var, item: var): bool {
        while (item) { if (item === content) return true; item = item.parent; }
        return false;
    }
    function galleryModalProbe(): void {
        const dialog = findChild(window.contentItem, "playlistDialog");
        const folderDialog = findChild(window.contentItem, "folderSelectionDialog");
        switch (stage) {
        case 10:
            if (!dialog.opened) return;
            check(contained(dialog.contentItem, window.activeFocusItem), "modal initial focus");
            for (let i = 0; i < 10; ++i) {
                input.keyClick(Qt.Key_Tab, Qt.NoModifier, 0);
                check(contained(dialog.contentItem, window.activeFocusItem), "Tab contained in modal");
                input.keyClick(Qt.Key_Tab, Qt.ShiftModifier, 0);
                check(contained(dialog.contentItem, window.activeFocusItem), "Shift+Tab contained in modal");
            }
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0);
            stage = 11;
            break;
        case 11:
            if (dialog.visible) return;
            check(findChild(window.contentItem, "openModal").activeFocus, "modal restores opener focus");
            dialog.open();
            stage = 12;
            break;
        case 12:
            if (!dialog.opened) return;
            input.mouseClick(window.contentItem, 4, 120, Qt.LeftButton, Qt.NoModifier, 0);
            stage = 13;
            break;
        case 13:
            if (dialog.visible) return;
            findChild(window.contentItem, "openFolderModal").clicked();
            stage = 14;
            break;
        case 14:
            if (!folderDialog.opened) return;
            check(folderDialog.width === 740 && folderDialog.height === 620, "reference modal proportions");
            window.width = 1024;
            window.height = 640;
            mock.dispatch("folderToggle", "/demo/osu/1/marker");
            mock.dispatch("folderToggle", "/demo/osu/2/marker");
            mock.dispatch("folderApply", "");
            stage = 15;
            break;
        case 15:
            check(folderDialog.width <= window.width - 48 && folderDialog.height <= window.height - 48, "responsive modal fits window");
            check(folderDialog.rows[1].registered && folderDialog.rows[1].action === "", "gallery successful sibling saved");
            check(folderDialog.rows[2].action === "add" && folderDialog.rows[2].error.length > 0, "gallery failed sibling retained");
            if (screenshotPath.length > 0) {
                input.grabImage(window.contentItem).save(screenshotPath);
            }
            mock.dispatch("folderApply", "");
            folderDialog.close();
            stage = 16;
            break;
        case 16:
            if (folderDialog.visible) return;
            finish();
            break;
        }
    }
    function folderProbe(): void {
        if (!bridge.connected || bridge.foldersLoading || bridge.libraryLoading) return;
        const dialog = findChild(window.contentItem, "folderSelectionDialog");
        const fresh = "/fixtures/101/osu!.db";
        const failed = "/fixtures/102/client.realm";
        switch (stage) {
        case 0:
            bridge.addFolder();
            stage = 1;
            break;
        case 1:
            if (bridge.folderSelectionDiscovering || bridge.folderSelectionRows.length !== 4 || !dialog.opened) return;
            if (bridge.folderSelectionRows.some(row => row.countPending)) return;
            check(bridge.folderSelectionRows[3].countError.includes("fixture preview"), "count failure is separate from staging");
            bridge.retryFolderCount(failed);
            stage = 2;
            break;
        case 2:
            if (bridge.folderSelectionRows[3].countPending) return;
            check(bridge.folderSelectionRows[3].count === "0", "zero is a real count");
            bridge.toggleFolderSelection(fresh);
            bridge.toggleFolderSelection(fresh);
            stage = 3;
            break;
        case 3:
            if (bridge.folderSelectionRows[2].action !== "") return;
            check(bridge.folders.length === 2, "choosing and staging do not save");
            bridge.toggleFolderSelection(fresh);
            bridge.toggleFolderSelection(failed);
            bridge.toggleFolderSelection("/fixtures/31/client.realm");
            stage = 4;
            break;
        case 4:
            if (bridge.folderSelectionRows[2].action !== "add" || bridge.folderSelectionRows[0].action !== "remove") return;
            bridge.applyFolderSelection();
            stage = 5;
            break;
        case 5:
            if (!bridge.folderSelectionApplying) return;
            check(!dialog.dismissible && !findChild(dialog.contentItem, "applyFolders").enabled, "Apply disables editing and dismissal");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0);
            bridge.closeFolderSelection();
            stage = 6;
            break;
        case 6:
            if (bridge.folderSelectionApplying) return;
            check(bridge.folderSelectionOpen && dialog.visible, "partial failure remains open");
            check(bridge.folderSelectionRows[2].registered && bridge.folderSelectionRows[2].action === "", "successful addition clears immediately");
            check(!bridge.folderSelectionRows[0].registered && bridge.folderSelectionRows[0].action === "", "successful removal clears immediately");
            check(bridge.folderSelectionRows[3].action === "add" && bridge.folderSelectionRows[3].error.includes("fixture import"), "failed action retained");
            check(bridge.folders.length === 2, "saved folders refreshed after partial success");
            bridge.applyFolderSelection();
            stage = 7;
            break;
        case 7:
            if (bridge.folderSelectionOpen || bridge.folderSelectionApplying) return;
            check(bridge.folders.length === 3, "retry saves remaining action");
            bridge.addFolder();
            stage = 8;
            break;
        case 8:
            if (!bridge.folderSelectionOpen) return;
            bridge.closeFolderSelection();
            stage = 9;
            break;
        case 9:
            if (bridge.folderSelectionOpen) return;
            check(bridge.folderSelectionRows.length === 0, "closing cancels and discards preview state");
            finish();
            break;
        }
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
    function playbackProbe(): void {
        if (!bridge.connected || bridge.libraryLoading) return;
        const play = findChild(window.contentItem, "playPauseButton");
        const progress = findChild(window.contentItem, "playbackProgress");
        switch (stage) {
        case 0:
            if (bridge.trackCount !== 3) return;
            check(bridge.currentAudioId === -1 && bridge.loadingAudioId === -1, "selection alone does not start audio");
            check(play.enabled && play.accessibleName === "Play", "selected track can play");
            check(!progress.enabled && bridge.playbackPositionLabel === "00:00", "noncurrent selection has no seeking");
            bridge.selectTrack(42);
            stage = 1;
            break;
        case 1:
            if (bridge.selectedAudioId !== 42) return;
            findChild(window.contentItem, "volumeButton").clicked();
            stage = 2;
            break;
        case 2: {
            const volume = findChild(window.contentItem, "volumeSlider");
            if (!volume || !volume.visible) return;
            volume.value = 0.25;
            volume.moved();
            play.clicked();
            stage = 3;
            break;
        }
        case 3:
            if (bridge.loadingAudioId !== 42 || bridge.volume !== 0.25) return;
            check(bridge.playbackMessage.includes("Loading"), "loading status projects immediately");
            check(bridge.currentAudioId === -1, "download has not committed a track");
            bridge.selectTrack(7);
            stage = 4;
            break;
        case 4:
            if (bridge.selectedAudioId !== 7 || bridge.loadingAudioId !== -1) return;
            check(bridge.playbackMessage.includes("fixture audio unavailable"), "download error is visible beside controls");
            check(bridge.currentAudioId === -1 && !progress.enabled, "failed download leaves playback empty");
            check(bridge.volume === 0.25, "volume survives selection changes and failed download");
            finish();
            break;
        }
    }
    function step(): void {
        if (passed) return;
        if (gallery) { galleryModalProbe(); return; }
        if (testCase === "folders") { folderProbe(); return; }
        if (testCase === "search") { searchProbe(); return; }
        if (testCase === "playback") { playbackProbe(); return; }
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
        running: !probe.passed
        onTriggered: probe.step()
    }
    Component.onCompleted: {
        if (gallery) galleryProbe();
    }
}
