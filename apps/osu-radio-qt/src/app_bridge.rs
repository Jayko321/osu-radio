//! Typed Qt projection; workflows remain in osu-radio-client.
use crate::runtime::{RuntimeContext, ffi as native};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{
    QByteArray, QHash, QHashPair_i32_QByteArray, QList, QMap, QMapPair_QString_QVariant,
    QModelIndex, QString, QVariant,
};
use osu_radio_client::{
    ServerOptions, Track, TrackNamePreferences,
    controller::{
        AppCommand, AppController, AppUpdate, ConnectionStatus, FolderAction, FolderSelection,
        MediaTicket, PlaylistAction, PlaylistsState, QueueView, TrackSort,
    },
    playback::{Playback, PlayerState},
};
use std::{
    collections::HashMap,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

/// Preserve Qt's canonical list name in generated QML metadata.
#[repr(transparent)]
#[derive(Clone, Default, PartialEq)]
pub struct VariantList(QList<QVariant>);
// SAFETY: transparent alias of the relocatable QList<QVariant>; QVariantList is
// the identical C++ typedef. Clone and destruction delegate to that owned list.
unsafe impl cxx::ExternType for VariantList {
    type Id = cxx::type_id!("QVariantList");
    type Kind = cxx::kind::Trivial;
}
impl FromIterator<QVariant> for VariantList {
    fn from_iter<T: IntoIterator<Item = QVariant>>(values: T) -> Self {
        Self(values.into_iter().collect())
    }
}

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++Qt" {
        include!(<QtCore/QAbstractListModel>);
        #[qobject]
        type QAbstractListModel;
    }
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qlist.h");
        #[cxx_name = "QVariantList"]
        type QVariantList = super::VariantList;
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qvector.h");
        type QVector_i32 = cxx_qt_lib::QVector<i32>;
    }
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(
            bool,
            discord_rich_presence,
            cxx_name = "discordRichPresence",
            READ,
            NOTIFY
        )]
        #[qproperty(
            QString,
            discord_preference_message,
            cxx_name = "discordPreferenceMessage",
            READ,
            NOTIFY
        )]
        #[qproperty(bool, use_unicode_titles, cxx_name = "useUnicodeTitles", READ, NOTIFY)]
        #[qproperty(
            bool,
            use_unicode_artists,
            cxx_name = "useUnicodeArtists",
            READ,
            NOTIFY
        )]
        #[qproperty(
            QString,
            track_name_preferences_message,
            cxx_name = "trackNamePreferencesMessage",
            READ,
            NOTIFY
        )]
        #[qproperty(i32, track_count, cxx_name = "trackCount", READ, NOTIFY)]
        #[qproperty(i32, track_sort_index, cxx_name = "trackSortIndex", READ, NOTIFY)]
        #[qproperty(i32, selected_audio_id, cxx_name = "selectedAudioId", READ, NOTIFY)]
        #[qproperty(QString, selected_title, cxx_name = "selectedTitle", READ, NOTIFY)]
        #[qproperty(QString, selected_artist, cxx_name = "selectedArtist", READ, NOTIFY)]
        #[qproperty(
            QString,
            selected_subtitle,
            cxx_name = "selectedSubtitle",
            READ,
            NOTIFY
        )]
        #[qproperty(
            QString,
            selected_duration_label,
            cxx_name = "selectedDurationLabel",
            READ,
            NOTIFY
        )]
        #[qproperty(
            QString,
            selected_artwork_url,
            cxx_name = "selectedArtworkUrl",
            READ,
            NOTIFY
        )]
        #[qproperty(bool, has_selection, cxx_name = "hasSelection", READ, NOTIFY)]
        #[qproperty(QVariantList, folders, cxx_name = "folders", READ, NOTIFY)]
        #[qproperty(
            QVariantList,
            folder_selection_rows,
            cxx_name = "folderSelectionRows",
            READ,
            NOTIFY
        )]
        #[qproperty(
            bool,
            folder_selection_open,
            cxx_name = "folderSelectionOpen",
            READ,
            NOTIFY
        )]
        #[qproperty(
            bool,
            folder_selection_applying,
            cxx_name = "folderSelectionApplying",
            READ,
            NOTIFY
        )]
        #[qproperty(
            bool,
            folder_selection_discovering,
            cxx_name = "folderSelectionDiscovering",
            READ,
            NOTIFY
        )]
        #[qproperty(
            bool,
            folder_selection_picking,
            cxx_name = "folderSelectionPicking",
            READ,
            NOTIFY
        )]
        #[qproperty(
            QString,
            folder_selection_message,
            cxx_name = "folderSelectionMessage",
            READ,
            NOTIFY
        )]
        #[qproperty(i32, selected_folder_id, cxx_name = "selectedFolderId", READ, NOTIFY)]
        #[qproperty(
            QString,
            connection_status,
            cxx_name = "connectionStatus",
            READ,
            NOTIFY
        )]
        #[qproperty(bool, connected, cxx_name = "connected", READ, NOTIFY)]
        #[qproperty(bool, connecting, cxx_name = "connecting", READ, NOTIFY)]
        #[qproperty(QString, library_message, cxx_name = "libraryMessage", READ, NOTIFY)]
        #[qproperty(bool, library_loading, cxx_name = "libraryLoading", READ, NOTIFY)]
        #[qproperty(QString, folder_message, cxx_name = "folderMessage", READ, NOTIFY)]
        #[qproperty(bool, folders_loading, cxx_name = "foldersLoading", READ, NOTIFY)]
        #[qproperty(bool, folder_busy, cxx_name = "folderBusy", READ, NOTIFY)]
        #[qproperty(bool, folder_can_retry, cxx_name = "folderCanRetry", READ, NOTIFY)]
        #[qproperty(i32, current_audio_id, cxx_name = "currentAudioId", READ, NOTIFY)]
        #[qproperty(i32, loading_audio_id, cxx_name = "loadingAudioId", READ, NOTIFY)]
        #[qproperty(
            bool,
            selected_is_playing,
            cxx_name = "selectedIsPlaying",
            READ,
            NOTIFY
        )]
        #[qproperty(bool, can_seek, cxx_name = "canSeek", READ, NOTIFY)]
        #[qproperty(bool, can_next, cxx_name = "canNext", READ, NOTIFY)]
        #[qproperty(bool, can_previous, cxx_name = "canPrevious", READ, NOTIFY)]
        #[qproperty(QVariantList, queue_tracks, cxx_name = "queueTracks", READ, NOTIFY)]
        #[qproperty(bool, queue_loading, cxx_name = "queueLoading", READ, NOTIFY)]
        #[qproperty(QString, queue_message, cxx_name = "queueMessage", READ, NOTIFY)]
        #[qproperty(f64, playback_position, cxx_name = "playbackPosition", READ, NOTIFY)]
        #[qproperty(f64, playback_duration, cxx_name = "playbackDuration", READ, NOTIFY)]
        #[qproperty(
            QString,
            playback_position_label,
            cxx_name = "playbackPositionLabel",
            READ,
            NOTIFY
        )]
        #[qproperty(QString, playback_message, cxx_name = "playbackMessage", READ, NOTIFY)]
        #[qproperty(
            bool,
            individual_volume_enabled,
            cxx_name = "individualVolumeEnabled",
            READ,
            NOTIFY
        )]
        #[qproperty(
            i32,
            global_volume_percent,
            cxx_name = "globalVolumePercent",
            READ,
            NOTIFY
        )]
        #[qproperty(
            bool,
            audio_settings_loaded,
            cxx_name = "audioSettingsLoaded",
            READ,
            NOTIFY
        )]
        #[qproperty(bool, volume_enabled, cxx_name = "volumeEnabled", READ, NOTIFY)]
        #[qproperty(
            bool,
            volume_has_override,
            cxx_name = "volumeHasOverride",
            READ,
            NOTIFY
        )]
        #[qproperty(
            QString,
            audio_settings_message,
            cxx_name = "audioSettingsMessage",
            READ,
            NOTIFY
        )]
        #[qproperty(
            bool,
            audio_settings_can_retry,
            cxx_name = "audioSettingsCanRetry",
            READ,
            NOTIFY
        )]
        #[qproperty(f32, volume, cxx_name = "volume", READ, NOTIFY)]
        #[qproperty(
            bool,
            playlist_cover_preparing,
            cxx_name = "playlistCoverPreparing",
            READ,
            NOTIFY
        )]
        #[qproperty(
            u64,
            playlist_editor_epoch,
            cxx_name = "playlistEditorEpoch",
            READ,
            NOTIFY
        )]
        #[qproperty(bool, playlist_visible, cxx_name = "playlistVisible", READ, NOTIFY)]
        #[qproperty(QString, playlist_query, cxx_name = "playlistQuery", READ, NOTIFY)]
        #[qproperty(
            bool,
            playlist_editor_open,
            cxx_name = "playlistEditorOpen",
            READ,
            NOTIFY
        )]
        #[qproperty(
            QString,
            playlist_editor_name,
            cxx_name = "playlistEditorName",
            READ,
            NOTIFY
        )]
        #[qproperty(i32, playlist_editor_id, cxx_name = "playlistEditorId", READ, NOTIFY)]
        #[qproperty(
            QString,
            playlist_editor_artwork_url,
            cxx_name = "playlistEditorArtworkUrl",
            READ,
            NOTIFY
        )]
        #[qproperty(
            QVariantList,
            filtered_playlists,
            cxx_name = "filteredPlaylists",
            READ,
            NOTIFY
        )]
        #[qproperty(QVariantList, playlists, cxx_name = "playlists", READ, NOTIFY)]
        #[qproperty(
            QVariantList,
            playlist_candidates,
            cxx_name = "playlistCandidates",
            READ,
            NOTIFY
        )]
        #[qproperty(bool, playlist_open, cxx_name = "playlistOpen", READ, NOTIFY)]
        #[qproperty(bool, playlist_adding, cxx_name = "playlistAdding", READ, NOTIFY)]
        #[qproperty(bool, playlist_busy, cxx_name = "playlistBusy", READ, NOTIFY)]
        #[qproperty(bool, playlist_loading, cxx_name = "playlistLoading", READ, NOTIFY)]
        #[qproperty(QString, playlist_message, cxx_name = "playlistMessage", READ, NOTIFY)]
        #[qproperty(i32, active_playlist_id, cxx_name = "activePlaylistId", READ, NOTIFY)]
        #[qproperty(
            QString,
            active_playlist_name,
            cxx_name = "activePlaylistName",
            READ,
            NOTIFY
        )]
        #[qproperty(
            i32,
            selected_playlist_item_id,
            cxx_name = "selectedPlaylistItemId",
            READ,
            NOTIFY
        )]
        #[qproperty(i32, playlist_target_id, cxx_name = "playlistTargetId", READ, NOTIFY)]
        #[qproperty(bool, selected_available, cxx_name = "selectedAvailable", READ, NOTIFY)]
        #[qproperty(bool, can_add_playlist, cxx_name = "canAddPlaylist", READ, NOTIFY)]
        type AppBridge = super::AppBridgeRust;
        #[qinvokable]
        #[cxx_name = "showLibrary"]
        fn show_library(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "showPlaylists"]
        fn show_playlists(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "searchPlaylists"]
        fn search_playlists(self: Pin<&mut AppBridge>, query: &QString);
        #[qinvokable]
        #[cxx_name = "beginPlaylistCreate"]
        fn begin_playlist_create(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "beginPlaylistEdit"]
        fn begin_playlist_edit(self: Pin<&mut AppBridge>, id: i32);
        #[qinvokable]
        #[cxx_name = "setPlaylistEditorName"]
        fn edit_playlist_name(self: Pin<&mut AppBridge>, name: &QString);
        #[qinvokable]
        #[cxx_name = "cancelPlaylistEditor"]
        fn cancel_playlist_editor(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "savePlaylistEditor"]
        fn save_playlist_editor(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "choosePlaylistCover"]
        fn choose_playlist_cover(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "resetPlaylistCover"]
        fn reset_playlist_cover(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "completePlaylistCoverPick"]
        fn complete_playlist_cover_pick(self: Pin<&mut AppBridge>, epoch: u64, url: &QString);
        #[qinvokable]
        #[cxx_name = "requestPlaylistArtwork"]
        fn request_playlist_artwork(self: Pin<&mut AppBridge>, id: i32);
        #[qinvokable]
        #[cxx_name = "playPlaylistFrom"]
        fn play_playlist_from(self: Pin<&mut AppBridge>, id: i32);
        #[qsignal]
        #[cxx_name = "playlistCoverPickerRequested"]
        fn playlist_cover_picker_requested(self: Pin<&mut AppBridge>, epoch: u64);
        #[qinvokable]
        #[cxx_name = "openPlaylists"]
        fn open_playlists(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "closePlaylists"]
        fn close_playlists(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "refreshPlaylists"]
        fn refresh_playlists(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "createPlaylist"]
        fn create_playlist(self: Pin<&mut AppBridge>, name: &QString);
        #[qinvokable]
        #[cxx_name = "renamePlaylist"]
        fn rename_playlist(self: Pin<&mut AppBridge>, id: i32, name: &QString);
        #[qinvokable]
        #[cxx_name = "deletePlaylist"]
        fn delete_playlist(self: Pin<&mut AppBridge>, id: i32);
        #[qinvokable]
        #[cxx_name = "addPlaylistToQueue"]
        fn add_playlist_to_queue(self: Pin<&mut AppBridge>, id: i32);
        #[qinvokable]
        #[cxx_name = "selectPlaylist"]
        fn select_playlist(self: Pin<&mut AppBridge>, id: i32);
        #[qinvokable]
        #[cxx_name = "selectPlaylistItem"]
        fn select_playlist_item(self: Pin<&mut AppBridge>, id: i32);
        #[qinvokable]
        #[cxx_name = "playPlaylist"]
        fn play_playlist(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "openPlaylistAdd"]
        fn open_playlist_add(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "choosePlaylistTarget"]
        fn choose_playlist_target(self: Pin<&mut AppBridge>, id: i32);
        #[qinvokable]
        #[cxx_name = "togglePlaylistDifficulty"]
        fn toggle_playlist_difficulty(self: Pin<&mut AppBridge>, id: i32);
        #[qinvokable]
        #[cxx_name = "addPlaylistItems"]
        fn add_playlist_items(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "removePlaylistItem"]
        fn remove_playlist_item(self: Pin<&mut AppBridge>, id: i32);
        #[qinvokable]
        #[cxx_override]
        fn data(self: &AppBridge, index: &QModelIndex, role: i32) -> QVariant;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &AppBridge, parent: &QModelIndex) -> i32;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &AppBridge) -> QHash_i32_QByteArray;
        #[qinvokable]
        #[cxx_name = "setUseUnicodeTitles"]
        fn change_unicode_titles(self: Pin<&mut AppBridge>, enabled: bool);
        #[qinvokable]
        #[cxx_name = "setDiscordRichPresence"]
        fn change_discord_rich_presence(self: Pin<&mut AppBridge>, enabled: bool);
        #[qinvokable]
        #[cxx_name = "setUseUnicodeArtists"]
        fn change_unicode_artists(self: Pin<&mut AppBridge>, enabled: bool);
        #[qinvokable]
        #[cxx_name = "connectSession"]
        fn connect_session(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "searchLibrary"]
        fn search_library(self: Pin<&mut AppBridge>, query: &QString);
        #[qinvokable]
        #[cxx_name = "setTrackSort"]
        fn set_track_sort(self: Pin<&mut AppBridge>, index: i32);

        #[qinvokable]
        #[cxx_name = "refreshLibrary"]
        fn refresh_library(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "refreshFolders"]
        fn refresh_folders(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "selectTrack"]
        fn select_track(self: Pin<&mut AppBridge>, id: i32);
        #[qinvokable]
        #[cxx_name = "togglePlayback"]
        fn toggle_playback(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "nextTrack"]
        fn next_track(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "previousTrack"]
        fn previous_track(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "setQueueVisible"]
        fn set_queue_visible(self: Pin<&mut AppBridge>, visible: bool);
        #[qinvokable]
        #[cxx_name = "seekPlayback"]
        fn seek_playback(self: Pin<&mut AppBridge>, seconds: f64);
        #[qinvokable]
        #[cxx_name = "changeVolume"]
        fn change_volume(self: Pin<&mut AppBridge>, volume: f32);
        #[qinvokable]
        #[cxx_name = "setIndividualVolumeEnabled"]
        fn change_individual_volume(self: Pin<&mut AppBridge>, enabled: bool);
        #[qinvokable]
        #[cxx_name = "setGlobalVolume"]
        fn change_global_volume(self: Pin<&mut AppBridge>, percent: i32);
        #[qinvokable]
        #[cxx_name = "changeTrackVolume"]
        fn change_track_volume(self: Pin<&mut AppBridge>, id: i32, percent: i32);
        #[qinvokable]
        #[cxx_name = "useGlobalVolume"]
        fn use_global_volume(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "retryAudioSettings"]
        fn retry_audio_settings(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "selectFolder"]
        fn select_folder(self: Pin<&mut AppBridge>, id: i32);
        #[qinvokable]
        #[cxx_name = "addFolder"]
        fn add_folder(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "closeFolderSelection"]
        fn close_folder_selection(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "browseFolderSelection"]
        fn browse_folder_selection(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "toggleFolderSelection"]
        fn toggle_folder_selection(self: Pin<&mut AppBridge>, path: &QString);
        #[qinvokable]
        #[cxx_name = "refreshFolderSelection"]
        fn refresh_folder_selection(self: Pin<&mut AppBridge>, path: &QString);

        #[qinvokable]
        #[cxx_name = "retryFolderCount"]
        fn retry_folder_count(self: Pin<&mut AppBridge>, path: &QString);
        #[qinvokable]
        #[cxx_name = "applyFolderSelection"]
        fn apply_folder_selection(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "retryFolders"]
        fn retry_folders(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "requestMedia"]
        fn request_media(self: Pin<&mut AppBridge>, id: i32);
        #[qinvokable]
        #[cxx_name = "setVisibleMedia"]
        fn set_visible_media(self: Pin<&mut AppBridge>, ids: &QVariantList);
        #[qsignal]
        #[cxx_name = "mediaViewChanged"]
        fn media_view_changed(self: Pin<&mut AppBridge>);
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut AppBridge>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut AppBridge>);
    }
    unsafe extern "RustQt" {
        #[inherit]
        fn index(self: &AppBridge, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
        #[inherit]
        #[qsignal]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut AppBridge>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QVector_i32,
        );
    }
    impl cxx_qt::Threading for AppBridge {}
}

