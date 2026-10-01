pragma ComponentBehavior: Bound
import QtQuick
import QtTest

QtObject {
    id: probe
    // qmllint disable unqualified
    readonly property var window: probeWindow
    readonly property bool gallery: probeGallery
    readonly property string screenshotPath: probeScreenshotPath
    readonly property string testCase: probeCase
    readonly property string coverPath: probeCoverPath
    // qmllint enable unqualified
    readonly property var bridge: gallery || !window ? null : window.appBridge
    readonly property var mock: gallery && window ? window.galleryStore.bridge : null
    readonly property TestCase input: TestCase { parent: probe.window ? probe.window.contentItem : null; when: false }
    property int stage: 0
    property int scrollStep: 0
    property int issues: 0
    property bool passed: false
    property var opener: null
    property int originalWidth: 0
    property int originalHeight: 0
    onStageChanged: console.info("visual stage", stage)

    property bool monitorArtwork: false
    property string retainedArtwork: ""
    property int continuityFrames: 0
    property int steadyTicks: 0
    property int warmedResets: 0
    property int modelResets: 0
    property var retainedQueueDelegate: null
    readonly property Connections flickerModel: Connections {
        target: probe.bridge
        ignoreUnknownSignals: true
        function onModelReset() { ++probe.modelResets; }
        function onSelectedArtworkUrlChanged() {
            if (probe.monitorArtwork) probe.check(probe.bridge.selectedArtworkUrl === probe.retainedArtwork, "selected source changes during warmed navigation/scroll");
        }
    }
    function imageWithSource(item: var, source: string, wide: bool): var {
        if (!item) return null;
        if (item.source !== undefined && item.status !== undefined && item.fillMode !== undefined
            && item.source.toString() === source && (!wide || item.width === window.width)) return item;
        if (item.contentItem && item.contentItem !== item) {
            const found = imageWithSource(item.contentItem, source, wide);
            if (found) return found;
        }
        const children = item.data && typeof item.data !== "function" ? item.data : (item.children || []);
        for (let i = 0; i < children.length; ++i) {
            const found = imageWithSource(children[i], source, wide);
            if (found) return found;
        }
        return null;
    }
    function visibleRows(list: var): var {
        const rows = [];
        for (let i = 0; i < list.count; ++i) {
            const row = list.itemAtIndex(i);
            if (row && row.available && row.y + row.height >= list.contentY && row.y <= list.contentY + list.height) rows.push(row);
        }
        return rows;
    }
    function rowsReady(rows: var, queue: bool): bool {
        if (rows.length === 0) return false;
        for (const row of rows) {
            const data = queue ? row.modelData : row;
            const image = imageWithSource(row, data.artworkUrl, false);
            if (!data.artworkUrl || !image || image.status !== Image.Ready || data.durationLabel === "--:--") return false;
        }
        return true;
    }
    function queueRows(item: var, rows: var): void {
        if (!item) return;
        if (item.objectName === "queueTrackCard" && item.inViewport && rows.indexOf(item) < 0) rows.push(item);
        if (item.contentItem && item.contentItem !== item) queueRows(item.contentItem, rows);
        const children = item.data && typeof item.data !== "function" ? item.data : (item.children || []);
        for (let i = 0; i < children.length; ++i) queueRows(children[i], rows);
    }
    readonly property Timer continuity: Timer {
        interval: 40
        repeat: true
        running: probe.monitorArtwork && !probe.passed
        onTriggered: {
            const art = probe.imageWithSource(probe.find(probe.window.contentItem, "selectedArtwork"), probe.retainedArtwork, false);
            const backdrop = probe.imageWithSource(probe.window.contentItem, probe.retainedArtwork, true);
            probe.check(probe.bridge.selectedArtworkUrl === probe.retainedArtwork && art && art.status === Image.Ready,
                "player image loses readiness/source");
            probe.check(backdrop && backdrop.status === Image.Ready, "background image loses readiness/source");
            if (probe.stage >= 3 && probe.stage <= 6 && probe.continuityFrames < 18) {
                probe.shot("nav-frame-" + String(++probe.continuityFrames).padStart(2, "0"));
            }
        }
    }
    function find(item: var, name: string): var {
        if (!item) return null;
        if (item.objectName === name) return item;
        if (item.contentItem && item.contentItem !== item) {
            const found = find(item.contentItem, name);
            if (found) return found;
        }
        const children = item.data && typeof item.data !== "function" ? item.data : (item.children || []);
        for (let i = 0; i < children.length; ++i) {
            const found = find(children[i], name);
            if (found) return found;
        }
        return null;
    }
    function contained(parent: var, item: var): bool {
        while (item) { if (item === parent) return true; item = item.parent; }
        return false;
    }
    function check(value: bool, label: string): void {
        if (!value) { ++issues; console.warn("VISUAL ISSUE:", label); }
    }
    function shot(label: string): void {
        if (screenshotPath.length > 0) input.grabImage(window.contentItem).save(screenshotPath + "-" + label + ".png");
    }
    function overlap(a: var, b: var): bool {
        const p = a.mapToItem(window.contentItem, 0, 0);
        const q = b.mapToItem(window.contentItem, 0, 0);
        return p.x < q.x + b.width && q.x < p.x + a.width && p.y < q.y + b.height && q.y < p.y + a.height;
    }
    function finish(): void {
        passed = true;
        console.info("Visual probe finished:", issues, "issues", window.width, window.height);
        window.close();
        Qt.exit(issues > 0 ? 1 : 0);
    }
    function galleryStep(): void {
        const scroll = find(window.contentItem, "galleryScroll");
        const pane = find(window.contentItem, "demoPlaylistPane");
        switch (stage) {
        case 0:
            shot("gallery-top");
            if (window.screen.name.length === 0) {
                // Offscreen has its own cursor; desktop probes leave the user's cursor alone.
                const button = find(window.contentItem, "songsTabButton");
                button.forceActiveFocus();
                input.mouseMove(button, button.width / 2, button.height / 2, 0);
                check(button.hovered && button.activeFocus, "button hover and keyboard focus");
                shot("gallery-hover-focus");
            }
            input.mouseWheel(scroll.contentItem, 100, 100, 0, -120, Qt.NoButton, Qt.NoModifier, 0);
            stage = 1; break;
        case 1:
            if (scrollStep === 0) check(scroll.contentItem.contentY > 0, "gallery wheel scrolls downward");
            scroll.contentItem.contentY = Math.max(0, scroll.contentItem.contentHeight - scroll.contentItem.height) * (++scrollStep / 4);
            stage = 2; break;
        case 2:
            shot("gallery-scroll-" + scrollStep);
            if (scrollStep === 1) {
                mock.dispatch("selectTab", "2"); mock.dispatch("toggleTag", ""); mock.dispatch("cycleFilter", "");
                shot("gallery-included");
                mock.dispatch("cycleFilter", ""); shot("gallery-excluded");
            }
            stage = scrollStep < 4 ? 1 : 3; break;
        case 3:
            window.selectedTab = 1; stage = 4; break;
        case 4:
            pane.createRequested(); pane.nameEdited("Длинное название плейлиста / 日本語 " + "long ".repeat(12)); pane.coverRequested(); stage = 5; break;
        case 5:
            shot("playlist-editor");
            check(find(pane, "playlistEditorName").activeFocus, "editor initially focuses name");
            pane.saveRequested(); stage = 6; break;
        case 6:
            shot("playlists");
            const list = find(pane, "playlistList");
            opener = list.itemAtIndex(1);
            if (!opener) return;
            opener.forceActiveFocus(); input.keyClick(Qt.Key_F10, Qt.ShiftModifier, 0); stage = 61; break;
        case 61: {
            const menu = find(opener, "playlistActionsMenu");
            if (!menu.opened) return;
            check(menu.currentIndex === 1 && menu.itemAt(1).activeFocus, "empty playlist menu focuses enabled Edit action");
            shot("playlist-actions");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0); stage = 62; break;
        }
        case 62:
            check(opener.activeFocus, "playlist actions menu restores card focus");
            pane.selectRequested(1); stage = 7; break;
        case 7:
            if (!pane.tracksList.itemAtIndex(0)) return;
            shot("playlist-detail");
            opener = pane.tracksList.itemAtIndex(0);
            opener.forceActiveFocus();
            input.keyClick(Qt.Key_F10, Qt.ShiftModifier, 0);
            stage = 8; break;
        case 8: {
            const menu = find(opener, "playlistItemMenu");
            if (!menu.opened) return;
            shot("playlist-context");
            check(contained(menu.contentItem, window.activeFocusItem), "track menu focus");
            const origin = menu.contentItem.mapToItem(window.contentItem, 0, 0);
            const cardOrigin = opener.mapToItem(window.contentItem, 0, 0);
            check(Math.abs(origin.x - cardOrigin.x) < 20, "keyboard menu is anchored to its track");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0); stage = 9; break;
        }
        case 9:
            check(opener.activeFocus, "track menu restores focus");
            mock.dispatch("playlistAddOpen", ""); stage = 10; break;
        case 10: {
            const dialog = find(window.contentItem, "demoPlaylistsDialog");
            if (!dialog.opened) return;
            shot("add-difficulties");
            for (let i = 0; i < 12; ++i) {
                input.keyClick(Qt.Key_Tab, Qt.NoModifier, 0);
                check(contained(dialog.contentItem, window.activeFocusItem), "add modal Tab focus");
            }
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0); stage = 11; break;
        }
        case 11:
            window.selectedTab = 0;
            find(window.contentItem, "openFolderModal").clicked(); stage = 12; break;
        case 12: {
            const dialog = find(window.contentItem, "folderSelectionDialog");
            if (!dialog.opened) return;
            shot("folders");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0); stage = 13; break;
        }
        case 13:
            mock.dispatch("galleryDisabled", "true"); scroll.contentItem.contentY = 0; stage = 14; break;
        case 14:
            shot("gallery-disabled"); finish(); break;
        }
    }
    function liveStep(): void {
        const pane = find(window.contentItem, "playlistsPane");
        const menu = find(window.contentItem, "trackSortMenu");
        const popup = find(window.contentItem, "volumePopup");
        switch (stage) {
        case 0:
            if (!bridge.connected || bridge.libraryLoading || bridge.foldersLoading || bridge.trackCount === 0 || bridge.selectedArtworkUrl.length === 0) return;
            stage = 1; break;
        case 1:
            shot("songs");
            if (testCase !== "visual-real") {
                const list = find(window.contentItem, "songList");
                input.mouseWheel(list, 100, list.height / 2, 0, -120, Qt.NoButton, Qt.NoModifier, 0);
            }
            check(!overlap(find(window.contentItem, "volumeButton"), find(window.contentItem, "shuffleButton")), "volume and shuffle hit areas overlap");
            check(!overlap(find(window.contentItem, "addToPlaylistButton"), find(window.contentItem, "repeatButton")), "add and repeat hit areas overlap");
            menu.forceActiveFocus(); menu.popup.open(); stage = 2; break;
        case 2:
            if (!menu.popup.opened) return;
            if (testCase !== "visual-real") check(find(window.contentItem, "songList").contentY > 0, "songs wheel scrolls downward");
            shot("sort-menu");
            check(contained(menu.popup.contentItem, window.activeFocusItem), "sort menu initial focus");
            input.keyClick(Qt.Key_Tab, Qt.NoModifier, 0);
            check(contained(menu.popup.contentItem, window.activeFocusItem), "sort menu Tab focus");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0); stage = 3; break;
        case 3:
            check(menu.activeFocus, "sort restores focus");
            find(window.contentItem, "settingsButton").clicked(); stage = 4; break;
        case 4:
            shot("settings");
            check(find(window.contentItem, "settingsPane").width === 480, "Settings sidebar width");
            check(find(window.contentItem, "settingsSearch").height === 44
                && find(window.contentItem, "folderMenu").height === 44, "Settings field height");
            find(window.contentItem, "audioSettingsMessage").text = "Long audio status / Длинное сообщение / 日本語 / ".repeat(40);
            stage = 41; break;
        case 41: {
            const scroll = find(window.contentItem, "settingsScroll").contentItem;
            check(scroll.contentHeight > scroll.height, "long Settings status requires vertical scrolling");
            input.mouseWheel(scroll, 100, scroll.height / 2, 0, -120, Qt.NoButton, Qt.NoModifier, 0);
            stage = 42; break;
        }
        case 42: {
            const scroll = find(window.contentItem, "settingsScroll").contentItem;
            check(scroll.contentY > 0, "Settings wheel scrolls downward");
            shot("settings-scrolled");
            find(window.contentItem, "audioSettingsMessage").text = Qt.binding(() => bridge.audioSettingsMessage);
            scroll.contentY = 0;
            find(window.contentItem, "settingsSearch").forceActiveFocus();
            input.keyClick(Qt.Key_Tab, Qt.NoModifier, 0);
            stage = 43; break;
        }
        case 43:
            check(find(window.contentItem, "folderMenu").activeFocus, "Settings search Tab reaches the folder menu");
            find(window.contentItem, "folderMenu").popup.open(); stage = 5; break;
        case 5:
            if (!find(window.contentItem, "folderMenu").popup.opened) return;
            shot("folder-menu");
            check(contained(find(window.contentItem, "folderMenu").popup.contentItem, window.activeFocusItem), "folder menu initial focus");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0);
            stage = 51; break;
        case 51:
            check(find(window.contentItem, "folderMenu").activeFocus, "folder menu restores focus in Settings");
            opener = find(window.contentItem, "volumeButton"); opener.forceActiveFocus(); opener.clicked(); stage = 6; break;
        case 6:
            if (!popup.opened) return;
            shot("volume");
            check(contained(popup.contentItem, window.activeFocusItem), "volume opens with keyboard focus");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0); stage = 7; break;
        case 7:
            check(!popup.visible && opener.activeFocus, "volume Escape restores focus"); popup.close();
            find(window.contentItem, "playlistsTabButton").clicked(); stage = 8; break;
        case 8:
            if (bridge.playlistLoading) return;
            shot("playlists-empty");
            pane.createRequested(); pane.nameEdited("Длинное название плейлиста / 日本語 " + "long ".repeat(12)); stage = 9; break;
        case 9:
            if (!pane.editorOpen) return;
            shot("playlist-editor");
            pane.cancelRequested();
            // Real-library checks never start unscoped discovery.
            if (testCase === "visual-real") { finish(); return; }
            find(window.contentItem, "settingsButton").clicked(); bridge.addFolder(); stage = 10; break;
        case 10: {
            const dialog = find(window.contentItem, "folderSelectionDialog");
            if (!dialog.opened || bridge.folderSelectionDiscovering) return;
            shot("folders");
            for (let i = 0; i < 12; ++i) {
                input.keyClick(Qt.Key_Tab, Qt.NoModifier, 0);
                check(contained(dialog.contentItem, window.activeFocusItem), "folder modal Tab focus");
                input.keyClick(Qt.Key_Tab, Qt.ShiftModifier, 0);
                check(contained(dialog.contentItem, window.activeFocusItem), "folder modal Shift+Tab focus");
            }
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0); stage = 11; break;
        }
        case 11:
            bridge.togglePlayback(); stage = 12; break;
        case 12:
            shot("playback-loading"); stage = 13; break;
        case 13:
            if (!bridge.playbackMessage.includes("fixture audio unavailable")) return;
            shot("playback-error");
            find(window.contentItem, "songsTabButton").clicked();
            bridge.searchLibrary("missing"); stage = 14; break;
        case 14:
            if (bridge.libraryLoading || bridge.trackCount !== 0) return;
            shot("songs-empty"); bridge.searchLibrary("visual-error"); stage = 15; break;
        case 15:
            if (bridge.libraryLoading || !bridge.libraryMessage.includes("visual fixture error")) return;
            shot("songs-error"); finish(); break;
        }
    }
    function nativeStep(): void {
        const picker = find(window.contentItem, "playlistCoverPicker");
        switch (stage) {
        case 0:
            if (!bridge.connected || bridge.libraryLoading || !bridge.audioSettingsLoaded) return;
            if (testCase === "native-picker") {
                find(window.contentItem, "playlistsTabButton").clicked();
                bridge.beginPlaylistCreate(); stage = 41; break;
            }
            originalWidth = window.width; originalHeight = window.height;
            window.showMinimized(); stage = 1; break;
        case 1:
            if (window.visibility !== Window.Minimized) return;
            window.showNormal(); stage = 2; break;
        case 2:
            if (window.visibility !== Window.Windowed) return;
            window.showMaximized(); stage = 3; break;
        case 3:
            if (window.visibility !== Window.Maximized) return;
            shot("maximized"); window.showNormal(); stage = 4; break;
        case 4:
            if (window.visibility !== Window.Windowed) return;
            check(window.width === originalWidth && window.height === originalHeight, "restored window geometry");
            find(window.contentItem, "playlistsTabButton").clicked();
            bridge.beginPlaylistCreate(); stage = 41; break;
        case 41:
            if (!bridge.playlistEditorOpen) return;
            if (coverPath.length > 0) picker.currentFolder = "file://" + coverPath.slice(0, coverPath.lastIndexOf("/"));
            bridge.choosePlaylistCover(); stage = 5; break;
        case 5:
            if (!picker.visible) return;
            console.info("Cover picker visible"); shot("cover-picker"); stage = 6; break;
        case 6:
            picker.close();
            bridge.completePlaylistCoverPick(bridge.playlistEditorEpoch, ""); stage = 7; break;
        case 7:
            check(!picker.visible && bridge.playlistEditorOpen && !bridge.playlistCoverPreparing, "native picker cancellation retains editor");
            bridge.cancelPlaylistEditor(); finish(); break;
        }
    }
    function queueStep(): void {
        const panel = find(window.contentItem, "queuePanel");
        switch (stage) {
        case 0:
            if (!bridge.connected || bridge.libraryLoading || !bridge.audioSettingsLoaded) return;
            find(window.contentItem, "queueButton").clicked(); stage = 1; break;
        case 1:
            if (!panel.opened || bridge.queueLoading || bridge.queueTracks.length !== 2) return;
            if (bridge.queueTracks.some(track => track.artworkUrl.length === 0 || track.durationLabel === "--:--")) return;
            check(bridge.queueTracks[0].audioId === 42 && bridge.queueTracks[1].audioId === 103, "queue visual order");
            stage = 2; break;
        case 2:
            stage = 4; break;
        case 4:
            shot("queue");
            window.width = 1024; window.height = 640; stage = 3; break;
        case 3:
            shot("queue-compact");
            check(panel.width === 430 && panel.height > 200, "queue fits the minimum window");
            finish(); break;
        }
    }
    function flickerStep(): void {
        const list = find(window.contentItem, "songList");
        const pane = find(window.contentItem, "playlistsPane");
        const panel = find(window.contentItem, "queuePanel");
        switch (stage) {
        case 0: {
            if (!bridge.connected || bridge.libraryLoading || bridge.trackCount !== 80 || !bridge.selectedArtworkUrl) return;
            const image = imageWithSource(find(window.contentItem, "selectedArtwork"), bridge.selectedArtworkUrl, false);
            if (!image || image.status !== Image.Ready) return;
            check(bridge.selectedAudioId === 1 && bridge.selectedDurationLabel === "--:--", "ready artwork precedes slow duration HTTP");
            console.info("flicker early artwork ready before duration");
            shot("early-art"); stage = 1; break;
        }
        case 1:
            if (!rowsReady(visibleRows(list), false) || bridge.selectedDurationLabel === "--:--" || bridge.playlistLoading) return;
            retainedArtwork = bridge.selectedArtworkUrl;
            monitorArtwork = true;
            warmedResets = modelResets;
            shot("warm-songs");
            window.selectedTab = 1; stage = 3; break;
        case 3:
            if (bridge.playlistLoading || bridge.playlists.length !== 1 || !bridge.playlists[0].artworkUrl) return;
            check(modelResets === warmedResets, "identical library rows avoid model reset on playlist list");
            shot("nav-list"); bridge.selectPlaylist(1); stage = 4; break;
        case 4:
            if (bridge.activePlaylistId !== 1 || bridge.trackCount !== 2 || bridge.selectedPlaylistItemId !== 1
                || !rowsReady(visibleRows(pane.tracksList), false)) return;
            check(pane.tracksList.itemAtIndex(0).audioId === pane.tracksList.itemAtIndex(1).audioId
                && pane.tracksList.itemAtIndex(0).playlistItemId !== pane.tracksList.itemAtIndex(1).playlistItemId,
                "duplicate audio keeps separate playlist item identities");
            shot("nav-detail"); bridge.selectPlaylistItem(2); stage = 5; break;
        case 5:
            if (bridge.selectedPlaylistItemId !== 2) return;
            shot("nav-duplicate"); window.selectedTab = 0; stage = 6; break;
        case 6:
            if (bridge.playlistVisible || bridge.trackCount !== 80 || !rowsReady(visibleRows(list), false)) return;
            shot("nav-songs"); scrollStep = 0; stage = 7; break;
        case 7:
            list.contentY = Math.max(0, list.contentHeight - list.height) * (++scrollStep / 8);
            if (scrollStep === 1 || scrollStep === 4 || scrollStep === 8) shot("scroll-" + scrollStep);
            if (scrollStep === 8) stage = 8;
            break;
        case 8:
            if (!rowsReady(visibleRows(list), false)) return;
            check(list.itemAtIndex(79) && list.itemAtIndex(79).audioId === 80, "stopped viewport reaches the last row");
            console.info("flicker stopped viewport ready");
            shot("scroll-end"); list.contentY = 0; stage = 9; break;
        case 9:
            if (!rowsReady(visibleRows(list), false)) return;
            shot("scroll-return"); originalHeight = window.height;
            if (window.height < 1200) window.height = 1200;
            find(window.contentItem, "queueButton").clicked(); stage = 11; break;
        case 11: {
            if (!panel.opened || bridge.queueLoading || bridge.queueTracks.length !== 16) return;
            const rows = []; queueRows(panel.contentItem, rows);
            if (rows.length === 0) return;
            retainedQueueDelegate = rows[0]; stage = 12; break;
        }
        case 12: {
            const rows = []; queueRows(panel.contentItem, rows);
            if (!rowsReady(rows, true) || !rowsReady(visibleRows(list), false)) return;
            const ids = {};
            for (const row of visibleRows(list)) ids[row.audioId] = true;
            for (const row of rows) ids[row.audioId] = true;
            check(Object.keys(ids).length > 10, "stationary visible artwork exceeds the 64 MiB provider budget");
            check(rows[0] === retainedQueueDelegate, "image/duration projection preserves queue delegates");
            console.info("flicker pressure checkpoint", Date.now(), Object.keys(ids).length, "distinct 1280px RGBA images");
            shot("pressure-ready"); steadyTicks = 0; stage = 13; break;
        }
        case 13: {
            const rows = []; queueRows(panel.contentItem, rows);
            check(rowsReady(rows, true) && rowsReady(visibleRows(list), false), "stationary displayed cards keep Image.Ready under pressure");
            check(rows[0] === retainedQueueDelegate, "stationary queue projection keeps delegates");
            if (++steadyTicks < 10) return;
            console.info("flicker stationary pressure settled without empty selected source", Date.now());
            shot("pressure-settled"); panel.close(); window.height = originalHeight; stage = 14; break;
        }
        case 14:
            if (panel.visible) return;
            shot("final");
            check(continuityFrames > 0, "navigation frame sequence captured");
            monitorArtwork = false; finish(); break;
        }
    }
    readonly property Timer poll: Timer {
        interval: probe.testCase === "visual-flicker" ? 60 : 150
        repeat: false
        running: !probe.passed
        onTriggered: {
            if (probe.testCase === "visual-flicker") probe.flickerStep();
            else if (probe.testCase === "visual-queue") probe.queueStep();
            else if (probe.testCase.startsWith("native")) probe.nativeStep();
            else if (probe.gallery) probe.galleryStep(); else probe.liveStep();
            if (!probe.passed) restart();
        }
    }
}
