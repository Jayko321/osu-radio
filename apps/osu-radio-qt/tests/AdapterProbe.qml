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
    readonly property string coverPath: probeCoverPath
    // qmllint enable unqualified
    readonly property var bridge: gallery || !window ? null : window.appBridge
    property int stage: 0
    onStageChanged: console.info(testCase + " probe stage", stage)
    property int notifications: 0
    property int rowChanges: 0
    property int resets: 0
    property int retainedResets: 0
    property bool passed: false
    property var sortMedia: ({})
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
        const settingsTab = findChild(window.contentItem, "settingsButton");
        check(settingsTab.text === "Settings" && !settingsTab.enabled, "gallery exposes an unavailable Settings tab");
        const sortMenu = findChild(window.contentItem, "demoTrackSortMenu");
        const selectedId = JSON.parse(mock.stateJson).tracks[JSON.parse(mock.stateJson).selected].id;
        sortMenu.choose(1);
        let sorted = JSON.parse(mock.stateJson);
        check(sortMenu.currentIndex === 1 && sorted.tracks[0].artist === "Creepy Nuts", "gallery artist sort uses shared mock action");
        check(sorted.tracks[sorted.selected].id === selectedId, "gallery sorting retains selection by ID");
        sortMenu.choose(2);
        check(sortMenu.currentIndex === 2, "gallery recent mode");
        sortMenu.choose(0);
        sorted = JSON.parse(mock.stateJson);
        check(sorted.tracks[0].title === "Alice" && sorted.tracks[sorted.selected].id === selectedId, "gallery default title sort restored");
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
        case 16: {
            if (folderDialog.visible) return;
            findChild(window.contentItem, "demoOpenPlaylists").clicked();
            check(window.selectedTab === 1, "gallery has a separate playlists tab");
            const pane = findChild(window.contentItem, "demoPlaylistPane");
            check(pane.activeId === -1 && pane.playlists.length === 1 && pane.playlists[0].itemCount === 3,
                "gallery list counts unavailable entries");
            pane.searchEdited("no-playlist-matches");
            stage = 161;
            break;
        }
        case 161: {
            const pane = findChild(window.contentItem, "demoPlaylistPane");
            const list = findChild(pane, "playlistList");
            if (list.count !== 0) return;
            check(pane.playlists.length === 0 && JSON.parse(mock.stateJson).playlists.playlists.length === 1,
                "gallery search filters actual list and preserves summary collection");
            pane.searchEdited("Evening");
            stage = 162;
            break;
        }
        case 162: {
            const pane = findChild(window.contentItem, "demoPlaylistPane");
            const list = findChild(pane, "playlistList");
            if (list.count !== 1 || !list.itemAtIndex(0)) return;
            check(list.itemAtIndex(0).modelData.name === "Evening", "gallery query matches visible playlist card");
            pane.searchEdited("");
            if (screenshotPath.length > 0) input.grabImage(window.contentItem).save(screenshotPath + ".playlists-list.png");
            findChild(list.itemAtIndex(0), "playlistActionsButton").clicked();
            stage = 163;
            break;
        }
        case 163: {
            const pane = findChild(window.contentItem, "demoPlaylistPane");
            const card = findChild(pane, "playlistList").itemAtIndex(0);
            const menu = findChild(card, "playlistActionsMenu");
            if (!menu.opened) return;
            check(menu.count === 3 && menu.itemAt(0).text === "Add to queue"
                && menu.itemAt(1).text === "Edit" && menu.itemAt(2).text === "Delete", "playlist menu action order");
            check(menu.width === 236 && menu.background.radius === 12, "rounded playlist menu proportions");
            check(menu.itemAt(0).iconName === "add-to-queue" && menu.itemAt(1).iconName === "playlist-edit"
                && menu.itemAt(2).iconName === "playlist-delete", "supplied trailing icons");
            check(menu.itemAt(2).foreground.toString() === "#ed6666", "Delete is red");
            if (screenshotPath.length > 0) input.grabImage(window.contentItem).save(screenshotPath + ".playlist-actions.png");
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            stage = 164;
            break;
        }
        case 164: {
            const pane = findChild(window.contentItem, "demoPlaylistPane");
            const card = findChild(pane, "playlistList").itemAtIndex(0);
            check(pane.message.includes("offline gallery"), "Add to queue dispatches the offline action");
            check(findChild(card, "playlistActionsButton").activeFocus, "menu restores focus to three dots");
            card.forceActiveFocus();
            input.keyClick(Qt.Key_F10, Qt.ShiftModifier, 0);
            stage = 165;
            break;
        }
        case 165: {
            const pane = findChild(window.contentItem, "demoPlaylistPane");
            const menu = findChild(findChild(pane, "playlistList").itemAtIndex(0), "playlistActionsMenu");
            if (!menu.opened) return;
            input.keyClick(Qt.Key_Down, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            check(pane.editorOpen && pane.editorId === 1, "Edit opens the selected playlist editor");
            pane.cancelRequested();
            pane.selectRequested(1);
            stage = 17;
            break;
        }
        case 17: {
            const pane = findChild(window.contentItem, "demoPlaylistPane");
            const view = JSON.parse(mock.stateJson).playlists;
            if (pane.tracksList.count !== 3) return;
            check(view.active.items[0].audio_source_id === view.active.items[1].audio_source_id, "separate demo difficulties share audio");
            check(view.active.items[2].audio_source_id === null, "demo unavailable membership");
            if (screenshotPath.length > 0) input.grabImage(window.contentItem).save(screenshotPath + ".playlist-detail.png");
            pane.selectRequested(-1);
            pane.createRequested();
            pane.nameEdited(" New ");
            pane.coverRequested();
            check(JSON.parse(mock.stateJson).playlist_cover_present, "gallery cover choice previews in inline editor");
            pane.coverResetRequested();
            check(!JSON.parse(mock.stateJson).playlist_cover_present, "gallery cover reset");
            pane.coverRequested();
            pane.saveRequested();
            check(JSON.parse(mock.stateJson).playlists.playlists.length === 2 && !pane.editorOpen, "offline create closes inline card");
            pane.editRequested(2);
            check(!JSON.parse(mock.stateJson).playlist_cover_present && pane.editorArtworkUrl.toString().length > 0,
                "gallery editor previews saved cover without a new draft");
            pane.coverResetRequested();
            check(pane.editorArtworkUrl.toString().length === 0, "gallery reset hides saved cover preview");
            pane.cancelRequested();
            pane.editRequested(2);
            check(pane.editorArtworkUrl.toString().length > 0, "cancelled reset preserves saved cover on reopening");
            pane.cancelRequested();
            mock.dispatch("playlistAddOpen", "");
            stage = 18;
            break;
        }
        case 18: {
            const playlists = findChild(window.contentItem, "demoPlaylistsDialog");
            if (!playlists.opened || !playlists.adding) return;
            check(contained(playlists.contentItem, window.activeFocusItem), "add modal focus");
            check(playlists.candidates.length === 2, "add modal shows concrete difficulties");
            playlists.targetRequested(2);
            playlists.difficultyRequested(1);
            check(!playlists.candidates[0].checked && playlists.candidates[1].checked, "difficulty checkbox selection");
            playlists.addRequested();
            mock.dispatch("playlistSelect", "2");
            stage = 19;
            break;
        }
        case 19: {
            const pane = findChild(window.contentItem, "demoPlaylistPane");
            if (pane.tracksList.count !== 1) return;
            const view = JSON.parse(mock.stateJson).playlists;
            check(view.active.items.length === 1 && view.active.items[0].difficulty_name === "Hard", "only selected difficulty is added");
            const card = pane.tracksList.itemAtIndex(0);
            if (!card) return;
            card.forceActiveFocus();
            input.keyClick(Qt.Key_F10, Qt.ShiftModifier, 0);
            stage = 20;
            break;
        }
        case 20: {
            const pane = findChild(window.contentItem, "demoPlaylistPane");
            const card = pane.tracksList.itemAtIndex(0);
            if (!card) return;
            const menu = findChild(card, "playlistItemMenu");
            if (!menu.opened) return;
            const menuOrigin = menu.contentItem.mapToItem(window.contentItem, 0, 0);
            const cardOrigin = card.mapToItem(window.contentItem, 0, 0);
            check(Math.abs(menuOrigin.x - cardOrigin.x) < 20, "keyboard removal menu is anchored to its card");
            check(contained(menu.contentItem, window.activeFocusItem), "removal menu receives keyboard focus");
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            stage = 21;
            break;
        }
        case 21: {
            const pane = findChild(window.contentItem, "demoPlaylistPane");
            if (pane.tracksList.count !== 0) return;
            check(JSON.parse(mock.stateJson).playlists.active.items.length === 0, "keyboard context menu removes difficulty");
            pane.selectRequested(-1);
            stage = 22;
            break;
        }
        case 22: {
            const pane = findChild(window.contentItem, "demoPlaylistPane");
            const card = findChild(pane, "playlistList").itemAtIndex(1);
            if (!card) return;
            card.forceActiveFocus();
            input.keyClick(Qt.Key_F10, Qt.ShiftModifier, 0);
            stage = 23;
            break;
        }
        case 23: {
            const pane = findChild(window.contentItem, "demoPlaylistPane");
            const menu = findChild(findChild(pane, "playlistList").itemAtIndex(1), "playlistActionsMenu");
            if (!menu.opened) return;
            check(!menu.itemAt(0).enabled && menu.currentIndex === 1, "empty playlist skips Add to queue");
            input.keyClick(Qt.Key_Down, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            stage = 24;
            break;
        }
        case 24: {
            const pane = findChild(window.contentItem, "demoPlaylistPane");
            if (pane.playlists.length !== 1) return;
            check(pane.playlists[0].id === 1, "Delete removes the selected playlist");
            finish();
            break;
        }
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
            if (!bridge.folderSelectionOpen || bridge.folderSelectionRows.length === 0 || !dialog.opened
                || bridge.folderSelectionDiscovering || bridge.folderSelectionRows.some(row => row.countPending)) return;
            const refresh = findChild(dialog.contentItem, "folderRefresh0");
            check(refresh.accessibleName === "Обновить импорт", "registered row has accessible refresh control");
            refresh.forceActiveFocus();
            input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
            stage = 9;
            break;
        case 9:
            if (bridge.folderSelectionRows[0].action !== "refresh") return;
            bridge.toggleFolderSelection("/fixtures/52/client.realm");
            stage = 10;
            break;
        case 10:
            if (bridge.folderSelectionRows[0].action !== "remove") return;
            bridge.refreshFolderSelection("/fixtures/52/client.realm");
            stage = 11;
            break;
        case 11:
            if (bridge.folderSelectionRows[0].action !== "refresh") return;
            bridge.applyFolderSelection();
            stage = 12;
            break;
        case 12:
            if (bridge.folderSelectionOpen || bridge.folderSelectionApplying) return;
            check(bridge.folders.length === 3, "refresh preserves registered folders");
            check(bridge.folderSelectionRows.length === 0, "successful refresh closes and discards preview state");
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
            findChild(window.contentItem, "settingsSearch").text = "settings query";
            bridge.selectTrack(42);
            search.text = "r";
            search.text = "ro";
            search.text = "roc hard";
            stage = 1;
            break;
        case 1:
            check(bridge.trackCount === 1 && bridge.selectedAudioId === 42, "search preserves selected audio ID");
            check(bridge.selectedSubtitle === "Fixture artist | Hard", "filtered split label remains stable");
            check(findChild(window.contentItem, "settingsSearch").text === "settings query", "Songs search preserves the independent Settings query");
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
        if (!bridge.connected || bridge.libraryLoading || !bridge.audioSettingsLoaded) return;
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
            const popup = findChild(window.contentItem, "volumePopup");
            if (!popup.opened) return;
            check(contained(popup.contentItem, window.activeFocusItem), "volume popup receives keyboard focus");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0);
            stage = 21;
            break;
        }
        case 21:
            if (findChild(window.contentItem, "volumePopup").visible) return;
            check(findChild(window.contentItem, "volumeButton").activeFocus, "Escape restores volume button focus");
            findChild(window.contentItem, "volumeButton").clicked();
            stage = 22;
            break;
        case 22: {
            if (!findChild(window.contentItem, "volumePopup").opened) return;
            const volume = findChild(window.contentItem, "volumeSlider");
            if (!volume || !volume.visible) return;
            volume.value = 0.25;
            volume.moved();
            play.clicked();
            stage = 3;
            break;
        }
        case 3:
            if (bridge.loadingAudioId !== 42 || Math.round(bridge.volume * 100) !== 25) return;
            check(bridge.playbackMessage.includes("Loading"), "loading status projects immediately");
            check(bridge.currentAudioId === 42 && !progress.enabled, "assignment selects current before audio finishes loading");
            bridge.selectTrack(7);
            stage = 4;
            break;
        case 4:
            if (bridge.selectedAudioId !== 7 || bridge.loadingAudioId !== -1 || bridge.currentAudioId !== -1) return;
            check(bridge.playbackMessage.includes("fixture audio unavailable"), "download error is visible beside controls");
            check(bridge.currentAudioId === -1 && !progress.enabled, "failed download leaves playback empty");
            check(Math.round(bridge.volume * 100) === 25, "volume survives selection changes and failed download");
            finish();
            break;
        }
    }
    function volumeProbe(): void {
        if (!bridge.connected || bridge.libraryLoading) return;
        const toggle = findChild(window.contentItem, "individualVolumeSwitch");
        const global = findChild(window.contentItem, "globalVolumeSlider");
        const play = findChild(window.contentItem, "playPauseButton");
        const popup = findChild(window.contentItem, "volumePopup");
        const slider = findChild(window.contentItem, "volumeSlider");
        switch (stage) {
        case 0:
            if (!bridge.audioSettingsCanRetry) return;
            check(!bridge.audioSettingsLoaded && !play.enabled && !toggle.enabled, "loading failure blocks playback and editing");
            input.mouseClick(findChild(window.contentItem, "settingsButton"));
            check(findChild(window.contentItem, "audioSettingsMessage").visible, "loading failure appears in Audio");
            const settingsScroll = findChild(window.contentItem, "settingsScroll");
            check(input.waitForRendering(settingsScroll, 1000), "Settings renders before scrolling to Audio");
            const settingsContent = settingsScroll.contentItem;
            settingsContent.contentY = Math.max(0, settingsContent.contentHeight - settingsContent.height);
            check(input.waitForRendering(settingsScroll, 1000), "Audio scroll position renders before activation");
            const retry = findChild(window.contentItem, "retryAudioSettings");
            const retryCenter = retry.mapToItem(settingsScroll, retry.width / 2, retry.height / 2);
            check(retryCenter.x >= 0 && retryCenter.x < settingsScroll.width
                && retryCenter.y >= 0 && retryCenter.y < settingsScroll.height,
                "Audio Retry is reachable inside the Settings viewport");
            input.mouseClick(retry); stage = 1; break;
        case 1:
            if (!bridge.audioSettingsLoaded || bridge.currentAudioId !== 7) return;
            check(bridge.globalVolumePercent === 20 && Math.round(bridge.volume * 100) === 20 && !global.visible, "loaded general volume");
            input.mouseClick(toggle);
            stage = 2; break;
        case 2:
            if (!bridge.individualVolumeEnabled) return;
            check(global.visible && global.enabled, "individual mode shows general slider");
            bridge.selectTrack(42); stage = 3; break;
        case 3:
            if (bridge.selectedAudioId !== 42) return;
            findChild(window.contentItem, "volumeButton").clicked(); stage = 4; break;
        case 4:
            if (!popup.opened) return;
            check(slider.enabled && Math.round(bridge.volume * 100) === 20, "B inherits general volume");
            slider.value = 0.1; slider.moved(); stage = 5; break;
        case 5:
            if (!bridge.audioSettingsCanRetry || Math.round(bridge.volume * 100) !== 10) return;
            check(bridge.currentAudioId === 7 && bridge.volumeHasOverride, "editing B preserves current A and exposes retry");
            findChild(window.contentItem, "retryVolumeSave").clicked(); stage = 6; break;
        case 6:
            if (bridge.audioSettingsCanRetry) return;
            global.value = 40; global.moved(); stage = 7; break;
        case 7:
            if (bridge.globalVolumePercent !== 40) return;
            check(Math.round(bridge.volume * 100) === 10, "absolute B volume survives general increase");
            input.mouseClick(findChild(window.contentItem, "songsTabButton"));
            input.mouseClick(findChild(window.contentItem, "settingsButton"));
            check(toggle.checked && global.value === 40 && bridge.selectedAudioId === 42, "Settings navigation retains volume preferences and selection");
            findChild(window.contentItem, "useGlobalVolumeButton").clicked(); stage = 8; break;
        case 8:
            if (bridge.volumeHasOverride || Math.round(bridge.volume * 100) !== 40) return;
            slider.value = 0.1; slider.moved(); stage = 9; break;
        case 9:
            if (!bridge.volumeHasOverride || Math.round(bridge.volume * 100) !== 10) return;
            input.mouseClick(toggle); stage = 10; break;
        case 10:
            if (bridge.individualVolumeEnabled) return;
            check(Math.round(bridge.volume * 100) === 40 && !global.visible && !popup.opened, "disabled mode uses general and ends popup editing");
            input.mouseClick(toggle); stage = 11; break;
        case 11:
            if (!bridge.individualVolumeEnabled) return;
            check(Math.round(bridge.volume * 100) === 10 && bridge.volumeHasOverride, "reenabling restores B override");
            bridge.selectTrack(7); stage = 12; break;
        case 12:
            if (bridge.selectedAudioId !== 7) return;
            check(Math.round(bridge.volume * 100) === 40, "A retains general volume");
            findChild(window.contentItem, "volumeButton").clicked(); stage = 13; break;
        case 13:
            if (!popup.opened) return;
            bridge.selectTrack(-1); stage = 14; break;
        case 14:
            if (bridge.hasSelection) return;
            check(!popup.opened && !bridge.volumeEnabled && !slider.enabled, "selection change closes editing and disables unavailable volume");
            bridge.setGlobalVolume(55);
            finish(); break;
        }
    }
    function queueProbe(): void {
        if (!bridge.connected || bridge.libraryLoading) return;
        const next = findChild(window.contentItem, "nextTrackButton");
        const previous = findChild(window.contentItem, "previousTrackButton");
        const queueButton = findChild(window.contentItem, "queueButton");
        const panel = findChild(window.contentItem, "queuePanel");
        switch (stage) {
        case 0:
            if (bridge.currentAudioId !== 7 || !next.enabled || !previous.enabled) return;
            check(bridge.selectedAudioId === 7 && !bridge.selectedIsPlaying, "restored queue selects current and stays paused");
            const queueOrigin = queueButton.mapToItem(window.contentItem, 0, 0);
            check(queueOrigin.y === 0 && queueButton.height === 50 && queueOrigin.x + queueButton.width === window.width - 3 * 46,
                "queue button is in the title bar immediately before window controls");
            queueButton.forceActiveFocus();
            input.mouseClick(queueButton, queueButton.width / 2, queueButton.height / 2, Qt.LeftButton, Qt.NoModifier, 0);
            stage = 10; break;
        case 10:
            if (!panel.opened || bridge.queueLoading || !bridge.queueMessage.includes("fixture queue unavailable")) return;
            check(panel.width === 430 && panel.x + panel.width === findChild(window.contentItem, "playerPane").width,
                "queue panel aligns with the player right edge");
            findChild(window.contentItem, "queueRetryButton").clicked(); stage = 11; break;
        case 11:
            if (bridge.queueLoading || bridge.queueTracks.length !== 2) return;
            if (bridge.queueTracks.some(track => track.artworkUrl.length === 0 || track.durationLabel !== "02:05")) return;
            check(bridge.queueTracks[0].audioId === 42 && bridge.queueTracks[1].audioId === 103, "queue excludes current track and preserves order");
            check(contained(panel.contentItem, window.activeFocusItem), "queue opens with keyboard focus");
            for (let i = 0; i < 4; ++i) {
                input.keyClick(Qt.Key_Tab, Qt.NoModifier, 0);
                check(contained(panel.contentItem, window.activeFocusItem), "queue contains Tab focus");
            }
            panel.tracks = Array.from({length: 16}, (_, i) => bridge.queueTracks[i % 2]);
            stage = 18; break;
        case 18: {
            const scroll = findChild(window.contentItem, "queueScroll").contentItem;
            check(scroll.contentHeight > scroll.height, "long queues require vertical scrolling");
            input.mouseWheel(scroll, 100, scroll.height / 2, 0, -120, Qt.NoButton, Qt.NoModifier, 0);
            stage = 19; break;
        }
        case 19: {
            const scroll = findChild(window.contentItem, "queueScroll").contentItem;
            check(scroll.contentY > 0, "queue wheel scrolls to later songs");
            scroll.contentY = 0;
            panel.tracks = Qt.binding(() => bridge.queueTracks);
            bridge.nextTrack(); stage = 1; break;
        }
        case 1:
            if (bridge.currentAudioId !== 42) return;
            check(bridge.selectedAudioId === 42 && !bridge.selectedIsPlaying, "Next changes current and preserves pause");
            if (bridge.queueLoading || bridge.queueTracks.length !== 1 || bridge.queueTracks[0].audioId !== 103) return;
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0); stage = 12; break;
        case 12:
            if (panel.visible) return;
            check(queueButton.activeFocus, "queue Escape restores opener focus");
            previous.clicked(); stage = 2; break;
        case 2:
            if (bridge.currentAudioId !== 7) return;
            check(bridge.selectedAudioId === 7 && !bridge.selectedIsPlaying, "Previous changes current and preserves pause");
            next.clicked(); stage = 3; break;
        case 3:
            if (bridge.currentAudioId !== 42) return;
            bridge.searchLibrary("missing"); stage = 4; break;
        case 4:
            if (bridge.trackCount !== 0 || bridge.libraryLoading || bridge.selectedArtworkUrl.length === 0) return;
            check(bridge.currentAudioId === 42 && bridge.selectedAudioId === 42, "search does not discard current selection");
            check(bridge.selectedTitle === "Track 42" && bridge.selectedDurationLabel === "02:05", "current metadata survives empty search");
            check(next.enabled && previous.enabled && !bridge.selectedIsPlaying, "queue controls survive empty search on pause");
            queueButton.forceActiveFocus(); queueButton.clicked(); stage = 13; break;
        case 13:
            if (!panel.opened || bridge.queueLoading || bridge.queueTracks.length !== 1) return;
            if (bridge.queueTracks[0].artworkUrl.length === 0 || bridge.queueTracks[0].durationLabel !== "02:05") return;
            check(bridge.queueTracks[0].audioId === 103 && bridge.trackCount === 0, "queue media works outside Songs search");
            bridge.nextTrack(); stage = 14; break;
        case 14:
            if (bridge.currentAudioId !== 103 || bridge.queueLoading || bridge.queueTracks.length !== 0) return;
            check(findChild(window.contentItem, "queueStatus").text === "No upcoming songs", "exhausted upcoming list has an empty state");
            input.mouseClick(window.contentItem, 490, 150, Qt.LeftButton, Qt.NoModifier, 0); stage = 15; break;
        case 15:
            if (panel.visible) return;
            check(bridge.selectedAudioId === 103, "outside click closes queue without selecting a track");
            queueButton.clicked(); stage = 16; break;
        case 16:
            if (!panel.opened) return;
            findChild(window.contentItem, "queueCloseButton").clicked(); stage = 17; break;
        case 17:
            if (panel.visible) return;
            finish(); break;
        }
    }
    function sortedRowsMatch(order: var): bool {
        const list = findChild(window.contentItem, "songList");
        for (let i = 0; i < order.length; ++i) {
            const row = list.itemAtIndex(i);
            if (!row || row.audioId !== order[i]) return false;
            check(row.artworkUrl === sortMedia[row.audioId].artwork && row.durationLabel === sortMedia[row.audioId].duration,
                "sorting retains media by audio ID");
        }
        check(bridge.selectedAudioId === 42 && bridge.selectedTitle === "Track 42", "sorting retains selection");
        check(bridge.selectedDurationLabel === "02:05" && bridge.selectedArtworkUrl === sortMedia[42].artwork,
            "sorting retains player media");
        check(resets === retainedResets, "sorting uses row notifications without model reset");
        return true;
    }
    function sortProbe(): void {
        if (!bridge.connected || bridge.libraryLoading || bridge.trackCount !== 3) return;
        const menu = findChild(window.contentItem, "trackSortMenu");
        const list = findChild(window.contentItem, "songList");
        switch (stage) {
        case 0:
            check(menu.currentIndex === 0 && menu.entries.length === 3, "default title sort and three modes");
            bridge.selectTrack(42);
            stage = 1;
            break;
        case 1:
            if (bridge.selectedAudioId !== 42 || bridge.selectedDurationLabel !== "02:05" || bridge.selectedArtworkUrl === "") return;
            for (let i = 0; i < 3; ++i) {
                const row = list.itemAtIndex(i);
                if (!row || row.durationLabel !== "02:05" || row.artworkUrl === "") return;
            }
            check(list.itemAtIndex(0).audioId === 103 && list.itemAtIndex(2).audioId === 7, "initial title order");
            for (let i = 0; i < 3; ++i) {
                const row = list.itemAtIndex(i);
                sortMedia[row.audioId] = {artwork: row.artworkUrl, duration: row.durationLabel};
            }
            retainedResets = resets;
            menu.forceActiveFocus();
            menu.popup.open();
            stage = 2;
            break;
        case 2:
            if (!menu.popup.opened) return;
            check(contained(menu.popup.contentItem, window.activeFocusItem), "sort popup contains keyboard focus");
            input.keyClick(Qt.Key_Down, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            stage = 3;
            break;
        case 3:
            if (bridge.trackSortIndex !== 1 || menu.popup.visible || !sortedRowsMatch([42, 103, 7])) return;
            check(menu.activeFocus, "sort selection restores trigger focus");
            menu.popup.open();
            stage = 4;
            break;
        case 4:
            if (!menu.popup.opened) return;
            input.keyClick(Qt.Key_End, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
            stage = 5;
            break;
        case 5:
            if (bridge.trackSortIndex !== 2 || menu.popup.visible || !sortedRowsMatch([7, 42, 103])) return;
            check(menu.activeFocus, "recent selection restores trigger focus");
            menu.popup.open();
            stage = 6;
            break;
        case 6:
            if (!menu.popup.opened) return;
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0);
            stage = 7;
            break;
        case 7:
            if (menu.popup.visible) return;
            check(menu.activeFocus && bridge.trackSortIndex === 2, "Escape retains sort and restores focus");
            menu.popup.open();
            stage = 8;
            break;
        case 8:
            if (!menu.popup.opened) return;
            input.keyClick(Qt.Key_Home, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            stage = 9;
            break;
        case 9:
            if (bridge.trackSortIndex !== 0 || menu.popup.visible || !sortedRowsMatch([103, 42, 7])) return;
            finish();
            break;
        }
    }
    function playlistsProbe(): void {
        if (!bridge.connected || bridge.libraryLoading || bridge.foldersLoading || bridge.playlistBusy || bridge.playlistLoading) return;
        const dialog = findChild(window.contentItem, "playlistsDialog");
        const pane = findChild(window.contentItem, "playlistsPane");
        switch (stage) {
        case 0:
            if (bridge.playlists.length !== 1 || bridge.trackCount !== 1) return;
            check(!findChild(window.contentItem, "playlistsButton"), "previous playlist entry removed");
            findChild(window.contentItem, "playlistsTabButton").clicked();
            stage = 1;
            break;
        case 1: {
            if (!pane.visible || pane.activeId >= 0) return;
            check(window.selectedTab === 1 && !dialog.visible, "Playlists opens separate left pane");
            check(bridge.playlists[0].itemCount === 1, "list counts unavailable difficulty");
            const list = findChild(pane, "playlistList");
            const card = list.itemAtIndex(0);
            if (!card) return;
            card.clicked();
            stage = 2;
            break;
        }
        case 2:
            if (bridge.activePlaylistId !== 1 || bridge.trackCount !== 1 || bridge.playlistMessage.length > 0) return;
            check(bridge.selectedSubtitle.includes("Unavailable") && !bridge.selectedAvailable, "missing entry remains selectable and unavailable");
            check(!findChild(window.contentItem, "playPauseButton").enabled, "unavailable playlist entry cannot play");
            findChild(pane, "playlistBackButton").clicked();
            stage = 3;
            break;
        case 3:
            if (bridge.activePlaylistId >= 0) return;
            findChild(pane, "createPlaylistButton").clicked();
            stage = 31;
            break;
        case 31:
            if (!bridge.playlistEditorOpen) return;
            check(findChild(pane, "playlistEditor").visible, "plus opens inline create card");
            bridge.setPlaylistEditorName(" New playlist ");
            findChild(pane, "savePlaylistEditor").clicked();
            stage = 4;
            break;
        case 4:
            if (bridge.playlists.length !== 2 || bridge.playlistEditorOpen) return;
            check(bridge.playlists[1].name === "New playlist", "live creation trims name and closes editor");
            findChild(window.contentItem, "songsTabButton").clicked();
            stage = 41;
            break;
        case 41:
            if (bridge.playlistVisible || bridge.trackCount !== 1) return;
            check(findChild(window.contentItem, "songSearch").enabled, "Songs always presents searchable library");
            findChild(window.contentItem, "addToPlaylistButton").clicked();
            stage = 5;
            break;
        case 5:
            if (!dialog.opened || !bridge.playlistAdding) return;
            check(bridge.playlistCandidates.length === 2, "selected song exposes every concrete difficulty");
            dialog.targetRequested(2);
            dialog.difficultyRequested(2);
            dialog.addRequested();
            stage = 6;
            break;
        case 6:
            if (bridge.playlistOpen) return;
            findChild(window.contentItem, "songSearch").text = "no-song-matches-playlists";
            stage = 61;
            break;
        case 61:
            if (bridge.trackCount !== 0 || bridge.libraryLoading) return;
            findChild(window.contentItem, "playlistsTabButton").clicked();
            bridge.selectPlaylist(2);
            stage = 7;
            break;
        case 7:
            if (bridge.activePlaylistId !== 2 || bridge.trackCount !== 1 || bridge.playlistMessage.length > 0) return;
            check(bridge.selectedSubtitle.includes("Easy"), "chosen difficulty displayed separately");
            findChild(window.contentItem, "addToPlaylistButton").clicked();
            stage = 8;
            break;
        case 8:
            if (!dialog.opened || !bridge.playlistAdding || bridge.playlistCandidates.length !== 2) return;
            check(bridge.playlistCandidates.length === 2, "adding from playlist outside search reloads all song difficulties");
            dialog.targetRequested(2);
            dialog.addRequested();
            stage = 9;
            break;
        case 9: {
            if (bridge.playlistOpen || bridge.trackCount !== 2 || bridge.playlistMessage.length > 0) return;
            const list = findChild(pane, "playlistTrackList");
            const first = list.itemAtIndex(0);
            const second = list.itemAtIndex(1);
            if (!first || !second) return;
            check(first.audioId === second.audioId && first.playlistItemId !== second.playlistItemId,
                "playlist cards retain separate item identities for shared audio");
            sortMedia = {first: first.playlistItemId, selected: second.playlistItemId};
            second.clicked();
            retainedResets = resets;
            findChild(pane, "playlistTrackSortMenu").choose(1);
            stage = 91;
            break;
        }
        case 91:
            if (bridge.trackSortIndex !== 1 || bridge.selectedPlaylistItemId !== sortMedia.selected) return;
            check(bridge.selectedSubtitle.includes("Hard") && !bridge.selectedIsPlaying,
                "sorting preserves selected difficulty without starting playback");
            check(resets === retainedResets, "playlist sorting does not reset model");
            findChild(window.contentItem, "songsTabButton").clicked();
            stage = 92;
            break;
        case 92:
            if (bridge.playlistVisible || bridge.trackCount !== 0) return;
            check(findChild(window.contentItem, "songSearch").text === "no-song-matches-playlists", "Songs keeps its own query");
            findChild(window.contentItem, "playlistsTabButton").clicked();
            stage = 93;
            break;
        case 93:
            if (!bridge.playlistVisible || bridge.trackCount !== 2) return;
            check(bridge.activePlaylistId === 2 && bridge.selectedPlaylistItemId === sortMedia.selected,
                "return to Playlists restores detail and selected item");
            findChild(pane, "editPlaylistButton").clicked();
            bridge.setPlaylistEditorName(" Renamed ");
            findChild(pane, "savePlaylistEditor").clicked();
            stage = 10;
            break;
        case 10:
            if (bridge.activePlaylistName !== "Renamed" || bridge.playlistEditorOpen) return;
            check(findChild(pane, "activePlaylistTitle").text === "Renamed", "inline edit updates detail header");
            bridge.selectPlaylistItem(sortMedia.first);
            bridge.setTrackSort(0);
            stage = 11;
            break;
        case 11: {
            const list = findChild(pane, "playlistTrackList");
            const card = list.itemAtIndex(0);
            if (!card || card.playlistItemId !== sortMedia.first) return;
            card.forceActiveFocus();
            input.keyClick(Qt.Key_F10, Qt.ShiftModifier, 0);
            stage = 111;
            break;
        }
        case 111: {
            const card = findChild(pane, "playlistTrackList").itemAtIndex(0);
            if (!card) return;
            const menu = findChild(card, "playlistItemMenu");
            if (!menu.opened) return;
            check(contained(menu.contentItem, window.activeFocusItem), "keyboard context menu receives focus");
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            stage = 12;
            break;
        }
        case 12:
            if (bridge.trackCount !== 1 || bridge.playlistMessage.length > 0) return;
            check(bridge.selectedSubtitle.includes("Hard"), "context removal retains other difficulty");
            bridge.deletePlaylist(2);
            stage = 13;
            break;
        case 13:
            if (bridge.activePlaylistId >= 0 || bridge.playlists.length !== 1) return;
            check(window.selectedTab === 1 && pane.activeId < 0, "deleting active playlist returns to list");
            pane.searchEdited("no-playlist-matches");
            stage = 14;
            break;
        case 14:
            if (bridge.filteredPlaylists.length !== 0 || findChild(pane, "playlistList").count !== 0) return;
            check(bridge.playlists.length === 1 && pane.playlists.length === 0,
                "playlist search filters actual cards without dropping canonical summaries");
            findChild(window.contentItem, "songsTabButton").clicked();
            findChild(window.contentItem, "songSearch").text = "";
            stage = 15;
            break;
        case 15:
            if (bridge.trackCount !== 1 || bridge.libraryLoading) return;
            check(bridge.selectedTitle === "Test song", "Songs library remains independent of playlist search");
            findChild(window.contentItem, "playlistsTabButton").clicked();
            stage = 16;
            break;
        case 16:
            if (!bridge.playlistVisible) return;
            check(bridge.playlistQuery === "no-playlist-matches" && pane.playlists.length === 0, "playlist query survives tab switches");
            pane.searchEdited("");
            stage = 17;
            break;
        case 17:
            if (bridge.filteredPlaylists.length !== 1 || findChild(pane, "playlistList").count !== 1) return;
            check(bridge.playlistQuery === "" && pane.playlists.length === 1, "clearing query restores actual playlist cards");
            finish();
            break;
        }
    }
    function playlistCoversProbe(): void {
        if (!bridge.connected || bridge.libraryLoading || bridge.foldersLoading || bridge.playlistBusy || bridge.playlistLoading) return;
        const pane = findChild(window.contentItem, "playlistsPane");
        switch (stage) {
        case 0:
            if (bridge.trackCount === 0) return;
            findChild(window.contentItem, "playlistsTabButton").clicked();
            bridge.beginPlaylistCreate();
            bridge.setPlaylistEditorName("Cover retry");
            stage = 10;
            break;
        case 10:
            if (!bridge.playlistEditorOpen || bridge.playlistEditorName !== "Cover retry") return;
            check(coverPath.length > 0, "local cover fixture provided");
            bridge.completePlaylistCoverPick(bridge.playlistEditorEpoch, "file://" + coverPath);
            check(bridge.playlistCoverPreparing && !findChild(pane, "savePlaylistEditor").enabled,
                "Save waits for asynchronous cover preparation");
            bridge.savePlaylistEditor();
            check(bridge.playlists.length === 0, "saving during preparation does not create playlist");
            stage = 1;
            break;
        case 1:
            if (bridge.playlistCoverPreparing || bridge.playlistEditorArtworkUrl.length === 0) return;
            check(findChild(pane, "savePlaylistEditor").enabled, "Save resumes after preparation completes");
            check(findChild(pane, "playlistEditor").artworkUrl.toString().length > 0, "prepared cover shown before save");
            sortMedia = {preview: bridge.playlistEditorArtworkUrl};
            findChild(pane, "savePlaylistEditor").clicked();
            stage = 2;
            break;
        case 2:
            if (bridge.playlistMessage.length === 0 || bridge.playlistEditorId < 0) return;
            check(bridge.playlists.length === 1 && bridge.playlistEditorOpen, "cover upload failure retains created playlist and inline editor");
            check(bridge.playlistEditorName === "Cover retry" && bridge.playlistEditorArtworkUrl === sortMedia.preview,
                "cover error preserves editor name and prepared preview");
            sortMedia.id = bridge.playlistEditorId;
            findChild(pane, "savePlaylistEditor").clicked();
            stage = 3;
            break;
        case 3:
            if (bridge.playlistEditorOpen || bridge.playlists.length !== 1) return;
            check(bridge.playlists[0].id === sortMedia.id, "retry updates same playlist without duplicate create");
            bridge.requestPlaylistArtwork(sortMedia.id);
            stage = 4;
            break;
        case 4:
            if (bridge.playlists[0].artworkUrl.length === 0) return;
            sortMedia.saved = bridge.playlists[0].artworkUrl;
            bridge.beginPlaylistEdit(sortMedia.id);
            stage = 5;
            break;
        case 5:
            if (bridge.playlistEditorArtworkUrl.length === 0) return;
            check(bridge.playlistEditorName === "Cover retry", "existing editor retains name and saved cover");
            sortMedia.epoch = bridge.playlistEditorEpoch;
            bridge.completePlaylistCoverPick(sortMedia.epoch, "");
            check(bridge.playlistEditorArtworkUrl.length > 0, "cancelled chooser retains existing cover");
            bridge.cancelPlaylistEditor();
            stage = 51;
            break;
        case 51:
            if (bridge.playlistEditorOpen) return;
            bridge.beginPlaylistEdit(sortMedia.id);
            stage = 52;
            break;
        case 52:
            if (!bridge.playlistEditorOpen || bridge.playlistEditorEpoch === sortMedia.epoch) return;
            bridge.completePlaylistCoverPick(sortMedia.epoch, "file://" + coverPath);
            check(!bridge.playlistCoverPreparing, "obsolete picker result ignored before starting preparation");
            bridge.resetPlaylistCover();
            stage = 53;
            break;
        case 53:
            if (bridge.playlistCoverPreparing || bridge.playlistEditorArtworkUrl.length > 0) return;
            findChild(pane, "savePlaylistEditor").clicked();
            stage = 6;
            break;
        case 6:
            if (bridge.playlistEditorOpen) return;
            check(bridge.playlists.length === 1 && bridge.playlists[0].artworkUrl.length === 0, "cover reset restores automatic placeholder for empty playlist");
            findChild(window.contentItem, "songsTabButton").clicked();
            stage = 7;
            break;
        case 7:
            if (bridge.playlistVisible || bridge.trackCount === 0) return;
            findChild(window.contentItem, "playlistsTabButton").clicked();
            stage = 8;
            break;
        case 8:
            if (!bridge.playlistVisible) return;
            check(bridge.playlists.length === 1 && bridge.playlists[0].id === sortMedia.id, "tab switch retains saved playlist");
            finish();
            break;
        }
    }
    function playlistCoverPreviewProbe(): void {
        if (!bridge.connected || bridge.libraryLoading || bridge.foldersLoading || bridge.playlistBusy
            || (stage !== 0 && bridge.playlistLoading)) return;
        const pane = findChild(window.contentItem, "playlistsPane");
        const editor = findChild(pane, "playlistEditor");
        const save = findChild(editor, "savePlaylistEditor");
        switch (stage) {
        case 0:
            if (bridge.trackCount !== 3 || bridge.selectedDurationLabel === "--:--" || bridge.selectedArtworkUrl.length === 0) return;
            check(bridge.playlists.length === 0, "initial playlist response stays pending before tab opens");
            retainedResets = resets;
            findChild(window.contentItem, "playlistsTabButton").clicked();
            check(window.selectedTab === 1, "playlist tab opens before its first list response");
            stage = 1;
            break;
        case 1:
            if (bridge.playlists.length !== 1 || bridge.playlists[0].artworkUrl.length === 0) return;
            console.info("initial visible playlist receives custom artwork");
            check(resets === retainedResets, "identical rows avoid model reset when opening playlist list");
            sortMedia.saved = bridge.playlists[0].artworkUrl;
            bridge.selectPlaylist(1);
            stage = 2;
            break;
        case 2:
            if (bridge.activePlaylistId !== 1 || bridge.trackCount !== 1) return;
            check(bridge.playlists[0].artworkUrl === sortMedia.saved, "detail navigation preserves decoded playlist cache");
            findChild(pane, "editPlaylistButton").clicked();
            stage = 3;
            break;
        case 3:
            if (!bridge.playlistEditorOpen || bridge.playlistEditorArtworkUrl.length === 0) return;
            check(bridge.playlistEditorArtworkUrl === sortMedia.saved, "editor reuses saved custom cover inside playlist");
            check(Math.abs(save.width - (editor.width - 32)) < 1, "Save fills editor content width");
            sortMedia.saved = bridge.playlistEditorArtworkUrl;
            findChild(editor, "resetPlaylistCover").clicked();
            stage = 4;
            break;
        case 4:
            if (!bridge.playlistEditorArtworkUrl.includes("auto-7")) return;
            check(editor.artworkUrl.toString() === bridge.playlistEditorArtworkUrl, "automatic cover previews before saving");
            check(bridge.playlists[0].artworkUrl === sortMedia.saved, "automatic preview leaves saved custom cover intact");
            findChild(editor, "closePlaylistEditor").clicked();
            stage = 5;
            break;
        case 5:
            if (bridge.playlistEditorOpen) return;
            findChild(pane, "editPlaylistButton").clicked();
            stage = 6;
            break;
        case 6:
            if (!bridge.playlistEditorOpen || bridge.playlistEditorArtworkUrl !== sortMedia.saved) return;
            findChild(editor, "resetPlaylistCover").clicked();
            stage = 7;
            break;
        case 7:
            if (!bridge.playlistEditorArtworkUrl.includes("auto-7")) return;
            save.clicked();
            stage = 8;
            break;
        case 8:
            if (bridge.playlistEditorOpen || bridge.trackCount !== 1) return;
            findChild(pane, "editPlaylistButton").clicked();
            stage = 9;
            break;
        case 9:
            if (!bridge.playlistEditorOpen || !bridge.playlistEditorArtworkUrl.includes("auto-7")) return;
            check(bridge.playlistEditorId === 1, "editor loads saved automatic cover inside playlist");
            findChild(editor, "closePlaylistEditor").clicked();
            stage = 10;
            break;
        case 10:
            if (bridge.playlistEditorOpen) return;
            bridge.beginPlaylistCreate();
            bridge.setPlaylistEditorName("New playlist");
            stage = 11;
            break;
        case 11:
            if (!bridge.playlistEditorOpen || bridge.playlistEditorId >= 0) return;
            check(save.text === "Create" && Math.abs(save.width - (editor.width - 32)) < 1, "Create fills editor content width");
            finish();
            break;
        }
    }
    function trackNamePreferencesProbe(): void {
        const titles = findChild(window.contentItem, "unicodeTitlesSwitch");
        const artists = findChild(window.contentItem, "unicodeArtistsSwitch");
        const message = findChild(window.contentItem, "trackNamePreferencesStatus");
        if (stage === 0) {
            findChild(window.contentItem, "settingsButton").clicked();
            check(titles.text === "Use Unicode track titles" && artists.text === "Use Unicode artist names", "English preference labels");
            check(titles.enabled && artists.enabled, "local preferences work offline");
            check(contained(findChild(window.contentItem, "generalSettingsSection"), titles)
                && contained(findChild(window.contentItem, "generalSettingsSection"), artists), "preferences belong to General");
            if (testCase === "names-read") {
                check(titles.checked && artists.checked, "both preferences persist between processes");
                bridge.setUseUnicodeTitles(false);
                check(!bridge.useUnicodeTitles && bridge.useUnicodeArtists, "changing one key retains the other");
                bridge.setUseUnicodeTitles(true);
                bridge.connectSession();
                check(bridge.useUnicodeTitles && bridge.useUnicodeArtists, "reconnect retains local choices");
                finish(); return;
            }
            check(!titles.checked && !artists.checked, "default preferences are independent and off");
            if (testCase === "names-read-error") {
                check(message.text === "Could not read track name preferences. Using default names.", "read failure uses defaults and English message");
                finish(); return;
            }
            titles.forceActiveFocus();
            stage = 1;
            return;
        }
        if (stage === 1) {
            input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
            check(bridge.useUnicodeTitles && !bridge.useUnicodeArtists, "keyboard title toggle preserves artist preference");
            bridge.setUseUnicodeArtists(true);
            bridge.setUseUnicodeTitles(false);
            bridge.setUseUnicodeArtists(false);
            bridge.setUseUnicodeTitles(true);
            bridge.setUseUnicodeArtists(true);
            check(bridge.useUnicodeTitles && bridge.useUnicodeArtists, "rapid toggles merge synchronously");
            if (testCase === "names-write-error") {
                check(message.text === "Could not save track name preferences. Your choice applies for this session.", "write failure retains choices and English message");
            } else {
                check(message.text.length === 0, "successful persistence has no error");
            }
            bridge.connectSession();
            check(bridge.useUnicodeTitles && bridge.useUnicodeArtists, "reconnect does not replace session choices");
            finish();
        }
    }
    function step(): void {
        if (passed) return;
        if (gallery) { galleryModalProbe(); return; }
        if (testCase.startsWith("names-")) { trackNamePreferencesProbe(); return; }
        if (testCase === "folders") { folderProbe(); return; }
        if (testCase === "search") { searchProbe(); return; }
        if (testCase === "playback") { playbackProbe(); return; }
        if (testCase === "queue") { queueProbe(); return; }
        if (testCase === "volume") { volumeProbe(); return; }
        if (testCase === "playlists") { playlistsProbe(); return; }
        if (testCase === "playlist-covers") { playlistCoversProbe(); return; }
        if (testCase === "playlist-cover-preview") { playlistCoverPreviewProbe(); return; }
        if (testCase === "sorting") { sortProbe(); return; }
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
            check(card.audioId === 103 && card.title === "Track 103" && card.durationLabel === "02:05", "typed live model roles");
            check(card.subtitle === "Fixture artist | Hard", "shared subtitle role");
            check(bridge.selectedAudioId === 103 && bridge.selectedTitle === "Track 103", "first sorted track uses database ID");
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
            input.mouseClick(findChild(window.contentItem, "settingsButton"));
            check(window.selectedTab === 2 && findChild(window.contentItem, "settingsPane").visible
                && !findChild(window.contentItem, "songsPane").visible && !findChild(window.contentItem, "playlistsPane").visible,
                "Settings tab shows only the settings panel");
            check(findChild(window.contentItem, "settingsButton").variant === "light"
                && findChild(window.contentItem, "songsTabButton").variant === "link", "active Settings uses the selected tab treatment");
            check(contained(findChild(window.contentItem, "generalSettingsSection"), findChild(window.contentItem, "folderMenu"))
                && contained(findChild(window.contentItem, "audioSettingsSection"), findChild(window.contentItem, "individualVolumeSwitch")),
                "folders belong to General and volume belongs to Audio");
            check(findChild(window.contentItem, "selectedTitle").text === "Track 42", "persistent player follows selection");
            check(findChild(window.contentItem, "playerPane").visible, "player stays visible in Settings");
            check(findChild(window.contentItem, "songSearch").text === "", "search fields are independent");
            check(bridge.selectedAudioId === 42, "settings preserves player selection");
            input.mouseClick(findChild(window.contentItem, "playlistsTabButton"));
            stage = 30;
            break;
        case 30:
            if (bridge.playlistLoading) return;
            check(window.selectedTab === 1 && findChild(window.contentItem, "playlistsPane").visible
                && !findChild(window.contentItem, "settingsPane").visible, "Settings returns to Playlists");
            check(bridge.selectedAudioId === 42, "Playlists list retains the selected player track");
            input.mouseClick(findChild(window.contentItem, "settingsButton"));
            check(findChild(window.contentItem, "settingsSearch").text === "settings query", "Settings query survives Playlists navigation");
            input.mouseClick(findChild(window.contentItem, "songsTabButton"));
            stage = 31;
            break;
        case 31:
            if (bridge.trackCount !== 3 || bridge.selectedAudioId !== 42) return;
            check(window.selectedTab === 0 && findChild(window.contentItem, "songsPane").visible
                && !findChild(window.contentItem, "settingsPane").visible, "Settings returns to Songs");
            check(findChild(window.contentItem, "selectedTitle").text === "Track 42"
                && findChild(window.contentItem, "songSearch").text === "", "navigation retains track and Songs query");
            retainedResets = resets;
            input.mouseClick(findChild(window.contentItem, "settingsButton"));
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
        // QtTest input spins the event loop. Restart after the step to avoid reentry.
        repeat: false
        running: !probe.passed
        onTriggered: { probe.step(); if (!probe.passed) restart(); }
    }
    Component.onCompleted: {
        if (gallery) galleryProbe();
    }
}