// Independent operation flags are exposed as individual typed QML properties.
#[allow(clippy::struct_excessive_bools)]
pub struct AppBridgeRust {
    playlist_state: PlaylistsState,
    playlist_cover_preparing: bool,
    playlist_editor_epoch: u64,
    cover_pick_serial: u64,
    draft_serial: u64,
    playlist_visible: bool,
    playlist_query: QString,
    playlist_editor_open: bool,
    playlist_editor_name: QString,
    playlist_editor_id: i32,
    playlist_editor_artwork_url: QString,

    filtered_playlists: VariantList,
    playlists: VariantList,
    playlist_candidates: VariantList,
    playlist_open: bool,
    playlist_adding: bool,
    playlist_busy: bool,
    playlist_loading: bool,
    playlist_message: QString,
    active_playlist_id: i32,
    active_playlist_name: QString,
    selected_playlist_item_id: i32,
    playlist_target_id: i32,
    selected_available: bool,
    can_add_playlist: bool,

    folder_selection_rows: VariantList,
    folder_selection_open: bool,
    folder_selection_applying: bool,
    folder_selection_discovering: bool,
    folder_selection_picking: bool,
    folder_selection_message: QString,
    current_audio_id: i32,
    loading_audio_id: i32,
    selected_is_playing: bool,
    can_seek: bool,
    can_next: bool,
    can_previous: bool,
    queue_tracks: VariantList,
    queue_loading: bool,
    queue_message: QString,
    queue_state: QueueView,
    queue_artwork: HashMap<i32, (Option<i32>, QString)>,
    playlist_artwork_urls: HashMap<(i32, Option<i64>, Option<i32>), QString>,
    playback_position: f64,
    playback_duration: f64,
    playback_position_label: QString,
    playback_message: QString,
    individual_volume_enabled: bool,
    global_volume_percent: i32,
    audio_settings_loaded: bool,
    volume_enabled: bool,
    volume_has_override: bool,
    audio_settings_message: QString,
    audio_settings_can_retry: bool,
    volume: f32,
    playback: Playback,
    selected_track_duration: Option<Duration>,
    selected_cover_id: Option<i32>,
    track_count: i32,
    track_sort_index: i32,
    selected_audio_id: i32,
    selected_title: QString,
    selected_artist: QString,
    selected_subtitle: QString,
    selected_duration_label: QString,
    selected_artwork_url: QString,
    has_selection: bool,
    folders: VariantList,
    selected_folder_id: i32,
    connection_status: QString,
    connected: bool,
    connecting: bool,
    library_message: QString,
    library_loading: bool,
    folder_message: QString,
    folders_loading: bool,
    folder_busy: bool,
    folder_can_retry: bool,
    rows: Vec<Row>,
    cover_rows: HashMap<i32, Vec<usize>>,
    audio_rows: HashMap<i32, usize>,
    discord_rich_presence: bool,
    discord_preference_message: QString,
    use_unicode_titles: bool,
    use_unicode_artists: bool,
    track_name_preferences_message: QString,
    local_preferences_loaded: bool,
    controller: Option<AppController>,
    generation: u64,
    cache_epoch: u64,
}
impl Default for AppBridgeRust {
    fn default() -> Self {
        Self {
            playlist_state: PlaylistsState::default(),
            playlist_cover_preparing: false,
            playlist_editor_epoch: 0,
            cover_pick_serial: 0,
            draft_serial: 0,
            playlist_visible: Default::default(),
            playlist_query: QString::default(),
            playlist_editor_open: Default::default(),
            playlist_editor_name: QString::default(),
            playlist_editor_id: -1,
            playlist_editor_artwork_url: QString::default(),

            filtered_playlists: VariantList::default(),
            playlists: VariantList::default(),
            playlist_candidates: VariantList::default(),
            playlist_open: Default::default(),
            playlist_adding: Default::default(),
            playlist_busy: Default::default(),
            playlist_loading: Default::default(),
            playlist_message: QString::default(),
            active_playlist_id: -1,
            active_playlist_name: QString::default(),
            selected_playlist_item_id: -1,
            playlist_target_id: -1,
            selected_available: Default::default(),
            can_add_playlist: Default::default(),
            folder_selection_rows: VariantList::default(),
            folder_selection_open: false,
            folder_selection_applying: false,
            folder_selection_discovering: false,
            folder_selection_picking: false,
            folder_selection_message: QString::default(),
            current_audio_id: -1,
            loading_audio_id: -1,
            selected_is_playing: false,
            can_seek: false,
            can_next: false,
            can_previous: false,
            queue_tracks: VariantList::default(),
            queue_loading: false,
            queue_message: QString::default(),
            queue_state: QueueView::default(),
            queue_artwork: HashMap::new(),
            playlist_artwork_urls: HashMap::new(),
            playback_position: 0.0,
            playback_duration: 0.0,
            playback_position_label: QString::from("00:00"),
            playback_message: QString::default(),
            individual_volume_enabled: false,
            global_volume_percent: 100,
            audio_settings_loaded: false,
            volume_enabled: false,
            volume_has_override: false,
            audio_settings_message: QString::default(),
            audio_settings_can_retry: false,
            volume: 1.0,
            playback: Playback::default(),
            selected_track_duration: None,
            selected_cover_id: None,
            track_count: Default::default(),
            track_sort_index: 0,
            selected_audio_id: -1,
            selected_title: QString::default(),
            selected_artist: QString::default(),
            selected_subtitle: QString::default(),
            selected_duration_label: QString::from("--:--"),
            selected_artwork_url: QString::default(),
            has_selection: Default::default(),
            folders: VariantList::default(),
            selected_folder_id: -1,
            connection_status: QString::default(),
            connected: Default::default(),
            connecting: Default::default(),
            library_message: QString::default(),
            library_loading: Default::default(),
            folder_message: QString::default(),
            folders_loading: Default::default(),
            folder_busy: Default::default(),
            folder_can_retry: Default::default(),
            rows: Vec::new(),
            cover_rows: HashMap::new(),
            audio_rows: HashMap::new(),
            discord_rich_presence: false,
            discord_preference_message: QString::default(),
            use_unicode_titles: false,
            use_unicode_artists: false,
            track_name_preferences_message: QString::default(),
            local_preferences_loaded: false,
            controller: None,
            generation: 0,
            cache_epoch: 0,
        }
    }
}
struct Row {
    playlist_item_id: Option<i32>,
    track: Track,
    artwork: QString,
}
const ROLES: [(i32, &str); 8] = [
    (256, "audioId"),
    (257, "title"),
    (258, "artist"),
    (259, "subtitle"),
    (260, "durationLabel"),
    (261, "artworkUrl"),
    (262, "playlistItemId"),
    (263, "available"),
];
macro_rules! property_setter {
    ($setter:ident, $field:ident, $notify:ident, $ty:ty) => {
        // Notify on exact property changes, including directly projected engine floats.
        #[allow(clippy::float_cmp)]
        fn $setter(mut self: Pin<&mut Self>, value: $ty) {
            if self.rust().$field != value {
                self.as_mut().rust_mut().$field = value;
                self.$notify();
            }
        }
    };
}
impl ffi::AppBridge {
    property_setter!(
        set_discord_rich_presence,
        discord_rich_presence,
        discord_rich_presence_changed,
        bool
    );
    property_setter!(
        set_discord_preference_message,
        discord_preference_message,
        discord_preference_message_changed,
        QString
    );
    pub fn change_discord_rich_presence(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().load_local_preferences();
        if self.discord_rich_presence == enabled {
            return;
        }
        self.as_mut().set_discord_rich_presence(enabled);
        self.command(AppCommand::SetDiscordRichPresence(enabled));
        if RuntimeContext::current().is_some() {
            let message = native::save_discord_rich_presence(enabled);
            self.set_discord_preference_message(message);
        }
    }
    property_setter!(
        set_use_unicode_titles,
        use_unicode_titles,
        use_unicode_titles_changed,
        bool
    );
    property_setter!(
        set_use_unicode_artists,
        use_unicode_artists,
        use_unicode_artists_changed,
        bool
    );
    property_setter!(
        set_track_name_preferences_message,
        track_name_preferences_message,
        track_name_preferences_message_changed,
        QString
    );

