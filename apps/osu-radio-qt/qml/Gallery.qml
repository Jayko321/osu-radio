pragma ComponentBehavior: Bound
import QtQuick 6.8
import QtQuick.Controls.Basic as Basic
import OsuRadio 1.0
import "components"

Basic.ApplicationWindow {
    id: root
    objectName: "galleryWindow"
    width: 1200
    height: 900
    minimumWidth: 1024
    minimumHeight: 640
    visible: true
    title: "osu! radio · Components"
    flags: Qt.Window | Qt.FramelessWindowHint
    color: Theme.background
    font.family: Theme.fontFamily
    font.pixelSize: 16
    readonly property var demo: store.state.gallery
    property int selectedTab: 0
    readonly property var playlistView: store.state.playlists
    onPlaylistViewChanged: {
        if (playlistView.open && playlistView.adding) demoPlaylistsDialog.open();
        else demoPlaylistsDialog.close();
    }
    onSelectedTabChanged: store.send(selectedTab === 1 ? "playlistShow" : "playlistLibrary")
    Store { id: store }
    readonly property alias galleryStore: store

    component Heading: Text {
        color: Theme.text
        font.family: Theme.fontFamily
        font.pixelSize: 24
        font.weight: Font.DemiBold
    }
    component Caption: Text {
        color: Theme.muted
        font.family: Theme.fontFamily
        font.pixelSize: 14
        wrapMode: Text.Wrap
    }
    component DemoSection: Column {
        width: parent.width
        spacing: 16
    }

    Item {
        id: applicationScene
        anchors.fill: parent
        WindowBar { id: titleBar; width: parent.width; window: root; gallery: true; selectedTab: root.selectedTab; onTabSelected: index => root.selectedTab = index }

        Basic.ScrollView {
            id: scroll
            objectName: "galleryScroll"
            visible: root.selectedTab === 0
            anchors.top: titleBar.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            clip: true
            contentWidth: availableWidth
            Basic.ScrollBar.horizontal.policy: Basic.ScrollBar.AlwaysOff
            Column {
                id: page
                x: 40
                width: scroll.availableWidth - 80
                spacing: 32
                topPadding: 32
                bottomPadding: 40
                Column {
                    width: parent.width
                    spacing: 12
                    Heading { text: "osu! radio / COMPONENTS"; font.pixelSize: 32 }
                    Caption {
                        width: parent.width
                        text: "Tab to focus · Enter / Space to activate · Arrow keys in menus · Escape to dismiss"
                    }
                    AppSwitch {
                        objectName: "globalDisable"
                        text: "Disable demo controls"
                        checked: root.demo.disabled
                        onToggled: store.send("galleryDisabled", checked)
                    }
                    Caption { text: "Button presses: " + root.demo.presses }
                }

                Column {
                    width: parent.width
                    spacing: 32
                    enabled: !root.demo.disabled

                    DemoSection {
                        Heading { text: "Buttons" }
                        Row {
                            spacing: 12
                            Repeater {
                                model: ["light", "alternate", "accent", "link"]
                                AppButton {
                                    required property string modelData
                                    variant: modelData
                                    text: modelData.charAt(0).toUpperCase() + modelData.slice(1)
                                    onClicked: store.send("press")
                                }
                            }
                            IconButton { iconName: "circle-plus"; accessibleName: "Add"; onClicked: store.send("press") }
                            AppButton { variant: "accent"; text: "Disabled"; enabled: false }
                            IconButton { iconName: "play"; accessibleName: "Playback unavailable"; enabled: false }
                        }
                    }

                    DemoSection {
                        Heading { text: "Fields & search" }
                        Caption { text: "Playlist name"; color: Theme.text }
                        AppField {
                            objectName: "galleryField"
                            width: parent.width
                            placeholderText: "New playlist"
                            text: root.demo.field
                            Accessible.name: "Playlist name example"
                            onTextEdited: store.send("field", text)
                        }
                        Caption { text: "Give your collection a name." }
                        AppField {
                            width: parent.width
                            text: root.demo.filled
                            Accessible.name: "Filled field example"
                            onTextEdited: store.send("filled", text)
                        }
                        AppField {
                            width: parent.width
                            leftPadding: 44
                            placeholderText: "Search songs..."
                            text: root.demo.search
                            Accessible.name: "Search example"
                            onTextEdited: store.send("gallerySearch", text)
                            AppIcon { x: 12; anchors.verticalCenter: parent.verticalCenter; name: "search" }
                        }
                        AppField { width: parent.width; text: "Unavailable field"; enabled: false }
                    }

                    DemoSection {
                        Heading { text: "Switches & tabs" }
                        AppSwitch {
                            text: "Off / On"
                            checked: root.demo.switches[0]
                            onToggled: store.send("toggleSwitch", 0)
                        }
                        AppSwitch {
                            text: "On / Off"
                            checked: root.demo.switches[1]
                            onToggled: store.send("toggleSwitch", 1)
                        }
                        AppSwitch { text: "Disabled switch"; checked: true; enabled: false }
                        AppTabs {
                            labels: ["Songs", "Playlists", "Settings"]
                            currentIndex: root.demo.tab
                            onSelected: index => store.send("selectTab", index)
                        }
                        Caption { text: "Selected tab: " + root.demo.tab }
                    }

                    DemoSection {
                        Heading { text: "Tags" }
                        Row {
                            spacing: 12
                            AppTag {
                                text: "Electronic"
                                selected: root.demo.tag
                                onClicked: store.send("toggleTag")
                            }
                            AppTag { text: "Selected"; selected: !root.demo.tag; onClicked: store.send("toggleTag") }
                            AppTag { text: "Disabled"; selected: true; enabled: false }
                        }
                        Row {
                            spacing: 12
                            AppTag {
                                text: root.demo.filter === 2 ? "Ambient · excluded" : root.demo.filter === 1 ? "Ambient · included" : "Ambient"
                                filterState: root.demo.filter
                                onClicked: store.send("cycleFilter")
                            }
                            AppTag { text: "Included"; filterState: 1; enabled: false }
                            AppTag { text: "Excluded"; filterState: 2; enabled: false }
                        }
                        Caption { text: "Filter cycle: neutral → included → excluded → neutral" }
                    }

                    DemoSection {
                        Heading { text: "Menus" }
                        AppMenu {
                            objectName: "demoTrackSortMenu"
                            width: 172
                            entries: [
                                {index: 0, label: "Title A–Z"},
                                {index: 1, label: "Artist A–Z"},
                                {index: 2, label: "Recently played"}
                            ]
                            currentIndex: store.state.track_sort_index
                            onChosen: index => store.send("trackSort", index)
                            Accessible.name: "Sort demo tracks"
                        }
                        Caption {
                            width: parent.width
                            text: store.state.tracks.map(track => track.title).join(" · ")
                        }
                        Caption { text: "Selected track: " + store.state.tracks[store.state.selected].title }
                        AppMenu {
                            width: parent.width
                            entries: root.demo.menu_items.map((label, index) => ({index: index, label: label}))
                            currentIndex: root.demo.menu_selected
                            displayText: root.demo.menu_items[root.demo.menu_selected]
                            onChosen: index => store.send("selectMenu", index)
                            Accessible.name: "All playlists"
                        }
                        AppMenu {
                            objectName: "searchableMenu"
                            width: parent.width
                            entries: root.demo.menu_results
                            searchable: true
                            query: root.demo.menu_query
                            currentIndex: entries.findIndex(entry => entry.index === root.demo.menu_selected)
                            displayText: root.demo.menu_items[root.demo.menu_selected]
                            onQueryEdited: value => store.send("menuQuery", value)
                            onChosen: index => store.send("selectMenu", index)
                            Accessible.name: "Search playlists"
                        }
                        AppMenu {
                            width: parent.width
                            entries: []
                            searchable: true
                            query: root.demo.menu_query
                            displayText: "Empty menu"
                            onQueryEdited: value => store.send("menuQuery", value)
                            Accessible.name: "Empty menu example"
                        }
                        AppMenu { width: parent.width; displayText: "Disabled menu"; enabled: false }
                    }

                    DemoSection {
                        Heading { text: "Playlists" }
                        Caption { text: "The Playlists tab shares the application's list, editor and track cards. Changes stay in memory." }
                        Row {
                            spacing: 12
                            AppButton { objectName: "demoOpenPlaylists"; text: "Playlists"; onClicked: { root.selectedTab = 1; store.send("playlistSelect", -1); } }
                            AppButton { objectName: "demoAddToPlaylist"; text: "Add difficulties"; onClicked: store.send("playlistAddOpen") }
                        }
                    }
                    DemoSection {
                        Heading { text: "Modal" }
                        AppButton {
                            id: openModal
                            objectName: "openModal"
                            variant: "accent"
                            text: "Create playlist"
                            onClicked: { openModal.forceActiveFocus(); playlistDialog.open(); }
                        }
                        Caption { text: "The modal edits demo state only; it creates no playlist." }
                        AppButton {
                            objectName: "openFolderModal"
                            text: "Select osu! folders"
                            onClicked: { forceActiveFocus(); store.send("folderReset"); folderDialog.open(); }
                        }
                    }
                }

                DemoSection {
                    Heading { text: "Materials" }
                    Item {
                        width: parent.width
                        height: 180
                        Item {
                            id: materialBackdrop
                            anchors.fill: parent
                            Row {
                                anchors.fill: parent
                                Repeater {
                                    model: [Theme.accent, Theme.green, Theme.red]
                                    Rectangle { required property color modelData; width: materialBackdrop.width / 3; height: materialBackdrop.height; color: modelData }
                                }
                            }
                        }
                        Row {
                            anchors.fill: parent
                            anchors.margins: 16
                            spacing: 16
                            Repeater {
                                model: ["regular", "thick", "thin"]
                                MaterialSurface {
                                    id: material
                                    required property string modelData
                                    width: (parent.width - 32) / 3
                                    height: parent.height
                                    kind: modelData
                                    sourceItem: materialBackdrop
                                    radius: 16
                                    Column {
                                        anchors.centerIn: parent
                                        spacing: 8
                                        Heading { text: material.modelData.charAt(0).toUpperCase() + material.modelData.slice(1); font.pixelSize: 20 }
                                        Caption { text: "Sharp foreground"; color: Theme.text }
                                    }
                                }
                            }
                        }
                    }
                }

                DemoSection {
                    Heading { text: "Typography" }
                    Heading { text: "Poppins / Aa Bb Cc 0123"; font.pixelSize: 32 }
                    Caption { text: "Regular · Medium · Semibold · Bold" }
                    Row {
                        spacing: 32
                        Repeater {
                            model: [Font.Normal, Font.Medium, Font.DemiBold, Font.Bold]
                            Text {
                                required property int modelData
                                text: "osu! radio"
                                color: Theme.text
                                font.family: Theme.fontFamily
                                font.pixelSize: 18
                                font.weight: modelData
                            }
                        }
                    }
                }

                DemoSection {
                    Heading { text: "Lucide icons" }
                    Flow {
                        width: parent.width
                        spacing: 20
                        Repeater {
                            model: ["music", "arrow-left", "settings", "search", "chevron-down", "pencil", "plus", "circle-plus", "layers", "play", "skip-back", "skip-forward", "shuffle", "repeat-2", "volume-2", "rotate-cw", "minus", "square", "copy", "x"]
                            Column {
                                id: iconSample
                                required property string modelData
                                width: 90
                                height: 64
                                spacing: 8
                                AppIcon { anchors.horizontalCenter: parent.horizontalCenter; name: iconSample.modelData }
                                Caption { width: parent.width; text: iconSample.modelData; font.pixelSize: 11; horizontalAlignment: Text.AlignHCenter }
                            }
                        }
                    }
                }
            }
        }

        Item {
            anchors.top: titleBar.bottom
            anchors.bottom: parent.bottom
            width: parent.width
            visible: root.selectedTab === 1
            Rectangle { width: 480; height: parent.height; color: "#200e0e0e"; border.color: Theme.border }
            PlaylistPane {
                id: demoPlaylistPane
                objectName: "demoPlaylistPane"
                width: 480
                height: parent.height
                enabled: !root.demo.disabled
                playlists: store.state.filtered_playlists.map(playlist => ({
                    id: playlist.id, name: playlist.name, itemCount: playlist.item_count,
                    artworkUrl: playlist.custom_cover_revision !== null ? "qrc:/assets/logos/lazer.png" : ""
                }))
                activeId: root.playlistView.active_id === null ? -1 : root.playlistView.active_id
                activeName: root.playlistView.active ? root.playlistView.active.name : ""
                query: root.playlistView.query
                message: root.playlistView.message
                editorOpen: root.playlistView.editor_open
                editorId: root.playlistView.editor_id === null ? -1 : root.playlistView.editor_id
                editorName: root.playlistView.editor_name
                editorArtworkUrl: store.state.playlist_cover_present
                    || (!root.playlistView.editor_cover_reset && root.playlistView.playlists.some(playlist =>
                        playlist.id === root.playlistView.editor_id && playlist.custom_cover_revision !== null))
                    ? "qrc:/assets/logos/lazer.png" : ""
                sortIndex: store.state.track_sort_index
                tracksModel: root.playlistView.active ? root.playlistView.active.items : []
                trackCount: root.playlistView.active ? root.playlistView.active.items.length : 0
                onSearchEdited: query => store.send("playlistSearch", query)
                onCreateRequested: store.send("playlistBeginCreate")
                onEditRequested: id => store.send("playlistBeginEdit", id)
                onSelectRequested: id => store.send("playlistSelect", id)
                onQueueRequested: id => store.send("playlistQueue", id)
                onDeleteRequested: id => store.send("playlistDelete", id)
                onNameEdited: name => store.send("playlistEditorName", name)
                onCoverRequested: store.send("playlistCoverChoose", root.playlistView.editor_epoch)
                onCoverResetRequested: store.send("playlistCoverReset")
                onSaveRequested: store.send("playlistEditorSave")
                onCancelRequested: store.send("playlistEditorCancel")
                onRefreshRequested: store.send("playlistRefresh")
                onSortChosen: index => store.send("trackSort", index)
                trackDelegate: PlaylistTrackCard {
                    required property var modelData
                    width: demoPlaylistPane.tracksList.width
                    audioId: modelData.audio_source_id === null ? -1 : modelData.audio_source_id
                    playlistItemId: modelData.id
                    available: modelData.audio_source_id !== null
                    title: modelData.title || "Unknown title"
                    artist: modelData.artist || "Unknown artist"
                    subtitle: artist + " | " + (modelData.difficulty_name || "Unknown difficulty") + (available ? "" : " · Unavailable")
                    durationLabel: "--:--"
                    artworkUrl: ""
                    selected: root.playlistView.selected_item_id === playlistItemId
                    onClicked: store.send("playlistItemSelect", playlistItemId)
                    onRemoveRequested: itemId => store.send("playlistRemoveItem", itemId)
                }
            }
            Column {
                x: 520
                y: 80
                width: Math.max(0, parent.width - x - 40)
                spacing: 16
                Heading { width: parent.width; text: "Playlist presentation"; elide: Text.ElideRight }
                Caption { width: parent.width; text: "Open Evening to compare shared-audio difficulties and an unavailable entry. Use + to test the inline editor, its cover preview and reset. Right click a track, or press Menu / Shift+F10, to remove it." }
                AppButton { objectName: "galleryAddDifficulties"; text: "Add difficulties"; onClicked: store.send("playlistAddOpen") }
            }
        }
    }
    AppModal {
        id: playlistDialog
        objectName: "playlistDialog"
        scene: applicationScene
        title: "Create playlist"
        panelWidth: 561
        panelHeight: 370
        initialFocus: playlistName
        Column {
            anchors.fill: parent
            spacing: 24
            Caption {
                width: parent.width
                color: Theme.text
                text: "Collect your favourite songs in one place. This example stays in memory."
            }
            AppField {
                id: playlistName
                width: parent.width
                enabled: !root.demo.disabled
                placeholderText: "My playlist"
                text: root.demo.playlist_name
                Accessible.name: "Playlist name"
                onTextEdited: store.send("playlistName", text)
            }
            Caption { width: parent.width; text: "Try Tab / Shift+Tab, Escape and clicking the backdrop." }
        }
        footer: AppButton {
            anchors.right: parent.right
            variant: "accent"
            text: "Create"
            enabled: !root.demo.disabled
            onClicked: { store.send("press"); playlistDialog.close(); }
        }
    }
    PlaylistsModal {
        id: demoPlaylistsDialog
        objectName: "demoPlaylistsDialog"
        scene: applicationScene
        playlists: root.playlistView.playlists
        candidates: root.playlistView.candidates.map(candidate => ({id: candidate.beatmap_id, name: candidate.name, checked: candidate.checked}))
        adding: root.playlistView.adding
        message: root.playlistView.message
        targetId: root.playlistView.target_id === null ? -1 : root.playlistView.target_id
        onTargetRequested: id => store.send("playlistTarget", id)
        onDifficultyRequested: id => store.send("playlistDifficulty", id)
        onAddRequested: store.send("playlistAdd")
        onRefreshRequested: store.send("playlistRefresh")
        onClosed: if (root.playlistView.open) store.send("playlistClose")
    }
    FolderSelectionModal {
        id: folderDialog
        objectName: "folderSelectionDialog"
        scene: applicationScene
        rows: root.demo.folder_rows
        message: root.demo.folder_message
        onToggleRequested: path => store.send("folderToggle", path)
        onRetryCountRequested: path => store.send("folderCount", path)
        onBrowseRequested: store.send("folderBrowse")
        onApplyRequested: {
            store.send("folderApply");
            if (!root.demo.folder_rows.some(row => row.action.length > 0)) close();
        }
    }
}
