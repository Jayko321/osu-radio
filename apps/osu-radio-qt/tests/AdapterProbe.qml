pragma ComponentBehavior: Bound
import QtQuick
import QtTest
import OsuRadio 1.0 as Radio

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
    property var controls: null
    readonly property Component windowFixture: Component {
        Item {
            id: fixture
            x: 10; y: 70; width: 1000; height: 700
            property int tabs: -1
            property int queues: 0
            property alias bar: bar
            property alias surface: surface
            property alias windowDouble: windowDouble
            QtObject {
                id: windowDouble
                property int visibility: Window.Windowed
                property alias contentItem: surface
                property int moves: 0
                property int closes: 0
                property var edges: []
                function startSystemMove(): bool { moves++; return true; }
                function startSystemResize(edge: int): bool { edges.push(edge); return true; }
                function showMinimized(): void { visibility = Window.Minimized; }
                function showMaximized(): void { visibility = Window.Maximized; }
                function showNormal(): void { visibility = Window.Windowed; }
                function close(): void { closes++; }
            }
            Item {
                id: surface
                width: 1000; height: 600
                Radio.WindowBar {
                    id: bar
                    width: parent.width
                    window: windowDouble
                    onTabSelected: index => { fixture.tabs = index; selectedTab = index; }
                    onQueueRequested: { fixture.queues++; queueVisible = !queueVisible; }
                }
            }
        }
    }
    readonly property Component panelFixture: Component {
        Item {
            id: fixture
            x: 10; y: 60; width: 1000; height: 780
            property var actions: []
            property int delegatesAdded: 0
            property int delegatesRemoved: 0
            property int viewportChanges: 0
            property alias pane: pane
            property alias folders: folders
            property alias playlists: playlists
            property alias queue: queue
            property alias queueMock: fixtureQueueBridge
            Radio.MockBridge { id: fixtureQueueBridge }
            Radio.PlaylistPane {
                id: pane
                width: 480; height: 740
                tracksModel: []
                onNameEdited: name => { editorName = name; fixture.actions.push("name"); }
                onCoverRequested: fixture.actions.push("cover")
                onCoverResetRequested: fixture.actions.push("reset")
                onSaveRequested: fixture.actions.push("save")
                onCancelRequested: fixture.actions.push("cancel")
                onCreateRequested: fixture.actions.push("create")
                onEditRequested: id => fixture.actions.push("edit:" + id)
                onSelectRequested: id => fixture.actions.push("select:" + id)
                onQueueRequested: id => fixture.actions.push("queue:" + id)
                onPlayRequested: id => fixture.actions.push("play:" + id)
                onDeleteRequested: id => fixture.actions.push("delete:" + id)
                onRefreshRequested: fixture.actions.push("refresh")
            }
            Radio.FolderSelectionModal {
                id: folders
                scene: fixture
                onToggleRequested: path => fixture.actions.push("toggle:" + path)
                onRefreshRequested: path => fixture.actions.push("folder-refresh:" + path)
                onRetryCountRequested: path => fixture.actions.push("count:" + path)
                onBrowseRequested: fixture.actions.push("browse")
                onApplyRequested: fixture.actions.push("apply")
            }
            Radio.PlaylistsModal {
                id: playlists
                scene: fixture
                onTargetRequested: id => { targetId = id; fixture.actions.push("target:" + id); }
                onDifficultyRequested: id => fixture.actions.push("difficulty:" + id)
                onAddRequested: fixture.actions.push("add")
                onRefreshRequested: fixture.actions.push("list-refresh")
            }
            Radio.QueuePanel {
                id: queue
                host: fixture
                tracks: null
                onRetryRequested: fixture.actions.push("queue-retry")
                onPlayRequested: id => fixture.actions.push("queue-play:" + id)
                onRemoveRequested: id => fixture.actions.push("queue-remove:" + id)
                onMoveRequested: (id, before) => fixture.actions.push("queue-move:" + id + ">" + before)
                onVisibleEntriesChanged: fixture.viewportChanges++
            }
        }
    }
    readonly property Component controlFixture: Component {
        Column {
            id: fixture
            x: 20; y: 80; spacing: 8
            property int presses: 0
            property int toggles: 0
            property int edits: 0
            property int chosen: -1
            property int menuValue: -1
            property int menuEdits: 0
            property alias button: button
            property alias icon: icon
            property alias field: field
            property alias toggle: toggle
            property alias tabs: tabs
            property alias tag: tag
            property alias artwork: artwork
            property alias material: material
            property alias menu: menu
            property alias modal: modal
            property alias modalField: modalField
            property alias ctx: ctx
            property alias ctxDestructive: ctxDestructive
            property alias ctxDisabled: ctxDisabled
            property var ctxTriggered: []
            Radio.AppButton { id: button; text: "Activate"; onClicked: fixture.presses++ }
            Radio.IconButton { id: icon; iconName: "play"; accessibleName: "Play fixture"; onClicked: fixture.presses++ }
            Radio.AppField { id: field; onTextEdited: fixture.edits++ }
            Radio.AppSwitch { id: toggle; text: "Enable"; onToggled: fixture.toggles++ }
            Radio.AppTabs {
                id: tabs
                labels: ["First", "Second"]
                onSelected: index => { fixture.chosen = index; currentIndex = index; }
            }
            Radio.AppTag { id: tag; text: "Genre"; onClicked: selected = !selected }
            Radio.AppIcon { name: "search"; size: 32; color: Radio.Theme.accent }
            Radio.Artwork { id: artwork; width: 80; height: 60; source: "qrc:/assets/logos/stable.png" }
            Radio.MaterialSurface {
                id: material
                width: 100; height: 80
                sourceItem: fixture
                Rectangle { objectName: "materialForeground"; anchors.fill: parent; color: "red" }
            }
            Radio.AppMenu {
                id: menu
                valueRole: "id"
                searchable: true
                onChosen: value => { fixture.menuValue = value; currentIndex = entries.findIndex(entry => entry.id === value); }
                onQueryEdited: value => { query = value; fixture.menuEdits++; }
            }
            Radio.AppContextMenu {
                id: ctx
                Radio.AppMenuItem { text: "Plain"; iconName: "play"; onTriggered: fixture.ctxTriggered.push("plain") }
                Radio.AppMenuItem { id: ctxDestructive; text: "Remove"; iconName: "trash"; destructive: true; onTriggered: fixture.ctxTriggered.push("destructive") }
                Radio.AppMenuItem { id: ctxDisabled; text: "Off"; iconName: "play"; enabled: false; onTriggered: fixture.ctxTriggered.push("disabled") }
            }
            Radio.AppModal {
                id: modal
                scene: probe.window.contentItem
                panelWidth: 400; panelHeight: 300
                title: "Fixture dialog"
                initialFocus: modalField
                Radio.AppField { id: modalField; width: parent.width }
            }
        }
    }
    readonly property Component modsFixture: Component {
        Item {
            id: fixture
            width: probe.window.width; height: probe.window.height
            property var actions: []
            property alias opener: opener
            property alias popup: popup
            Radio.AppButton {
                id: opener
                x: parent.width - width - 20; y: parent.height - height - 20
                text: "Mods"
                onClicked: popup.open()
                Radio.AudioModsPopup {
                    id: popup
                    x: opener.width - width; y: -height - 8
                    onDtCycleRequested: fixture.actions.push("dt")
                    onHtToggleRequested: fixture.actions.push("ht")
                    onSpeedRequested: percent => fixture.actions.push("speed:" + percent)
                    onPitchRequested: adjust => fixture.actions.push("pitch:" + adjust)
                    onClearRequested: fixture.actions.push("clear")
                    onRetryRequested: fixture.actions.push("retry")
                }
            }
        }
    }
    readonly property Component progressFixture: Component {
        Item {
            id: fixture
            width: probe.window.width; height: probe.window.height
            property var seeks: []
            property alias progress: progress
            property alias fade: fade
            Radio.CrossfadeSource { id: fade; duration: 100000 }
            Radio.PlaybackProgress {
                id: progress
                x: 100; y: 100; width: 400; height: 24
                playbackPosition: 40
                playbackDuration: 160
                scaleKey: "42:-1:1:150"
                onSeekRequested: seconds => fixture.seeks.push(seconds)
            }
        }
    }
    property var sortMedia: ({})
    property int settingsScenario: 0
    readonly property var settingsQueries: [
        { query: " GeNeRaL ", general: true, audio: false, individual: false },
        { query: " FoLdErS ", general: true, audio: false, individual: false },
        { query: "TrAcK", general: false, audio: true, individual: true, mods: true },
        { query: "volume", general: false, audio: true, individual: true, mods: false },
        { query: " MoDs ", general: false, audio: true, individual: false, mods: true },
        { query: "Global volume", general: false, audio: false, individual: false },
        { query: "[missing]", general: false, audio: false, individual: false },
        { query: " AbOuT ", general: false, audio: false, individual: false, about: true, version: true, license: true, project: true },
        { query: " VeRsIoN ", general: false, audio: false, individual: false, about: true, version: true },
        { query: "lIcEnSe", general: false, audio: false, individual: false, about: true, license: true },
        { query: "pRoJeCt", general: false, audio: false, individual: false, about: true, project: true },
        { query: "MIT", general: false, audio: false, individual: false },
        { query: "github.com", general: false, audio: false, individual: false },
        { query: "imported from", general: false, audio: false, individual: false },
        { query: "remembers", general: false, audio: false, individual: false },
        { query: "its own speed", general: false, audio: false, individual: false },
        { query: "", general: true, audio: true, individual: true, mods: true, about: true, version: true, license: true, project: true },
        { query: "   ", general: true, audio: true, individual: true, mods: true, about: true, version: true, license: true, project: true }
    ]
    readonly property TestCase input: TestCase { parent: probe.window ? probe.window.contentItem : null; when: false }
    readonly property var mock: gallery && window ? window.galleryStore.bridge : null
    readonly property Connections mockConnection: Connections {
        target: probe.mock
        function onStateJsonChanged() { probe.notifications++; }
    }
    readonly property Connections modelConnection: Connections {
        target: probe.bridge ? probe.bridge.library.tracks : null
        ignoreUnknownSignals: true
        function onDataChanged() { probe.rowChanges++; }
        function onModelReset() { probe.resets++; }
    }
    readonly property Connections playlistModelConnection: Connections {
        target: probe.bridge ? probe.bridge.playlists.tracks : null
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
    function textChild(item: var, text: string): var {
        if (!item) return null;
        if (item.text === text) return item;
        if (item.contentItem && item.contentItem !== item) {
            const found = textChild(item.contentItem, text);
            if (found) return found;
        }
        const children = item.data && typeof item.data !== "function" ? item.data : (item.children || []);
        for (const child of children) {
            const found = textChild(child, text);
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
        for (const name of ["demoContextButton", "demoConfirmButton", "demoToastButton", "demoTipButton"])
            check(findChild(window.contentItem, name) !== null, "gallery shows the " + name + " feedback demo");
        check(findChild(window.contentItem, "demoContextButton").data.some(o => o.objectName === "demoContextMenu"), "gallery feedback demo includes a context menu");
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
        const field = findChild(window.contentItem, "galleryField");
        field.forceActiveFocus();
        input.keyClick(Qt.Key_A, Qt.ControlModifier, 0);
        input.keyClick(Qt.Key_Backspace, Qt.NoModifier, 0);
        input.keyClick(Qt.Key_A, Qt.NoModifier, 0);
        check(JSON.parse(mock.stateJson).gallery.field === "a", "gallery field edits reach the store");
        const tag = textChild(window.contentItem, "Electronic");
        const oldTag = JSON.parse(mock.stateJson).gallery.tag;
        tag.forceActiveFocus(); input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
        check(JSON.parse(mock.stateJson).gallery.tag !== oldTag, "gallery tag activates the shared action");
        const button = textChild(window.contentItem, "Accent");
        const oldPresses = JSON.parse(mock.stateJson).gallery.presses;
        button.forceActiveFocus(); input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
        check(JSON.parse(mock.stateJson).gallery.presses === oldPresses + 1, "gallery button reaches the shared action");
        const toggle = findChild(window.contentItem, "globalDisable");
        toggle.forceActiveFocus(); input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
        check(!field.enabled && !button.enabled && JSON.parse(mock.stateJson).gallery.disabled, "gallery switch disables its real control subtree");
        toggle.forceActiveFocus(); input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
        check(field.enabled && button.enabled && !JSON.parse(mock.stateJson).gallery.disabled, "gallery controls reenable through the UI");
        const opener = findChild(window.contentItem, "openModal");
        opener.forceActiveFocus();
        opener.clicked();
        stage = 10;
    }
    function controlsProbe(): void {
        if (stage === 0) {
            controls = controlFixture.createObject(window.contentItem);
            check(controls !== null, "create production controls in the offline root");
            stage = 1;
            return;
        }
        const c = controls;
        if (stage === 1) {
            const artImage = c.artwork.children.find(child => child.fillMode !== undefined);
            if (artImage.status === Image.Loading) return;
            check(artImage.status === Image.Ready, "artwork fixture decodes");
            c.button.forceActiveFocus();
            input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
            check(c.presses === 1, "Space activates the button once");
            Radio.Theme.motion = false;
            c.button.enabled = false;
            input.mouseClick(c.button);
            input.mouseMove(c.button, 5, 5);
            input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
            check(c.presses === 1 && c.button.opacity === 0.4, "disabled button suppresses input and ignores hover");
            c.button.enabled = true;
            check(c.button.opacity === 1 || c.button.opacity === 0.8, "motion off applies button opacity without a transition");
            Radio.Theme.motion = true;
            for (const variant of ["light", "alternate", "accent", "link"]) {
                c.button.variant = variant;
                const expected = variant === "light" ? Radio.Theme.text : variant === "accent" ? Radio.Theme.accent
                    : variant === "link" ? Qt.rgba(0, 0, 0, 0) : Radio.Theme.surface;
                check(c.button.background.color.toString() === expected.toString(), "button variant " + variant);
            }
            c.icon.forceActiveFocus();
            input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
            check(c.presses === 2 && c.icon.icon.source.toString().endsWith("/play.svg"), "icon button activates through keyboard");
            c.icon.enabled = false;
            input.mouseClick(c.icon);
            check(c.presses === 2, "disabled icon suppresses input");
            c.field.forceActiveFocus();
            input.keyClick(Qt.Key_A, Qt.NoModifier, 0);
            check(c.field.text === "a" && c.edits === 1, "field edits emit through native keyboard handling");
            c.field.enabled = false;
            input.keyClick(Qt.Key_B, Qt.NoModifier, 0);
            check(c.field.text === "a" && c.edits === 1 && c.field.opacity === 0.4, "disabled field cannot edit");
            c.field.enabled = true;
            c.field.text = "日本語 / Поле";
            c.field.forceActiveFocus();
            input.keyClick(Qt.Key_A, Qt.ControlModifier, 0);
            input.keyClick(Qt.Key_Backspace, Qt.NoModifier, 0);
            check(c.field.text === "" && c.edits === 2, "native selection clears Unicode content");
            c.toggle.forceActiveFocus();
            input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
            check(c.toggle.checked && c.toggles === 1, "switch toggles once with Space");
            c.toggle.enabled = false;
            input.mouseClick(c.toggle);
            check(c.toggle.checked && c.toggles === 1, "disabled switch retains its state");
            c.toggle.enabled = true;
            input.mouseClick(c.toggle);
            check(!c.toggle.checked && c.toggles === 2, "switch toggles with the pointer");
            const thumb = c.toggle.indicator.children[0];
            check(thumb.x === 2, "unchecked thumb rests at the left");
            Radio.Theme.motion = false;
            input.mouseClick(c.toggle);
            check(c.toggle.checked && thumb.x === 19 && c.toggle.indicator.color.toString() === Radio.Theme.accent.toString(), "motion off moves the switch thumb and track at once");
            input.mouseClick(c.toggle);
            check(!c.toggle.checked && thumb.x === 2, "motion off returns the thumb at once");
            Radio.Theme.motion = true;
            input.mouseClick(c.toggle);
            check(c.toggle.checked && thumb.x < 19, "motion on slides the thumb instead of jumping");
            input.mouseClick(c.toggle);
            const row = c.tabs.children.find(child => child.children.length > 1);
            check(row !== undefined, "tabs have rendered choices");
            const second = row.children.find(child => child.text === "Second");
            second.forceActiveFocus();
            input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
            check(c.chosen === 1 && c.tabs.currentIndex === 1 && second.variant === "light", "tab signal selects its original index");
            const slider = findChild(c.tabs, "tabsIndicator");
            const first = row.children.find(child => child.text === "First");
            Radio.Theme.motion = false;
            c.tabs.currentIndex = 0;
            check(slider.x === row.x + first.x && slider.width === first.width, "motion off puts the tab indicator under the selected tab at once");
            c.tabs.currentIndex = 1;
            check(slider.x === row.x + second.x && slider.width === second.width, "tab indicator follows the selection back");
            Radio.Theme.motion = true;
            c.tabs.enabled = false;
            input.mouseClick(row.children.find(child => child.text === "First"));
            check(c.chosen === 1, "disabled tabs do not dispatch selection");
            input.mouseClick(c.tag);
            check(c.tag.selected && c.tag.included, "tag fallback follows selection");
            for (const state of [0, 1, 2]) {
                c.tag.filterState = state;
                check(c.tag.included === (state === 1), "explicit tag filter overrides selection");
                check(c.tag.background.border.color.toString() === (state === 2 ? Radio.Theme.red : state === 1 ? Radio.Theme.accent : Radio.Theme.border).toString(), "tag filter border");
            }
            c.tag.enabled = false;
            input.mouseClick(c.tag);
            check(c.tag.selected && c.tag.foreground.toString() === Radio.Theme.text.toString(), "disabled excluded tag retains state with readable foreground");
            check(!artImage.cache && artImage.fillMode === Image.PreserveAspectCrop, "artwork decodes without a second image cache");
            c.artwork.source = "";
            check(artImage.status === Image.Null, "empty artwork clears the decoded source");
            const foreground = findChild(c.material, "materialForeground");
            check(foreground && foreground.width === c.material.width && foreground.height === c.material.height, "material default content fills the foreground");
            check(c.material.sourceOrigin.x === 0 && c.material.sourceOrigin.y === c.material.y, "material capture maps to its source");
            c.material.sourceItem = null;
            check(c.material.sourceOrigin.x === 0 && c.material.sourceOrigin.y === 0, "material without a source has a neutral origin");
            c.material.sourceItem = window.contentItem;
            stage = 2;
            return;
        }
        if (stage === 2) {
            check(c.material.sourceOrigin.x === c.x && c.material.sourceOrigin.y === c.y + c.material.y, "material capture follows ancestor coordinates");
            c.material.x = 12;
            c.material.width = 140;
            stage = 3;
            return;
        }
        check(c.material.sourceOrigin.x === c.x + 12 && findChild(c.material, "materialForeground").width === 140, "material capture and foreground update after layout changes");
        const tint = c.material.children.find(child => child.border !== undefined
            && child.border.color.toString() === Radio.Theme.border.toString());
        check(tint !== undefined, "material has its tinted border layer");
        for (const kind of ["thin", "regular", "thick"]) {
            c.material.kind = kind;
            check(tint.color.toString() === (kind === "thin" ? "#80000000" : kind === "regular" ? "#cc121212" : "#f20d0d0d"), "material tint " + kind);
        }
        for (const loader of [Radio.Theme.regularFont, Radio.Theme.mediumFont, Radio.Theme.semiboldFont, Radio.Theme.boldFont, Radio.Theme.fallbackFont]) {
            if (loader.status === FontLoader.Loading) return;
            check(loader.status === FontLoader.Ready, "Theme singleton loads every font");
        }
        check(Radio.Theme.fontFamily === "Poppins", "Theme font family comes from its loaded face");
        const icon = c.children.find(child => child.name === "search");
        check(icon.implicitWidth === 32 && icon.implicitHeight === 32 && !icon.children[0].enabled
            && icon.children[0].icon.source.toString().endsWith("/search.svg"), "decorative icon preserves size and native source");
        window.galleryStore.send("field", 0);
        check(window.galleryStore.state.gallery.field === "0", "Store stringifies numeric zero");
        window.galleryStore.send("field", false);
        check(window.galleryStore.state.gallery.field === "false", "Store stringifies false");
        window.galleryStore.send("field", "日本語 / Поле");
        check(window.galleryStore.state.gallery.field === "日本語 / Поле", "Store retains Unicode");
        window.galleryStore.send("field");
        check(window.galleryStore.state.gallery.field === "", "Store omitted value maps to an empty payload");
        finish();
    }
    function menuModalProbe(): void {
        if (stage === 0) {
            controls = controlFixture.createObject(window.contentItem);
            check(controls !== null, "create menu/modal fixture");
            const menu = controls.menu;
            menu.entries = [{ id: 7, label: "First" }, { id: 42, label: "Second" }, { id: 100, label: "Third" }];
            menu.choose(-1); menu.choose(3);
            check(controls.menuValue === -1, "out-of-range menu choices do not dispatch");
            menu.move(-1);
            check(menu.focusedRow === 2, "menu wraps backward");
            menu.move(1);
            check(menu.focusedRow === 0, "menu wraps forward");
            menu.entries = [];
            menu.move(1); menu.choose(0);
            check(controls.menuValue === -1 && menu.focusedRow === 0, "empty menu has no selectable result");
            menu.entries = [{ id: 7, label: "First" }, { id: 42, label: "Second" }, { id: 100, label: "Third" }];
            menu.forceActiveFocus(); menu.popup.open(); stage = 1;
            return;
        }
        const c = controls;
        const menu = c.menu;
        switch (stage) {
        case 1:
            if (!menu.popup.opened) return;
            check(window.activeFocusItem.placeholderText === "Search...", "searchable menu initially focuses its field");
            input.keyClick(Qt.Key_A, Qt.NoModifier, 0);
            check(menu.query === "a" && c.menuEdits === 1, "menu search emits its edited query");
            input.keyClick(Qt.Key_Down, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_End, Qt.NoModifier, 0);
            check(menu.focusedRow === 2, "End reaches the final menu row");
            input.keyClick(Qt.Key_Home, Qt.NoModifier, 0);
            check(menu.focusedRow === 0, "Home reaches the first menu row");
            input.keyClick(Qt.Key_Up, Qt.NoModifier, 0);
            check(menu.focusedRow === 2, "keyboard Up wraps");
            input.keyClick(Qt.Key_Down, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            stage = 2; break;
        case 2:
            if (menu.popup.visible) return;
            check(c.menuValue === 7 && menu.activeFocus, "Return selects the value role and restores focus");
            menu.searchable = false; menu.currentIndex = 1;
            menu.popup.open(); stage = 3; break;
        case 3:
            if (!menu.popup.opened) return;
            check(menu.focusedRow === 1, "menu starts at its selected row");
            check(menu.popup.modal && menu.popup.dim === false, "dropdown stays modal but does not dim the window");
            input.keyClick(Qt.Key_Space, Qt.NoModifier, 0); stage = 4; break;
        case 4:
            if (menu.popup.visible) return;
            check(c.menuValue === 42, "Space chooses the original ID");
            menu.popup.open(); stage = 5; break;
        case 5:
            if (!menu.popup.opened) return;
            menu.enabled = false; menu.choose(0); stage = 6; break;
        case 6:
            if (menu.popup.visible) return;
            check(c.menuValue === 42, "disabling the menu closes it and suppresses choices");
            c.button.forceActiveFocus();
            c.modal.dismissible = false;
            c.modal.open(); stage = 7; break;
        case 7:
            if (!c.modal.opened) return;
            check(c.modalField.activeFocus && !findChild(c.modal.contentItem, "modalClose").enabled, "nondismissible modal focuses its field and disables close");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0);
            input.mouseClick(window.contentItem, 10, 120, Qt.LeftButton, Qt.NoModifier, 0);
            stage = 8; break;
        case 8:
            check(c.modal.opened, "nondismissible modal ignores Escape and backdrop");
            for (let i = 0; i < 4; ++i) {
                input.keyClick(Qt.Key_Tab, Qt.NoModifier, 0);
                check(contained(c.modal.contentItem, window.activeFocusItem), "modal retains keyboard traversal");
            }
            c.modal.dismissible = true;
            input.mouseClick(findChild(c.modal.contentItem, "modalClose"));
            stage = 9; break;
        case 9:
            if (c.modal.visible) return;
            check(c.button.activeFocus, "closing the modal restores the recorded opener");
            c.ctx.popup(c.button); stage = 10; break;
        case 10:
            if (!c.ctx.opened) return;
            check(c.ctx.itemAt(0).activeFocus, "context menu focuses its first enabled item");
            check(c.ctxDestructive.foreground === Radio.Theme.red && c.ctxDestructive.contentItem.children[0].color === Radio.Theme.red,
                "destructive item text uses the red token");
            check(c.ctx.itemAt(0).foreground === Radio.Theme.text, "ordinary item text uses the text token");
            input.mouseClick(c.ctxDisabled);
            check(c.ctx.opened && c.ctxTriggered.length === 0, "disabled item does not activate");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0); stage = 11; break;
        case 11:
            if (c.ctx.visible) return;
            check(c.button.activeFocus, "Escape returns focus to the row that opened the menu");
            c.ctx.popup(c.button); stage = 12; break;
        case 12:
            if (!c.ctx.opened) return;
            input.keyClick(Qt.Key_Down, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0); stage = 13; break;
        case 13:
            if (c.ctx.visible) return;
            check(c.ctxTriggered.join() === "destructive" && c.button.activeFocus, "keyboard activates the destructive item and restores focus");
            check(Radio.Theme.motion, "motion is on by default");
            c.ctx.popup(c.button);
            check(c.ctx.opacity < 1 && c.ctx.scale < 1, "context menu opens with a fade and scale, not a jump");
            stage = 14; break;
        case 14:
            if (!c.ctx.opened) return;
            check(c.ctx.opacity === 1 && c.ctx.scale === 1, "context menu enter transition settles at full size");
            c.ctx.close();
            check(c.ctx.visible && !c.ctx.opened, "exit keeps the menu visible until the transition ends");
            stage = 16; break;
        case 16:
            if (c.ctx.visible) return;
            check(c.button.activeFocus, "menu exit returns focus to the opener");
            c.ctx.popup(c.button); c.ctx.close(); c.ctx.popup(c.button); c.ctx.close();
            stage = 17; break;
        case 17:
            if (c.ctx.visible) return;
            check(!c.ctx.opened && c.button.activeFocus, "rapid open, close, open (ignored during exit) and close leaves the menu hidden with focus intact");
            c.modal.open();
            check(c.modal.opacity < 1 && c.modal.slide > 0, "modal panel starts faded and shifted");
            check(c.modal.parent.children.some(item => item.children.some(child => child.objectName === "modalDimmerFade" && child.opacity < 1)), "modal backdrop starts faded");
            stage = 18; break;
        case 18: {
            if (!c.modal.opened) return;
            check(c.modal.opacity === 1 && c.modal.slide === 0, "modal enter transition settles");
            check(c.modal.parent.children.some(item => item.children.some(child => child.objectName === "modalDimmerFade" && child.opacity === 1)), "modal backdrop fades in");
            c.modal.close();
            stage = 19; break;
        }
        case 19:
            check(c.modal.visible && !c.modal.opened, "modal stays visible through its exit");
            stage = 20; break;
        case 20:
            if (c.modal.visible) return;
            check(c.button.activeFocus, "modal exit restores opener focus");
            Radio.Theme.motion = false;
            c.ctx.popup(c.button);
            stage = 21; break;
        case 21:
            if (!c.ctx.opened) return;
            check(c.ctx.opacity === 1 && c.ctx.scale === 1, "motion off opens at full opacity");
            c.ctx.close();
            stage = 22; break;
        case 22:
            // One 10 ms poll is far below the 80 ms exit duration, so only a zero duration passes.
            check(!c.ctx.visible, "motion off closes without delay");
            Radio.Theme.motion = true;
            input.mouseMove(c.icon, 5, 5);
            stage = 23; break;
        case 23: {
            const tip = c.icon.data.find(o => o.objectName === "buttonToolTip");
            check(tip.delay === 700 && tip.text === "Play fixture", "icon tooltip takes its text from accessibleName with a 700 ms delay");
            if (!tip.opened) return;
            c.icon.visible = false;
            check(!tip.opened, "hiding the owner closes its tooltip");
            stage = 24; break;
        }
        case 24: {
            const tip = c.icon.data.find(o => o.objectName === "buttonToolTip");
            if (tip.visible) return;
            c.icon.visible = true;
            input.mouseMove(c.button, 5, 5);
            stage = 25; break;
        }
        case 25: {
            const tip = c.icon.data.find(o => o.objectName === "buttonToolTip");
            if (tip.visible) return;
            check(!c.icon.hovered, "tooltip is gone after the pointer leaves");
            finish(); break;
        }
        }
    }
    function modsProbe(): void {
        if (stage === 0) {
            window.width = 1024; window.height = 640;
            controls = modsFixture.createObject(window.contentItem);
            check(controls !== null, "create offline mods fixture");
            stage = 1; return;
        }
        const c = controls;
        const popup = c.popup;
        const dt = findChild(popup.contentItem, "modsDtButton");
        const ht = findChild(popup.contentItem, "modsHtButton");
        const speed = findChild(popup.contentItem, "modsSpeedSlider");
        const pitch = findChild(popup.contentItem, "modsPitchSwitch");
        const clear = findChild(popup.contentItem, "modsClearButton");
        const retry = findChild(popup.contentItem, "modsRetryButton");
        const status = findChild(popup.contentItem, "modsSaveStatus");
        switch (stage) {
        case 1:
            c.opener.forceActiveFocus();
            input.mouseClick(c.opener);
            stage = 2; break;
        case 2:
            if (!popup.opened) return;
            check(dt.activeFocus && popup.mode === "none" && speed.value === 100
                && !speed.enabled && !pitch.enabled && !clear.enabled, "none state and initial focus");
            check(findChild(popup.contentItem, "modsSpeedLabel").text === "Speed: 1.00×"
                && !status.visible && !retry.visible, "none presentation");
            input.mouseClick(pitch); input.mouseClick(clear); input.mouseClick(speed);
            check(c.actions.length === 0, "inactive mod controls suppress mouse actions");
            dt.forceActiveFocus();
            input.keyClick(Qt.Key_D, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_E, Qt.NoModifier, 0);
            input.mouseClick(dt); input.mouseClick(ht);
            check(c.actions.join(",") === "dt,ht,dt,ht", "D/E and mouse emit one identical intent each");
            input.keyClick(Qt.Key_D, Qt.ControlModifier, 0);
            input.keyClick(Qt.Key_E, Qt.AltModifier, 0);
            check(c.actions.join(",") === "dt,ht,dt,ht", "modified letter shortcuts do not dispatch mods");
            check(popup.mode === "none" && popup.speedPercent === 100 && !popup.adjustPitch,
                "intents do not compute defaults or cycle state in QML");
            c.actions = [];
            popup.mode = "dt"; popup.speedPercent = 150;
            stage = 3; break;
        case 3:
            check(dt.variant === "accent" && dt.text === "DT (D)" && ht.variant === "alternate"
                && speed.from === 101 && speed.to === 200 && speed.stepSize === 1
                && speed.value === 150 && speed.enabled && pitch.enabled && !pitch.checked,
                "DT state and slider range");
            speed.forceActiveFocus(); input.keyClick(Qt.Key_Left, Qt.NoModifier, 0);
            check(c.actions.join(",") === "speed:149" && popup.speedPercent === 150, "one percent keyboard speed intent");
            c.actions = []; popup.speedPercent = 101;
            input.keyClick(Qt.Key_Left, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_Right, Qt.NoModifier, 0);
            check(c.actions.join(",") === "speed:102", "DT lower bound");
            c.actions = []; popup.speedPercent = 200;
            input.keyClick(Qt.Key_Right, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_Left, Qt.NoModifier, 0);
            check(c.actions.join(",") === "speed:199", "DT upper bound");
            c.actions = []; popup.speedPercent = 120; popup.adjustPitch = true;
            stage = 4; break;
        case 4:
            check(dt.text === "NC (D)" && dt.variant === "accent" && pitch.checked
                && speed.value === 120 && findChild(popup.contentItem, "modsSpeedLabel").text === "Speed: 1.20×",
                "NC uses supplied DT speed and pitch");
            input.mouseClick(pitch);
            check(c.actions.join(",") === "pitch:false" && popup.adjustPitch, "mouse pitch intent keeps input state");
            popup.adjustPitch = false;
            pitch.forceActiveFocus(); input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
            check(c.actions.join(",") === "pitch:false,pitch:true", "keyboard pitch intent");
            input.keyClick(Qt.Key_D, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_E, Qt.NoModifier, 0);
            check(c.actions.join(",") === "pitch:false,pitch:true,dt,ht", "D/E bubble from focused switch once");
            c.actions = []; popup.mode = "ht"; popup.speedPercent = 75;
            stage = 5; break;
        case 5:
            check(ht.variant === "accent" && dt.variant === "alternate" && speed.from === 50
                && speed.to === 99 && speed.value === 75, "HT state and slider range");
            speed.forceActiveFocus(); popup.speedPercent = 50;
            input.keyClick(Qt.Key_Left, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_Right, Qt.NoModifier, 0);
            check(c.actions.join(",") === "speed:51", "HT lower bound");
            c.actions = []; popup.speedPercent = 99;
            input.keyClick(Qt.Key_Right, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_Left, Qt.NoModifier, 0);
            check(c.actions.join(",") === "speed:98", "HT upper bound");
            c.actions = [];
            input.mouseClick(speed, speed.width / 2, speed.height / 2, Qt.LeftButton, Qt.NoModifier, 0);
            check(c.actions.length > 0 && c.actions.every(action => /^speed:\d+$/.test(action)
                && Number(action.slice(6)) >= 50 && Number(action.slice(6)) <= 99), "mouse speed intents use integer percent in range");
            c.actions = [];
            input.mouseClick(clear);
            clear.forceActiveFocus(); input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
            check(c.actions.join(",") === "clear,clear" && popup.mode === "ht", "mouse and keyboard clear intents");
            c.actions = []; popup.saving = true;
            stage = 6; break;
        case 6:
            check(status.visible && status.text === "Saving…" && !retry.visible && dt.enabled,
                "saving presentation allows further edits");
            popup.saving = false; popup.errorMessage = "Could not save mods. Check the connection and retry.";
            popup.canRetry = true;
            stage = 7; break;
        case 7: {
            check(status.visible && status.text === popup.errorMessage && retry.visible && retry.enabled, "save error and retry");
            const origin = popup.contentItem.mapToItem(window.contentItem, 0, 0);
            check(origin.x >= 0 && origin.y >= 0 && origin.x + popup.contentItem.width <= window.width
                && origin.y + popup.contentItem.height <= window.height, "mods popup fits 1024x640");
            for (const control of [dt, ht, speed, pitch, clear, retry]) {
                const point = control.mapToItem(popup.contentItem, 0, 0);
                check(control.Accessible.name.length > 0 && point.x >= 0 && point.y >= 0
                    && point.x + control.width <= popup.contentItem.width
                    && point.y + control.height <= popup.contentItem.height, "named mod control fits popup");
            }
            input.mouseClick(retry);
            retry.forceActiveFocus(); input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
            check(c.actions.join(",") === "retry,retry", "mouse and keyboard Retry intents");
            c.actions = []; popup.canRetry = false;
            input.mouseClick(retry);
            check(c.actions.length === 0, "retry availability is supplied by caller");
            popup.canRetry = true; popup.saving = true;
            input.mouseClick(retry);
            check(!retry.enabled && c.actions.length === 0, "pending save suppresses Retry");
            popup.saving = false;
            dt.forceActiveFocus();
            const reached = [];
            for (let i = 0; i < 10; ++i) {
                input.keyClick(Qt.Key_Tab, Qt.NoModifier, 0);
                check(contained(popup.contentItem, window.activeFocusItem), "mods Tab stays in popup");
                reached.push(window.activeFocusItem.objectName);
            }
            for (const control of [dt, ht, speed, pitch, clear, retry])
                check(reached.indexOf(control.objectName) >= 0, "mod control reachable by Tab: " + control.objectName);
            for (let i = 0; i < 10; ++i) {
                input.keyClick(Qt.Key_Backtab, Qt.ShiftModifier, 0);
                check(contained(popup.contentItem, window.activeFocusItem), "mods Shift+Tab stays in popup");
            }
            popup.available = false;
            stage = 8; break;
        }
        case 8:
            check(!dt.enabled && !ht.enabled && !speed.enabled && !pitch.enabled && !clear.enabled,
                "unavailable state disables edits");
            findChild(popup.contentItem, "modsCloseButton").forceActiveFocus();
            input.keyClick(Qt.Key_D, Qt.NoModifier, 0); input.keyClick(Qt.Key_E, Qt.NoModifier, 0);
            input.mouseClick(dt); input.mouseClick(ht); input.mouseClick(pitch); input.mouseClick(clear);
            check(c.actions.length === 0, "disabled edits suppress keyboard and mouse intents");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0);
            stage = 9; break;
        case 9:
            if (popup.visible) return;
            check(c.opener.activeFocus, "mods Escape restores opener focus");
            input.keyClick(Qt.Key_D, Qt.NoModifier, 0); input.keyClick(Qt.Key_E, Qt.NoModifier, 0);
            check(c.actions.length === 0, "closed popup ignores D/E");
            popup.open();
            stage = 10; break;
        case 10:
            if (!popup.opened) return;
            check(findChild(popup.contentItem, "modsCloseButton").activeFocus, "unavailable popup focuses reachable close button");
            popup.available = true;
            input.mouseClick(window.contentItem, 4, 120, Qt.LeftButton, Qt.NoModifier, 0);
            stage = 11; break;
        case 11:
            if (popup.visible) return;
            check(c.opener.activeFocus && !contained(popup.contentItem, window.activeFocusItem), "outside dismiss restores focus");
            popup.open(); stage = 12; break;
        case 12:
            if (!popup.opened) return;
            input.mouseClick(findChild(popup.contentItem, "modsCloseButton"));
            stage = 13; break;
        case 13:
            if (popup.visible) return;
            check(c.opener.activeFocus, "close button restores focus");
            input.keyClick(Qt.Key_D, Qt.NoModifier, 0); input.keyClick(Qt.Key_E, Qt.NoModifier, 0);
            check(c.actions.length === 0, "hidden mod controls retain no keyboard focus");
            finish(); break;
        }
    }
    function windowProbe(): void {
        if (stage === 0) {
            controls = windowFixture.createObject(window.contentItem);
            check(controls !== null, "create title-bar fixture");
            stage = 1; return;
        }
        const c = controls;
        if (stage === 2) {
            const bar = c.bar;
            const indicator = findChild(bar, "tabIndicator");
            const target = findChild(bar, "playlistsTabButton");
            if (Math.abs(indicator.x - target.mapToItem(bar, 0, 0).x) > 0.5) return;
            check(Math.abs(indicator.width - target.width) < 0.5 && c.tabs === 1, "rapid tab switches leave the indicator under the last selected tab");
            finish(); return;
        }
        const bar = c.bar;
        const fake = c.windowDouble;
        const resize = c.surface.children.find(child => child.z === 1000);
        check(resize !== undefined, "resize handles belong to the complete window");
        const right = bar.children.find(child => child.children.some(button => button.objectName === "queueButton"));
        const maximize = right.children.find(button => button.contentItem && button.contentItem.name === "square");
        const minimize = right.children.find(button => button.contentItem && button.contentItem.name === "minus");
        const close = right.children.find(button => button.contentItem && button.contentItem.name === "x");
        input.mouseClick(findChild(bar, "playlistsTabButton"));
        check(c.tabs === 1 && bar.selectedTab === 1 && fake.moves === 0, "tab hit target dispatches without starting a window move");
        input.mouseClick(findChild(bar, "settingsButton"));
        check(c.tabs === 2 && findChild(bar, "settingsButton").variant === "light", "Settings tab selects its own index");
        const queue = findChild(bar, "queueButton");
        input.mouseClick(queue);
        check(c.queues === 0, "disconnected queue control suppresses activation");
        bar.queueEnabled = true;
        input.mouseClick(queue);
        check(c.queues === 1 && queue.accessibleName === "Hide queue", "queue action projects its open state");
        input.mouseClick(queue);
        check(c.queues === 2 && queue.accessibleName === "Show queue", "queue action projects its closed state");
        const previousMoves = fake.moves;
        input.mouseClick(bar, 650, 25, Qt.LeftButton, Qt.NoModifier, 0);
        check(fake.moves === previousMoves + 1, "unoccupied title-bar space requests a system move");
        const handles = resize.children.filter(child => child.modelData !== undefined);
        check(handles.length === 8, "eight edge/corner resize handles");
        for (const handle of handles) {
            check(handle.width > 0 && handle.height > 0 && handle.x >= 0 && handle.y >= 0
                && handle.x + handle.width <= c.surface.width && handle.y + handle.height <= c.surface.height, "resize target stays inside window bounds");
            input.mouseClick(handle, handle.width / 2, handle.height / 2, Qt.LeftButton, Qt.NoModifier, 0);
        }
        const expected = [Qt.TopEdge, Qt.BottomEdge, Qt.LeftEdge, Qt.RightEdge,
            Qt.TopEdge | Qt.LeftEdge, Qt.TopEdge | Qt.RightEdge,
            Qt.BottomEdge | Qt.LeftEdge, Qt.BottomEdge | Qt.RightEdge];
        check(JSON.stringify(fake.edges) === JSON.stringify(expected), "resize handlers forward the exact edge/corner flags");
        input.mouseClick(maximize);
        check(fake.visibility === Window.Maximized && bar.maximized && !resize.visible && maximize.contentItem.name === "copy", "maximize updates observed state and hides resize handles");
        input.mouseClick(maximize);
        check(fake.visibility === Window.Windowed && !bar.maximized && resize.visible, "Restore returns to windowed state");
        fake.visibility = Window.FullScreen;
        check(!resize.visible, "fullscreen suppresses resize handles");
        fake.showNormal();
        input.mouseClick(minimize);
        check(fake.visibility === Window.Minimized, "minimize routes to the window");
        fake.showNormal();
        bar.gallery = true;
        check(!queue.visible && !findChild(bar, "settingsButton").enabled
            && findChild(bar, "songsTabButton").text === "Components", "gallery removes live-only window controls");
        input.mouseClick(findChild(bar, "settingsButton"));
        check(c.tabs === 2, "gallery Settings remains unavailable");
        input.mouseClick(findChild(bar, "songsTabButton"));
        check(c.tabs === 0, "Components navigation retains the Songs index");
        input.mouseClick(close);
        check(fake.closes === 1, "close routes to the window");
        const indicator = findChild(bar, "tabIndicator");
        const songs = findChild(bar, "songsTabButton");
        const playlists = findChild(bar, "playlistsTabButton");
        check(Math.abs(indicator.x - songs.mapToItem(bar, 0, 0).x) < 0.5, "indicator starts under the selected tab");
        Radio.Theme.motion = false;
        input.mouseClick(playlists);
        check(indicator.x === playlists.mapToItem(bar, 0, 0).x && indicator.width === playlists.width, "motion off puts the window indicator under the tab at once");
        Radio.Theme.motion = true;
        input.mouseClick(songs);
        check(indicator.x > songs.mapToItem(bar, 0, 0).x, "motion on slides the indicator instead of jumping");
        input.mouseClick(playlists);
        input.mouseClick(songs);
        input.mouseClick(playlists);
        stage = 2;
    }
    function panelProbe(): void {
        if (stage === 0) {
            controls = panelFixture.createObject(window.contentItem);
            check(controls !== null, "create composed panels");
            controls.pane.editorOpen = true;
            stage = 1; return;
        }
        const c = controls;
        const pane = c.pane;
        const editor = findChild(pane, "playlistEditor");
        const name = findChild(editor, "playlistEditorName");
        const save = findChild(editor, "savePlaylistEditor");
        const folders = c.folders;
        const lists = c.playlists;
        switch (stage) {
        case 1:
            check(textChild(pane, "Create your first playlist with +").visible && name.activeFocus, "empty pane and editor initial focus");
            pane.editorName = "  ";
            check(!save.enabled, "blank playlist name cannot save");
            name.forceActiveFocus(); input.keyClick(Qt.Key_A, Qt.NoModifier, 0);
            check(pane.editorName.trim() === "a" && c.actions.includes("name") && save.enabled, "name edits reach the composed owner");
            pane.coverPreparing = true;
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            check(!save.enabled && findChild(editor, "playlistCoverPreparationStatus").visible && !c.actions.includes("save"), "preparation gates keyboard save");
            pane.coverPreparing = false;
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            check(c.actions.filter(action => action === "save").length === 1, "Return saves once when ready");
            input.mouseClick(findChild(editor, "choosePlaylistCover"));
            input.mouseClick(findChild(editor, "resetPlaylistCover"));
            check(c.actions.includes("cover") && c.actions.includes("reset"), "editor forwards cover actions");
            pane.busy = true;
            check(!name.enabled && !save.enabled && !findChild(editor, "closePlaylistEditor").enabled
                && !findChild(editor, "choosePlaylistCover").enabled && !findChild(editor, "resetPlaylistCover").enabled, "busy editor suppresses every mutation control");
            pane.busy = false;
            input.mouseClick(findChild(editor, "closePlaylistEditor"));
            check(c.actions.includes("cancel"), "editor close forwards cancellation");
            pane.editorOpen = false;
            pane.query = "missing";
            pane.connected = false;
            stage = 2; break;
        case 2:
            check(textChild(pane, "No matching playlists").visible && !findChild(pane, "createPlaylistButton").enabled
                && !findChild(pane, "refreshPlaylists").enabled, "empty search and offline controls");
            pane.connected = true;
            pane.playlists = [{ id: 7, name: "Empty", itemCount: 0, artworkUrl: "" }, { id: 42, name: "Songs", itemCount: 2, artworkUrl: "" }];
            stage = 3; break;
        case 3: {
            const cards = findChild(pane, "playlistList");
            const card = cards.itemAtIndex(0);
            if (!card || !cards.itemAtIndex(1)) return;
            input.mouseClick(card);
            check(c.actions.includes("select:7"), "playlist card forwards the actual playlist ID");
            findChild(card, "playlistActionsButton").clicked();
            stage = 4; break;
        }
        case 4: {
            const card = findChild(pane, "playlistList").itemAtIndex(0);
            const menu = findChild(card, "playlistActionsMenu");
            if (!menu.opened) return;
            check(!menu.itemAt(0).enabled && menu.itemAt(1).activeFocus, "empty playlist disables queue and focuses Edit");
            const populated = findChild(findChild(pane, "playlistList").itemAtIndex(1), "playlistActionsMenu");
            pane.queueBusy = true;
            check(!populated.itemAt(0).enabled && populated.itemAt(1).enabled, "queue busy gates playlist append independently of editing");
            pane.queueBusy = false;
            pane.connected = false;
            check(!populated.itemAt(0).enabled, "offline gates playlist append");
            pane.connected = true;
            check(populated.itemAt(0).enabled, "connected idle playlist append recovers");
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            stage = 5; break;
        }
        case 5:
            check(c.actions.includes("edit:7"), "playlist menu preserves the selected ID");
            pane.activeId = 42; pane.activeName = "Songs";
            stage = 6; break;
        case 6:
            check(findChild(pane, "activePlaylistTitle").text === "Songs" && pane.tracksList.visible
                && textChild(pane, "This playlist is empty").visible, "detail header and empty track list");
            {
                const header = findChild(pane, "playlistDetailHeader");
                const addQueue = findChild(header, "playlistDetailAddQueue");
                const play = findChild(header, "playlistDetailPlay");
                check(header.visible && textChild(header, "0 tracks").visible && !addQueue.enabled && !play.enabled,
                    "detail header shows the count and disables actions for an empty playlist");
                pane.trackCount = 1;
                check(textChild(header, "1 track").visible && addQueue.enabled && play.enabled, "single track uses the singular and enables actions");
                pane.queueBusy = true;
                check(!addQueue.enabled && play.enabled, "queue busy gates only Add to queue");
                pane.queueBusy = false;
                pane.busy = true;
                check(!addQueue.enabled && !play.enabled, "busy gates both header actions");
                pane.busy = false;
                input.mouseClick(addQueue);
                input.mouseClick(play);
                check(c.actions.includes("queue:42") && c.actions.includes("play:42"), "header actions forward the active playlist ID");
                pane.trackCount = 0;
                pane.activeId = -1;
                check(!header.visible, "header hides in the list view");
                pane.activeId = 42;
            }
            input.mouseClick(findChild(pane, "playlistBackButton"));
            check(c.actions.includes("select:-1"), "back forwards the sentinel list selection");
            folders.rows = [
                { kind: "stable", path: "/fixture/stable", markerPath: "/fixture/osu!.db", registered: true, action: "remove", countPending: true, countError: "", count: 0, error: "" },
                { kind: "lazer", path: "/fixture/lazer", markerPath: "/fixture/client.realm", registered: false, action: "add", countPending: false, countError: "retry count", count: 0, error: "import failed" }
            ];
            folders.open(); stage = 7; break;
        case 7:
            if (!folders.opened) return;
            check(folders.hasPending && findChild(folders.contentItem, "folderSelectionRow0").border.color.toString() === Radio.Theme.red.toString()
                && textChild(folders.contentItem, "Count unavailable").visible, "folder actions and count failure project independently");
            const status = i => findChild(folders.contentItem, "folderStatusText" + i);
            check(status(0).text === "Will be removed" && status(1).text === "Will be added"
                && findChild(folders.contentItem, "folderStatus0").visible, "state capsules name the staged action in words");
            check(textChild(folders.contentItem, "Need help?") === null, "help link is gone");
            const stagedRows = folders.rows;
            const withAction = action => stagedRows.map(r => Object.assign({}, r, { action }));
            folders.rows = withAction("refresh");
            check(status(0).text === "Will be refreshed" && findChild(folders.contentItem, "folderStatus1").visible, "refresh capsule appears");
            folders.rows = withAction("");
            check(!findChild(folders.contentItem, "folderStatus0").visible && !findChild(folders.contentItem, "folderStatus1").visible, "capsules disappear without a staged action");
            folders.rows = stagedRows;
            input.mouseClick(findChild(folders.contentItem, "folderRefresh0"));
            input.mouseClick(findChild(folders.contentItem, "folderToggle1"));
            check(c.actions.includes("folder-refresh:/fixture/osu!.db") && c.actions.includes("toggle:/fixture/client.realm"), "folder controls retain marker identities");
            input.mouseClick(findChild(folders.contentItem, "browseFolders"));
            input.mouseClick(findChild(folders.contentItem, "applyFolders"));
            check(c.actions.includes("browse") && c.actions.includes("apply"), "folder footer actions dispatch");
            folders.applying = true;
            check(!folders.dismissible && !findChild(folders.contentItem, "browseFolders").enabled
                && !findChild(folders.contentItem, "applyFolders").enabled && !findChild(folders.contentItem, "folderToggle0").enabled, "applying locks folder mutation and dismissal");
            folders.applying = false; folders.picking = true;
            check(!findChild(folders.contentItem, "browseFolders").enabled && !findChild(folders.contentItem, "applyFolders").enabled, "picker gates browse/apply");
            folders.picking = false;
            folders.close(); stage = 8; break;
        case 8:
            if (folders.visible) return;
            lists.playlists = [{ id: 42, name: "Target" }];
            lists.candidates = [{ id: 701, name: "Hard", checked: false }];
            lists.open(); stage = 9; break;
        case 9:
            if (!lists.opened) return;
            check(!findChild(lists.contentItem, "confirmPlaylistAdd").enabled, "playlist add needs target and checked difficulty");
            findChild(lists.contentItem, "playlistTargetMenu").choose(0);
            const candidate = textChild(lists.contentItem, "Hard");
            input.mouseClick(candidate);
            check(c.actions.includes("target:42") && c.actions.includes("difficulty:701"), "add modal forwards target/difficulty IDs");
            lists.candidates = [{ id: 701, name: "Hard", checked: true }];
            stage = 10; break;
        case 10:
            input.mouseClick(findChild(lists.contentItem, "confirmPlaylistAdd"));
            check(c.actions.includes("add"), "ready modal dispatches addition");
            lists.busy = true;
            check(!lists.dismissible && !findChild(lists.contentItem, "confirmPlaylistAdd").enabled
                && !findChild(lists.contentItem, "playlistTargetMenu").enabled, "busy add modal locks controls");
            lists.busy = false; lists.close(); stage = 11; break;
        case 11:
            if (lists.visible) return;
            c.queue.loading = true; c.queue.open(); stage = 12; break;
        case 12:
            if (!c.queue.opened) return;
            check(findChild(c.queue.contentItem, "queueStatus").text === "Loading queue…", "empty queue loading state");
            c.queue.message = "Retry queue";
            check(!findChild(c.queue.contentItem, "queueRetryButton").enabled, "loading queue suppresses retry");
            stage = 120; break;
        case 120:
            c.queue.loading = false;
            check(input.waitForRendering(c.queue.contentItem, 1000), "queue retry layout renders before pointer activation");
            input.mouseClick(findChild(c.queue.contentItem, "queueRetryButton"));
            check(c.actions.includes("queue-retry"), "queue retry forwards to the owner");
            c.queue.message = "";
            c.queue.tracks = c.queueMock.queueModel;
            stage = 13; break;
        case 13:
            if (c.queue.liveDelegates !== 5) return;
            check(c.viewportChanges > 0 && c.queue.mediaDelegates.some(row => row.entryId === "1")
                && c.queue.mediaDelegates.some(row => row.entryId === "3"), "queue retains duplicate entries and registers viewport demand");
            check(queueSections(c.queue).join() === "HISTORY,NOW PLAYING,UP NEXT", "queue shows history, current and upcoming sections");
            check(findChild(c.queue.contentItem, "queueLeft").text === "· 3 left", "queue header counts upcoming rows only");
            check(findChild(c.queue.contentItem, "queueClearUpcoming") && findChild(c.queue.contentItem, "queueClearAll"), "header keeps both clear actions");
            queueSectionDropChecks(c);
            c.queueMock.dispatch("queueClearAll", ""); stage = 14; break;
        case 14:
            if (c.queue.liveDelegates !== 0) return;
            check(findChild(c.queue.contentItem, "queueStatus").text === "Queue is empty", "queue shrink releases delegates and restores empty state");
            check(queueSections(c.queue).length === 0 && !findChild(c.queue.contentItem, "queueLeft").visible, "empty queue has no sections or counter");
            input.mouseClick(findChild(c.queue.contentItem, "queueCloseButton"));
            stage = 15; break;
        case 15:
            if (c.queue.visible) return;
            finish(); break;
        }
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
            check(menu.width === 252 && menu.background.radius === 12, "rounded playlist menu proportions");
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
            if (findChild(card, "playlistActionsMenu").visible) return;
            check(JSON.parse(mock.stateJson).queue.entries.length > 5 && JSON.parse(mock.stateJson).queue.mode === "paused", "Add to queue appends in memory without starting music");
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
            const frame = card.background.children[card.background.children.length - 1];
            const wasSelected = card.selected;
            Radio.Theme.motion = false;
            card.selected = true;
            check(frame.border.color.toString() === Radio.Theme.accent.toString() && frame.border.width === (card.activeFocus ? 2 : 1), "selected playlist card uses the shared accent frame");
            card.selected = wasSelected;
            Radio.Theme.motion = true;
            const menuOrigin = menu.contentItem.mapToItem(window.contentItem, 0, 0);
            const cardOrigin = card.mapToItem(window.contentItem, 0, 0);
            check(Math.abs(menuOrigin.x - cardOrigin.x) < 20, "keyboard removal menu is anchored to its card");
            check(contained(menu.contentItem, window.activeFocusItem), "removal menu receives keyboard focus");
            findChild(card, "removePlaylistTrack").forceActiveFocus();
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
            const dialog = findChild(window.contentItem, "confirmDialog");
            if (!dialog.opened) return;
            check(pane.playlists.length === 2 && dialog.title.startsWith("Delete “"), "Delete asks first and names the playlist");
            input.mouseClick(findChild(dialog, "confirmAccept"));
            stage = 25;
            break;
        }
        case 25: {
            const pane = findChild(window.contentItem, "demoPlaylistPane");
            if (pane.playlists.length !== 1) return;
            check(pane.playlists[0].id === 1, "Delete removes the selected playlist");
            check(findChild(window.contentItem, "toast").text === "Playlist deleted", "Delete confirms with a toast");
            finish();
            break;
        }
        }
    }
    function folderProbe(): void {
        if (!bridge.connected || bridge.settings.foldersLoading || bridge.library.libraryLoading) return;
        const dialog = findChild(window.contentItem, "folderSelectionDialog");
        const fresh = "/fixtures/101/osu!.db";
        const failed = "/fixtures/102/client.realm";
        switch (stage) {
        case 0:
            bridge.settings.addFolder();
            stage = 1;
            break;
        case 1:
            if (bridge.settings.folderSelectionDiscovering || bridge.settings.folderSelectionRows.length !== 4 || !dialog.opened) return;
            if (bridge.settings.folderSelectionRows.some(row => row.countPending)) return;
            check(bridge.settings.folderSelectionRows[3].countError.includes("fixture preview"), "count failure is separate from staging");
            bridge.settings.retryFolderCount(failed);
            stage = 2;
            break;
        case 2:
            if (bridge.settings.folderSelectionRows[3].countPending) return;
            check(bridge.settings.folderSelectionRows[3].count === "0", "zero is a real count");
            bridge.settings.toggleFolderSelection(fresh);
            bridge.settings.toggleFolderSelection(fresh);
            stage = 3;
            break;
        case 3:
            if (bridge.settings.folderSelectionRows[2].action !== "") return;
            check(bridge.settings.folders.length === 2, "choosing and staging do not save");
            bridge.settings.toggleFolderSelection(fresh);
            bridge.settings.toggleFolderSelection(failed);
            bridge.settings.toggleFolderSelection("/fixtures/31/client.realm");
            stage = 4;
            break;
        case 4:
            if (bridge.settings.folderSelectionRows[2].action !== "add" || bridge.settings.folderSelectionRows[0].action !== "remove") return;
            bridge.settings.applyFolderSelection();
            stage = 5;
            break;
        case 5:
            if (!bridge.settings.folderSelectionApplying) return;
            check(!dialog.dismissible && !findChild(dialog.contentItem, "applyFolders").enabled, "Apply disables editing and dismissal");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0);
            bridge.settings.closeFolderSelection();
            stage = 6;
            break;
        case 6:
            if (bridge.settings.folderSelectionApplying) return;
            check(bridge.settings.folderSelectionOpen && dialog.visible, "partial failure remains open");
            check(bridge.settings.folderSelectionRows[2].registered && bridge.settings.folderSelectionRows[2].action === "", "successful addition clears immediately");
            check(!bridge.settings.folderSelectionRows[0].registered && bridge.settings.folderSelectionRows[0].action === "", "successful removal clears immediately");
            check(bridge.settings.folderSelectionRows[3].action === "add" && bridge.settings.folderSelectionRows[3].error.includes("fixture import"), "failed action retained");
            check(bridge.settings.folders.length === 2, "saved folders refreshed after partial success");
            bridge.settings.applyFolderSelection();
            stage = 7;
            break;
        case 7:
            if (bridge.settings.folderSelectionOpen || bridge.settings.folderSelectionApplying) return;
            check(bridge.settings.folders.length === 3, "retry saves remaining action");
            bridge.settings.addFolder();
            stage = 8;
            break;
        case 8:
            if (!bridge.settings.folderSelectionOpen || bridge.settings.folderSelectionRows.length === 0 || !dialog.opened
                || bridge.settings.folderSelectionDiscovering || bridge.settings.folderSelectionRows.some(row => row.countPending)) return;
            const refresh = findChild(dialog.contentItem, "folderRefresh0");
            check(refresh.accessibleName === "Refresh import", "registered row has accessible refresh control");
            refresh.forceActiveFocus();
            input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
            stage = 9;
            break;
        case 9:
            if (bridge.settings.folderSelectionRows[0].action !== "refresh") return;
            bridge.settings.toggleFolderSelection("/fixtures/52/client.realm");
            stage = 10;
            break;
        case 10:
            if (bridge.settings.folderSelectionRows[0].action !== "remove") return;
            bridge.settings.refreshFolderSelection("/fixtures/52/client.realm");
            stage = 11;
            break;
        case 11:
            if (bridge.settings.folderSelectionRows[0].action !== "refresh") return;
            bridge.settings.applyFolderSelection();
            stage = 12;
            break;
        case 12:
            if (bridge.settings.folderSelectionOpen || bridge.settings.folderSelectionApplying) return;
            check(bridge.settings.folders.length === 3, "refresh preserves registered folders");
            check(bridge.settings.folderSelectionRows.length === 0, "successful refresh closes and discards preview state");
            finish();
            break;
        }
    }
    function settingsSearchProbe(): void {
        if (!bridge.connected || bridge.library.libraryLoading || bridge.settings.foldersLoading || !bridge.settings.audioSettingsLoaded) return;
        const pane = findChild(window.contentItem, "settingsPane");
        const search = findChild(pane, "settingsSearch");
        const general = findChild(pane, "generalSettingsSection");
        const audio = findChild(pane, "audioSettingsSection");
        const about = findChild(pane, "aboutSettingsSection");
        const discord = findChild(pane, "discordSettingsSection");
        const version = findChild(pane, "aboutVersion");
        const license = findChild(pane, "aboutLicense");
        const project = findChild(pane, "aboutProjectSetting");
        const link = findChild(pane, "aboutProjectLink");
        const individual = findChild(pane, "individualVolumeSwitch");
        const modsSwitch = findChild(pane, "individualModsSwitch");
        const global = findChild(pane, "globalVolumeSetting");
        const empty = findChild(pane, "settingsSearchEmpty");
        const content = findChild(pane, "settingsContent");
        const scroll = findChild(pane, "settingsScroll").contentItem;
        const message = findChild(pane, "audioSettingsMessage");
        switch (stage) {
        case 0:
            if ((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 3) return;
            bridge.library.selectTrack(42);
            input.mouseClick(findChild(window.contentItem, "settingsButton"));
            check(general.visible && audio.visible && about.visible && individual.visible && !global.visible && !empty.visible,
                "empty query shows current settings and respects conditional global volume");
            check(general.y < audio.y && audio.y < discord.y && discord.y < about.y,
                "sections are ordered General, Audio, Integrations, About");
            check(Qt.application.version === "0.1.0-alpha" && version.text === "Version — " + Qt.application.version,
                "About shows the nonempty release version from Qt application metadata");
            check(license.text === "License — MIT" && link.text === "https://github.com/Jayko321/osu-radio"
                && link.Accessible.name === "Project: " + link.text && link.enabled,
                "About exposes MIT and the accessible project link");
            input.mouseClick(search);
            input.keyClick(Qt.Key_A, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_U, Qt.ShiftModifier, 0);
            input.keyClick(Qt.Key_D, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_I, Qt.ShiftModifier, 0);
            input.keyClick(Qt.Key_O, Qt.NoModifier, 0);
            stage = 1;
            break;
        case 1:
            check(search.text.toLowerCase() === "audio" && !general.visible && audio.visible && individual.visible && !empty.visible,
                "real keyboard edits filter by the case-insensitive section title: " + search.text);
            check(audio.y === 0, "hidden General leaves no gap before Audio");
            search.text = settingsQueries[0].query;
            stage = 2;
            break;
        case 2: {
            const expected = settingsQueries[settingsScenario];
            check(general.visible === expected.general && audio.visible === expected.audio
                && individual.visible === expected.individual && !global.visible,
                "section/setting substring filter: " + expected.query);
            check(expected.mods === undefined || modsSwitch.visible === expected.mods, "individual mods switch filter: " + expected.query);
            check(about.visible === !!expected.about && version.visible === !!expected.version
                && license.visible === !!expected.license && project.visible === !!expected.project,
                "About section/field substring filter: " + expected.query);
            if (about.visible && !expected.general && !expected.audio) {
                const first = expected.version ? version : expected.license ? license : project;
                check(about.y === 0 && first.mapToItem(about, 0, 0).y === findChild(about, "aboutSettingsHeader").height + about.spacing + 16,
                    "hidden About rows/sections leave no gaps: " + expected.query);
            }
            check(empty.visible === (!expected.general && !expected.audio && !expected.about), "empty result state: " + expected.query);
            if (empty.visible) {
                check(empty.text.includes("No settings found") && empty.y === 0 && content.implicitHeight === empty.height,
                    "empty state is readable without hidden-section gaps");
            }
            if (++settingsScenario < settingsQueries.length) search.text = settingsQueries[settingsScenario].query;
            else {
                message.text = "Long audio status / Длинное сообщение / 日本語 / ".repeat(50);
                stage = 3;
            }
            break;
        }
        case 3:
            check(scroll.contentHeight > scroll.height, "wrapped status extends the Settings scroll content");
            scroll.contentY = scroll.contentHeight - scroll.height;
            check(scroll.contentY > 0, "long Settings content can be scrolled");
            search.text = "no such setting";
            stage = 4;
            break;
        case 4:
            check(empty.visible && scroll.contentY === 0 && scroll.contentHeight === empty.height,
                "narrowing from the bottom resets scroll and collapses hidden content");
            search.forceActiveFocus();
            input.keyClick(Qt.Key_A, Qt.ControlModifier, 0);
            input.keyClick(Qt.Key_Backspace, Qt.NoModifier, 0);
            stage = 5;
            break;
        case 5:
            check(search.text === "" && general.visible && audio.visible && about.visible && !empty.visible
                && scroll.contentHeight > scroll.height, "keyboard clear restores the full scroll layout");
            message.text = Qt.binding(() => bridge.settings.audioSettingsMessage);
            bridge.settings.setIndividualVolumeEnabled(true);
            stage = 6;
            break;
        case 6:
            if (!bridge.settings.individualVolumeEnabled) return;
            check(global.visible, "global volume remains conditional on individual mode");
            search.text = "gLoBaL vOlUmE";
            stage = 7;
            break;
        case 7:
            check(!general.visible && audio.visible && !individual.visible && global.visible && !modsSwitch.visible && !empty.visible,
                "matching a setting hides other settings inside the same section");
            check(global.y === 0, "hidden individual-volume row leaves no layout gap");
            check(bridge.player.selectedAudioId === 42 && findChild(window.contentItem, "playerPane").visible,
                "filtering preserves the selected player");
            input.mouseClick(findChild(window.contentItem, "songsTabButton"));
            stage = 8;
            break;
        case 8:
            if (bridge.library.libraryLoading) return;
            check(findChild(window.contentItem, "songSearch").text === "" && bridge.player.selectedAudioId === 42,
                "Settings query is independent of Songs and selection");
            input.mouseClick(findChild(window.contentItem, "settingsButton"));
            stage = 9;
            break;
        case 9:
            check(search.text === "gLoBaL vOlUmE" && global.visible && !individual.visible,
                "returning to Settings retains query and results");
            search.text = "AuDiO";
            stage = 10;
            break;
        case 10:
            check(audio.visible && individual.visible && global.visible && modsSwitch.visible,
                "matching a section shows all currently available settings");
            bridge.settings.setIndividualVolumeEnabled(false);
            stage = 11;
            break;
        case 11:
            if (bridge.settings.individualVolumeEnabled) return;
            search.text = "global volume";
            stage = 12;
            break;
        case 12:
            check(empty.visible && !audio.visible && !global.visible,
                "search never reveals a conditionally unavailable setting");
            search.text = "version";
            Qt.application.version = "metadata-binding-probe";
            stage = 13;
            break;
        case 13:
            check(version.text === "Version — metadata-binding-probe", "About follows application metadata changes");
            Qt.application.version = "0.1.0-alpha";
            stage = 14;
            break;
        case 14:
            check(version.text === "Version — 0.1.0-alpha", "About restores the release metadata binding");
            finish();
            break;
        }
    }
    function searchProbe(): void {
        if (!bridge.connected || bridge.library.libraryLoading) return;
        const search = findChild(window.contentItem, "songSearch");
        switch (stage) {
        case 0:
            if ((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 3) return;
            findChild(window.contentItem, "settingsSearch").text = "settings query";
            bridge.library.selectTrack(42);
            search.text = "r";
            search.text = "ro";
            search.text = "roc hard";
            stage = 1;
            break;
        case 1:
            check((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) === 1 && bridge.player.selectedAudioId === 42, "search preserves selected audio ID");
            check(bridge.player.selectedSubtitle === "Fixture artist | Hard", "filtered split label remains stable");
            check(findChild(window.contentItem, "settingsSearch").text === "settings query", "Songs search preserves the independent Settings query");
            search.text = "missing";
            stage = 2;
            break;
        case 2:
            check((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) === 0 && bridge.library.libraryMessage === "Nothing found.", "separate search empty state");
            check(findChild(window.contentItem, "songsEmptyState").visible && findChild(window.contentItem, "songsClearSearch"), "Songs shows its empty state");
            search.text = "retry";
            stage = 3;
            break;
        case 3:
            check(bridge.library.libraryMessage.includes("fixture search"), "search error visible");
            check(!findChild(window.contentItem, "songsEmptyState").visible, "a load error is not shown as an empty result");
            bridge.library.refreshLibrary();
            stage = 4;
            break;
        case 4:
            check((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) === 1 && bridge.player.selectedAudioId === 42, "refresh retries current query");
            const clear = findChild(search, "clearSearchButton");
            check(clear.visible, "clear button follows search text");
            clear.clicked();
            check(search.text === "" && !clear.visible, "clear button empties the field and hides");
            stage = 5;
            break;
        case 5:
            check((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) === 3 && bridge.player.selectedAudioId === 42, "clear restores library and selection");
            finish();
            break;
        }
    }
    property var menuRow: null
    function playbackProbe(): void {
        if (!bridge.connected || bridge.library.libraryLoading || !bridge.settings.audioSettingsLoaded) return;
        const play = findChild(window.contentItem, "playPauseButton");
        const progress = findChild(window.contentItem, "playbackProgress");
        switch (stage) {
        case 0:
            if ((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 3) return;
            check(bridge.player.currentAudioId === -1 && bridge.player.loadingAudioId === -1, "selection alone does not start audio");
            check(play.enabled && play.accessibleName === "Play", "selected track can play");
            check(!progress.enabled && bridge.player.playbackPositionLabel === "00:00", "noncurrent selection has no seeking");
            check(findChild(window.contentItem, "shuffleButton") !== null && findChild(window.contentItem, "repeatButton") !== null,
                "transport mode buttons exist");
            window.width = 1024; window.height = 640;
            stage = 30;
            break;
        case 10: {
            const panel = findChild(window.contentItem, "playerPane");
            check(panel.width === window.width - 480 && panel.contentWidth <= panel.width * 0.85
                && panel.contentLeft >= 0 && panel.coverSize <= Math.max(panel.height - 225, 0), "player geometry stays inside the minimum window");
            check(progress.from === 0 && progress.to >= 1 && progress.value === 0, "empty playback keeps a valid progress range");
            bridge.library.selectTrack(42);
            stage = 1;
            break;
        }
        case 1:
            if (bridge.player.selectedAudioId !== 42) return;
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
            if (bridge.player.loadingAudioId !== 42 || Math.round(bridge.settings.volume * 100) !== 25) return;
            check(bridge.player.playbackMessage.includes("Loading"), "loading status projects immediately");
            check(bridge.player.currentAudioId === 42 && !progress.enabled, "assignment selects current before audio finishes loading");
            bridge.library.selectTrack(7);
            stage = 4;
            break;
        case 4: {
            if (bridge.player.selectedAudioId !== 7 || bridge.player.loadingAudioId !== -1 || bridge.player.currentAudioId !== -1) return;
            check(bridge.player.playbackMessage.includes("fixture audio unavailable"), "download error is visible beside controls");
            check(bridge.player.currentAudioId === -1 && !progress.enabled, "failed download leaves playback empty");
            check(Math.round(bridge.settings.volume * 100) === 25, "volume survives selection changes and failed download");
            const songs = findChild(window.contentItem, "songList");
            for (let i = 0; i < songs.count; ++i) if (songs.itemAtIndex(i) && songs.itemAtIndex(i).audioId === 42) menuRow = songs.itemAtIndex(i);
            if (!menuRow) return;
            menuRow.forceActiveFocus();
            input.keyClick(Qt.Key_Menu, Qt.NoModifier, 0);
            stage = 5;
            break;
        }
        case 5: {
            const menu = findChild(menuRow, "songQueueMenu");
            if (!menu.opened) return;
            check(bridge.player.selectedAudioId === 7, "menu row differs from the selection");
            const command = findChild(menuRow, "playQueueTrack");
            command.forceActiveFocus();
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            stage = 6;
            break;
        }
        case 6:
            if (bridge.player.selectedAudioId !== menuRow.audioId || bridge.player.loadingAudioId !== menuRow.audioId) return;
            check(bridge.player.currentAudioId === menuRow.audioId, "menu Play selects the row and starts it");
            stage = 7;
            break;
        case 7:
            if (findChild(menuRow, "songQueueMenu").visible) return;
            menuRow.forceActiveFocus();
            input.keyClick(Qt.Key_Menu, Qt.NoModifier, 0);
            stage = 8;
            break;
        case 8:
            if (!findChild(menuRow, "songQueueMenu").opened) return;
            findChild(menuRow, "addTrackToPlaylist").forceActiveFocus();
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            stage = 9;
            break;
        case 9:
            if (!bridge.playlists.playlistAdding) return;
            check(bridge.player.selectedAudioId === menuRow.audioId, "Add to playlist opens the dialog for the menu row");
            finish();
            break;
        case 30: {
            // Modes: icon, accessible name and accent per state, and the Off -> All -> One -> Off cycle through the adapter.
            const shuffle = findChild(window.contentItem, "shuffleButton");
            const repeat = findChild(window.contentItem, "repeatButton");
            if (!bridge.player.canChangeModes) return;
            check(shuffle.enabled && repeat.enabled, "mode buttons are enabled when connected with an assignment");
            check(!bridge.player.shuffle && shuffle.accessibleName === "Shuffle: off" && shuffle.iconName === "shuffle" && shuffle.foreground === Radio.Theme.text,
                "shuffle starts off");
            check(bridge.player.repeatMode === "off" && repeat.accessibleName === "Repeat: off" && repeat.iconName === "repeat-2" && repeat.foreground === Radio.Theme.text,
                "repeat starts off");
            shuffle.clicked();
            stage = 31;
            break;
        }
        case 31: {
            if (!bridge.player.shuffle) return;
            const shuffle = findChild(window.contentItem, "shuffleButton");
            check(shuffle.accessibleName === "Shuffle: on" && shuffle.foreground === Radio.Theme.accent && shuffle.enabled, "shuffle on is named and highlighted");
            findChild(window.contentItem, "repeatButton").clicked();
            stage = 32;
            break;
        }
        case 32: {
            if (bridge.player.repeatMode !== "all") return;
            const repeat = findChild(window.contentItem, "repeatButton");
            check(repeat.accessibleName === "Repeat: all" && repeat.iconName === "repeat-2" && repeat.foreground === Radio.Theme.accent, "repeat all");
            repeat.clicked();
            stage = 33;
            break;
        }
        case 33: {
            if (bridge.player.repeatMode !== "one") return;
            const repeat = findChild(window.contentItem, "repeatButton");
            check(repeat.accessibleName === "Repeat: one" && repeat.iconName === "repeat-1" && repeat.foreground === Radio.Theme.accent, "repeat one");
            repeat.clicked();
            findChild(window.contentItem, "shuffleButton").clicked();
            stage = 34;
            break;
        }
        case 34:
            if (bridge.player.repeatMode !== "off" || bridge.player.shuffle) return;
            check(findChild(window.contentItem, "repeatButton").iconName === "repeat-2" && findChild(window.contentItem, "shuffleButton").accessibleName === "Shuffle: off",
                "modes return to off");
            stage = 10;
            break;
        }
    }
    property int quietTicks: 0
    // True after n further polls, to assert that something did not happen.
    function quiet(n: int): bool { return ++quietTicks > n; }
    function hotkeysProbe(): void {
        if (!bridge.connected || bridge.library.libraryLoading || !bridge.settings.audioSettingsLoaded) return;
        const settings = bridge.settings;
        const root = window.contentItem;
        const search = findChild(root, "songSearch");
        const volumeButton = findChild(root, "volumeButton");
        const volumePopup = findChild(root, "volumePopup");
        const modsButton = findChild(root, "modsButton");
        const modsPopup = findChild(root, "modsPopup");
        const part = name => findChild(modsPopup.contentItem, name);
        const volume = () => Math.round(settings.volume * 100);
        const mods = () => settings.modsMode + ":" + settings.modsSpeedPercent + ":" + settings.modsAdjustPitch;
        const badge = () => findChild(root, "modsBadge");
        const idleIcon = "#eff1f5";
        const steps = [
            { act: () => bridge.library.selectTrack(42), until: () => bridge.player.selectedAudioId === 42 && bridge.library.tracks.trackCount === 3,
              check: () => !search.activeFocus, message: "Songs does not focus the search field" },
            { act: () => input.keyClick(Qt.Key_F, Qt.ControlModifier, 0), until: () => search.activeFocus, message: "Ctrl+F focuses the Songs search" },
            { act: () => input.keyClick(Qt.Key_Space, Qt.NoModifier, 0), until: () => quiet(4),
              check: () => search.text === " " && bridge.player.loadingAudioId === -1 && bridge.player.currentAudioId === -1,
              message: "Space types into the search field and does not play" },
            { act: () => search.clear(), until: () => bridge.library.tracks.trackCount === 3 },
            { act: () => input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0), until: () => !search.activeFocus, message: "Escape leaves the search field" },
            { act: () => input.keyClick(Qt.Key_Space, Qt.NoModifier, 0), until: () => bridge.player.currentAudioId === 42,
              message: "Space after Escape starts the selected track" },
            // Left/Right have no observable effect without a seekable engine; this only proves they are inert.
            { act: () => { input.keyClick(Qt.Key_Left, Qt.NoModifier, 0); input.keyClick(Qt.Key_Right, Qt.NoModifier, 0); }, until: () => quiet(2),
              check: () => !bridge.player.canSeek && bridge.player.playbackPosition === 0, message: "arrows without canSeek do nothing" },
            { act: () => volumeButton.clicked(), until: () => volumePopup.opened && findChild(root, "volumeSlider").activeFocus,
              check: () => findChild(volumePopup.contentItem, "volumeScope").text === "All tracks", message: "popup names the general volume" },
            { act: () => input.keyClick(Qt.Key_Left, Qt.NoModifier, 0), until: () => volume() === 99, message: "Left in the open popup moves the slider" },
            { act: () => input.mouseWheel(volumeButton, 20, 20, 0, -120), until: () => volume() === 94, message: "one wheel notch is exactly -5%" },
            { act: () => input.mouseWheel(volumePopup.contentItem, 20, 5, 0, 120), until: () => volume() === 99, message: "the wheel also works over the popup" },
            { act: () => input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0), until: () => !volumePopup.visible && volumeButton.activeFocus },
            { act: () => settings.changeVolume(0.3), until: () => volume() === 30,
              check: () => volumeButton.iconName === "volume-1", message: "low volume icon" },
            { act: () => settings.changeVolume(0), until: () => volume() === 0,
              check: () => volumeButton.iconName === "volume-x", message: "muted volume icon" },
            { act: () => settings.changeVolume(0.4), until: () => volume() === 40,
              check: () => volumeButton.iconName === "volume-2", message: "40% is the normal icon" },
            // Mute is player state: the saved volume and the slider keep their value throughout.
            { act: () => input.keyClick(Qt.Key_M, Qt.NoModifier, 0), until: () => bridge.player.muted,
              check: () => volume() === 40 && volumeButton.iconName === "volume-x" && !settings.audioSettingsCanRetry, message: "M mutes without touching the volume" },
            { act: () => volumeButton.clicked(), until: () => volumePopup.opened,
              check: () => findChild(volumePopup.contentItem, "volumePercent").text === "0%"
                && Math.abs(findChild(volumePopup.contentItem, "volumeSlider").value - 0.4) < 0.001
                && findChild(volumePopup.contentItem, "muteButton").Accessible.name === "Mute",
              message: "muted popup shows 0% and keeps the slider value" },
            { act: () => input.keyClick(Qt.Key_M, Qt.NoModifier, 0), until: () => quiet(2),
              check: () => bridge.player.muted, message: "M is ignored while the volume popup is open" },
            { act: () => findChild(volumePopup.contentItem, "muteButton").clicked(), until: () => !bridge.player.muted,
              check: () => volume() === 40 && volumeButton.iconName === "volume-2" && findChild(volumePopup.contentItem, "volumePercent").text === "40%",
              message: "the popup button restores the previous level" },
            { act: () => findChild(volumePopup.contentItem, "muteButton").clicked(), until: () => bridge.player.muted },
            { act: () => { const slider = findChild(volumePopup.contentItem, "volumeSlider"); slider.value = 0.5; slider.moved(); },
              until: () => volume() === 50 && !bridge.player.muted, message: "dragging the slider ends mute and applies the new value" },
            { act: () => input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0), until: () => !volumePopup.visible },
            { act: () => settings.changeVolume(0.4), until: () => volume() === 40 },
            { act: () => input.keyClick(Qt.Key_F, Qt.ControlModifier, 0), until: () => search.activeFocus },
            { act: () => input.keyClick(Qt.Key_M, Qt.NoModifier, 0), until: () => quiet(4),
              check: () => search.text === "m" && !bridge.player.muted, message: "M types into the search field and does not mute" },
            { act: () => search.clear(), until: () => bridge.library.tracks.trackCount === 3 },
            { act: () => input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0), until: () => !search.activeFocus },
            { act: () => settings.setIndividualVolumeEnabled(true), until: () => settings.individualVolumeEnabled && volume() === 40 },
            { act: () => input.mouseWheel(volumeButton, 20, 20, 0, -120), until: () => volume() === 35 && settings.volumeHasOverride,
              message: "the wheel edits the selected track in individual mode" },
            { act: () => volumeButton.clicked(), until: () => volumePopup.opened,
              check: () => findChild(volumePopup.contentItem, "volumeScope").text === "This track", message: "popup names the track volume" },
            { act: () => input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0), until: () => !volumePopup.visible },
            { act: () => {}, until: () => settings.modsLoaded && settings.modsAvailable,
              check: () => !badge().visible && modsButton.accessibleName === "Mods (F1)", message: "no badge without mods" },
            { act: () => input.keyClick(Qt.Key_F1, Qt.NoModifier, 0), until: () => modsPopup.opened && modsPopup.available && part("modsDtButton").activeFocus },
            { act: () => input.keyClick(Qt.Key_D, Qt.NoModifier, 0), until: () => mods() === "dt:150:false" && badge().visible,
              check: () => badge().label === "DT 1.5×" && modsButton.accessibleName === "Mods (F1): DT 1.5×" && String(modsButton.icon.color) !== idleIcon,
              message: "DT shows the accent icon and the 1.5× badge" },
            { act: () => { const speed = part("modsSpeedSlider"); speed.value = 125; speed.moved(); }, until: () => mods() === "dt:125:false",
              check: () => badge().label === "DT 1.25×" && modsButton.accessibleName === "Mods (F1): DT 1.25×", message: "badge keeps two decimals only when needed" },
            { act: () => input.mouseClick(part("modsPitchSwitch")), until: () => mods() === "dt:125:true",
              check: () => modsButton.accessibleName === "Mods (F1): NC 1.25×", message: "NC is named in the tooltip" },
            { act: () => input.mouseClick(part("modsClearButton")), until: () => mods() === "none:100:false",
              check: () => !badge().visible && modsButton.accessibleName === "Mods (F1)" && String(modsButton.icon.color) === idleIcon,
              message: "clearing mods hides the badge and the accent" },
            { act: () => input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0), until: () => !modsPopup.visible },
            { act: () => input.mouseClick(findChild(root, "playlistsTabButton")), until: () => window.selectedTab === 1 },
            { act: () => input.keyClick(Qt.Key_F, Qt.ControlModifier, 0), until: () => findChild(root, "playlistSearch").activeFocus,
              message: "Ctrl+F focuses the playlist search" },
            { act: () => input.mouseClick(findChild(root, "settingsButton")), until: () => window.selectedTab === 2 },
            { act: () => input.keyClick(Qt.Key_F, Qt.ControlModifier, 0), until: () => findChild(root, "settingsSearch").activeFocus,
              message: "Ctrl+F focuses the settings search" },
            { act: () => input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0), until: () => !findChild(root, "settingsSearch").activeFocus },
        ];
        if (stage >= steps.length) { finish(); return; }
        const current = steps[stage];
        if (!modsStepActed) { current.act(); modsStepActed = true; }
        if (current.until && !current.until()) return;
        if (current.check) check(current.check(), current.message || "hotkeys step " + stage);
        modsStepActed = false;
        quietTicks = 0;
        stage++;
    }
    function volumeProbe(): void {
        if (!bridge.connected || bridge.library.libraryLoading) return;
        const toggle = findChild(window.contentItem, "individualVolumeSwitch");
        const global = findChild(window.contentItem, "globalVolumeSlider");
        const play = findChild(window.contentItem, "playPauseButton");
        const popup = findChild(window.contentItem, "volumePopup");
        const slider = findChild(window.contentItem, "volumeSlider");
        switch (stage) {
        case 0:
            if (!bridge.settings.audioSettingsCanRetry) return;
            check(!bridge.settings.audioSettingsLoaded && !play.enabled && !toggle.enabled, "loading failure blocks playback and editing");
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
            if (!bridge.settings.audioSettingsLoaded || bridge.player.currentAudioId !== 7) return;
            check(bridge.settings.globalVolumePercent === 20 && Math.round(bridge.settings.volume * 100) === 20 && !global.visible, "loaded general volume");
            input.mouseClick(toggle);
            stage = 2; break;
        case 2:
            if (!bridge.settings.individualVolumeEnabled) return;
            check(global.visible && global.enabled, "individual mode shows general slider");
            bridge.library.selectTrack(42); stage = 3; break;
        case 3:
            if (bridge.player.selectedAudioId !== 42) return;
            findChild(window.contentItem, "volumeButton").clicked(); stage = 4; break;
        case 4:
            if (!popup.opened) return;
            check(slider.enabled && Math.round(bridge.settings.volume * 100) === 20, "B inherits general volume");
            slider.value = 0.1; slider.moved(); stage = 5; break;
        case 5:
            if (!bridge.settings.audioSettingsCanRetry || Math.round(bridge.settings.volume * 100) !== 10) return;
            check(bridge.player.currentAudioId === 7 && bridge.settings.volumeHasOverride, "editing B preserves current A and exposes retry");
            findChild(window.contentItem, "retryVolumeSave").clicked(); stage = 6; break;
        case 6:
            if (bridge.settings.audioSettingsCanRetry) return;
            global.value = 40; global.moved(); stage = 7; break;
        case 7:
            if (bridge.settings.globalVolumePercent !== 40) return;
            check(Math.round(bridge.settings.volume * 100) === 10, "absolute B volume survives general increase");
            input.mouseClick(findChild(window.contentItem, "songsTabButton"));
            input.mouseClick(findChild(window.contentItem, "settingsButton"));
            check(toggle.checked && global.value === 40 && bridge.player.selectedAudioId === 42, "Settings navigation retains volume preferences and selection");
            findChild(window.contentItem, "useGlobalVolumeButton").clicked(); stage = 8; break;
        case 8:
            if (bridge.settings.volumeHasOverride || Math.round(bridge.settings.volume * 100) !== 40) return;
            slider.value = 0.1; slider.moved(); stage = 9; break;
        case 9:
            if (!bridge.settings.volumeHasOverride || Math.round(bridge.settings.volume * 100) !== 10) return;
            if (!popup.opened) {
                findChild(window.contentItem, "volumeButton").clicked();
                return;
            }
            check(contained(popup.contentItem, window.activeFocusItem), "volume editor has focus before mode change");
            bridge.settings.setIndividualVolumeEnabled(false); stage = 10; break;
        case 10:
            if (bridge.settings.individualVolumeEnabled) return;
            if (popup.visible) return;
            check(Math.round(bridge.settings.volume * 100) === 40 && !global.visible && !popup.opened, "disabled mode uses general and ends popup editing");
            check(findChild(window.contentItem, "volumeButton").activeFocus, "mode change closes volume editing and restores button focus");
            input.mouseClick(toggle); stage = 11; break;
        case 11:
            if (!bridge.settings.individualVolumeEnabled) return;
            check(Math.round(bridge.settings.volume * 100) === 10 && bridge.settings.volumeHasOverride, "reenabling restores B override");
            bridge.library.selectTrack(7); stage = 12; break;
        case 12:
            if (bridge.player.selectedAudioId !== 7) return;
            check(Math.round(bridge.settings.volume * 100) === 40, "A retains general volume");
            findChild(window.contentItem, "volumeButton").clicked(); stage = 13; break;
        case 13:
            if (!popup.opened) return;
            bridge.library.selectTrack(-1); stage = 14; break;
        case 14:
            if (bridge.player.hasSelection) return;
            check(!popup.opened && !bridge.settings.volumeEnabled && !slider.enabled, "selection change closes editing and disables unavailable volume");
            bridge.settings.setGlobalVolume(55);
            finish(); break;
        }
    }
    function progressProbe(): void {
        if (stage === 0) {
            window.width = 1024; window.height = 640;
            controls = progressFixture.createObject(window.contentItem);
            check(controls !== null, "create offline progress fixture");
            stage = 1; return;
        }
        const c = controls;
        const p = c.progress;
        const drag = to => {
            input.mousePress(p, p.width / 2, p.height / 2, Qt.LeftButton, Qt.NoModifier, 0);
            input.mouseMove(p, p.width * to, p.height / 2, 0);
        };
        switch (stage) {
        case 1:
            check(p.enabled && p.value === 40 && p.from === 0 && p.to === 160, "presentation seconds bind unchanged");
            const fill = p.background.children[0];
            check(fill.height === p.background.height && fill.height > 0 && fill.width > 0, "played part has the track's height");
            p.playbackPosition = 0;
            check(fill.width === 0, "no played part at position zero");
            p.playbackPosition = 40;
            const tip = findChild(p, "playbackTip");
            check(!tip.visible, "no time bubble without the pointer");
            input.mouseMove(p, p.width / 2, p.height / 2, 0);
            check(tip.visible && findChild(p, "playbackTipText").text === "1:20", "pointer at 50% shows half the duration: " + findChild(p, "playbackTipText").text);
            p.enabled = false;
            check(!tip.visible, "inactive progress shows no time bubble");
            p.enabled = true;
            input.mouseMove(p, p.width * 2, p.height / 2, 0);
            drag(0.75);
            check(p.pressed, "press starts a drag");
            p.playbackPosition = 100;
            check(p.value !== 100, "position ticks do not move a dragged handle");
            input.mouseRelease(p, p.width * 0.75, p.height / 2, Qt.LeftButton, Qt.NoModifier, 0);
            check(c.seeks.length === 1 && Math.abs(c.seeks[0] - 120) < 6, "released drag seeks once in presentation seconds: " + c.seeks);
            check(p.value === 100, "released slider follows the reported position again");
            c.seeks = []; p.playbackPosition = 40;
            stage = 2; break;
        case 2:
            drag(0.75);
            p.scaleKey = "42:-1:1:120";
            check(p.stale && p.value === 40, "speed change cancels the drag and restores the position");
            input.mouseRelease(p, p.width * 0.75, p.height / 2, Qt.LeftButton, Qt.NoModifier, 0);
            check(c.seeks.length === 0 && !p.stale, "stale speed drag sends no seek");
            drag(0.75);
            p.scaleKey = "42:-1:2:120";
            input.mouseRelease(p, p.width * 0.75, p.height / 2, Qt.LeftButton, Qt.NoModifier, 0);
            check(c.seeks.length === 0, "new launch token of the same audio sends no seek");
            drag(0.75);
            p.scaleKey = "7:-1:2:120";
            p.scaleKey = "42:-1:2:120";
            input.mouseRelease(p, p.width * 0.75, p.height / 2, Qt.LeftButton, Qt.NoModifier, 0);
            check(c.seeks.length === 0, "a scale that changed and returned is still another scale");
            stage = 3; break;
        case 3:
            drag(0.25);
            input.mouseRelease(p, p.width * 0.25, p.height / 2, Qt.LeftButton, Qt.NoModifier, 0);
            check(c.seeks.length === 1 && Math.abs(c.seeks[0] - 40) < 6, "the next drag is not poisoned by a cancelled one: " + c.seeks);
            c.seeks = [];
            p.enabled = false;
            drag(0.75);
            input.mouseRelease(p, p.width * 0.75, p.height / 2, Qt.LeftButton, Qt.NoModifier, 0);
            check(c.seeks.length === 0, "unavailable seeking sends nothing");
            p.enabled = true; p.forceActiveFocus();
            input.keyClick(Qt.Key_Right, Qt.NoModifier, 0);
            check(c.seeks.length === 1 && c.seeks[0] > 40, "keyboard seek reports one presentation-second value: " + c.seeks);
            c.fade.source = "qrc:/assets/logos/stable.png";
            check(c.fade.previous.toString() === "" && c.fade.progress === 1, "first image is shown at once");
            c.fade.source = "qrc:/assets/logos/lazer.png";
            check(c.fade.previous.toString().endsWith("stable.png"), "replacement keeps the old image underneath");
            c.fade.source = "";
            check(c.fade.previous.toString() === "" && c.fade.progress === 1, "clearing drops the old image at once");
            finish(); break;
        }
    }
    function galleryModsProbe(): void {
        const store = window.galleryStore;
        const button = findChild(window.contentItem, "galleryModsButton");
        const popup = findChild(window.contentItem, "galleryModsPopup");
        const mods = () => store.state.mods.mode + ":" + store.state.mods.speed_percent + ":" + store.state.mods.adjust_pitch;
        const control = name => findChild(popup.contentItem, name);
        switch (stage) {
        case 0:
            check(mods() === "none:100:false" && store.state.mods.available && !store.state.mods.individual, "gallery starts without mods");
            button.forceActiveFocus(); button.clicked(); stage = 1; break;
        case 1:
            if (!popup.opened) return;
            check(control("modsDtButton").activeFocus && popup.available, "gallery popup opens on the shared state");
            input.keyClick(Qt.Key_D, Qt.NoModifier, 0);
            check(mods() === "dt:150:false" && popup.mode === "dt" && popup.speedPercent === 150, "D selects DT through the shared mock rules");
            input.keyClick(Qt.Key_D, Qt.NoModifier, 0);
            check(mods() === "dt:150:true" && control("modsDtButton").text === "NC (D)", "second D is NC at the same speed");
            const speed = control("modsSpeedSlider");
            speed.value = 120; speed.moved();
            check(mods() === "dt:120:true", "NC keeps its own speed");
            input.keyClick(Qt.Key_D, Qt.NoModifier, 0);
            check(mods() === "none:100:false", "third D clears mods");
            input.keyClick(Qt.Key_E, Qt.NoModifier, 0);
            check(mods() === "ht:75:false", "E selects HT");
            input.mouseClick(control("modsPitchSwitch"));
            check(mods() === "ht:75:true", "mouse pitch uses the same command");
            input.mouseClick(control("modsClearButton"));
            check(mods() === "none:100:false", "mouse Clear uses the same command");
            input.keyClick(Qt.Key_E, Qt.NoModifier, 0);
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0);
            stage = 2; break;
        case 2:
            if (popup.visible) return;
            check(mods() === "ht:75:false", "closing keeps the edit");
            input.keyClick(Qt.Key_D, Qt.NoModifier, 0);
            check(mods() === "ht:75:false", "a closed popup ignores D");
            button.clicked();
            stage = 3; break;
        case 3: {
            if (!popup.opened) return;
            store.send("modsIndividual", true);
            check(store.state.mods.individual && mods() === "none:100:false", "individual mode does not inherit the shared profile");
            stage = 4; break;
        }
        case 4: {
            if (popup.visible) return;
            const before = mods();
            store.send("modsCycleDt");
            check(mods() === before, "an edit after the popup ended is rejected");
            finish(); break;
        }
        }
    }
    function modsLiveProbe(): void {
        if (!bridge.connected || bridge.library.libraryLoading) return;
        const settings = bridge.settings;
        const play = findChild(window.contentItem, "playPauseButton");
        const popup = findChild(window.contentItem, "modsPopup");
        const part = name => findChild(popup.contentItem, name);
        const mods = () => settings.modsMode + ":" + settings.modsSpeedPercent + ":" + settings.modsAdjustPitch;
        const ready = () => settings.modsLoaded && settings.modsMessage === "" && !settings.modsSaving;
        const durationLabel = () => bridge.player.selectedDurationLabel;
        // Each step acts once, then waits for the asynchronous client state it expects.
        const steps = [
            { act: () => {}, until: () => settings.modsMessage.includes("Failed to load mods") && !settings.modsLoaded,
              check: () => !play.enabled && !settings.modsAvailable && !findChild(window.contentItem, "individualModsSwitch").enabled,
              message: "a mods load failure blocks playback and editing" },
            { act: () => input.keyClick(Qt.Key_F1, Qt.NoModifier, 0), until: () => popup.opened,
              check: () => !popup.available && popup.errorMessage.includes("Failed to load mods") && part("modsRetryButton").visible && !part("modsDtButton").enabled,
              message: "F1 opens a disabled popup with the load error" },
            { act: () => { input.keyClick(Qt.Key_D, Qt.NoModifier, 0); input.mouseClick(part("modsRetryButton")); },
              until: () => ready() && popup.available,
              check: () => mods() === "none:100:false", message: "Retry loads mods and enables the open popup without a reopen" },
            { act: () => input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0), until: () => !popup.visible },
            { act: () => bridge.library.selectTrack(42), until: () => bridge.player.selectedAudioId === 42 && durationLabel() !== "--:--",
              check: () => durationLabel() === "04:00" && bridge.player.currentAudioId === 7 && play.enabled,
              message: "A is current, B selected; global mode has no mods, so B stays 4:00" },
            { act: () => input.keyClick(Qt.Key_F1, Qt.NoModifier, 0), until: () => popup.opened && popup.available && part("modsDtButton").activeFocus },
            { act: () => input.keyClick(Qt.Key_D, Qt.NoModifier, 0), until: () => mods() === "dt:150:false" && settings.modsMessage.includes("Failed to save mods"),
              check: () => popup.mode === "dt" && popup.errorMessage.includes("Failed to save mods") && part("modsRetryButton").enabled,
              message: "F1, D selects DT 1.50x and the failed first save is visible in the menu" },
            { act: () => input.mouseClick(part("modsRetryButton")), until: () => ready() && mods() === "dt:150:false",
              check: () => !part("modsRetryButton").visible, message: "Retry saves the edit and the open editor stays addressable" },
            { act: () => input.keyClick(Qt.Key_D, Qt.NoModifier, 0), until: () => mods() === "dt:150:true", check: () => part("modsDtButton").text === "NC (D)", message: "second D is NC at 1.50x" },
            { act: () => input.keyClick(Qt.Key_D, Qt.NoModifier, 0), until: () => mods() === "none:100:false", message: "third D clears" },
            { act: () => input.keyClick(Qt.Key_E, Qt.NoModifier, 0), until: () => mods() === "ht:75:false", message: "E selects HT 0.75x" },
            { act: () => input.keyClick(Qt.Key_E, Qt.NoModifier, 0), until: () => mods() === "none:100:false", message: "second E clears HT" },
            { act: () => input.keyClick(Qt.Key_D, Qt.NoModifier, 0), until: () => mods() === "dt:150:false" },
            { act: () => { const speed = part("modsSpeedSlider"); speed.value = 120; speed.moved(); }, until: () => mods() === "dt:120:false",
              check: () => part("modsSpeedLabel").text === "Speed: 1.20×", message: "slider changes the speed" },
            { act: () => input.mouseClick(part("modsPitchSwitch")), until: () => mods() === "dt:120:true", check: () => part("modsDtButton").text === "NC (D)", message: "Adjust pitch makes NC" },
            { act: () => input.mouseClick(part("modsClearButton")), until: () => mods() === "none:100:false", message: "Clear removes mods" },
            { act: () => input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0), until: () => !popup.visible },
            // Individual mode: B has its own DT 1.50x, shown on the 4:00 timescale as 2:40.
            { act: () => input.mouseClick(findChild(window.contentItem, "settingsButton")), until: () => window.selectedTab === 2 },
            { act: () => input.mouseClick(findChild(window.contentItem, "individualModsSwitch")), until: () => settings.individualModsEnabled && ready() && durationLabel() === "02:40",
              check: () => mods() === "dt:150:false" && !settings.individualVolumeEnabled && bridge.player.playbackSpeedPercent === 150,
              message: "individual switch shows B's own profile (4:00 becomes 2:40) and leaves volume mode alone" },
            { act: () => { findChild(window.contentItem, "settingsSearch").text = "mods"; }, until: () => !findChild(window.contentItem, "generalSettingsSection").visible,
              check: () => findChild(window.contentItem, "individualModsSwitch").visible && !findChild(window.contentItem, "individualVolumeSwitch").visible, message: "search finds the switch" },
            { act: () => { findChild(window.contentItem, "settingsSearch").text = ""; input.mouseClick(findChild(window.contentItem, "songsTabButton")); }, until: () => window.selectedTab === 0 },
            { act: () => bridge.library.selectTrack(103), until: () => bridge.player.selectedAudioId === 103 && durationLabel() !== "--:--" && mods() === "none:100:false",
              check: () => durationLabel() === "04:00", message: "a track without a profile is 1x in individual mode" },
            { act: () => bridge.library.selectTrack(42), until: () => bridge.player.selectedAudioId === 42 && mods() === "dt:150:false" },
            { act: () => input.keyClick(Qt.Key_F1, Qt.NoModifier, 0), until: () => popup.opened && popup.available && part("modsDtButton").activeFocus },
            { act: () => input.keyClick(Qt.Key_D, Qt.NoModifier, 0), until: () => mods() === "dt:150:true",
              check: () => bridge.player.currentAudioId === 7 && bridge.player.loadingAudioId === -1, message: "editing B leaves current A alone" },
            // A late edit addressed to B must not reach the track selected next.
            { act: () => { bridge.library.selectTrack(103); settings.cycleDtMod(); }, until: () => bridge.player.selectedAudioId === 103 && !popup.visible,
              check: () => mods() === "none:100:false", message: "selection change closes the popup; the late edit does not land on 103" },
            { act: () => bridge.library.selectTrack(42), until: () => bridge.player.selectedAudioId === 42 && mods() === "dt:150:true" },
            { act: () => input.keyClick(Qt.Key_F1, Qt.NoModifier, 0), until: () => popup.opened && popup.available },
            { act: () => settings.setIndividualModsEnabled(false), until: () => !settings.individualModsEnabled && !popup.visible,
              check: () => mods() === "none:100:false", message: "a mode change closes the popup and applies the global profile" },
            { act: () => settings.setIndividualModsEnabled(true), until: () => settings.individualModsEnabled && mods() === "dt:150:true", message: "re-enabling restores B's profile" },
            { act: () => bridge.library.selectTrack(-1), until: () => !bridge.player.hasSelection && !settings.modsAvailable,
              check: () => !popup.visible, message: "no selection leaves no individual addressee" },
            { act: () => input.keyClick(Qt.Key_F1, Qt.NoModifier, 0), until: () => popup.opened,
              check: () => !popup.available && !part("modsDtButton").enabled, message: "F1 with no selection opens a disabled popup" },
            { act: () => input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0), until: () => !popup.visible },
            { act: () => bridge.library.selectTrack(42), until: () => bridge.player.selectedAudioId === 42 && settings.modsAvailable },
            { act: () => input.keyClick(Qt.Key_F1, Qt.NoModifier, 0), until: () => popup.opened && popup.available && part("modsDtButton").activeFocus },
            { act: () => input.keyClick(Qt.Key_E, Qt.NoModifier, 0), until: () => mods() === "ht:75:false", message: "last edit stays pending for the close flush" },
        ];
        if (stage >= steps.length) { finish(); return; }
        const step = steps[stage];
        if (!modsStepActed) { step.act(); modsStepActed = true; }
        if (step.until && !step.until()) return;
        if (step.check) check(step.check(), step.message || "mods step " + stage);
        modsStepActed = false;
        stage++;
    }
    property bool modsStepActed: false
    function queueSections(panel: var): var {
        return findChild(panel.contentItem, "queueScroll").contentItem.children
            .filter(item => item.objectName === "queueSectionLabel" && item.visible).sort((a, b) => a.y - b.y).map(item => item.children[0].text);
    }
    // Rows are history, current, upcoming; section labels shift row positions, so drops follow real geometry.
    function queueSectionDropChecks(c: var): void {
        const list = findChild(c.queue.contentItem, "queueScroll");
        const cards = c.queue.mediaDelegates.slice().sort((a, b) => a.index - b.index);
        const at = (card, offset) => Qt.point(10, card.y - list.contentY + offset);
        check(cards[0].background.opacity === 0.55 && cards[1].background.opacity === 1, "history is dimmed to 55 percent");
        const guard = findChild(cards[0], "queueHiddenActionGuard");
        check(!cards[0].actionsShown && guard.enabled, "idle row hides its actions and swallows pointer input over them");
        input.mouseMove(cards[0], 100, 40, 0);
        check(cards[0].actionsShown && !guard.enabled, "hover shows the actions and hands pointer input to them");
        input.mouseMove(window.contentItem, 2, 2, 0);
        c.queue.dragId = cards[0].entryId; c.queue.dragging = true;
        c.queue.updateDrop(at(cards[1], 37));
        check(c.queue.insertionIndex === 1, "above the current midpoint drops before current");
        c.queue.updateDrop(at(cards[1], 45));
        check(c.queue.insertionIndex === 2, "below the current midpoint drops after current");
        c.queue.updateDrop(at(cards[2], -20));
        check(c.queue.insertionIndex === 2 && c.queue.insertionY === cards[2].y - 7, "section label gap drops before the next row");
        c.queue.updateDrop(at(cards[4], 70));
        check(c.queue.insertionIndex === 5 && c.queue.insertionY === cards[4].y + cards[4].height + 7, "below the last row appends");
        c.queue.updateDrop(at(cards[1], 45));
        const before = c.actions.length;
        c.queue.dropDrag();
        check(c.actions.length === before + 1 && c.actions[before] === "queue-move:" + cards[0].entryId + ">" + c.queueMock.queueModel.entryIdAt(2), "drop between sections sends one move with the exact anchor");
        cards[2].forceActiveFocus();
        input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
        input.keyClick(Qt.Key_Delete, Qt.NoModifier, 0);
        check(c.actions.includes("queue-play:" + cards[2].entryId) && c.actions.includes("queue-remove:" + cards[2].entryId), "Enter plays and Delete removes the focused row");
    }
    function queueCards(panel: var): var {
        return panel.mediaDelegates.filter(row => row.inViewport);
    }
    function queueReady(panel: var): bool {
        const rows = queueCards(panel).filter(row => row.audioId >= 0);
        return rows.length > 0 && rows.every(row => row.artworkUrl.length > 0 && row.durationLabel !== "--:--");
    }
    function queueProbe(): void {
        if (!bridge.connected || bridge.library.libraryLoading) return;
        const next = findChild(window.contentItem, "nextTrackButton");
        const previous = findChild(window.contentItem, "previousTrackButton");
        const queueButton = findChild(window.contentItem, "queueButton");
        const panel = findChild(window.contentItem, "queuePanel");
        const list = findChild(panel.contentItem, "queueScroll");
        switch (stage) {
        case 0:
            if (bridge.player.currentAudioId !== 7 || !next.enabled || !previous.enabled) return;
            check(bridge.player.selectedAudioId !== 7 && !bridge.player.selectedIsPlaying && bridge.player.loadingAudioId === -1, "paused recovery preserves selection and starts no download");
            queueButton.forceActiveFocus(); queueButton.clicked(); stage = 10; break;
        case 10:
            if (!panel.opened || bridge.queue.queueLoading || !bridge.queue.queueMessage.includes("fixture queue unavailable")) return;
            check(panel.width === 430 && panel.x + panel.width === findChild(window.contentItem, "playerPane").width, "queue aligns with player");
            findChild(panel.contentItem, "queueRetryButton").clicked(); stage = 11; break;
        case 11:
            if (bridge.queue.queueLoading || list.count !== 3 || !queueReady(panel)) return;
            check(bridge.queue.tracks.entryIdAt(0) === "1" && list.itemAtIndex(0).model.isCurrent, "full queue includes current");
            check(queueSections(panel).join() === "NOW PLAYING,UP NEXT" && findChild(panel.contentItem, "queueLeft").text === "· 2 left", "queue without history has no History section");
            for (let i = 0; i < 6; ++i) {
                input.keyClick(Qt.Key_Tab, Qt.NoModifier, 0);
                check(contained(panel.contentItem, window.activeFocusItem), "queue contains Tab focus");
                input.keyClick(Qt.Key_Backtab, Qt.ShiftModifier, 0);
                check(contained(panel.contentItem, window.activeFocusItem), "queue contains Shift Tab focus");
            }
            bridge.player.nextTrack(); stage = 1; break;
        case 1:
            if (bridge.player.currentAudioId !== 42 || bridge.queue.queueLoading) return;
            check(list.count === 3 && list.itemAtIndex(0).model.isPast, "Next preserves history");
            check(queueSections(panel).join() === "HISTORY,NOW PLAYING,UP NEXT" && findChild(panel.contentItem, "queueLeft").text === "· 1 left", "live queue shows three sections and the upcoming count");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0); stage = 12; break;
        case 12:
            if (panel.visible) return;
            check(queueButton.activeFocus, "Escape restores opener focus");
            previous.clicked(); stage = 2; break;
        case 2:
            if (bridge.player.currentAudioId !== 7) return;
            check(!bridge.player.selectedIsPlaying, "Previous preserves pause");
            next.clicked(); stage = 3; break;
        case 3:
            if (bridge.player.currentAudioId !== 42) return;
            bridge.library.searchLibrary("missing"); stage = 4; break;
        case 4:
            if (bridge.library.tracks.trackCount !== 0 || bridge.library.libraryLoading || bridge.player.selectedArtworkUrl.length === 0) return;
            check(bridge.player.selectedTitle === "Track 42", "current metadata survives empty search");
            queueButton.forceActiveFocus(); queueButton.clicked(); stage = 13; break;
        case 13:
            if (!panel.opened || bridge.queue.queueLoading || !queueReady(panel)) return;
            check(list.count === 3 && list.currentIndex === 1, "reopening positions once on current outside library search");
            bridge.player.nextTrack(); stage = 14; break;
        case 14:
            if (bridge.player.currentAudioId !== 103 || bridge.queue.queueLoading) return;
            check(list.count === 3, "last track retains history");
            input.mouseClick(window.contentItem, 490, 150, Qt.LeftButton, Qt.NoModifier, 0); stage = 15; break;
        case 15:
            if (panel.visible) return;
            check(bridge.player.selectedAudioId === 103, "backdrop closes without selecting");
            queueButton.clicked(); stage = 16; break;
        case 16:
            if (!panel.opened) return;
            findChild(panel.contentItem, "queueCloseButton").clicked(); stage = 17; break;
        case 17: if (!panel.visible) finish(); break;
        }
    }
    property var queueDemand: []
    property int queueSnapshots: 0
    property int queueResets: 0
    property int queueInitialCreated: 0
    property real queueSavedY: 0
    property var queueRetained: null
    property string queueRetainedArtwork: ""
    property var queueDemandTarget: null
    readonly property Connections queueDemandConnection: Connections {
        target: probe.queueDemandTarget
        function onVisibleEntriesChanged(ids: var): void { probe.queueDemand = ids; probe.queueSnapshots++; }
    }
    readonly property Connections queueModelConnection: Connections {
        target: probe.bridge ? probe.bridge.queue.tracks : null
        function onModelReset(): void { probe.queueResets++; }
    }
    function galleryQueueProbe(): void {
        const panel = findChild(window.contentItem, "galleryQueuePanel");
        const list = findChild(panel.contentItem, "queueScroll");
        const store = window.galleryStore;
        switch (stage) {
        case 0:
            // Offline modes: the mock store applies synchronously and the shared buttons follow it.
            const shuffle = findChild(window.contentItem, "galleryShuffleButton");
            const repeat = findChild(window.contentItem, "galleryRepeatButton");
            check(shuffle.enabled && repeat.enabled && shuffle.accessibleName === "Shuffle: off" && repeat.accessibleName === "Repeat: off" && repeat.iconName === "repeat-2",
                "offline modes start off");
            shuffle.clicked();
            check(shuffle.accessibleName === "Shuffle: on" && shuffle.foreground === Radio.Theme.accent, "offline shuffle toggles");
            for (const [mode, icon] of [["all", "repeat-2"], ["one", "repeat-1"], ["off", "repeat-2"]]) {
                repeat.clicked();
                check(repeat.accessibleName === "Repeat: " + mode && repeat.iconName === icon, "offline repeat cycles to " + mode);
            }
            shuffle.clicked();
            check(shuffle.accessibleName === "Shuffle: off", "offline shuffle toggles back");
            findChild(window.contentItem, "galleryQueueButton").forceActiveFocus();
            findChild(window.contentItem, "galleryQueueButton").clicked(); stage = 1; break;
        case 1:
            if (!panel.opened || list.count !== 5) return;
            check(list.itemAtIndex(0).model.isPast && list.itemAtIndex(1).model.isCurrent, "offline model shows history/current");
            check(list.itemAtIndex(0).audioId === list.itemAtIndex(2).audioId && list.itemAtIndex(0).entryId !== list.itemAtIndex(2).entryId, "offline duplicates have independent IDs");
            list.itemAtIndex(0).forceActiveFocus(); input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
            check(store.state.queue.current_entry_id === "1" && list.count === 5, "offline Play past reuses entry");
            list.itemAtIndex(3).forceActiveFocus(); input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            check(store.state.queue.waiting_for_check, "offline checking waits");
            store.send("queuePlay", "5");
            check(panel.message.length > 0 && list.count === 5, "offline errors retain confirmed list");
            findChild(panel.contentItem, "queueRetryButton").clicked();
            check(panel.message.length === 0 && store.state.queue.current_entry_id === "4", "offline refresh clears error without Play");
            findChild(list.itemAtIndex(4), "queueRemoveEntry").clicked();
            check(list.count === 4 && store.state.queue.current_entry_id === "4", "offline unavailable removal keeps current");
            list.itemAtIndex(2).forceActiveFocus(); input.keyClick(Qt.Key_Up, Qt.ControlModifier, 0);
            check(store.queueModel.entryIdAt(1) === "3", "offline keyboard move by ID");
            confirmTarget = findChild(window.contentItem, "confirmDialog");
            galleryCount = list.count;
            findChild(panel.contentItem, "queueClearUpcoming").clicked();
            check(findChild(window.contentItem, "toast").text === "Upcoming cleared" && findChild(window.contentItem, "toast").visible, "Clear upcoming acts at once with a toast and no dialog");
            check(!confirmTarget.visible && confirmations === 0, "Clear upcoming asks nothing");
            galleryCount = list.count;
            findChild(panel.contentItem, "queueClearAll").clicked(); stage = 20; break;
        case 20:
            if (!confirmTarget.opened) return;
            check(window.activeFocusItem === findChild(confirmTarget, "confirmCancel"), "Cancel holds the initial focus");
            check(list.count === galleryCount && findChild(confirmTarget, "confirmMessage").text.includes("stops playback"), "Clear all waits for confirmation and states what it does");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0); stage = 21; break;
        case 21:
            if (confirmTarget.visible) return;
            check(confirmations === 0 && list.count === galleryCount && panel.visible, "Escape dismisses the dialog without clearing");
            findChild(panel.contentItem, "queueClearAll").clicked(); stage = 22; break;
        case 22:
            if (!confirmTarget.opened) return;
            input.mouseClick(findChild(confirmTarget, "confirmCancel")); stage = 23; break;
        case 23:
            if (confirmTarget.visible) return;
            check(confirmations === 0 && list.count === galleryCount, "Cancel does not clear");
            findChild(panel.contentItem, "queueClearAll").clicked(); stage = 24; break;
        case 24:
            if (!confirmTarget.opened) return;
            input.mouseClick(window.contentItem, 4, 4, Qt.LeftButton, Qt.NoModifier, 0); stage = 25; break;
        case 25:
            if (confirmTarget.visible) return;
            check(confirmations === 0 && list.count === galleryCount, "A click outside dismisses without clearing");
            findChild(panel.contentItem, "queueClearAll").clicked(); stage = 26; break;
        case 26:
            if (!confirmTarget.opened) return;
            toastShownAt = Date.now();
            input.mouseClick(findChild(confirmTarget, "confirmAccept")); stage = 27; break;
        case 27:
            if (confirmTarget.visible) return;
            check(confirmations === 1 && list.count === 0, "Confirm clears exactly once");
            check(findChild(window.contentItem, "toast").text === "Queue cleared", "Clear all shows its toast");
            panel.close(); stage = 2; break;
        case 2:
            if (panel.visible) return;
            check(findChild(window.contentItem, "galleryQueueButton").activeFocus, "offline focus returns");
            stage = 30; break;
        case 30: {
            const toast = findChild(window.contentItem, "toast");
            if (toast.visible) return;
            const elapsed = Date.now() - toastShownAt;
            check(elapsed >= 1000 && elapsed < 2500, "toast stays about one second: " + elapsed + " ms");
            toast.show("one"); toastShownAt = Date.now(); stage = 31; break;
        }
        case 31:
            if (Date.now() - toastShownAt < 600) return;
            findChild(window.contentItem, "toast").show("two"); toastRestartedAt = Date.now(); stage = 32; break;
        case 32: {
            const toast = findChild(window.contentItem, "toast");
            if (toast.visible) return;
            check(toast.text === "two" && Date.now() - toastShownAt >= 1550 && Date.now() - toastRestartedAt >= 1000, "a new call restarts the timer");
            stage = 33; break;
        }
        case 33:
            watcherProbe(); break;
        }
    }
    property var confirmTarget: null
    property int confirmations: 0
    property int galleryCount: 0
    property double toastShownAt: 0
    property double toastRestartedAt: 0
    readonly property Connections confirmConnection: Connections {
        target: probe.confirmTarget
        function onConfirmed(): void { probe.confirmations++; }
    }
    // Busy and message change in separate notifications; success needs a rise, a fall and no error.
    readonly property Radio.OperationWatcher watcher: Radio.OperationWatcher {}
    property var watched: []
    readonly property Connections watcherConnection: Connections {
        target: probe.watcher
        function onSucceeded(text: string): void { probe.watched.push(text); }
    }
    property int watcherStep: 0
    function watcherProbe(): void {
        switch (watcherStep) {
        case 0:
            watcher.expect("ok"); watcher.busy = true; watcher.busy = false; watcherStep = 1; break;
        case 1:
            if (watched.length === 0) return;
            check(watched.join() === "ok", "a quiet busy cycle reports success once");
            watcher.expect("failed"); watcher.busy = true; watcher.busy = false; watcher.message = "boom"; watcherStep = 2; break;
        case 2:
            // The failing message lands in the same turn as the fall, so the verdict must see it.
            if (watcher.pending.length) return;
            check(watched.join() === "ok", "an error message suppresses the success toast");
            watcher.message = ""; watcher.expect("never"); watcherStep = 3; toastShownAt = Date.now(); break;
        case 3:
            if (Date.now() - toastShownAt < 1700) return;
            watcher.busy = true; watcher.busy = false; watcherStep = 4; break;
        case 4:
            if (Date.now() - toastShownAt < 1900) return;
            check(watched.join() === "ok", "a rejected action that never became busy does not claim a later operation");
            finish(); break;
        }
    }
    property int galleryInsertion: 0
    function galleryInsertionProbe(): void {
        const store = window.galleryStore;
        const pane = findChild(window.contentItem, "demoPlaylistPane");
        switch (stage) {
        case 0:
            if (galleryInsertion === 4) {
                window.selectedTab = 1;
                store.send("playlistSelect", 1);
            }
            stage = 1; break;
        case 1:
            insertionCard = galleryInsertion < 4 ? findChild(window.contentItem, "galleryQueueSource" + Math.floor(galleryInsertion / 2)) : pane.tracksList.itemAtIndex(2);
            if (!insertionCard) return;
            if (galleryInsertion < 4) {
                const scroll = findChild(window.contentItem, "galleryScroll");
                scroll.contentItem.contentY += insertionCard.mapToItem(scroll, 0, 0).y - 150;
            }
            insertionCard.forceActiveFocus();
            input.keyClick(galleryInsertion % 2 ? Qt.Key_F10 : Qt.Key_Menu, galleryInsertion % 2 ? Qt.ShiftModifier : Qt.NoModifier, 0);
            stage = 2; break;
        case 2: {
            const menu = findChild(insertionCard, galleryInsertion < 4 ? "songQueueMenu" : "playlistItemMenu");
            if (!menu.opened) return;
            const before = store.state.queue;
            const selected = store.state.selected;
            const playlistSelected = store.state.playlists.selected_item_id;
            const command = findChild(insertionCard, galleryInsertion % 2 ? "insertQueueTrackNext" : "appendQueueTrack");
            command.forceActiveFocus();
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            check(store.state.queue.entries.length === before.entries.length + 1, "gallery source uses structured insertion");
            check(store.state.queue.mode === before.mode && store.state.queue.current_entry_id === before.current_entry_id && store.state.queue.playback_token === before.playback_token, "gallery insertion preserves transport");
            check(store.state.selected === selected && store.state.playlists.selected_item_id === playlistSelected, "gallery insertion preserves selection");
            if (galleryInsertion >= 4) {
                const row = store.state.queue.entries.find(row => Number(row.id) > 9);
                check(row && row.playlist_item_id === 3 && row.availability === "unavailable", "gallery unavailable item identity survives");
            }
            stage = 3; break;
        }
        case 3:
            if (!insertionCard.activeFocus) return;
            galleryInsertion++;
            if (galleryInsertion < 6) { stage = 0; break; }
            const menu = findChild(insertionCard, "playlistItemMenu");
            insertionCard.canInsert = false;
            menu.openAt(Qt.point(0, insertionCard.height));
            stage = 4; break;
        case 4: {
            const menu = findChild(insertionCard, "playlistItemMenu");
            if (!menu.opened) return;
            check(!findChild(insertionCard, "appendQueueTrack").enabled && !findChild(insertionCard, "insertQueueTrackNext").enabled, "disconnected/busy gate disables both commands");
            insertionCard.canInsert = true;
            insertionCard.busy = true;
            check(!findChild(insertionCard, "appendQueueTrack").enabled && !findChild(insertionCard, "insertQueueTrackNext").enabled, "playlist busy gates both row insertions");
            insertionCard.busy = false;
            check(findChild(insertionCard, "appendQueueTrack").enabled && findChild(insertionCard, "insertQueueTrackNext").enabled, "idle row insertions recover");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0);
            stage = 5; break;
        }
        case 5:
            if (!insertionCard.activeFocus) return;
            finish(); break;
        }
    }
    property int insertionStep: 0
    property var insertionCard: null
    property bool insertionBusySeen: false
    function insertionProbe(): void {
        if (!bridge.connected || bridge.library.libraryLoading || bridge.playlists.playlistLoading) return;
        const songs = findChild(window.contentItem, "songList");
        const pane = findChild(window.contentItem, "playlistsPane");
        switch (stage) {
        case 0:
            if (songs.count !== 3) return;
            bridge.library.selectTrack(42);
            bridge.queue.setQueueVisible(true);
            insertionCard = songs.itemAtIndex(0);
            insertionCard.forceActiveFocus();
            input.keyClick(Qt.Key_Menu, Qt.NoModifier, 0);
            stage = 1; break;
        case 1: {
            const menu = findChild(insertionCard, "songQueueMenu");
            if (!menu.opened) return;
            check(contained(menu.contentItem, window.activeFocusItem), "Menu key focuses insertion menu");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0);
            stage = 2; break;
        }
        case 2:
            if (findChild(insertionCard, "songQueueMenu").visible) return;
            check(insertionCard.activeFocus && bridge.library.selectedAudioId === 42, "Escape returns focus without selection");
            stage = 10; break;
        case 10:
            if (bridge.queue.queueBusy || bridge.queue.queueLoading || bridge.playlists.playlistBusy) return;
            if (insertionStep === 2 && songs.count !== 2) {
                findChild(window.contentItem, "songSearch").text = "Track 103";
                return;
            }
            if (insertionStep === 4 && window.selectedTab !== 1) {
                findChild(window.contentItem, "playlistsTabButton").clicked();
                bridge.playlists.selectPlaylist(1);
                return;
            }
            if (insertionStep >= 4 && (pane.tracksList.count !== 3 || !pane.tracksList.itemAtIndex(2))) return;
            insertionCard = null;
            if (insertionStep < 4) {
                for (let i = 0; i < songs.count; ++i) {
                    const card = songs.itemAtIndex(i);
                    if (card && card.audioId === (insertionStep < 2 ? 7 : 103)) insertionCard = card;
                }
            } else insertionCard = pane.tracksList.itemAtIndex(insertionStep - 4);
            if (!insertionCard) return;
            if (insertionStep % 2 === 0) input.mouseClick(insertionCard, 30, 30, Qt.RightButton, Qt.NoModifier, 0);
            else {
                insertionCard.forceActiveFocus();
                input.keyClick(Qt.Key_F10, Qt.ShiftModifier, 0);
            }
            stage = 11; break;
        case 11: {
            const menu = findChild(insertionCard, insertionStep < 4 ? "songQueueMenu" : "playlistItemMenu");
            if (!menu.opened) return;
            const name = insertionStep % 2 === 1 ? "insertQueueTrackNext" : "appendQueueTrack";
            const command = findChild(insertionCard, name);
            const names = [0, 1, 2, 3, 4].map(i => menu.itemAt(i).objectName || "|");
            check(names.join() === "playQueueTrack,appendQueueTrack,insertQueueTrackNext,|,addTrackToPlaylist", "track menu item order");
            check(menu.itemAt(0).text === "Play" && menu.itemAt(4).text === "Add to playlist…", "track menu captions");
            const remove = findChild(insertionCard, "removePlaylistTrack");
            check(remove.visible === (insertionStep >= 4) && remove.destructive && (insertionStep < 4 || menu.itemAt(6) === remove), "Remove from playlist only in playlist menu, last and destructive");
            check(command.enabled && command.Accessible.name.length > 0, "insertion command has accessible name");
            check(bridge.library.selectedAudioId === 42 && bridge.playlists.selectedPlaylistItemId === (insertionStep < 4 ? -1 : 11), "context menu never selects its row");
            command.forceActiveFocus();
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            stage = 12; break;
        }
        case 12:
            if (bridge.queue.queueBusy) {
                insertionBusySeen = true;
                check(!findChild(insertionCard, "appendQueueTrack").enabled && !findChild(insertionCard, "insertQueueTrackNext").enabled, "both insertions disabled while queue busy");
                return;
            }
            if (bridge.queue.queueLoading || bridge.queue.tracks.trackCount !== 3 + insertionStep) return;
            const insertionToast = findChild(window.contentItem, "toast");
            // Songs and playlist items alternate End and Next, so a stale toast never matches.
            if (!insertionToast.visible || insertionToast.text !== (insertionStep % 2 === 1 ? "Will play next" : "Added to end of queue")) return;
            check(insertionCard.activeFocus, "command returns focus to source row");
            check(bridge.library.selectedAudioId === 42 && bridge.playlists.selectedPlaylistItemId === (insertionStep < 4 ? -1 : 11), "insertions preserve independent selections");
            check(bridge.player.currentAudioId === 7 && !bridge.player.selectedIsPlaying, "insertions preserve paused current");
            insertionStep++;
            if (insertionStep < 7) { stage = 10; break; }
            check(insertionBusySeen, "Qt projects queue busy");
            bridge.playlists.selectPlaylist(-1);
            stage = 20; break;
        case 20: {
            const card = findChild(pane, "playlistList").itemAtIndex(0);
            if (!card || bridge.playlists.activePlaylistId >= 0) return;
            input.mouseClick(card, 30, 30, Qt.RightButton, Qt.NoModifier, 0);
            stage = 21; break;
        }
        case 21: {
            const menu = findChild(findChild(pane, "playlistList").itemAtIndex(0), "playlistActionsMenu");
            if (!menu.opened) return;
            check(menu.previousFocus.objectName === "playlistActionsButton" && menu.count === 3, "right click opens the three-dots actions menu and returns focus to it");
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            stage = 22; break;
        }
        case 22:
            if (bridge.queue.queueBusy || bridge.queue.queueLoading || bridge.queue.tracks.trackCount !== 12) return;
            // Wait out the previous toast so a visible one below can only come from the rejected insert.
            if (findChild(window.contentItem, "toast").visible) return;
            window.expectQueueToast("Added to end of queue");
            bridge.library.insertTrack(999, false);
            stage = 23; break;
        case 23:
            if (bridge.queue.queueBusy || !bridge.queue.queueMessage.includes("fixture insert rejected")) return;
            check(bridge.queue.tracks.trackCount === 12, "failed insertion retains confirmed rows");
            check(bridge.library.selectedAudioId === 42 && bridge.player.currentAudioId === 7, "whole playlist and failure preserve selection/current");
            toastShownAt = Date.now(); stage = 24; break;
        case 24:
            if (Date.now() - toastShownAt < 400) return;
            check(!findChild(window.contentItem, "toast").visible, "a rejected insertion shows no success toast");
            finish(); break;
        }
    }
    function managedQueueProbe(): void {
        if (!bridge.connected || bridge.library.libraryLoading) return;
        const panel = findChild(window.contentItem, "queuePanel");
        const list = findChild(panel.contentItem, "queueScroll");
        const history = testCase === "queue-managed-small" ? 20 : 5000;
        const count = history + 5;
        const maximum = "9223372036854775807";
        const current = "9007199254740993";
        const duplicate = "9007199254740995";
        const checking = "9007199254740997";
        const unavailable = "9007199254740999";
        switch (stage) {
        case 0: {
            // Start after library delivery so startup decoding cannot overlap queue demand.
            const songs = findChild(window.contentItem, "songList");
            if (songs.count !== 3) return;
            for (let i = 0; i < songs.count; ++i) {
                const row = songs.itemAtIndex(i);
                if (!row || !row.artworkUrl.length) return;
            }
            queueDemandTarget = panel;
            findChild(window.contentItem, "queueButton").forceActiveFocus();
            findChild(window.contentItem, "queueButton").clicked(); stage = 1; break;
        }
        case 1:
            if (!panel.opened || bridge.queue.queueLoading || panel.positionPending || list.count !== count || !queueReady(panel)) return;
            check(list.currentIndex === history && bridge.queue.tracks.entryIdAt(history) === current, "opening positions on current exact ID: " + list.currentIndex + "/" + bridge.queue.startIndex + "/" + bridge.queue.tracks.entryIdAt(history));
            check(bridge.player.currentAudioId === 7 && !bridge.player.selectedIsPlaying, "opening does not Play");
            check(list.itemAtIndex(history + 3).model.availability === "checking", "checking row remains visible");
            check(list.itemAtIndex(history + 4).model.availability === "unavailable", "unavailable snapshot remains visible");
            check(!findChild(list.itemAtIndex(history + 4), "queuePlayEntry").enabled, "unavailable Play is disabled");
            input.mouseClick(list.itemAtIndex(history + 4));
            check(queueDemand.length > 0 && queueDemand.length <= Math.ceil(list.height / 96) + 1, "only viewport entries request media");
            queueInitialCreated = panel.createdDelegates;
            check(panel.liveDelegates < 24 && panel.createdDelegates < 32, "delegate creation is bounded on opening");
            queueRetained = list.itemAtIndex(history + 1);
            queueRetainedArtwork = queueRetained.artworkUrl;
            queueSavedY = list.contentY;
            bridge.queue.moveEntry(checking, maximum); stage = 2; break;
        case 2:
            if (!bridge.queue.queueMessage.includes("fixture move rejected")) return;
            check(bridge.queue.tracks.entryIdAt(history + 1) === maximum && list.count === count, "rejected move preserves confirmed order");
            check(list.contentY === queueSavedY && list.itemAtIndex(history + 1) === queueRetained && queueRetained.artworkUrl === queueRetainedArtwork, "error and media updates preserve scrolling, delegate and image");
            bridge.queue.moveEntry(maximum, duplicate); stage = 21; break;
        case 21:
            if (bridge.queue.queueLoading || bridge.queue.queueMessage.length) return;
            // Hide the error before measuring role updates at a fixed viewport height.
            list.forceLayout();
            list.positionViewAtIndex(0, ListView.Beginning); stage = 3; break;
        case 3:
            if (!queueReady(panel) || !queueDemand.includes("100000")) return;
            check(panel.liveDelegates < 24, "history scroll remains virtualized");
            list.positionViewAtIndex(history, ListView.Beginning); stage = 4; break;
        case 4:
            if (!queueDemand.includes(maximum) || !queueReady(panel)) return;
            console.info("Queue metrics", JSON.stringify({history: history, initialCreated: queueInitialCreated, created: panel.createdDelegates, live: panel.liveDelegates, viewport: list.height, demand: queueDemand.length}));
            queueSavedY = list.contentY;
            queueRetained = list.itemAtIndex(history + 1);
            queueRetainedArtwork = queueRetained.artworkUrl;
            const handle = findChild(queueRetained, "queueDragHandle");
            input.mouseMove(handle, 16, 20, 0);
            check(queueRetained.actionsShown, "hover reveals row actions");
            input.mousePress(handle, 16, 20, Qt.LeftButton, Qt.NoModifier, 0);
            input.mouseMove(handle, 16, 8, 0);
            check(panel.dragging && findChild(panel.contentItem, "queueInsertionIndicator").visible, "handle drag shows insertion indicator");
            input.mouseRelease(handle, 16, 8, Qt.LeftButton, Qt.NoModifier, 0);
            stage = 41; break;
        case 41:
            if (bridge.queue.queueLoading || bridge.queue.queueMessage.length) return;
            if (!list.itemAtIndex(history - 1) || list.itemAtIndex(history - 1).model.availability !== "checking") return;
            check(list.contentY === queueSavedY && list.itemAtIndex(history + 1) === queueRetained && queueRetained.artworkUrl === queueRetainedArtwork, "availability update after scrolling preserves position, delegate and artwork: " + list.contentY + "/" + queueSavedY + "/" + (list.itemAtIndex(history + 1) === queueRetained) + "/" + (queueRetained.artworkUrl === queueRetainedArtwork));
            check(!panel.dragging && bridge.player.currentAudioId === 7, "drop submits one move without Play");
            // Cancel another drag before release; it must not submit an operation.
            const cancelHandle = findChild(list.itemAtIndex(history + 1), "queueDragHandle");
            input.mouseMove(cancelHandle, 16, 20, 0);
            input.mousePress(cancelHandle, 16, 20, Qt.LeftButton, Qt.NoModifier, 0);
            const edge = list.mapToItem(cancelHandle, list.width - 16, 1);
            queueSavedY = list.contentY;
            input.mouseMove(cancelHandle, edge.x, edge.y, 0); stage = 42; break;
        case 42:
            if (list.contentY >= queueSavedY) return;
            check(panel.dragging, "drag auto-scrolls at viewport edge");
            const releaseHandle = findChild(list.itemAtIndex(history + 1), "queueDragHandle");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0);
            input.mouseRelease(releaseHandle, 16, 8, Qt.LeftButton, Qt.NoModifier, 0);
            check(!panel.dragging && panel.visible, "Escape cancels drag and preserves panel");
            list.positionViewAtIndex(history, ListView.Beginning);
            panel.focusRow(history + 1);
            input.keyClick(Qt.Key_Down, Qt.NoModifier, 0);
            check(list.itemAtIndex(history + 2).activeFocus, "Down moves focus without Play");
            input.keyClick(Qt.Key_Up, Qt.NoModifier, 0);
            check(list.itemAtIndex(history + 1).activeFocus, "Up returns focus without Play");
            input.keyClick(Qt.Key_Down, Qt.ControlModifier, 0); stage = 5; break;
        case 5:
            if (bridge.queue.queueLoading || bridge.queue.tracks.entryIdAt(history + 2) !== maximum) return;
            check(bridge.queue.tracks.entryIdAt(history + 1) === duplicate, "keyboard move distinguishes duplicate audio/item");
            list.forceLayout();
            const duplicateRow = list.itemAtIndex(history + 1);
            if (!duplicateRow || duplicateRow.entryId !== duplicate) return;
            duplicateRow.forceActiveFocus();
            input.keyClick(Qt.Key_Delete, Qt.NoModifier, 0); stage = 6; break;
        case 6:
            if (bridge.queue.queueLoading || list.count !== count - 1) return;
            check(bridge.queue.tracks.entryIdAt(history + 1) === maximum, "Delete removes only one duplicate");
            // A no-op with the maximum ID proves QML null reaches HTTP unchanged.
            bridge.queue.moveEntry(maximum, null); stage = 7; break;
        case 7:
            if (bridge.queue.queueLoading || bridge.queue.tracks.entryIdAt(count - 2) !== maximum) return;
            bridge.queue.moveEntry(maximum, checking); stage = 8; break;
        case 8:
            if (bridge.queue.queueLoading || bridge.queue.tracks.entryIdAt(history + 1) !== maximum) return;
            list.forceLayout();
            const checkingRow = list.itemAtIndex(history + 2);
            if (!checkingRow || checkingRow.entryId !== checking) return;
            input.mouseMove(checkingRow, 40, 40, 0);
            input.mouseClick(findChild(checkingRow, "queuePlayEntry")); stage = 9; break;
        case 9:
            if (bridge.player.currentAudioId !== 103 || bridge.queue.queueLoading) return;
            check(list.count === count - 1, "Play checking does not insert a copy");
            bridge.queue.playEntry("100000"); stage = 10; break;
        case 10:
            if (bridge.player.currentAudioId !== 10000 || bridge.queue.queueLoading) return;
            check(list.count === count - 1 && bridge.queue.startIndex === 0, "Play past preserves entry and list length");
            findChild(list.itemAtIndex(history + 3), "queueRemoveEntry").clicked(); stage = 101; break;
        case 101:
            if (bridge.queue.queueLoading || list.count !== count - 2) return;
            check(bridge.player.currentAudioId === 10000, "unavailable removal does not Play");
            findChild(panel.contentItem, "queueClearUpcoming").clicked(); stage = 11; break;
        case 11:
            if (bridge.queue.queueLoading || list.count !== 1 || !findChild(window.contentItem, "toast").visible) return;
            check(bridge.queue.tracks.entryIdAt(0) === "100000", "clear upcoming keeps current past entry");
            check(findChild(window.contentItem, "toast").text === "Upcoming cleared", "live Clear upcoming toasts after the server confirms");
            findChild(panel.contentItem, "queueClearAll").clicked(); stage = 110; break;
        case 110:
            if (!findChild(window.contentItem, "confirmDialog").opened) return;
            check(list.count === 1, "Clear all waits for the dialog");
            input.mouseClick(findChild(window.contentItem, "confirmAccept")); stage = 12; break;
        case 12:
            if (bridge.queue.queueLoading || list.count !== 0 || findChild(window.contentItem, "toast").text !== "Queue cleared") return;
            check(queueResets === 0, "queue never resets model");
            panel.close(); stage = 13; break;
        case 13:
            if (panel.visible || queueDemand.length !== 0) return;
            check(findChild(window.contentItem, "queueButton").activeFocus, "managed queue restores opener focus");
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
        check(bridge.player.selectedAudioId === 42 && bridge.player.selectedTitle === "Track 42", "sorting retains selection");
        check(bridge.player.selectedDurationLabel === "02:05" && bridge.player.selectedArtworkUrl === sortMedia[42].artwork,
            "sorting retains player media");
        check(resets === retainedResets, "sorting uses row notifications without model reset");
        return true;
    }
    function sortProbe(): void {
        if (!bridge.connected || bridge.library.libraryLoading || (window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 3) return;
        const menu = findChild(window.contentItem, "trackSortMenu");
        const list = findChild(window.contentItem, "songList");
        switch (stage) {
        case 0:
            check(menu.currentIndex === 0 && menu.entries.length === 3, "default title sort and three modes");
            bridge.library.selectTrack(42);
            stage = 1;
            break;
        case 1:
            if (bridge.player.selectedAudioId !== 42 || bridge.player.selectedDurationLabel !== "02:05" || bridge.player.selectedArtworkUrl === "") return;
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
            if (bridge.library.trackSortIndex !== 1 || menu.popup.visible || !sortedRowsMatch([42, 103, 7])) return;
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
            if (bridge.library.trackSortIndex !== 2 || menu.popup.visible || !sortedRowsMatch([7, 42, 103])) return;
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
            check(menu.activeFocus && bridge.library.trackSortIndex === 2, "Escape retains sort and restores focus");
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
            if (bridge.library.trackSortIndex !== 0 || menu.popup.visible || !sortedRowsMatch([103, 42, 7])) return;
            finish();
            break;
        }
    }
    function playlistsProbe(): void {
        if (!bridge.connected || bridge.library.libraryLoading || bridge.settings.foldersLoading || bridge.playlists.playlistBusy || bridge.playlists.playlistLoading) return;
        const dialog = findChild(window.contentItem, "playlistsDialog");
        const pane = findChild(window.contentItem, "playlistsPane");
        switch (stage) {
        case 0:
            if (bridge.playlists.playlists.length !== 1 || (window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 1) return;
            check(!findChild(window.contentItem, "playlistsButton"), "previous playlist entry removed");
            findChild(window.contentItem, "playlistsTabButton").clicked();
            stage = 1;
            break;
        case 1: {
            if (!pane.visible || pane.activeId >= 0) return;
            check(window.selectedTab === 1 && !dialog.visible, "Playlists opens separate left pane");
            check(bridge.playlists.playlists[0].itemCount === 1, "list counts unavailable difficulty");
            const list = findChild(pane, "playlistList");
            const card = list.itemAtIndex(0);
            if (!card) return;
            card.clicked();
            stage = 2;
            break;
        }
        case 2:
            if (bridge.playlists.activePlaylistId !== 1 || (window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 1 || bridge.playlists.playlistMessage.length > 0) return;
            check(bridge.player.selectedSubtitle.includes("Unavailable") && !bridge.player.selectedAvailable, "missing entry remains selectable and unavailable");
            check(!findChild(window.contentItem, "playPauseButton").enabled, "unavailable playlist entry cannot play");
            findChild(pane, "playlistBackButton").clicked();
            stage = 3;
            break;
        case 3:
            if (bridge.playlists.activePlaylistId >= 0) return;
            findChild(pane, "createPlaylistButton").clicked();
            stage = 31;
            break;
        case 31:
            if (!bridge.playlists.playlistEditorOpen) return;
            check(findChild(pane, "playlistEditor").visible, "plus opens inline create card");
            bridge.playlists.setPlaylistEditorName(" New playlist ");
            findChild(pane, "savePlaylistEditor").clicked();
            stage = 4;
            break;
        case 4:
            if (bridge.playlists.playlists.length !== 2 || bridge.playlists.playlistEditorOpen) return;
            check(bridge.playlists.playlists[1].name === "New playlist", "live creation trims name and closes editor");
            findChild(window.contentItem, "songsTabButton").clicked();
            stage = 41;
            break;
        case 41:
            if (bridge.playlists.playlistVisible || (window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 1) return;
            check(findChild(window.contentItem, "songSearch").enabled, "Songs always presents searchable library");
            findChild(window.contentItem, "addToPlaylistButton").clicked();
            stage = 5;
            break;
        case 5:
            if (!dialog.opened || !bridge.playlists.playlistAdding) return;
            check(bridge.playlists.playlistCandidates.length === 2, "selected song exposes every concrete difficulty");
            dialog.targetRequested(2);
            dialog.difficultyRequested(2);
            dialog.addRequested();
            stage = 6;
            break;
        case 6:
            if (bridge.playlists.playlistOpen) return;
            findChild(window.contentItem, "songSearch").text = "no-song-matches-playlists";
            stage = 61;
            break;
        case 61:
            if ((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 0 || bridge.library.libraryLoading) return;
            findChild(window.contentItem, "playlistsTabButton").clicked();
            bridge.playlists.selectPlaylist(2);
            stage = 7;
            break;
        case 7:
            if (bridge.playlists.activePlaylistId !== 2 || (window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 1 || bridge.playlists.playlistMessage.length > 0) return;
            check(bridge.player.selectedSubtitle.includes("Easy"), "chosen difficulty displayed separately");
            findChild(window.contentItem, "addToPlaylistButton").clicked();
            stage = 8;
            break;
        case 8:
            if (!dialog.opened || !bridge.playlists.playlistAdding || bridge.playlists.playlistCandidates.length !== 2) return;
            check(bridge.playlists.playlistCandidates.length === 2, "adding from playlist outside search reloads all song difficulties");
            dialog.targetRequested(2);
            dialog.addRequested();
            stage = 9;
            break;
        case 9: {
            if (bridge.playlists.playlistOpen || (window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 2 || bridge.playlists.playlistMessage.length > 0) return;
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
            if (bridge.library.trackSortIndex !== 1 || bridge.playlists.selectedPlaylistItemId !== sortMedia.selected) return;
            check(bridge.player.selectedSubtitle.includes("Hard") && !bridge.player.selectedIsPlaying,
                "sorting preserves selected difficulty without starting playback");
            check(resets === retainedResets, "playlist sorting does not reset model");
            findChild(window.contentItem, "songsTabButton").clicked();
            stage = 92;
            break;
        case 92:
            if (bridge.playlists.playlistVisible || (window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 0) return;
            check(findChild(window.contentItem, "songSearch").text === "no-song-matches-playlists", "Songs keeps its own query");
            findChild(window.contentItem, "playlistsTabButton").clicked();
            stage = 93;
            break;
        case 93:
            if (!bridge.playlists.playlistVisible || (window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 2) return;
            check(bridge.playlists.activePlaylistId === 2 && bridge.playlists.selectedPlaylistItemId === sortMedia.selected,
                "return to Playlists restores detail and selected item");
            findChild(pane, "editPlaylistButton").clicked();
            bridge.playlists.setPlaylistEditorName(" Renamed ");
            findChild(pane, "savePlaylistEditor").clicked();
            stage = 10;
            break;
        case 10:
            if (bridge.playlists.activePlaylistName !== "Renamed" || bridge.playlists.playlistEditorOpen) return;
            check(findChild(pane, "activePlaylistTitle").text === "Renamed", "inline edit updates detail header");
            bridge.playlists.selectPlaylistItem(sortMedia.first);
            bridge.library.setTrackSort(0);
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
            findChild(card, "removePlaylistTrack").forceActiveFocus();
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0);
            stage = 12;
            break;
        }
        case 12:
            if ((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 1 || bridge.playlists.playlistMessage.length > 0) return;
            check(bridge.player.selectedSubtitle.includes("Hard"), "context removal retains other difficulty");
            pane.deleteRequested(2);
            stage = 130;
            break;
        case 130: {
            const dialog = findChild(window.contentItem, "confirmDialog");
            if (!dialog.opened) return;
            check(dialog.title.startsWith("Delete “") && bridge.playlists.playlists.length === 2, "delete waits for confirmation and names the playlist");
            input.keyClick(Qt.Key_Escape, Qt.NoModifier, 0);
            stage = 131;
            break;
        }
        case 131: {
            const dialog = findChild(window.contentItem, "confirmDialog");
            if (dialog.visible) return;
            check(bridge.playlists.playlists.length === 2 && !bridge.playlists.playlistBusy, "Escape keeps the playlist");
            pane.deleteRequested(2);
            stage = 132;
            break;
        }
        case 132: {
            const dialog = findChild(window.contentItem, "confirmDialog");
            if (!dialog.opened) return;
            input.mouseClick(findChild(window.contentItem, "confirmAccept"));
            stage = 13;
            break;
        }
        case 13:
            if (bridge.playlists.activePlaylistId >= 0 || bridge.playlists.playlists.length !== 1) return;
            if (findChild(window.contentItem, "toast").text !== "Playlist deleted") return;
            check(window.selectedTab === 1 && pane.activeId < 0, "deleting active playlist returns to list");
            pane.searchEdited("no-playlist-matches");
            stage = 14;
            break;
        case 14:
            if (bridge.playlists.filteredPlaylists.length !== 0 || findChild(pane, "playlistList").count !== 0) return;
            check(bridge.playlists.playlists.length === 1 && pane.playlists.length === 0,
                "playlist search filters actual cards without dropping canonical summaries");
            findChild(window.contentItem, "songsTabButton").clicked();
            findChild(window.contentItem, "songSearch").text = "";
            stage = 15;
            break;
        case 15:
            if ((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 1 || bridge.library.libraryLoading) return;
            check(bridge.player.selectedTitle === "Test song", "Songs library remains independent of playlist search");
            findChild(window.contentItem, "playlistsTabButton").clicked();
            stage = 16;
            break;
        case 16:
            if (!bridge.playlists.playlistVisible) return;
            check(bridge.playlists.playlistQuery === "no-playlist-matches" && pane.playlists.length === 0, "playlist query survives tab switches");
            pane.searchEdited("");
            stage = 17;
            break;
        case 17:
            if (bridge.playlists.filteredPlaylists.length !== 1 || findChild(pane, "playlistList").count !== 1) return;
            check(bridge.playlists.playlistQuery === "" && pane.playlists.length === 1, "clearing query restores actual playlist cards");
            finish();
            break;
        }
    }
    function playlistCoversProbe(): void {
        if (!bridge.connected || bridge.library.libraryLoading || bridge.settings.foldersLoading || bridge.playlists.playlistBusy || bridge.playlists.playlistLoading) return;
        const pane = findChild(window.contentItem, "playlistsPane");
        switch (stage) {
        case 0:
            if ((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) === 0) return;
            findChild(window.contentItem, "playlistsTabButton").clicked();
            bridge.playlists.beginPlaylistCreate();
            bridge.playlists.setPlaylistEditorName("Cover retry");
            stage = 10;
            break;
        case 10:
            if (!bridge.playlists.playlistEditorOpen || bridge.playlists.playlistEditorName !== "Cover retry") return;
            check(coverPath.length > 0, "local cover fixture provided");
            bridge.playlists.completePlaylistCoverPick(bridge.playlists.playlistEditorEpoch, "file://" + coverPath);
            check(bridge.playlists.playlistCoverPreparing && !findChild(pane, "savePlaylistEditor").enabled,
                "Save waits for asynchronous cover preparation");
            bridge.playlists.savePlaylistEditor();
            check(bridge.playlists.playlists.length === 0, "saving during preparation does not create playlist");
            stage = 1;
            break;
        case 1:
            if (bridge.playlists.playlistCoverPreparing || bridge.playlists.playlistEditorArtworkUrl.length === 0) return;
            check(findChild(pane, "savePlaylistEditor").enabled, "Save resumes after preparation completes");
            check(findChild(pane, "playlistEditor").artworkUrl.toString().length > 0, "prepared cover shown before save");
            sortMedia = {preview: bridge.playlists.playlistEditorArtworkUrl};
            findChild(pane, "savePlaylistEditor").clicked();
            stage = 2;
            break;
        case 2:
            if (bridge.playlists.playlistMessage.length === 0 || bridge.playlists.playlistEditorId < 0) return;
            check(bridge.playlists.playlists.length === 1 && bridge.playlists.playlistEditorOpen, "cover upload failure retains created playlist and inline editor");
            check(bridge.playlists.playlistEditorName === "Cover retry" && bridge.playlists.playlistEditorArtworkUrl === sortMedia.preview,
                "cover error preserves editor name and prepared preview");
            sortMedia.id = bridge.playlists.playlistEditorId;
            findChild(pane, "savePlaylistEditor").clicked();
            stage = 3;
            break;
        case 3:
            if (bridge.playlists.playlistEditorOpen || bridge.playlists.playlists.length !== 1) return;
            check(bridge.playlists.playlists[0].id === sortMedia.id, "retry updates same playlist without duplicate create");
            bridge.playlists.requestPlaylistArtwork(sortMedia.id);
            stage = 4;
            break;
        case 4:
            if (bridge.playlists.playlists[0].artworkUrl.length === 0) return;
            sortMedia.saved = bridge.playlists.playlists[0].artworkUrl;
            bridge.playlists.beginPlaylistEdit(sortMedia.id);
            stage = 5;
            break;
        case 5:
            if (bridge.playlists.playlistEditorArtworkUrl.length === 0) return;
            check(bridge.playlists.playlistEditorName === "Cover retry", "existing editor retains name and saved cover");
            sortMedia.epoch = bridge.playlists.playlistEditorEpoch;
            bridge.playlists.completePlaylistCoverPick(sortMedia.epoch, "");
            check(bridge.playlists.playlistEditorArtworkUrl.length > 0, "cancelled chooser retains existing cover");
            bridge.playlists.cancelPlaylistEditor();
            stage = 51;
            break;
        case 51:
            if (bridge.playlists.playlistEditorOpen) return;
            bridge.playlists.beginPlaylistEdit(sortMedia.id);
            stage = 52;
            break;
        case 52:
            if (!bridge.playlists.playlistEditorOpen || bridge.playlists.playlistEditorEpoch === sortMedia.epoch) return;
            bridge.playlists.completePlaylistCoverPick(sortMedia.epoch, "file://" + coverPath);
            check(!bridge.playlists.playlistCoverPreparing, "obsolete picker result ignored before starting preparation");
            bridge.playlists.resetPlaylistCover();
            stage = 53;
            break;
        case 53:
            if (bridge.playlists.playlistCoverPreparing || bridge.playlists.playlistEditorArtworkUrl.length > 0) return;
            findChild(pane, "savePlaylistEditor").clicked();
            stage = 6;
            break;
        case 6:
            if (bridge.playlists.playlistEditorOpen) return;
            check(bridge.playlists.playlists.length === 1 && bridge.playlists.playlists[0].artworkUrl.length === 0, "cover reset restores automatic placeholder for empty playlist");
            findChild(window.contentItem, "songsTabButton").clicked();
            stage = 7;
            break;
        case 7:
            if (bridge.playlists.playlistVisible || (window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) === 0) return;
            findChild(window.contentItem, "playlistsTabButton").clicked();
            stage = 8;
            break;
        case 8:
            if (!bridge.playlists.playlistVisible) return;
            check(bridge.playlists.playlists.length === 1 && bridge.playlists.playlists[0].id === sortMedia.id, "tab switch retains saved playlist");
            finish();
            break;
        }
    }
    function playlistCoverPreviewProbe(): void {
        if (!bridge.connected || bridge.library.libraryLoading || bridge.settings.foldersLoading || bridge.playlists.playlistBusy
            || (stage !== 0 && bridge.playlists.playlistLoading)) return;
        const pane = findChild(window.contentItem, "playlistsPane");
        const editor = findChild(pane, "playlistEditor");
        const save = findChild(editor, "savePlaylistEditor");
        switch (stage) {
        case 0:
            if ((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 3 || bridge.player.selectedDurationLabel === "--:--" || bridge.player.selectedArtworkUrl.length === 0) return;
            check(bridge.playlists.playlists.length === 0, "initial playlist response stays pending before tab opens");
            retainedResets = resets;
            findChild(window.contentItem, "playlistsTabButton").clicked();
            check(window.selectedTab === 1, "playlist tab opens before its first list response");
            stage = 1;
            break;
        case 1:
            if (bridge.playlists.playlists.length !== 1 || bridge.playlists.playlists[0].artworkUrl.length === 0) return;
            console.info("initial visible playlist receives custom artwork");
            check(resets === retainedResets, "identical rows avoid model reset when opening playlist list");
            sortMedia.saved = bridge.playlists.playlists[0].artworkUrl;
            bridge.playlists.selectPlaylist(1);
            stage = 2;
            break;
        case 2:
            if (bridge.playlists.activePlaylistId !== 1 || (window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 1) return;
            check(bridge.playlists.playlists[0].artworkUrl === sortMedia.saved, "detail navigation preserves decoded playlist cache");
            findChild(pane, "editPlaylistButton").clicked();
            stage = 3;
            break;
        case 3:
            if (!bridge.playlists.playlistEditorOpen || bridge.playlists.playlistEditorArtworkUrl.length === 0) return;
            check(bridge.playlists.playlistEditorArtworkUrl === sortMedia.saved, "editor reuses saved custom cover inside playlist");
            check(Math.abs(save.width - (editor.width - 32)) < 1, "Save fills editor content width");
            sortMedia.saved = bridge.playlists.playlistEditorArtworkUrl;
            findChild(editor, "resetPlaylistCover").clicked();
            stage = 4;
            break;
        case 4:
            if (!bridge.playlists.playlistEditorArtworkUrl.includes("auto-7")) return;
            check(editor.artworkUrl.toString() === bridge.playlists.playlistEditorArtworkUrl, "automatic cover previews before saving");
            check(bridge.playlists.playlists[0].artworkUrl === sortMedia.saved, "automatic preview leaves saved custom cover intact");
            findChild(editor, "closePlaylistEditor").clicked();
            stage = 5;
            break;
        case 5:
            if (bridge.playlists.playlistEditorOpen) return;
            findChild(pane, "editPlaylistButton").clicked();
            stage = 6;
            break;
        case 6:
            if (!bridge.playlists.playlistEditorOpen || bridge.playlists.playlistEditorArtworkUrl !== sortMedia.saved) return;
            findChild(editor, "resetPlaylistCover").clicked();
            stage = 7;
            break;
        case 7:
            if (!bridge.playlists.playlistEditorArtworkUrl.includes("auto-7")) return;
            save.clicked();
            stage = 8;
            break;
        case 8:
            if (bridge.playlists.playlistEditorOpen || (window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 1) return;
            findChild(pane, "editPlaylistButton").clicked();
            stage = 9;
            break;
        case 9:
            if (!bridge.playlists.playlistEditorOpen || !bridge.playlists.playlistEditorArtworkUrl.includes("auto-7")) return;
            check(bridge.playlists.playlistEditorId === 1, "editor loads saved automatic cover inside playlist");
            findChild(editor, "closePlaylistEditor").clicked();
            stage = 10;
            break;
        case 10:
            if (bridge.playlists.playlistEditorOpen) return;
            bridge.playlists.beginPlaylistCreate();
            bridge.playlists.setPlaylistEditorName("New playlist");
            stage = 11;
            break;
        case 11:
            if (!bridge.playlists.playlistEditorOpen || bridge.playlists.playlistEditorId >= 0) return;
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
                bridge.settings.setUseUnicodeTitles(false);
                check(!bridge.settings.useUnicodeTitles && bridge.settings.useUnicodeArtists, "changing one key retains the other");
                bridge.settings.setUseUnicodeTitles(true);
                bridge.connectSession();
                check(bridge.settings.useUnicodeTitles && bridge.settings.useUnicodeArtists, "reconnect retains local choices");
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
            check(bridge.settings.useUnicodeTitles && !bridge.settings.useUnicodeArtists, "keyboard title toggle preserves artist preference");
            bridge.settings.setUseUnicodeArtists(true);
            bridge.settings.setUseUnicodeTitles(false);
            bridge.settings.setUseUnicodeArtists(false);
            bridge.settings.setUseUnicodeTitles(true);
            bridge.settings.setUseUnicodeArtists(true);
            check(bridge.settings.useUnicodeTitles && bridge.settings.useUnicodeArtists, "rapid toggles merge synchronously");
            if (testCase === "names-write-error") {
                check(message.text === "Could not save track name preferences. Your choice applies for this session.", "write failure retains choices and English message");
            } else {
                check(message.text.length === 0, "successful persistence has no error");
            }
            bridge.connectSession();
            check(bridge.settings.useUnicodeTitles && bridge.settings.useUnicodeArtists, "reconnect does not replace session choices");
            finish();
        }
    }
    function discordPreferenceProbe(): void {
        const toggle = findChild(window.contentItem, "discordRichPresenceSwitch");
        const section = findChild(window.contentItem, "discordSettingsSection");
        const search = findChild(window.contentItem, "settingsSearch");
        const message = findChild(window.contentItem, "discordPreferenceStatus");
        if (stage === 0) {
            findChild(window.contentItem, "settingsButton").clicked();
            check(toggle.enabled && toggle.text === "Discord Rich Presence", "RPC works offline with English label");
            check(toggle.checked === (testCase === "discord-read"), "RPC defaults off and restores persisted preference");
            if (testCase === "discord-read") check(bridge.settings.useUnicodeTitles && bridge.settings.useUnicodeArtists, "RPC preference reads preserve Unicode choices");
            if (testCase === "discord-read-error") {
                check(message.text.length > 0, "RPC read error reported inline");
                finish(); return;
            }
            search.text = "dIsCoRd";
            stage = 1; return;
        }
        if (stage === 1) {
            check(section.visible && !findChild(window.contentItem, "generalSettingsSection").visible
                && !findChild(window.contentItem, "audioSettingsSection").visible, "RPC case insensitive Settings Search");
            search.text = "no such integration";
            stage = 2; return;
        }
        if (stage === 2) {
            check(!section.visible && findChild(window.contentItem, "settingsSearchEmpty").visible, "RPC no-match empty state");
            search.text = "Integrations";
            stage = 3; return;
        }
        if (stage === 3) {
            check(section.visible, "RPC searchable by section title");
            toggle.forceActiveFocus();
            stage = 4; return;
        }
        if (stage === 4) {
            input.keyClick(Qt.Key_Space, Qt.NoModifier, 0);
            check(bridge.settings.discordRichPresence === (testCase !== "discord-read"), "RPC keyboard toggle changes live preference");
            if (testCase === "discord-write-error") check(message.text.length > 0, "RPC write failure remains usable");
            else check(message.text.length === 0, "RPC persistence succeeds quietly");
            bridge.settings.setDiscordRichPresence(false);
            bridge.settings.setDiscordRichPresence(true);
            bridge.settings.setDiscordRichPresence(false);
            bridge.settings.setDiscordRichPresence(true);
            bridge.connectSession();
            check(bridge.settings.discordRichPresence, "RPC rapid toggles and reconnect retain latest choice");
            check(bridge.settings.closeToTray === (testCase !== "discord-read") && !window.closeToTray, "close-to-tray defaults on, restores the saved choice and needs a tray host");
            bridge.settings.setCloseToTray(false);
            check(!bridge.settings.closeToTray, "close-to-tray toggle changes live preference");
            finish();
        }
    }
    property int realQueueSelection: -1
    function realQueueProbe(): void {
        if (!bridge.connected || bridge.library.libraryLoading || bridge.queue.queueBusy || bridge.queue.queueLoading) return;
        const panel = findChild(window.contentItem, "queuePanel");
        const list = panel ? findChild(panel.contentItem, "queueScroll") : null;
        const ids = () => Array.from({length: bridge.queue.tracks.trackCount}, (_, i) => bridge.queue.tracks.entryIdAt(i));
        switch (stage) {
        case 0:
            if (bridge.library.tracks.trackCount !== 4 || bridge.player.selectedAudioId < 0) return;
            realQueueSelection = bridge.player.selectedAudioId;
            findChild(window.contentItem, "queueButton").clicked(); stage = 1; break;
        case 1:
            if (!panel || !panel.opened || !list || list.count !== 4 || !list.itemAtIndex(3)) return;
            check(JSON.stringify(ids()) === JSON.stringify(["9007199254740993", "9007199254740994", "9007199254740995", "9007199254740996"]), "real BIGINT IDs survive server, client and QML");
            check(list.itemAtIndex(0).model.isPast && list.itemAtIndex(1).model.isCurrent && !list.itemAtIndex(2).model.isPast, "real recovered history/current/future");
            check(list.itemAtIndex(3).model.availability === "unavailable", "real worker rejects missing WAV and retains its row");
            check(bridge.player.selectedAudioId === realQueueSelection && !bridge.player.selectedIsPlaying && !bridge.player.canSeek && bridge.player.playbackPosition === 0, "opening recovered pause preserves selection and opens no engine");
            if (testCase === "queue-real-recovery") { finish(); return; }
            bridge.queue.moveEntry("9007199254740994", "9007199254740993"); stage = 2; break;
        case 2:
            if (ids()[0] !== "9007199254740994") return;
            list.forceLayout();
            if (!list.itemAtIndex(0) || list.itemAtIndex(0).entryId !== "9007199254740994") return;
            check(bridge.queue.startIndex === 0 && list.itemAtIndex(0).model.isCurrent, "real move follows current identity");
            bridge.queue.removeEntry("9007199254740993"); stage = 3; break;
        case 3:
            if (list.count !== 3) return;
            check(ids()[0] === "9007199254740994", "removing duplicate keeps current occurrence");
            bridge.queue.removeEntry("9007199254740994"); stage = 4; break;
        case 4:
            if (list.count !== 2 || ids()[0] !== "9007199254740995") return;
            list.forceLayout();
            if (!list.itemAtIndex(0) || list.itemAtIndex(0).entryId !== "9007199254740995") return;
            check(list.itemAtIndex(0).model.isCurrent && !bridge.player.selectedIsPlaying && !bridge.player.canSeek && bridge.player.playbackPosition === 0, "removing paused current selects next without opening engine");
            panel.close();
            findChild(window.contentItem, "songSearch").text = "C"; stage = 5; break;
        case 5:
            if (bridge.library.tracks.trackCount !== 1) return;
            insertionCard = findChild(window.contentItem, "songList").itemAtIndex(0);
            if (!insertionCard || insertionCard.title !== "C") return;
            insertionCard.forceActiveFocus(); input.keyClick(Qt.Key_Menu, Qt.NoModifier, 0); stage = 6; break;
        case 6:
            if (!findChild(insertionCard, "songQueueMenu").opened) return;
            findChild(insertionCard, "insertQueueTrackNext").forceActiveFocus();
            input.keyClick(Qt.Key_Return, Qt.NoModifier, 0); stage = 7; break;
        case 7:
            findChild(window.contentItem, "queueButton").clicked(); stage = 8; break;
        case 8:
            if (!panel.opened || list.count !== 3 || !list.itemAtIndex(1)) return;
            if (list.itemAtIndex(1).model.availability !== "available") return;
            check(list.itemAtIndex(1).model.title === "C" && ids()[2] === "9007199254740996", "search Next inserts into real queue and worker checks the file");
            bridge.playlists.addPlaylistToQueue(1); stage = 9; break;
        case 9:
            if (list.count !== 7) return;
            list.positionViewAtIndex(6, ListView.Contain);
            if (!list.itemAtIndex(6)) return;
            if (list.itemAtIndex(6).model.availability !== "unavailable") return;
            check(!bridge.player.selectedIsPlaying && !bridge.player.canSeek && bridge.player.playbackPosition === 0, "whole playlist append retains unavailable and pause");
            findChild(panel.contentItem, "queueClearUpcoming").clicked(); stage = 10; break;
        case 10:
            if (list.count !== 1) return;
            list.positionViewAtBeginning();
            if (!list.itemAtIndex(0)) return;
            check(ids()[0] === "9007199254740995" && list.itemAtIndex(0).model.isCurrent, "real clear upcoming keeps current");
            findChild(panel.contentItem, "queueClearAll").clicked(); stage = 100; break;
        case 100:
            if (!findChild(window.contentItem, "confirmDialog").opened) return;
            input.mouseClick(findChild(window.contentItem, "confirmAccept")); stage = 11; break;
        case 11:
            if (list.count !== 0) return;
            check(!bridge.player.selectedIsPlaying && !bridge.player.canSeek && bridge.player.playbackPosition === 0 && bridge.queue.queueMessage === "", "real clear all stops without errors");
            // Modes go through the real server; the fixture verifies they persist after this process exits.
            check(bridge.player.canChangeModes && !bridge.player.shuffle && bridge.player.repeatMode === "off", "real modes start off and are changeable on an empty queue");
            findChild(window.contentItem, "shuffleButton").clicked();
            findChild(window.contentItem, "repeatButton").clicked(); stage = 12; break;
        case 12:
            if (!bridge.player.shuffle || bridge.player.repeatMode !== "all") return;
            check(findChild(window.contentItem, "shuffleButton").accessibleName === "Shuffle: on" && findChild(window.contentItem, "repeatButton").accessibleName === "Repeat: all", "real server answers drive the mode buttons");
            finish(); break;
        }
    }
    function step(): void {
        if (passed) return;
        if (testCase === "controls") { controlsProbe(); return; }
        if (testCase === "controls-popups") { menuModalProbe(); return; }
        if (testCase === "controls-mods") { modsProbe(); return; }
        if (testCase === "controls-progress") { progressProbe(); return; }
        if (testCase === "gallery-mods") { galleryModsProbe(); return; }
        if (testCase === "mods") { modsLiveProbe(); return; }
        if (testCase === "controls-window") { windowProbe(); return; }
        if (testCase === "controls-panels") { panelProbe(); return; }
        if (testCase === "gallery-insert") { galleryInsertionProbe(); return; }
        if (testCase === "gallery-queue") { galleryQueueProbe(); return; }
        if (gallery) { galleryModalProbe(); return; }
        if (testCase.startsWith("discord-")) { discordPreferenceProbe(); return; }
        if (testCase.startsWith("names-")) { trackNamePreferencesProbe(); return; }
        if (testCase === "folders") { folderProbe(); return; }
        if (testCase === "search") { searchProbe(); return; }
        if (testCase === "settings-search") { settingsSearchProbe(); return; }
        if (testCase === "playback") { playbackProbe(); return; }
        if (testCase === "queue") { queueProbe(); return; }
        if (testCase === "queue-insert") { insertionProbe(); return; }
        if (testCase.startsWith("queue-real")) { realQueueProbe(); return; }
        if (testCase.startsWith("queue-managed")) { managedQueueProbe(); return; }
        if (testCase === "volume") { volumeProbe(); return; }
        if (testCase === "hotkeys") { hotkeysProbe(); return; }
        if (testCase === "playlists") { playlistsProbe(); return; }
        if (testCase === "playlist-covers") { playlistCoversProbe(); return; }
        if (testCase === "playlist-cover-preview") { playlistCoverPreviewProbe(); return; }
        if (testCase === "sorting") { sortProbe(); return; }
        if (testCase === "requests") {
            if (!bridge.connected || bridge.settings.folders.length !== 2) return;
            check(bridge.library.libraryLoading, "library HTTP request remains pending at close");
            finish();
            return;
        }
        if (testCase === "empty") {
            if (!bridge.connected || bridge.library.libraryLoading || bridge.settings.foldersLoading) return;
            check((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) === 0 && !bridge.player.hasSelection, "empty real library");
            check(bridge.player.selectedAudioId === -1 && bridge.player.selectedArtworkUrl === "", "explicit empty selection");
            check(bridge.settings.folders.length === 0 && bridge.settings.selectedFolderId === -1, "empty real folders");
            check(bridge.library.libraryMessage.length > 0, "empty library explanation");
            finish();
            return;
        }
        switch (stage) {
        case 0:
            if (bridge.connecting || bridge.connectionStatus.length === 0) return;
            check(!bridge.connected && !bridge.settings.folderBusy, "startup failure state");
            check(findChild(window.contentItem, "retryConnection").visible && !findChild(window.contentItem, "connectionSpinner").visible, "error banner shows Retry without the spinner");
            check(findChild(window.contentItem, "body").y === findChild(window.contentItem, "titleBar").height, "connection banner overlays the window");
            stage = 1;
            bridge.connectSession();
            break;
        case 1:
            if (bridge.connecting) check(!findChild(window.contentItem, "retryConnection").visible, "connecting banner hides Retry");
            if (!bridge.connected || bridge.library.libraryLoading || bridge.settings.foldersLoading) return;
            check(findChild(window.contentItem, "body").y === findChild(window.contentItem, "titleBar").height, "no layout shift once connected");
            if (bridge.settings.folders.length !== 2 || !bridge.library.libraryMessage.includes("fixture library")) return;
            check((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) === 0 && !bridge.player.hasSelection, "library failure independent from folders");
            check(bridge.settings.selectedFolderId === 31, "first folder by ID");
            check(findChild(window.contentItem, "addFolder").enabled, "Add enabled after independent folder load");
            check(bridge.settings.folders[1].id === 52 && bridge.settings.folders[1].label === "lazer - /fixtures/52", "typed folder entries");
            stage = 2;
            bridge.library.refreshLibrary();
            break;
        case 2:
            if (bridge.library.libraryLoading || (window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 3 || bridge.player.selectedDurationLabel !== "02:05" || bridge.player.selectedArtworkUrl === "") return;
            const list = findChild(window.contentItem, "songList");
            const card = list.itemAtIndex(0);
            if (!card) return;
            check(card.audioId === 103 && card.title === "Track 103" && card.durationLabel === "02:05", "typed live model roles");
            check(card.subtitle === "Fixture artist | Hard", "shared subtitle role");
            const songFrame = card.background.children[card.background.children.length - 1];
            if (card.selectedTrack && songFrame.border.color.toString() !== Radio.Theme.accent.toString()) return; // frame fades in
            check(card.selectedTrack && songFrame.border.color.toString() === Radio.Theme.accent.toString(), "selected Songs card uses the accent frame");
            check(bridge.player.selectedAudioId === 103 && bridge.player.selectedTitle === "Track 103", "first sorted track uses database ID");
            check(rowChanges > 0, "media uses targeted row notifications");
            const retained = window.mediaDelegates.slice();
            const hidden = { audioId: 999, inViewport: false, available: true };
            window.addMediaDelegate(hidden);
            window.removeMediaDelegate(hidden);
            window.removeMediaDelegate(hidden);
            check(window.mediaDelegates.length === retained.length
                && window.mediaDelegates.every((row, index) => row === retained[index]), "removing an absent media delegate preserves every retained row");
            retainedResets = resets;
            stage = 3;
            bridge.library.selectTrack(42);
            break;
        case 3:
            if (bridge.player.selectedAudioId !== 42 || bridge.player.selectedDurationLabel !== "02:05" || bridge.player.selectedArtworkUrl === "") return;
            check(bridge.player.selectedTitle === "Track 42" && bridge.player.selectedSubtitle === "Fixture artist | Hard", "shared selection formatting");
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
            check(bridge.player.selectedAudioId === 42, "settings preserves player selection");
            input.mouseClick(findChild(window.contentItem, "playlistsTabButton"));
            stage = 30;
            break;
        case 30:
            if (bridge.playlists.playlistLoading) return;
            check(window.selectedTab === 1 && findChild(window.contentItem, "playlistsPane").visible
                && !findChild(window.contentItem, "settingsPane").visible, "Settings returns to Playlists");
            check(bridge.player.selectedAudioId === 42, "Playlists list retains the selected player track");
            input.mouseClick(findChild(window.contentItem, "settingsButton"));
            check(findChild(window.contentItem, "settingsSearch").text === "settings query", "Settings query survives Playlists navigation");
            input.mouseClick(findChild(window.contentItem, "songsTabButton"));
            stage = 31;
            break;
        case 31:
            if ((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 3 || bridge.player.selectedAudioId !== 42) return;
            check(window.selectedTab === 0 && findChild(window.contentItem, "songsPane").visible
                && !findChild(window.contentItem, "settingsPane").visible, "Settings returns to Songs");
            check(findChild(window.contentItem, "selectedTitle").text === "Track 42"
                && findChild(window.contentItem, "songSearch").text === "", "navigation retains track and Songs query");
            retainedResets = resets;
            input.mouseClick(findChild(window.contentItem, "settingsButton"));
            findChild(window.contentItem, "settingsSearch").text = "";
            findChild(window.contentItem, "folderMenu").choose(1);
            check(bridge.player.selectedAudioId === 42, "folder selection does not filter songs");
            stage = 4;
            bridge.settings.refreshFolders();
            break;
        case 4:
            if (bridge.settings.foldersLoading || !bridge.settings.folderMessage.includes("fixture folders")) return;
            check(bridge.settings.folders.length === 2 && bridge.settings.selectedFolderId === 52, "failed folder refresh retains rows and ID");
            check(bridge.settings.folderCanRetry && !bridge.settings.folderBusy, "folder failure offers retry");
            stage = 5;
            bridge.settings.retryFolders();
            break;
        case 5:
            if (bridge.settings.foldersLoading || bridge.settings.folderCanRetry) return;
            check(bridge.settings.selectedFolderId === 52 && bridge.settings.folders.length === 2, "folder retry retains selected ID");
            input.mouseClick(findChild(window.contentItem, "songsTabButton"));
            stage = 6;
            findChild(window.contentItem, "refreshLibrary").clicked();
            break;
        case 6:
            if (bridge.library.libraryLoading || !bridge.library.libraryMessage.includes("fixture library")) return;
            check((window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) === 3 && bridge.player.selectedAudioId === 42, "failed refresh retains rows and selection");
            check(resets === retainedResets, "failed refresh must not reset model");
            stage = 7;
            findChild(window.contentItem, "refreshLibrary").clicked();
            break;
        case 7:
            if (bridge.library.libraryLoading || (window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 1) return;
            check(bridge.player.selectedAudioId === 103 && bridge.player.selectedTitle === "Track 103", "shrinking refresh selects first remaining ID");
            stage = 8;
            findChild(window.contentItem, "refreshLibrary").clicked();
            break;
        case 8:
            if (bridge.library.libraryLoading || (window.selectedTab === 1 ? bridge.playlists.tracks.trackCount : bridge.library.tracks.trackCount) !== 0) return;
            check(!bridge.player.hasSelection && bridge.player.selectedAudioId === -1, "empty refresh clears selection");
            check(bridge.player.selectedArtworkUrl === "" && bridge.player.selectedDurationLabel === "--:--", "empty player clears media");
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
        if (gallery && !testCase.startsWith("controls") && testCase !== "gallery-queue" && testCase !== "gallery-insert" && testCase !== "gallery-mods") galleryProbe();
    }
}