    fn track_name_preferences(&self) -> TrackNamePreferences {
        TrackNamePreferences {
            use_unicode_titles: self.use_unicode_titles,
            use_unicode_artists: self.use_unicode_artists,
        }
    }
    fn load_local_preferences(mut self: Pin<&mut Self>) {
        if self.local_preferences_loaded {
            return;
        }
        self.as_mut().rust_mut().local_preferences_loaded = true;
        // The offline gallery has no live runtime and never touches local settings.
        if RuntimeContext::current().is_none() {
            return;
        }
        let mut titles = false;
        let mut artists = false;
        let mut discord = false;
        // One QSettings read shares any FormatError across all preferences.
        let loaded = native::load_local_preferences(&mut titles, &mut artists, &mut discord);
        self.as_mut().set_use_unicode_titles(titles);
        self.as_mut().set_use_unicode_artists(artists);
        self.as_mut().set_discord_rich_presence(discord);
        if !loaded {
            self.as_mut()
                .set_track_name_preferences_message(QString::from(
                    "Could not read track name preferences. Using default names.",
                ));
            self.set_discord_preference_message(QString::from(
                "Could not read Discord preference. Rich Presence is disabled.",
            ));
        }
    }
    fn change_track_name_preference(mut self: Pin<&mut Self>, title: bool, enabled: bool) {
        self.as_mut().load_local_preferences();
        let current = if title {
            self.use_unicode_titles
        } else {
            self.use_unicode_artists
        };
        if current == enabled {
            return;
        }
        // Update the local pair synchronously; queued controller responses cannot overwrite it.
        if title {
            self.as_mut().set_use_unicode_titles(enabled);
        } else {
            self.as_mut().set_use_unicode_artists(enabled);
        }
        self.command(AppCommand::SetTrackNamePreferences(
            self.track_name_preferences(),
        ));
        if RuntimeContext::current().is_some() {
            let error = native::save_track_name_preference(title, enabled);
            self.set_track_name_preferences_message(error);
        }
    }
    pub fn change_unicode_titles(self: Pin<&mut Self>, enabled: bool) {
        self.change_track_name_preference(true, enabled);
    }
    pub fn change_unicode_artists(self: Pin<&mut Self>, enabled: bool) {
        self.change_track_name_preference(false, enabled);
    }
    property_setter!(
        set_playlist_cover_preparing,
        playlist_cover_preparing,
        playlist_cover_preparing_changed,
        bool
    );
    property_setter!(
        set_playlist_editor_epoch,
        playlist_editor_epoch,
        playlist_editor_epoch_changed,
        u64
    );
    property_setter!(
        set_playlist_visible,
        playlist_visible,
        playlist_visible_changed,
        bool
    );
    property_setter!(
        set_playlist_query,
        playlist_query,
        playlist_query_changed,
        QString
    );
    property_setter!(
        set_playlist_editor_open,
        playlist_editor_open,
        playlist_editor_open_changed,
        bool
    );
    property_setter!(
        set_playlist_editor_name,
        playlist_editor_name,
        playlist_editor_name_changed,
        QString
    );
    property_setter!(
        set_playlist_editor_id,
        playlist_editor_id,
        playlist_editor_id_changed,
        i32
    );
    property_setter!(
        set_playlist_editor_artwork_url,
        playlist_editor_artwork_url,
        playlist_editor_artwork_url_changed,
        QString
    );
    property_setter!(
        set_filtered_playlists,
        filtered_playlists,
        filtered_playlists_changed,
        VariantList
    );
    property_setter!(set_playlists, playlists, playlists_changed, VariantList);
    property_setter!(
        set_playlist_candidates,
        playlist_candidates,
        playlist_candidates_changed,
        VariantList
    );
    property_setter!(
        set_playlist_open,
        playlist_open,
        playlist_open_changed,
        bool
    );
    property_setter!(
        set_playlist_adding,
        playlist_adding,
        playlist_adding_changed,
        bool
    );
    property_setter!(
        set_playlist_busy,
        playlist_busy,
        playlist_busy_changed,
        bool
    );
    property_setter!(
        set_playlist_loading,
        playlist_loading,
        playlist_loading_changed,
        bool
    );
    property_setter!(
        set_playlist_message,
        playlist_message,
        playlist_message_changed,
        QString
    );
    property_setter!(
        set_active_playlist_id,
        active_playlist_id,
        active_playlist_id_changed,
        i32
    );
    property_setter!(
        set_active_playlist_name,
        active_playlist_name,
        active_playlist_name_changed,
        QString
    );
    property_setter!(
        set_selected_playlist_item_id,
        selected_playlist_item_id,
        selected_playlist_item_id_changed,
        i32
    );
    property_setter!(
        set_playlist_target_id,
        playlist_target_id,
        playlist_target_id_changed,
        i32
    );
    property_setter!(
        set_selected_available,
        selected_available,
        selected_available_changed,
        bool
    );
    property_setter!(
        set_can_add_playlist,
        can_add_playlist,
        can_add_playlist_changed,
        bool
    );
    property_setter!(
        set_folder_selection_rows,
        folder_selection_rows,
        folder_selection_rows_changed,
        VariantList
    );
    property_setter!(
        set_folder_selection_open,
        folder_selection_open,
        folder_selection_open_changed,
        bool
    );
    property_setter!(
        set_folder_selection_applying,
        folder_selection_applying,
        folder_selection_applying_changed,
        bool
    );
    property_setter!(
        set_folder_selection_discovering,
        folder_selection_discovering,
        folder_selection_discovering_changed,
        bool
    );
    property_setter!(
        set_folder_selection_picking,
        folder_selection_picking,
        folder_selection_picking_changed,
        bool
    );
    property_setter!(
        set_folder_selection_message,
        folder_selection_message,
        folder_selection_message_changed,
        QString
    );
    property_setter!(
        set_current_audio_id,
        current_audio_id,
        current_audio_id_changed,
        i32
    );
    property_setter!(
        set_loading_audio_id,
        loading_audio_id,
        loading_audio_id_changed,
        i32
    );
    property_setter!(
        set_selected_is_playing,
        selected_is_playing,
        selected_is_playing_changed,
        bool
    );
    property_setter!(set_can_seek, can_seek, can_seek_changed, bool);
    property_setter!(set_can_next, can_next, can_next_changed, bool);
    property_setter!(set_can_previous, can_previous, can_previous_changed, bool);
    property_setter!(
        set_queue_tracks,
        queue_tracks,
        queue_tracks_changed,
        VariantList
    );
    property_setter!(
        set_queue_loading,
        queue_loading,
        queue_loading_changed,
        bool
    );
    property_setter!(
        set_queue_message,
        queue_message,
        queue_message_changed,
        QString
    );
    property_setter!(
        set_playback_position,
        playback_position,
        playback_position_changed,
        f64
    );
    property_setter!(
        set_playback_duration,
        playback_duration,
        playback_duration_changed,
        f64
    );
    property_setter!(
        set_playback_position_label,
        playback_position_label,
        playback_position_label_changed,
        QString
    );
    property_setter!(
        set_playback_message,
        playback_message,
        playback_message_changed,
        QString
    );
    property_setter!(
        set_individual_volume_enabled,
        individual_volume_enabled,
        individual_volume_enabled_changed,
        bool
    );
    property_setter!(
        set_global_volume_percent,
        global_volume_percent,
        global_volume_percent_changed,
        i32
    );
    property_setter!(
        set_audio_settings_loaded,
        audio_settings_loaded,
        audio_settings_loaded_changed,
        bool
    );
    property_setter!(
        set_volume_enabled,
        volume_enabled,
        volume_enabled_changed,
        bool
    );
    property_setter!(
        set_volume_has_override,
        volume_has_override,
        volume_has_override_changed,
        bool
    );
    property_setter!(
        set_audio_settings_message,
        audio_settings_message,
        audio_settings_message_changed,
        QString
    );
    property_setter!(
        set_audio_settings_can_retry,
        audio_settings_can_retry,
        audio_settings_can_retry_changed,
        bool
    );
    property_setter!(set_volume, volume, volume_changed, f32);
    property_setter!(set_track_count, track_count, track_count_changed, i32);
    property_setter!(
        project_track_sort,
        track_sort_index,
        track_sort_index_changed,
        i32
    );
    property_setter!(
        set_selected_audio_id,
        selected_audio_id,
        selected_audio_id_changed,
        i32
    );
    property_setter!(
        set_selected_title,
        selected_title,
        selected_title_changed,
        QString
    );
    property_setter!(
        set_selected_artist,
        selected_artist,
        selected_artist_changed,
        QString
    );
    property_setter!(
        set_selected_subtitle,
        selected_subtitle,
        selected_subtitle_changed,
        QString
    );
    property_setter!(
        set_selected_duration_label,
        selected_duration_label,
        selected_duration_label_changed,
        QString
    );
    property_setter!(
        set_selected_artwork_url,
        selected_artwork_url,
        selected_artwork_url_changed,
        QString
    );
    property_setter!(
        set_has_selection,
        has_selection,
        has_selection_changed,
        bool
    );
    property_setter!(set_folders, folders, folders_changed, VariantList);
    property_setter!(
        set_selected_folder_id,
        selected_folder_id,
        selected_folder_id_changed,
        i32
    );
    property_setter!(
        set_connection_status,
        connection_status,
        connection_status_changed,
        QString
    );
    property_setter!(set_connected, connected, connected_changed, bool);
    property_setter!(set_connecting, connecting, connecting_changed, bool);
    property_setter!(
        set_library_message,
        library_message,
        library_message_changed,
        QString
    );
    property_setter!(
        set_library_loading,
        library_loading,
        library_loading_changed,
        bool
    );
    property_setter!(
        set_folder_message,
        folder_message,
        folder_message_changed,
        QString
    );
    property_setter!(
        set_folders_loading,
        folders_loading,
        folders_loading_changed,
        bool
    );
    property_setter!(set_folder_busy, folder_busy, folder_busy_changed, bool);
    property_setter!(
        set_folder_can_retry,
        folder_can_retry,
        folder_can_retry_changed,
        bool
    );

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(row) = usize::try_from(index.row())
            .ok()
            .and_then(|i| self.rows.get(i))
        else {
            return QVariant::default();
        };
        if !index.is_valid() || index.column() != 0 {
            return QVariant::default();
        }
        match role {
            256 => QVariant::from(&row.track.audio_source_id),
            257 => QVariant::from(&QString::from(&row.track.title)),
            258 => QVariant::from(&QString::from(&row.track.artist)),
            259 => QVariant::from(&QString::from(&row.track.subtitle)),
            260 => QVariant::from(&QString::from(row.track.duration_label())),
            261 => QVariant::from(&row.artwork),
            262 => QVariant::from(&row.playlist_item_id.unwrap_or(-1)),
            263 => QVariant::from(&(row.track.audio_source_id >= 0)),
            _ => QVariant::default(),
        }
    }
    pub fn row_count(&self, parent: &QModelIndex) -> i32 {
        if parent.is_valid() {
            0
        } else {
            i32::try_from(self.rows.len()).unwrap_or(i32::MAX)
        }
    }
    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        ROLES
            .iter()
            .map(|(id, name)| (*id, QByteArray::from(*name)))
            .collect()
    }
    fn command(&self, command: AppCommand) {
        if let Some(controller) = &self.controller {
            controller.send(command);
        }
    }
    pub fn connect_session(mut self: Pin<&mut Self>) {
        self.as_mut().load_local_preferences();
        if self.controller.is_none() {
            let Some(context) = RuntimeContext::current() else {
                return;
            };
            self.as_mut().set_selected_audio_id(-1);
            self.as_mut().set_selected_folder_id(-1);
            self.as_mut()
                .set_selected_duration_label(QString::from("--:--"));
            let thread = self.qt_thread();
            let slot = Arc::new(Mutex::new(None::<AppController>));
            let callback_slot = slot.clone();
            let controller =
                AppController::spawn(&context.handle, ServerOptions::default(), move |update| {
                    if let AppUpdate::Artwork {
                        ticket,
                        cover_id,
                        bytes,
                    } = update
                    {
                        let controller = callback_slot
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .clone();
                        if let Some(controller) = controller {
                            let ack = ArtworkAck {
                                controller,
                                ticket,
                                available: false,
                            };
                            // Dropping an undelivered closure acknowledges a destroyed QObject too.
                            let _ = thread.queue(move |object| object.decode(ack, cover_id, bytes));
                        }
                    } else {
                        let _ = thread.queue(move |object| object.apply(update));
                    }
                });
            *slot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(controller.clone());
            context.retain(controller.clone());
            self.as_mut().rust_mut().controller = Some(controller);
            self.command(AppCommand::SetDiscordRichPresence(
                self.discord_rich_presence,
            ));
            self.command(AppCommand::SetTrackNamePreferences(
                self.track_name_preferences(),
            ));
        }
        self.command(AppCommand::Connect);
    }
    fn playlist_command(&self, action: PlaylistAction) {
        self.command(AppCommand::Playlist(action));
    }
    pub fn show_library(self: Pin<&mut Self>) {
        self.playlist_command(PlaylistAction::ShowLibrary);
    }
    pub fn show_playlists(self: Pin<&mut Self>) {
        self.playlist_command(PlaylistAction::ShowPlaylists);
    }
    pub fn search_playlists(self: Pin<&mut Self>, query: &QString) {
        self.playlist_command(PlaylistAction::Search(query.to_string()));
    }
    pub fn begin_playlist_create(self: Pin<&mut Self>) {
        self.playlist_command(PlaylistAction::BeginCreate);
    }
    pub fn begin_playlist_edit(self: Pin<&mut Self>, id: i32) {
        self.playlist_command(PlaylistAction::BeginEdit(id));
    }
    pub fn edit_playlist_name(self: Pin<&mut Self>, name: &QString) {
        self.playlist_command(PlaylistAction::SetEditorName(name.to_string()));
    }
    pub fn cancel_playlist_editor(self: Pin<&mut Self>) {
        self.playlist_command(PlaylistAction::CancelEditor);
    }
    pub fn save_playlist_editor(self: Pin<&mut Self>) {
        if !self.playlist_cover_preparing {
            self.playlist_command(PlaylistAction::SaveEditor);
        }
    }
    pub fn reset_playlist_cover(self: Pin<&mut Self>) {
        self.playlist_command(PlaylistAction::ResetEditorCover);
    }
    pub fn play_playlist_from(self: Pin<&mut Self>, id: i32) {
        self.playlist_command(PlaylistAction::Play {
            id,
            start_item_id: None,
        });
    }
    pub fn choose_playlist_cover(mut self: Pin<&mut Self>) {
        if !self.playlist_editor_open || self.playlist_busy {
            return;
        }
        self.as_mut().rust_mut().cover_pick_serial = self.cover_pick_serial.wrapping_add(1);
        let epoch = self.playlist_state.editor_epoch;
        self.playlist_cover_picker_requested(epoch);
    }
    pub fn complete_playlist_cover_pick(mut self: Pin<&mut Self>, epoch: u64, url: &QString) {
        if epoch != self.playlist_state.editor_epoch || !self.playlist_editor_open {
            return;
        }
        if url.is_empty() {
            self.as_mut().set_playlist_cover_preparing(false);
            self.playlist_command(PlaylistAction::CompleteCoverPick {
                epoch,
                png: Ok(None),
            });
            return;
        }
        let Some(context) = RuntimeContext::current() else {
            return;
        };
        self.as_mut().set_playlist_cover_preparing(true);
        let thread = self.qt_thread();
        let serial = self.cover_pick_serial;
        let url = url.clone();
        let task = context.handle.spawn_blocking(move || {
            let mut error = QString::default();
            let png = native::prepare_playlist_cover(&url, &mut error);
            let result = if error.is_empty() {
                Ok(Some(png.as_slice().to_vec()))
            } else {
                Err(error.to_string())
            };
            let _ = thread.queue(move |mut object| {
                if object.cover_pick_serial == serial {
                    object.as_mut().set_playlist_cover_preparing(false);
                    object
                        .playlist_command(PlaylistAction::CompleteCoverPick { epoch, png: result });
                }
            });
        });
        context.track(task);
    }
    pub fn request_playlist_artwork(self: Pin<&mut Self>, id: i32) {
        if let Some(row) = self
            .playlist_state
            .playlists
            .iter()
            .find(|row| row.id == id)
        {
            self.playlist_artwork(row, false);
        }
    }
    fn playlist_artwork(
        &self,
        row: &osu_radio_client::models::PlaylistSummary,
        automatic: bool,
    ) -> QString {
        let revision = row.custom_cover_revision.filter(|_| !automatic);
        let url = native::playlist_artwork_url(
            self.cache_epoch,
            row.id,
            revision.unwrap_or(-1),
            row.cover_beatmap_id.unwrap_or(-1),
        );
        if url.is_empty() {
            if let Some(revision) = revision {
                self.playlist_command(PlaylistAction::RequestCover {
                    id: row.id,
                    revision,
                });
            } else if let Some(beatmap_id) = row.cover_beatmap_id {
                self.playlist_command(PlaylistAction::RequestArtwork {
                    id: row.id,
                    beatmap_id,
                });
            }
        }
        url
    }
    pub fn open_playlists(self: Pin<&mut Self>) {
        self.playlist_command(PlaylistAction::Open);
    }
    pub fn close_playlists(self: Pin<&mut Self>) {
        self.playlist_command(PlaylistAction::Close);
    }
    pub fn refresh_playlists(self: Pin<&mut Self>) {
        self.playlist_command(PlaylistAction::Refresh);
    }
    pub fn create_playlist(self: Pin<&mut Self>, name: &QString) {
        self.playlist_command(PlaylistAction::Create(name.to_string()));
    }
    pub fn rename_playlist(self: Pin<&mut Self>, id: i32, name: &QString) {
        self.playlist_command(PlaylistAction::Rename {
            id,
            name: name.to_string(),
        });
    }
    pub fn delete_playlist(self: Pin<&mut Self>, id: i32) {
        self.playlist_command(PlaylistAction::Delete(id));
    }
    pub fn add_playlist_to_queue(self: Pin<&mut Self>, id: i32) {
        self.playlist_command(PlaylistAction::AddToQueue(id));
    }
    pub fn select_playlist(self: Pin<&mut Self>, id: i32) {
        self.playlist_command(PlaylistAction::Select((id >= 0).then_some(id)));
    }
    pub fn select_playlist_item(self: Pin<&mut Self>, id: i32) {
        self.playlist_command(PlaylistAction::SelectItem(id));
    }
    pub fn play_playlist(self: Pin<&mut Self>) {
        self.playlist_command(PlaylistAction::Play {
            id: self.active_playlist_id,
            start_item_id: None,
        });
    }
    pub fn open_playlist_add(self: Pin<&mut Self>) {
        self.playlist_command(PlaylistAction::OpenAdd);
    }
    pub fn choose_playlist_target(self: Pin<&mut Self>, id: i32) {
        self.playlist_command(PlaylistAction::ChooseTarget(id));
    }
    pub fn toggle_playlist_difficulty(self: Pin<&mut Self>, id: i32) {
        self.playlist_command(PlaylistAction::ToggleDifficulty(id));
    }
    pub fn add_playlist_items(self: Pin<&mut Self>) {
        self.playlist_command(PlaylistAction::Add);
    }
    pub fn remove_playlist_item(self: Pin<&mut Self>, id: i32) {
        self.playlist_command(PlaylistAction::RemoveItem(id));
    }
    fn project_playlist_editor(mut self: Pin<&mut Self>, state: &PlaylistsState) {
        if state.editor_epoch != self.playlist_state.editor_epoch || !state.editor_open {
            self.as_mut().set_playlist_cover_preparing(false);
            self.as_mut().rust_mut().cover_pick_serial = self.cover_pick_serial.wrapping_add(1);
        }
        self.as_mut().set_playlist_editor_epoch(state.editor_epoch);
        self.as_mut().set_playlist_editor_open(state.editor_open);
        self.as_mut()
            .set_playlist_editor_name(QString::from(&state.editor_name));
        self.as_mut()
            .set_playlist_editor_id(state.editor_id.unwrap_or(-1));

        let preview = if let Some(bytes) = &state.editor_cover_png {
            if self.playlist_state.editor_cover_png != state.editor_cover_png
                || native::draft_artwork_url(self.cache_epoch, self.draft_serial).is_empty()
            {
                self.as_mut().rust_mut().draft_serial = self.draft_serial.wrapping_add(1);
                let affected: Vec<i32> = native::install_draft_artwork(
                    self.cache_epoch,
                    self.draft_serial,
                    &QByteArray::from(bytes.as_slice()),
                )
                .iter()
                .copied()
                .collect();
                self.as_mut().update_artwork(&affected);
            }
            native::draft_artwork_url(self.cache_epoch, self.draft_serial)
        } else {
            state
                .editor_id
                .filter(|_| state.editor_open)
                .and_then(|id| state.playlists.iter().find(|row| row.id == id))
                .map(|row| self.playlist_artwork(row, state.editor_cover_reset))
                .unwrap_or_default()
        };
        self.as_mut().set_playlist_editor_artwork_url(preview);
    }
    fn project_playlists(mut self: Pin<&mut Self>, state: &PlaylistsState) {
        self.as_mut().project_playlist_editor(state);
        // QML property signals can request artwork immediately for newly published rows.
        self.as_mut().rust_mut().playlist_state = state.clone();
        let mut previous = std::mem::take(&mut self.as_mut().rust_mut().playlist_artwork_urls);
        let mut artwork_urls = HashMap::new();
        for row in &state.playlists {
            let key = (row.id, row.custom_cover_revision, row.cover_beatmap_id);
            let mut url = native::playlist_artwork_url(
                self.cache_epoch,
                row.id,
                row.custom_cover_revision.unwrap_or(-1),
                row.cover_beatmap_id.unwrap_or(-1),
            );
            if url.is_empty() {
                url = previous.remove(&key).unwrap_or_default();
            }
            artwork_urls.insert(key, url);
        }
        let playlist_row = |playlist: &osu_radio_client::models::PlaylistSummary| {
            let mut map = QMap::<QMapPair_QString_QVariant>::default();
            map.insert(QString::from("id"), QVariant::from(&playlist.id));
            map.insert(
                QString::from("name"),
                QVariant::from(&QString::from(&playlist.name)),
            );
            map.insert(
                QString::from("itemCount"),
                QVariant::from(&i64::try_from(playlist.item_count).unwrap_or(i64::MAX)),
            );
            map.insert(
                QString::from("artworkUrl"),
                QVariant::from(
                    &artwork_urls
                        .get(&(
                            playlist.id,
                            playlist.custom_cover_revision,
                            playlist.cover_beatmap_id,
                        ))
                        .cloned()
                        .unwrap_or_default(),
                ),
            );
            QVariant::from(&map)
        };
        let playlists = state.playlists.iter().map(playlist_row).collect();
        let filtered = state
            .filtered_playlists()
            .into_iter()
            .map(playlist_row)
            .collect();
        self.as_mut().set_filtered_playlists(filtered);
        let candidates = state
            .candidates
            .iter()
            .map(|candidate| {
                let mut map = QMap::<QMapPair_QString_QVariant>::default();
                map.insert(QString::from("id"), QVariant::from(&candidate.beatmap_id));
                map.insert(
                    QString::from("name"),
                    QVariant::from(&QString::from(&candidate.name)),
                );
                map.insert(QString::from("checked"), QVariant::from(&candidate.checked));
                QVariant::from(&map)
            })
            .collect();
        self.as_mut().set_playlists(playlists);
        self.as_mut().rust_mut().playlist_artwork_urls = artwork_urls;
        self.as_mut().set_playlist_candidates(candidates);
        self.as_mut().set_playlist_busy(state.busy);
        self.as_mut().set_playlist_loading(state.loading);
        self.as_mut().set_playlist_adding(state.adding);
        self.as_mut().set_playlist_visible(state.visible);
        self.as_mut()
            .set_playlist_query(QString::from(&state.query));
        self.as_mut()
            .set_playlist_message(QString::from(&state.message));
        self.as_mut()
            .set_active_playlist_id(state.active_id.unwrap_or(-1));
        self.as_mut().set_active_playlist_name(QString::from(
            state
                .active
                .as_ref()
                .map_or("Playlist", |playlist| playlist.name.as_str()),
        ));
        self.as_mut()
            .set_selected_playlist_item_id(state.selected_item_id.unwrap_or(-1));
        self.as_mut()
            .set_playlist_target_id(state.target_id.unwrap_or(-1));
        let open = state.open;
        if self.playlist_state.showing_detail() && self.playlist_state.active.is_none() {
            self.as_mut().set_selected_available(false);
        }
        self.as_mut().project_playback();
        self.set_playlist_open(open);
    }
    pub fn search_library(self: Pin<&mut Self>, query: &QString) {
        self.command(AppCommand::SearchLibrary(query.to_string()));
    }
    pub fn set_track_sort(self: Pin<&mut Self>, index: i32) {
        if let Some(sort) = track_sort_from_index(index) {
            self.command(AppCommand::SetTrackSort(sort));
        }
    }

    pub fn refresh_library(self: Pin<&mut Self>) {
        self.command(AppCommand::RefreshLibrary);
    }
    pub fn refresh_folders(self: Pin<&mut Self>) {
        self.command(AppCommand::RefreshFolders);
    }
    pub fn select_track(self: Pin<&mut Self>, id: i32) {
        self.command(AppCommand::SelectTrack((id >= 0).then_some(id)));
    }
    fn transport_command(&self) -> Option<AppCommand> {
        if !self.has_selection || !self.selected_available {
            return None;
        }
        if self.playlist_state.showing_detail() {
            return Some(AppCommand::Playlist(PlaylistAction::TogglePlayback));
        }
        Some(
            if self.playback.current_audio_id == Some(self.selected_audio_id) {
                if self.playback.snapshot.state == PlayerState::Playing {
                    AppCommand::Pause
                } else {
                    AppCommand::Resume
                }
            } else {
                AppCommand::PlayTrack(self.selected_audio_id)
            },
        )
    }
    pub fn toggle_playback(self: Pin<&mut Self>) {
        if let Some(command) = self.transport_command() {
            self.command(command);
        }
    }
    pub fn next_track(self: Pin<&mut Self>) {
        if self.can_next {
            self.command(AppCommand::Next);
        }
    }
    pub fn previous_track(self: Pin<&mut Self>) {
        if self.can_previous {
            self.command(AppCommand::Previous);
        }
    }
    pub fn seek_playback(self: Pin<&mut Self>, seconds: f64) {
        if self.can_seek
            && let Ok(position) = Duration::try_from_secs_f64(seconds)
        {
            self.command(AppCommand::Seek(position));
        }
    }
    pub fn change_volume(self: Pin<&mut Self>, volume: f32) {
        self.command(AppCommand::SetVolume(volume));
    }
    pub fn change_individual_volume(self: Pin<&mut Self>, enabled: bool) {
        self.command(AppCommand::SetIndividualVolumeEnabled(enabled));
    }
    pub fn change_global_volume(self: Pin<&mut Self>, percent: i32) {
        if let Ok(percent) = u8::try_from(percent)
            && percent <= 100
        {
            self.command(AppCommand::SetGlobalVolume(percent));
        }
    }
    pub fn change_track_volume(self: Pin<&mut Self>, id: i32, percent: i32) {
        if let Ok(percent) = u8::try_from(percent)
            && percent <= 100
        {
            self.command(AppCommand::SetTrackVolume {
                audio_id: id,
                volume_percent: Some(percent),
            });
        }
    }
    pub fn use_global_volume(self: Pin<&mut Self>) {
        self.command(AppCommand::SetTrackVolume {
            audio_id: self.selected_audio_id,
            volume_percent: None,
        });
    }
    pub fn retry_audio_settings(self: Pin<&mut Self>) {
        self.command(AppCommand::RetryAudioSettings);
    }
    fn project_audio_settings(
        mut self: Pin<&mut Self>,
        state: osu_radio_client::controller::AudioSettingsState,
    ) {
        self.as_mut()
            .set_individual_volume_enabled(state.settings.individual_volume_enabled);
        self.as_mut()
            .set_global_volume_percent(i32::from(state.settings.global_volume_percent));
        self.as_mut().set_audio_settings_loaded(state.loaded);
        self.as_mut().set_volume_enabled(
            state.loaded
                && (!state.settings.individual_volume_enabled || state.selected_audio_id.is_some()),
        );
        self.as_mut()
            .set_volume_has_override(state.selected_has_override);
        self.as_mut()
            .set_volume(f32::from(state.selected_volume_percent) / 100.0);
        self.as_mut()
            .set_audio_settings_can_retry(!state.error.is_empty());
        self.set_audio_settings_message(QString::from(if state.loading {
            "Loading volume settings…".to_owned()
        } else {
            state.error
        }));
    }
    fn project_playback(mut self: Pin<&mut Self>) {
        let current = self.playback.current_audio_id;
        let selected_current = self.has_selection
            && current == Some(self.selected_audio_id)
            && (!self.playlist_state.showing_detail()
                || self.playback.current_playlist_item_id == Some(self.selected_playlist_item_id));
        let position = if selected_current {
            self.playback.snapshot.position
        } else {
            Duration::ZERO
        };
        let duration = if selected_current {
            self.playback
                .snapshot
                .duration
                .or(self.selected_track_duration)
        } else {
            None
        };
        let playing = selected_current && self.playback.snapshot.state == PlayerState::Playing;
        let loading = self.playback.loading_audio_id;
        let message = self.playback.error.clone().unwrap_or_else(|| {
            if loading.is_some() {
                "Loading audio…".into()
            } else {
                String::new()
            }
        });
        let can_seek =
            selected_current && self.playback.has_source && duration.is_some_and(|d| !d.is_zero());
        let can_next = self.playback.can_next;
        let can_previous = self.playback.can_previous;
        self.as_mut().set_current_audio_id(current.unwrap_or(-1));
        self.as_mut().set_loading_audio_id(loading.unwrap_or(-1));
        self.as_mut().set_selected_is_playing(playing);
        self.as_mut().set_can_seek(can_seek);
        self.as_mut().set_can_next(can_next);
        self.as_mut().set_can_previous(can_previous);
        self.as_mut()
            .set_playback_duration(duration.map_or(0.0, |d| d.as_secs_f64()));
        self.as_mut().set_playback_position(position.as_secs_f64());
        self.as_mut()
            .set_playback_position_label(QString::from(format_time(position)));
        self.as_mut().set_playback_message(QString::from(message));
        if let Some(duration) = duration {
            self.set_selected_duration_label(QString::from(format_time(duration)));
        }
    }
    pub fn select_folder(self: Pin<&mut Self>, id: i32) {
        self.command(AppCommand::SelectFolder((id >= 0).then_some(id)));
    }
    pub fn add_folder(self: Pin<&mut Self>) {
        self.command(AppCommand::OpenFolderSelection);
    }
    pub fn close_folder_selection(self: Pin<&mut Self>) {
        self.command(AppCommand::CloseFolderSelection);
    }
    pub fn browse_folder_selection(self: Pin<&mut Self>) {
        self.command(AppCommand::BrowseFolderSelection);
    }
    pub fn toggle_folder_selection(self: Pin<&mut Self>, path: &QString) {
        self.command(AppCommand::ToggleFolderSelection(path.to_string()));
    }
    pub fn refresh_folder_selection(self: Pin<&mut Self>, path: &QString) {
        self.command(AppCommand::RefreshFolderSelection(path.to_string()));
    }
    pub fn retry_folder_count(self: Pin<&mut Self>, path: &QString) {
        self.command(AppCommand::RetryFolderCount(path.to_string()));
    }
    pub fn apply_folder_selection(self: Pin<&mut Self>) {
        self.command(AppCommand::ApplyFolderSelection);
    }
    fn project_folder_selection(mut self: Pin<&mut Self>, selection: FolderSelection) {
        let rows = selection
            .rows
            .into_iter()
            .map(|row| {
                let mut map = QMap::<QMapPair_QString_QVariant>::default();
                for (key, value) in [
                    ("kind", row.kind),
                    ("path", row.root_path),
                    ("markerPath", row.marker_path),
                    (
                        "action",
                        match row.pending {
                            Some(FolderAction::Add) => "add",
                            Some(FolderAction::Remove) => "remove",
                            Some(FolderAction::Refresh) => "refresh",
                            None => "",
                        }
                        .into(),
                    ),
                    (
                        "count",
                        row.count.value().map_or_else(String::new, u64::to_string),
                    ),
                    ("countError", row.count.error().unwrap_or_default().into()),
                    ("error", row.error.unwrap_or_default()),
                ] {
                    map.insert(QString::from(key), QVariant::from(&QString::from(value)));
                }
                map.insert(
                    QString::from("registered"),
                    QVariant::from(&row.registered_id.is_some()),
                );
                map.insert(
                    QString::from("countPending"),
                    QVariant::from(&row.count.is_pending()),
                );
                QVariant::from(&map)
            })
            .collect();
        self.as_mut().set_folder_selection_rows(rows);
        self.as_mut()
            .set_folder_selection_applying(selection.applying);
        self.as_mut()
            .set_folder_selection_discovering(selection.discovering);
        self.as_mut()
            .set_folder_selection_picking(selection.picking);
        self.as_mut()
            .set_folder_selection_message(QString::from(selection.message));
        self.set_folder_selection_open(selection.open);
    }
    pub fn retry_folders(self: Pin<&mut Self>) {
        self.command(AppCommand::RetryFolders);
    }
    pub fn request_media(self: Pin<&mut Self>, id: i32) {
        let cover = if self.has_selection && id == self.selected_audio_id {
            self.selected_cover_id
        } else if let Some(row) = self
            .audio_rows
            .get(&id)
            .and_then(|index| self.rows.get(*index))
        {
            row.track.cover_beatmap_id
        } else if let Some(track) = self
            .queue_state
            .tracks
            .iter()
            .find(|track| track.audio_source_id == id)
        {
            track.cover_beatmap_id
        } else {
            return;
        };
        let missing =
            cover.is_some_and(|cover| native::artwork_url(self.cache_epoch, cover).is_empty());
        self.command(AppCommand::RequestMedia {
            audio_id: id,
            artwork_missing: missing,
        });
    }
    pub fn set_visible_media(self: Pin<&mut Self>, ids: &VariantList) {
        let visible = ids
            .0
            .iter()
            .filter_map(QVariant::value::<i32>)
            .filter_map(|id| {
                let track = self
                    .audio_rows
                    .get(&id)
                    .and_then(|index| self.rows.get(*index))
                    .map(|row| &row.track)
                    .or_else(|| {
                        self.queue_state
                            .tracks
                            .iter()
                            .find(|track| track.audio_source_id == id)
                    })?;
                let missing = track
                    .cover_beatmap_id
                    .is_some_and(|cover| native::artwork_url(self.cache_epoch, cover).is_empty());
                Some((id, missing))
            })
            .collect();
        self.command(AppCommand::SetVisibleMedia(visible));
    }
    pub fn set_queue_visible(self: Pin<&mut Self>, visible: bool) {
        self.command(AppCommand::SetQueueVisible(visible));
    }
    fn project_queue(mut self: Pin<&mut Self>) {
        let previous = std::mem::take(&mut self.as_mut().rust_mut().queue_artwork);
        let mut artwork_urls = HashMap::new();
        let rows = self
            .queue_state
            .tracks
            .iter()
            .map(|track| {
                let mut map = QMap::<QMapPair_QString_QVariant>::default();
                map.insert(
                    QString::from("audioId"),
                    QVariant::from(&track.audio_source_id),
                );
                let mut artwork = track
                    .cover_beatmap_id
                    .map(|id| native::artwork_url(self.cache_epoch, id))
                    .unwrap_or_default();
                if artwork.is_empty()
                    && let Some((cover, url)) = previous.get(&track.audio_source_id)
                    && *cover == track.cover_beatmap_id
                {
                    artwork = url.clone();
                }
                artwork_urls.insert(
                    track.audio_source_id,
                    (track.cover_beatmap_id, artwork.clone()),
                );
                for (key, value) in [
                    ("title", QString::from(track.title.as_str())),
                    ("artist", QString::from(track.artist.as_str())),
                    ("durationLabel", QString::from(track.duration_label())),
                    ("artworkUrl", artwork),
                ] {
                    map.insert(QString::from(key), QVariant::from(&value));
                }
                QVariant::from(&map)
            })
            .collect();
        let loading = self.queue_state.loading;
        let message = QString::from(self.queue_state.message.as_str());
        self.as_mut().set_queue_tracks(rows);
        self.as_mut().rust_mut().queue_artwork = artwork_urls;
        self.as_mut().set_queue_loading(loading);
        self.set_queue_message(message);
    }
    fn notify_row(mut self: Pin<&mut Self>, row: usize, roles: &[i32]) {
        let Ok(row) = i32::try_from(row) else {
            return;
        };
        let index = self.index(row, 0, &QModelIndex::default());
        let roles = roles.iter().copied().collect();
        self.as_mut().data_changed(&index, &index, &roles);
    }
    fn selection(mut self: Pin<&mut Self>, track: Option<Track>) {
        self.as_mut().rust_mut().selected_cover_id =
            track.as_ref().and_then(|track| track.cover_beatmap_id);
        self.as_mut().rust_mut().selected_track_duration =
            track.as_ref().and_then(|track| track.duration);
        native::protect_artwork(self.cache_epoch, self.selected_cover_id.unwrap_or(-1));
        let artwork = track
            .as_ref()
            .and_then(|t| t.cover_beatmap_id)
            .map(|id| native::artwork_url(self.cache_epoch, id))
            .unwrap_or_default();
        self.as_mut()
            .set_selected_audio_id(track.as_ref().map_or(-1, |t| t.audio_source_id));
        self.as_mut().set_has_selection(track.is_some());
        let detail_pending =
            self.playlist_state.showing_detail() && self.playlist_state.active.is_none();
        self.as_mut().set_selected_available(
            !detail_pending && track.as_ref().is_some_and(|t| t.audio_source_id >= 0),
        );
        self.as_mut()
            .set_can_add_playlist(track.as_ref().is_some_and(|t| !t.difficulties.is_empty()));
        self.as_mut().set_selected_title(QString::from(
            track.as_ref().map_or("", |t| t.title.as_str()),
        ));
        self.as_mut().set_selected_artist(QString::from(
            track.as_ref().map_or("", |t| t.artist.as_str()),
        ));
        self.as_mut().set_selected_subtitle(QString::from(
            track.as_ref().map_or("", |t| t.subtitle.as_str()),
        ));
        self.as_mut().set_selected_duration_label(QString::from(
            track
                .as_ref()
                .map_or_else(|| "--:--".to_owned(), Track::duration_label),
        ));
        self.as_mut().set_selected_artwork_url(artwork);
        self.as_mut().project_playback();
        if let Some(track) = track {
            self.request_media(track.audio_source_id);
        }
    }
    fn update_artwork(mut self: Pin<&mut Self>, affected: &[i32]) {
        for cover in affected {
            let url = native::artwork_url(self.cache_epoch, *cover);
            if url.is_empty() {
                continue;
            }
            let indices = self.cover_rows.get(cover).cloned().unwrap_or_default();
            for index in indices {
                if let Some(row) = self.as_mut().rust_mut().rows.get_mut(index) {
                    row.artwork = url.clone();
                }
                self.as_mut().notify_row(index, &[261]);
            }
        }
        if let Some(cover) = self.selected_cover_id
            && affected.contains(&cover)
        {
            let url = native::artwork_url(self.cache_epoch, cover);
            if !url.is_empty() {
                self.as_mut().set_selected_artwork_url(url);
            }
        }
        self.project_queue();
    }
    fn replace_tracks(mut self: Pin<&mut Self>, tracks: Vec<Track>, invalidate_artwork: bool) {
        self.as_mut().rust_mut().generation = self.generation.wrapping_add(1);
        if invalidate_artwork {
            self.as_mut().rust_mut().cache_epoch = native::reset_artwork();
            self.as_mut().rust_mut().queue_artwork.clear();
            self.as_mut().rust_mut().playlist_artwork_urls.clear();
            native::protect_artwork(self.cache_epoch, self.selected_cover_id.unwrap_or(-1));
        }
        let item_ids: Vec<_> = self
            .playlist_state
            .active
            .as_ref()
            .filter(|_| self.playlist_state.showing_detail())
            .map_or_else(Vec::new, |playlist| {
                playlist.items.iter().map(|item| item.id).collect()
            });
        let identical = !invalidate_artwork
            && tracks.len() == self.rows.len()
            && tracks
                .iter()
                .zip(&self.rows)
                .enumerate()
                .all(|(index, (track, row))| {
                    row.track == *track && row.playlist_item_id == item_ids.get(index).copied()
                });
        if identical {
            self.as_mut().media_view_changed();
            return;
        }
        // SAFETY: matching begin/end bracket the complete row replacement.
        unsafe {
            self.as_mut().begin_reset_model();
        }
        let epoch = self.cache_epoch;
        self.as_mut().rust_mut().rows = tracks
            .into_iter()
            .enumerate()
            .map(|(index, track)| {
                let artwork = track
                    .cover_beatmap_id
                    .map(|id| native::artwork_url(epoch, id))
                    .unwrap_or_default();
                Row {
                    playlist_item_id: item_ids.get(index).copied(),
                    track,
                    artwork,
                }
            })
            .collect();
        self.as_mut().index_rows();
        unsafe {
            self.as_mut().end_reset_model();
        }
        let count = i32::try_from(self.rows.len()).unwrap_or(i32::MAX);
        self.as_mut().set_track_count(count);
        let state = self.playlist_state.clone();
        self.as_mut().project_playlists(&state);
        self.as_mut().project_queue();
        self.media_view_changed();
    }
    fn reorder_tracks(mut self: Pin<&mut Self>, tracks: Vec<Track>) {
        let reset = tracks.len() != self.rows.len();
        if reset {
            // SAFETY: matching begin/end bracket the row-count change.
            unsafe { self.as_mut().begin_reset_model() };
        }
        let mut previous: HashMap<_, _> = std::mem::take(&mut self.as_mut().rust_mut().rows)
            .into_iter()
            .map(|row| ((row.playlist_item_id, row.track.audio_source_id), row))
            .collect();
        let item_ids: Vec<_> = self
            .playlist_state
            .active
            .as_ref()
            .filter(|_| self.playlist_state.showing_detail())
            .map_or_else(Vec::new, |playlist| {
                playlist.items.iter().map(|item| item.id).collect()
            });
        self.as_mut().rust_mut().rows = tracks
            .into_iter()
            .enumerate()
            .map(|(index, mut track)| {
                let playlist_item_id = item_ids.get(index).copied();
                let artwork = previous
                    .remove(&(playlist_item_id, track.audio_source_id))
                    .map_or_else(QString::default, |row| {
                        track.duration = track.duration.or(row.track.duration);
                        row.artwork
                    });
                Row {
                    playlist_item_id,
                    track,
                    artwork,
                }
            })
            .collect();
        self.as_mut().index_rows();
        if reset {
            unsafe { self.as_mut().end_reset_model() };
            let count = i32::try_from(self.rows.len()).unwrap_or(i32::MAX);
            self.set_track_count(count);
        } else if !self.rows.is_empty() {
            let first = self.index(0, 0, &QModelIndex::default());
            let last = self.index(
                self.track_count.saturating_sub(1),
                0,
                &QModelIndex::default(),
            );
            self.data_changed(&first, &last, &ROLES.iter().map(|(id, _)| *id).collect());
        }
    }
    fn index_rows(mut self: Pin<&mut Self>) {
        let mut cover_rows = HashMap::<i32, Vec<usize>>::new();
        for (index, row) in self.rows.iter().enumerate() {
            if let Some(cover) = row.track.cover_beatmap_id {
                cover_rows.entry(cover).or_default().push(index);
            }
        }
        let audio_rows = self
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| (row.track.audio_source_id, index))
            .collect();
        self.as_mut().rust_mut().cover_rows = cover_rows;
        self.as_mut().rust_mut().audio_rows = audio_rows;
    }
    fn apply(mut self: Pin<&mut Self>, update: AppUpdate) {
        match update {
            AppUpdate::Queue(state) => {
                self.as_mut().rust_mut().queue_state = state;
                self.project_queue();
            }
            AppUpdate::AudioSettings(state) => self.project_audio_settings(state),
            AppUpdate::Playlists(state) => self.project_playlists(&state),
            AppUpdate::PlaylistCover {
                id,
                revision,
                bytes,
            } => {
                self.install_playlist_image(id, revision, -1, &bytes);
            }
            AppUpdate::PlaylistArtwork {
                id,
                beatmap_id,
                bytes,
            } => {
                self.install_playlist_image(id, -1, beatmap_id, &bytes);
            }
            AppUpdate::FolderSelection(selection) => self.project_folder_selection(selection),
            AppUpdate::FolderSelectionPickerRequested(epoch) => self.pick_folder(Some(epoch)),
            AppUpdate::Playback(playback) => {
                self.as_mut().rust_mut().playback = playback;
                self.project_playback();
            }
            AppUpdate::Connection(status) => {
                self.as_mut()
                    .set_connected(matches!(status, ConnectionStatus::Connected));
                self.as_mut()
                    .set_connecting(matches!(status, ConnectionStatus::Connecting));
                let text = match status {
                    ConnectionStatus::Disconnected => "Disconnected".to_owned(),
                    ConnectionStatus::Connecting => "Connecting…".to_owned(),
                    ConnectionStatus::Connected => "Connected".to_owned(),
                    ConnectionStatus::Failed(error) => error,
                };
                self.set_connection_status(QString::from(text));
            }
            AppUpdate::LibraryStatus(status) => {
                self.as_mut().set_library_loading(status.loading);
                self.set_library_message(QString::from(status.message));
            }
            AppUpdate::FolderStatus(status) => {
                self.as_mut().set_folders_loading(status.loading);
                self.as_mut().set_folder_busy(status.busy);
                self.as_mut().set_folder_can_retry(status.retry.is_some());
                self.set_folder_message(QString::from(status.message));
            }
            AppUpdate::TracksReplaced {
                tracks,
                invalidate_artwork,
            } => self.replace_tracks(tracks, invalidate_artwork),
            AppUpdate::TracksReordered(tracks) => self.reorder_tracks(tracks),
            AppUpdate::TrackSort(sort) => self.project_track_sort(track_sort_index(sort)),
            AppUpdate::TrackChanged(track) => {
                let indices: Vec<_> = self
                    .rows
                    .iter()
                    .enumerate()
                    .filter_map(|(index, row)| {
                        (row.track.audio_source_id == track.audio_source_id).then_some(index)
                    })
                    .collect();
                for index in indices {
                    let playlist = self.playlist_state.showing_detail();
                    if let Some(row) = self.as_mut().rust_mut().rows.get_mut(index) {
                        if playlist {
                            row.track.duration = track.duration;
                        } else {
                            row.track = track.clone();
                        }
                    }
                    self.as_mut().notify_row(index, &[257, 258, 259, 260]);
                }
            }
            AppUpdate::TrackSelected(track) => self.selection(track),
            AppUpdate::FoldersReplaced(folders) => {
                let folders = folders
                    .into_iter()
                    .map(|folder| {
                        let mut map = QMap::<QMapPair_QString_QVariant>::default();
                        map.insert(QString::from("id"), QVariant::from(&folder.id));
                        map.insert(
                            QString::from("label"),
                            QVariant::from(&QString::from(format!(
                                "{} - {}",
                                folder.kind, folder.root_path
                            ))),
                        );
                        QVariant::from(&map)
                    })
                    .collect();
                self.set_folders(folders);
            }
            AppUpdate::FolderSelected(id) => self.set_selected_folder_id(id.unwrap_or(-1)),
            AppUpdate::FolderPickerRequested => self.pick_folder(None),
            AppUpdate::Artwork { .. } => {} // Dispatched with its acknowledgement guard.
        }
    }
    fn install_playlist_image(
        mut self: Pin<&mut Self>,
        id: i32,
        revision: i64,
        beatmap: i32,
        bytes: &[u8],
    ) {
        let valid = self.playlist_state.playlists.iter().any(|row| {
            row.id == id
                && if revision >= 0 {
                    row.custom_cover_revision == Some(revision)
                } else {
                    row.cover_beatmap_id == Some(beatmap)
                }
        });
        if !valid {
            return;
        }
        if !native::playlist_artwork_url(self.cache_epoch, id, revision, beatmap).is_empty() {
            return;
        }
        let affected: Vec<i32> = native::install_playlist_artwork(
            self.cache_epoch,
            id,
            revision,
            beatmap,
            &QByteArray::from(bytes),
        )
        .iter()
        .copied()
        .collect();
        self.as_mut().update_artwork(&affected);
        let state = self.playlist_state.clone();
        self.project_playlists(&state);
    }
    fn pick_folder(self: Pin<&mut Self>, epoch: Option<u64>) {
        if let (Some(context), Some(controller)) =
            (RuntimeContext::current(), self.controller.clone())
        {
            let task = context.handle.spawn(async move {
                let path = rfd::AsyncFileDialog::new()
                    .set_title("Select osu! installation")
                    .pick_folder()
                    .await
                    .map(|file| file.path().to_path_buf());
                controller.send(match epoch {
                    Some(epoch) => AppCommand::CompleteFolderSelectionPick { epoch, path },
                    None => AppCommand::CompleteFolderPick(path),
                });
            });
            context.track(task);
        }
    }
    fn decode(self: Pin<&mut Self>, mut ack: ArtworkAck, cover_id: i32, bytes: Vec<u8>) {
        let Some(context) = RuntimeContext::current() else {
            return;
        };
        let generation = self.generation;
        let cache_epoch = self.cache_epoch;
        let thread = self.qt_thread();
        let task = context.handle.spawn_blocking(move || {
            let affected: Vec<i32> = if ack.ticket.generation == generation {
                native::install_artwork(cache_epoch, cover_id, &QByteArray::from(bytes.as_slice()))
                    .iter()
                    .copied()
                    .collect()
            } else {
                Vec::new()
            };
            ack.available = !affected.is_empty();
            let _ = thread.queue(move |object| {
                if object.cache_epoch == cache_epoch {
                    object.update_artwork(&affected);
                }
            });
            // Installation has completed before releasing the controller's media slot.
            drop(ack);
        });
        context.track(task);
    }
}

fn format_time(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

pub(crate) const fn track_sort_from_index(index: i32) -> Option<TrackSort> {
    match index {
        0 => Some(TrackSort::TitleAsc),
        1 => Some(TrackSort::ArtistAsc),
        2 => Some(TrackSort::RecentlyPlayed),
        _ => None,
    }
}

pub(crate) const fn track_sort_index(sort: TrackSort) -> i32 {
    match sort {
        TrackSort::TitleAsc => 0,
        TrackSort::ArtistAsc => 1,
        TrackSort::RecentlyPlayed => 2,
    }
}

struct ArtworkAck {
    controller: AppController,
    ticket: MediaTicket,
    available: bool,
}
impl Drop for ArtworkAck {
    fn drop(&mut self) {
        self.controller.send(AppCommand::MediaInstalled {
            ticket: self.ticket,
            available: self.available,
        });
    }
}
impl Drop for AppBridgeRust {
    fn drop(&mut self) {
        native::reset_artwork();
        if let Some(controller) = &self.controller {
            controller.send(AppCommand::Shutdown);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };

    struct Dropped(Arc<AtomicBool>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Relaxed);
        }
    }

    fn playback_projection_keeps_selection_current_track_and_loading_independent() {
        let mut object = native::new_app_bridge();
        let mut bridge = object.pin_mut();
        assert!(bridge.transport_command().is_none());
        assert_eq!(bridge.volume.to_bits(), 1.0_f32.to_bits());
        let mut first = Track::new("A", "Artist", Duration::from_secs(90));
        first.audio_source_id = 7;
        let mut second = Track::new("B", "Artist", Duration::from_secs(120));
        second.audio_source_id = 42;
        bridge.as_mut().selection(Some(first.clone()));
        assert!(matches!(
            bridge.transport_command(),
            Some(AppCommand::PlayTrack(7))
        ));
        let mut playback = Playback {
            current_audio_id: Some(7),
            loading_audio_id: Some(42),
            snapshot: osu_radio_client::playback::Snapshot {
                state: PlayerState::Playing,
                position: Duration::from_secs(23),
                duration: Some(Duration::from_secs(91)),
                volume: 0.5,
            },
            error: None,
            has_source: true,
            can_next: true,
            can_previous: true,
            ..Playback::default()
        };
        bridge.as_mut().apply(AppUpdate::Playback(playback.clone()));
        assert_eq!(bridge.current_audio_id, 7);
        assert_eq!(bridge.loading_audio_id, 42);
        assert!(bridge.selected_is_playing && bridge.can_seek);
        assert_eq!(bridge.playback_position_label, QString::from("00:23"));
        assert_eq!(bridge.selected_duration_label, QString::from("01:31"));
        assert!(matches!(
            bridge.transport_command(),
            Some(AppCommand::Pause)
        ));
        bridge.as_mut().selection(Some(second));
        assert_eq!(bridge.current_audio_id, 7);
        assert!(!bridge.selected_is_playing && !bridge.can_seek);
        assert_eq!(bridge.playback_position_label, QString::from("00:00"));
        assert_eq!(bridge.selected_duration_label, QString::from("02:00"));
        assert_eq!(bridge.volume.to_bits(), 1.0_f32.to_bits());
        bridge.as_mut().apply(AppUpdate::AudioSettings(
            osu_radio_client::controller::AudioSettingsState {
                settings: osu_radio_client::models::AudioSettings {
                    individual_volume_enabled: true,
                    global_volume_percent: 20,
                },
                loaded: true,
                loading: false,
                saving: false,
                error: String::new(),
                selected_audio_id: Some(42),
                selected_volume_percent: 10,
                selected_has_override: true,
            },
        ));
        assert_eq!(bridge.volume.to_bits(), 0.1_f32.to_bits());
        assert!(
            bridge.individual_volume_enabled && bridge.volume_enabled && bridge.volume_has_override
        );
        assert!(matches!(
            bridge.transport_command(),
            Some(AppCommand::PlayTrack(42))
        ));
        playback.error = Some("Download failed".into());
        playback.loading_audio_id = None;
        bridge.as_mut().apply(AppUpdate::Playback(playback.clone()));
        assert_eq!(bridge.playback_message, QString::from("Download failed"));
        assert_eq!(bridge.current_audio_id, 7);
        bridge.as_mut().selection(Some(first));
        for state in [
            PlayerState::Paused,
            PlayerState::Stopped,
            PlayerState::Ended,
        ] {
            playback.snapshot.state = state;
            bridge.as_mut().apply(AppUpdate::Playback(playback.clone()));
            assert!(matches!(
                bridge.transport_command(),
                Some(AppCommand::Resume)
            ));
            assert!(bridge.can_seek);
        }
        bridge.as_mut().apply(AppUpdate::TracksReplaced {
            tracks: Vec::new(),
            invalidate_artwork: true,
        });
        bridge.as_mut().selection(None);
        assert_eq!(bridge.current_audio_id, 7);
        assert!(!bridge.can_seek);
        assert_eq!(bridge.playback_position_label, QString::from("00:00"));
    }

    #[test]
    fn native_model_cache_and_destroyed_object_contracts() {
        assert!(
            native::check_playlist_cover_preparation().is_empty(),
            "{}",
            native::check_playlist_cover_preparation()
        );
        playback_projection_keeps_selection_current_track_and_loading_independent();
        playlist_projection_keeps_duplicate_audio_rows_and_unavailable_selection_distinct();
        track_reordering_retains_selection_media_and_cache_generation();
        warmed_navigation_and_eviction_keep_visible_artwork_urls();
        duplicate_playlist_images_preserve_resident_installations();
        assert!(
            native::check_artwork_cache().is_empty(),
            "{}",
            native::check_artwork_cache()
        );
        let mut object = native::new_app_bridge();
        let mut model = object.pin_mut();
        assert_eq!(model.selected_audio_id, -1);
        assert_eq!(model.selected_folder_id, -1);
        assert_eq!(model.row_count(&QModelIndex::default()), 0);
        let mut track = Track::new("Title", "Artist", Duration::from_secs(91));
        track.audio_source_id = 72;
        track.subtitle = "Artist | Hard".into();
        model.as_mut().apply(AppUpdate::TracksReplaced {
            tracks: vec![track.clone()],
            invalidate_artwork: true,
        });
        assert_eq!(model.row_count(&QModelIndex::default()), 1);
        let index = model.index(0, 0, &QModelIndex::default());
        assert_eq!(model.data(&index, 256), QVariant::from(&72));
        assert_eq!(
            model.data(&index, 259),
            QVariant::from(&QString::from("Artist | Hard"))
        );
        assert_eq!(
            model.data(&index, 260),
            QVariant::from(&QString::from("01:31"))
        );
        assert_eq!(model.row_count(&index), 0);
        assert!(!model.data(&QModelIndex::default(), 257).is_valid());
        model
            .as_mut()
            .apply(AppUpdate::TrackSelected(Some(track.clone())));
        assert_eq!(model.selected_audio_id, 72);
        assert_eq!(model.selected_title, QString::from("Title"));
        track.duration = None;
        model.as_mut().apply(AppUpdate::TrackChanged(track));
        assert_eq!(
            model.data(&index, 260),
            QVariant::from(&QString::from("--:--"))
        );
        model.as_mut().apply(AppUpdate::TracksReplaced {
            tracks: Vec::new(),
            invalidate_artwork: true,
        });
        model.as_mut().apply(AppUpdate::TrackSelected(None));
        assert_eq!(model.track_count, 0);
        assert_eq!(model.selected_audio_id, -1);
        assert!(!model.has_selection);
        let thread = model.qt_thread();
        let dropped = Arc::new(AtomicBool::new(false));
        let guard = Dropped(dropped.clone());
        assert!(thread.queue(move |_| drop(guard)).is_ok());
        drop(object);
        assert!(dropped.load(Ordering::Relaxed));
        let executed = Arc::new(AtomicBool::new(false));
        let flag = executed.clone();
        assert!(
            thread
                .queue(move |_| {
                    flag.store(true, Ordering::Relaxed);
                })
                .is_err()
        );
        assert!(!executed.load(Ordering::Relaxed));
    }

    fn playlist_projection_keeps_duplicate_audio_rows_and_unavailable_selection_distinct() {
        let mut object = native::new_app_bridge();
        let mut model = object.pin_mut();
        let mut mock = osu_radio_client::mock::MockPlaylists::default();
        mock.apply(PlaylistAction::Select(Some(1)));
        let mut view = mock.view;
        let tracks = view.tracks();
        model.as_mut().apply(AppUpdate::Playlists(view.clone()));
        model.as_mut().apply(AppUpdate::TracksReplaced {
            tracks: tracks.clone(),
            invalidate_artwork: true,
        });
        model.as_mut().selection(tracks.first().cloned());
        assert_eq!(model.track_count, 3);
        let first = model.index(0, 0, &QModelIndex::default());
        let second = model.index(1, 0, &QModelIndex::default());
        assert_eq!(model.data(&first, 256), model.data(&second, 256));
        assert_ne!(model.data(&first, 262), model.data(&second, 262));
        let original_subtitle = model.data(&first, 259);
        let mut duration = tracks.get(1).unwrap().clone();
        duration.duration = Some(Duration::from_secs(90));
        model.as_mut().apply(AppUpdate::TrackChanged(duration));
        assert_eq!(model.data(&first, 259), original_subtitle);
        assert_eq!(model.data(&first, 260), model.data(&second, 260));
        let playback = Playback {
            current_audio_id: Some(1),
            current_playlist_item_id: Some(1),
            has_source: true,
            snapshot: osu_radio_client::playback::Snapshot {
                state: PlayerState::Playing,
                duration: Some(Duration::from_secs(90)),
                ..Default::default()
            },
            ..Default::default()
        };
        model.as_mut().apply(AppUpdate::Playback(playback));
        assert!(model.selected_is_playing && model.can_seek);
        view.selected_item_id = Some(2);
        model.as_mut().apply(AppUpdate::Playlists(view.clone()));
        model.as_mut().selection(tracks.get(1).cloned());
        assert!(!model.selected_is_playing && !model.can_seek);
        assert!(matches!(
            model.transport_command(),
            Some(AppCommand::Playlist(PlaylistAction::TogglePlayback))
        ));
        model.as_mut().rust_mut().rows[0].artwork = QString::from("first-cover");
        model.as_mut().rust_mut().rows[1].artwork = QString::from("second-cover");
        view.active.as_mut().unwrap().items.swap(0, 1);
        let reordered = view.tracks();
        model.as_mut().apply(AppUpdate::Playlists(view.clone()));
        model.as_mut().apply(AppUpdate::TracksReordered(reordered));
        assert_eq!(model.selected_playlist_item_id, 2);
        assert!(!model.selected_is_playing && !model.can_seek);
        assert_eq!(model.data(&first, 262), QVariant::from(&2));
        assert_eq!(model.data(&second, 262), QVariant::from(&1));
        assert_eq!(
            model.data(&first, 261),
            QVariant::from(&QString::from("second-cover"))
        );
        assert_eq!(
            model.data(&second, 261),
            QVariant::from(&QString::from("first-cover"))
        );
        assert!(model.rows[0].track.subtitle.contains("Hard"));
        assert!(model.rows[1].track.subtitle.contains("Easy"));
        assert_eq!(model.rows[0].track.duration, Some(Duration::from_secs(90)));
        assert_eq!(model.rows[1].track.duration, Some(Duration::from_secs(90)));
        let mut pending = view.clone();
        pending.loading = true;
        pending.active = None;
        model.as_mut().apply(AppUpdate::Playlists(pending));
        let mut renamed = tracks[0].clone();
        renamed.title = "Unicode title".to_owned();
        model
            .as_mut()
            .apply(AppUpdate::TrackSelected(Some(renamed)));
        assert_eq!(model.selected_title, QString::from("Unicode title"));
        assert!(model.has_selection && !model.selected_available);
        assert!(model.transport_command().is_none());
        view.selected_item_id = Some(3);
        model.as_mut().apply(AppUpdate::Playlists(view));
        model.as_mut().selection(tracks.get(2).cloned());
        assert!(model.has_selection && !model.selected_available);
        assert!(model.transport_command().is_none());
        assert!(model.selected_subtitle.to_string().contains("Unavailable"));
    }

    fn track_reordering_retains_selection_media_and_cache_generation() {
        let mut object = native::new_app_bridge();
        let mut model = object.pin_mut();
        let mut first = Track::new("A", "Z", Duration::from_secs(91));
        first.audio_source_id = 7;
        first.cover_beatmap_id = Some(17);
        let mut second = Track::new("B", "A", Duration::from_secs(125));
        second.audio_source_id = 42;
        second.cover_beatmap_id = Some(52);
        model
            .as_mut()
            .replace_tracks(vec![first.clone(), second.clone()], true);
        model.as_mut().selection(Some(first.clone()));
        let epoch = model.cache_epoch;
        let generation = model.generation;
        let ppm = QByteArray::from(b"P6\n1 1\n255\n\x50\x64\x78".as_slice());
        assert!(!native::install_artwork(epoch, 17, &ppm).is_empty());
        assert!(!native::install_artwork(epoch, 52, &ppm).is_empty());
        model.as_mut().update_artwork(&[17, 52]);
        let selected_artwork = model.selected_artwork_url.clone();
        let first_artwork = model.rows[0].artwork.clone();
        let second_artwork = model.rows[1].artwork.clone();
        first.duration = None;
        second.duration = None;
        model
            .as_mut()
            .apply(AppUpdate::TrackSort(TrackSort::ArtistAsc));
        model
            .as_mut()
            .apply(AppUpdate::TracksReordered(vec![second, first]));
        assert_eq!(model.track_sort_index, 1);
        assert_eq!(model.selected_audio_id, 7);
        assert_eq!(model.selected_title, QString::from("A"));
        assert_eq!(model.selected_duration_label, QString::from("01:31"));
        assert_eq!(model.selected_artwork_url, selected_artwork);
        assert_eq!(model.cache_epoch, epoch);
        assert_eq!(model.generation, generation);
        assert_eq!(model.rows[0].track.audio_source_id, 42);
        assert_eq!(model.rows[0].track.duration, Some(Duration::from_secs(125)));
        assert_eq!(model.rows[0].artwork, second_artwork);
        assert_eq!(model.rows[1].artwork, first_artwork);
        assert_eq!(model.audio_rows.get(&7), Some(&1));
        assert_eq!(model.cover_rows.get(&17), Some(&vec![1]));
        assert_eq!(native::artwork_url(epoch, 17), selected_artwork);
        assert!(!native::install_artwork(epoch, 17, &ppm).is_empty());
        for (index, sort) in [
            (0, TrackSort::TitleAsc),
            (1, TrackSort::ArtistAsc),
            (2, TrackSort::RecentlyPlayed),
        ] {
            assert_eq!(track_sort_from_index(index), Some(sort));
            assert_eq!(track_sort_index(sort), index);
        }
        assert!(track_sort_from_index(-1).is_none());
        assert!(track_sort_from_index(3).is_none());
    }
    fn warmed_navigation_and_eviction_keep_visible_artwork_urls() {
        let mut object = native::new_app_bridge();
        let mut model = object.pin_mut();
        let mut selected = Track::new("Selected", "Artist", Duration::from_secs(42));
        selected.audio_source_id = 7;
        selected.cover_beatmap_id = Some(17);
        let mut other = Track::new("Other", "Artist", Duration::from_secs(90));
        other.audio_source_id = 42;
        other.cover_beatmap_id = Some(52);
        let tracks = vec![selected.clone(), other.clone()];
        model.as_mut().replace_tracks(tracks.clone(), true);
        model.as_mut().selection(Some(selected.clone()));
        let epoch = model.cache_epoch;
        let ppm = QByteArray::from(b"P6\n1 1\n255\n\x50\x64\x78".as_slice());
        assert!(!native::install_artwork(epoch, 17, &ppm).is_empty());
        assert!(!native::install_artwork(epoch, 52, &ppm).is_empty());
        model.as_mut().update_artwork(&[17, 52]);
        let selected_url = model.selected_artwork_url.clone();
        let generation = model.generation;
        let row_url = model.rows[1].artwork.clone();
        model.as_mut().replace_tracks(tracks.clone(), false);
        assert_eq!(model.cache_epoch, epoch);
        assert_eq!(model.generation, generation.wrapping_add(1));
        assert_eq!(model.selected_artwork_url, selected_url);
        assert_eq!(model.rows[1].artwork, row_url);
        model.as_mut().apply(AppUpdate::Playlists(PlaylistsState {
            visible: true,
            active_id: Some(1),
            ..PlaylistsState::default()
        }));
        model.as_mut().replace_tracks(Vec::new(), false);
        assert_eq!(
            model.selected_artwork_url, selected_url,
            "pending detail does not blank player/backdrop"
        );
        assert!(!model.selected_available && model.transport_command().is_none());
        model
            .as_mut()
            .apply(AppUpdate::Playlists(PlaylistsState::default()));
        model.as_mut().replace_tracks(tracks.clone(), false);
        model.as_mut().selection(Some(selected.clone()));
        assert_eq!(model.rows[0].artwork, selected_url);
        assert_eq!(model.selected_artwork_url, selected_url);
        model.as_mut().replace_tracks(tracks, true);
        assert_ne!(model.cache_epoch, epoch);
        assert!(
            native::install_artwork(epoch, 17, &ppm).is_empty(),
            "true invalidation rejects old decode"
        );
        let epoch = model.cache_epoch;
        let mut pixels = b"P6\n1280 1280\n255\n".to_vec();
        pixels.resize(pixels.len() + 1280 * 1280 * 3, 100);
        let large = QByteArray::from(pixels.as_slice());
        assert!(!native::install_artwork(epoch, 17, &large).is_empty());
        assert!(!native::install_artwork(epoch, 52, &large).is_empty());
        model.as_mut().update_artwork(&[17, 52]);
        model.as_mut().selection(Some(selected));
        let pinned_url = model.selected_artwork_url.clone();
        let displayed_url = model.rows[1].artwork.clone();
        model.as_mut().rust_mut().queue_state.tracks = vec![other.clone(), other];
        model.as_mut().project_queue();
        let displayed_queue = model.queue_tracks.clone();
        assert_eq!(displayed_queue.0.len(), 2);
        assert_eq!(displayed_queue.0.get(0), displayed_queue.0.get(1));
        for id in 100..115 {
            let affected = native::install_artwork(epoch, id, &large);
            assert_eq!(affected.len(), 1, "eviction cannot clear displayed rows");
        }
        assert!(native::artwork_url(epoch, 52).is_empty());
        assert_eq!(native::artwork_url(epoch, 17), pinned_url);
        model.as_mut().update_artwork(&[52]); // Its queued publish arrives after a different decoder evicted it.
        assert_eq!(model.rows[1].artwork, displayed_url);
        assert_eq!(model.selected_artwork_url, pinned_url);
        for _ in 0..2 {
            model.as_mut().project_queue();
            assert!(
                model.queue_tracks == displayed_queue,
                "both duplicate queue rows retain shown artwork"
            );
        }
        assert!(!native::install_artwork(epoch, 52, &large).is_empty());
        model.as_mut().update_artwork(&[52]);
        assert_ne!(
            model.rows[1].artwork, displayed_url,
            "reentry uses a new installation version"
        );
        assert!(!model.rows[1].artwork.is_empty());
    }
    fn duplicate_playlist_images_preserve_resident_installations() {
        let mut object = native::new_app_bridge();
        let mut model = object.pin_mut();
        model.as_mut().replace_tracks(Vec::new(), true);
        let epoch = model.cache_epoch;
        let mut playlists = PlaylistsState {
            playlists: vec![osu_radio_client::models::PlaylistSummary {
                id: 11,
                name: "Resident cover".into(),
                item_count: 1,
                cover_beatmap_id: Some(17),
                custom_cover_revision: Some(1),
            }],
            ..PlaylistsState::default()
        };
        model
            .as_mut()
            .apply(AppUpdate::Playlists(playlists.clone()));
        let delivery = AppUpdate::PlaylistCover {
            id: 11,
            revision: 1,
            bytes: b"P6\n1 1\n255\n\x50\x64\x78".to_vec(),
        };
        model.as_mut().apply(delivery.clone());
        let resident = native::playlist_artwork_url(epoch, 11, 1, -1);
        assert!(!resident.is_empty());
        model.as_mut().apply(delivery);
        assert_eq!(
            native::playlist_artwork_url(epoch, 11, 1, -1),
            resident,
            "duplicate delivery preserves a resident cover installation"
        );
        playlists
            .playlists
            .first_mut()
            .unwrap()
            .custom_cover_revision = Some(2);
        model.as_mut().apply(AppUpdate::Playlists(playlists));
        model.as_mut().apply(AppUpdate::PlaylistCover {
            id: 11,
            revision: 2,
            bytes: b"P6\n1 1\n255\n\x50\x64\x78".to_vec(),
        });
        let revised = native::playlist_artwork_url(epoch, 11, 2, -1);
        assert!(
            !revised.is_empty() && revised != resident,
            "a new revision installs a distinct cover key"
        );
        let automatic = AppUpdate::PlaylistArtwork {
            id: 11,
            beatmap_id: 17,
            bytes: b"P6\n1 1\n255\n\x50\x64\x78".to_vec(),
        };
        model.as_mut().apply(automatic.clone());
        let automatic_url = native::playlist_artwork_url(epoch, 11, -1, 17);
        assert!(!automatic_url.is_empty());
        model.as_mut().apply(automatic);
        assert_eq!(
            native::playlist_artwork_url(epoch, 11, -1, 17),
            automatic_url
        );
    }
}
