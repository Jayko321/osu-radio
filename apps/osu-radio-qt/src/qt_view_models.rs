//! Typed domain adapters. Each object owns its Qt presentation and command sink.
//! Typed Qt projection; workflows remain in osu-radio-client.
use crate::app_bridge::{VariantList, track_sort_from_index};
use crate::runtime::{RuntimeContext, ffi as native};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{
    QByteArray, QHash, QHashPair_i32_QByteArray, QMap, QMapPair_QString_QVariant, QModelIndex,
    QString, QVariant,
};
use osu_radio_client::{
    AudioModsState, ModsAction, ModsEdit, ModsEditTarget, Track, TrackNamePreferences,
    controller::{AppCommand, AppController, OperationStatus},
    models::{AudioModsMode, QueueAvailability, QueueEntryId, QueueItemRef, QueuePlacement},
    view_models::{
        FolderAction, FolderSelection, PlaylistAction, PlaylistTrack, PlaylistsState, QueueAction,
        QueueView,
    },
};
use std::{collections::HashMap, pin::Pin, time::Duration};

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
        #[qproperty(i32, track_sort_index, cxx_name = "trackSortIndex", READ, NOTIFY)]
        #[qproperty(QString, library_message, cxx_name = "libraryMessage", READ, NOTIFY)]
        #[qproperty(bool, library_loading, cxx_name = "libraryLoading", READ, NOTIFY)]
        #[qproperty(*mut TrackListModel, tracks, READ = tracks_pointer, CONSTANT)]
        #[qproperty(i32, selected_audio_id, cxx_name = "selectedAudioId", READ, NOTIFY)]
        type LibraryViewModel = super::LibraryViewModelRust;
        fn tracks_pointer(self: &LibraryViewModel) -> *mut TrackListModel;
        #[qinvokable]
        #[cxx_name = "searchLibrary"]
        fn search_library(self: Pin<&mut LibraryViewModel>, query: &QString);
        #[qinvokable]
        #[cxx_name = "setTrackSort"]
        fn set_track_sort(self: Pin<&mut LibraryViewModel>, index: i32);
        #[qinvokable]
        #[cxx_name = "refreshLibrary"]
        fn refresh_library(self: Pin<&mut LibraryViewModel>);
        #[qinvokable]
        #[cxx_name = "selectTrack"]
        fn select_track(self: Pin<&mut LibraryViewModel>, id: i32);
        #[qinvokable]
        #[cxx_name = "insertTrack"]
        fn insert_track(self: Pin<&mut LibraryViewModel>, id: i32, next: bool);
        #[qobject]
        #[qml_element]
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
        #[qproperty(*mut TrackListModel, tracks, READ = tracks_pointer, CONSTANT)]
        #[qproperty(i32, track_sort_index, cxx_name = "trackSortIndex", READ, NOTIFY)]
        type PlaylistsViewModel = super::PlaylistsViewModelRust;
        fn tracks_pointer(self: &PlaylistsViewModel) -> *mut TrackListModel;
        #[qinvokable]
        #[cxx_name = "setTrackSort"]
        fn set_track_sort(self: Pin<&mut PlaylistsViewModel>, index: i32);
        #[qinvokable]
        #[cxx_name = "showLibrary"]
        fn show_library(self: Pin<&mut PlaylistsViewModel>);
        #[qinvokable]
        #[cxx_name = "showPlaylists"]
        fn show_playlists(self: Pin<&mut PlaylistsViewModel>);
        #[qinvokable]
        #[cxx_name = "searchPlaylists"]
        fn search_playlists(self: Pin<&mut PlaylistsViewModel>, query: &QString);
        #[qinvokable]
        #[cxx_name = "beginPlaylistCreate"]
        fn begin_playlist_create(self: Pin<&mut PlaylistsViewModel>);
        #[qinvokable]
        #[cxx_name = "beginPlaylistEdit"]
        fn begin_playlist_edit(self: Pin<&mut PlaylistsViewModel>, id: i32);
        #[qinvokable]
        #[cxx_name = "setPlaylistEditorName"]
        fn edit_playlist_name(self: Pin<&mut PlaylistsViewModel>, name: &QString);
        #[qinvokable]
        #[cxx_name = "cancelPlaylistEditor"]
        fn cancel_playlist_editor(self: Pin<&mut PlaylistsViewModel>);
        #[qinvokable]
        #[cxx_name = "savePlaylistEditor"]
        fn save_playlist_editor(self: Pin<&mut PlaylistsViewModel>);
        #[qinvokable]
        #[cxx_name = "resetPlaylistCover"]
        fn reset_playlist_cover(self: Pin<&mut PlaylistsViewModel>);
        #[qinvokable]
        #[cxx_name = "playPlaylistFrom"]
        fn play_playlist_from(self: Pin<&mut PlaylistsViewModel>, id: i32);
        #[qinvokable]
        #[cxx_name = "choosePlaylistCover"]
        fn choose_playlist_cover(self: Pin<&mut PlaylistsViewModel>);
        #[qinvokable]
        #[cxx_name = "completePlaylistCoverPick"]
        fn complete_playlist_cover_pick(
            self: Pin<&mut PlaylistsViewModel>,
            epoch: u64,
            url: &QString,
        );
        #[qinvokable]
        #[cxx_name = "requestPlaylistArtwork"]
        fn request_playlist_artwork(self: Pin<&mut PlaylistsViewModel>, id: i32);
        #[qinvokable]
        #[cxx_name = "openPlaylists"]
        fn open_playlists(self: Pin<&mut PlaylistsViewModel>);
        #[qinvokable]
        #[cxx_name = "closePlaylists"]
        fn close_playlists(self: Pin<&mut PlaylistsViewModel>);
        #[qinvokable]
        #[cxx_name = "refreshPlaylists"]
        fn refresh_playlists(self: Pin<&mut PlaylistsViewModel>);
        #[qinvokable]
        #[cxx_name = "createPlaylist"]
        fn create_playlist(self: Pin<&mut PlaylistsViewModel>, name: &QString);
        #[qinvokable]
        #[cxx_name = "renamePlaylist"]
        fn rename_playlist(self: Pin<&mut PlaylistsViewModel>, id: i32, name: &QString);
        #[qinvokable]
        #[cxx_name = "deletePlaylist"]
        fn delete_playlist(self: Pin<&mut PlaylistsViewModel>, id: i32);
        #[qinvokable]
        #[cxx_name = "addPlaylistToQueue"]
        fn add_playlist_to_queue(self: Pin<&mut PlaylistsViewModel>, id: i32);
        #[qinvokable]
        #[cxx_name = "insertPlaylistItem"]
        fn insert_playlist_item(self: Pin<&mut PlaylistsViewModel>, item_id: i32, next: bool);
        #[qinvokable]
        #[cxx_name = "selectPlaylist"]
        fn select_playlist(self: Pin<&mut PlaylistsViewModel>, id: i32);
        #[qinvokable]
        #[cxx_name = "selectPlaylistItem"]
        fn select_playlist_item(self: Pin<&mut PlaylistsViewModel>, id: i32);
        #[qinvokable]
        #[cxx_name = "playPlaylist"]
        fn play_playlist(self: Pin<&mut PlaylistsViewModel>);
        #[qinvokable]
        #[cxx_name = "openPlaylistAdd"]
        fn open_playlist_add(self: Pin<&mut PlaylistsViewModel>);
        #[qinvokable]
        #[cxx_name = "choosePlaylistTarget"]
        fn choose_playlist_target(self: Pin<&mut PlaylistsViewModel>, id: i32);
        #[qinvokable]
        #[cxx_name = "togglePlaylistDifficulty"]
        fn toggle_playlist_difficulty(self: Pin<&mut PlaylistsViewModel>, id: i32);
        #[qinvokable]
        #[cxx_name = "addPlaylistItems"]
        fn add_playlist_items(self: Pin<&mut PlaylistsViewModel>);
        #[qinvokable]
        #[cxx_name = "removePlaylistItem"]
        fn remove_playlist_item(self: Pin<&mut PlaylistsViewModel>, id: i32);
        #[qsignal]
        #[cxx_name = "playlistCoverPickerRequested"]
        fn playlist_cover_picker_requested(self: Pin<&mut PlaylistsViewModel>, epoch: u64);
        #[qobject]
        #[qml_element]
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
        #[qproperty(QString, folder_message, cxx_name = "folderMessage", READ, NOTIFY)]
        #[qproperty(bool, folders_loading, cxx_name = "foldersLoading", READ, NOTIFY)]
        #[qproperty(bool, folder_busy, cxx_name = "folderBusy", READ, NOTIFY)]
        #[qproperty(bool, folder_can_retry, cxx_name = "folderCanRetry", READ, NOTIFY)]
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
            individual_mods_enabled,
            cxx_name = "individualModsEnabled",
            READ,
            NOTIFY
        )]
        #[qproperty(bool, mods_loaded, cxx_name = "modsLoaded", READ, NOTIFY)]
        #[qproperty(bool, mods_available, cxx_name = "modsAvailable", READ, NOTIFY)]
        #[qproperty(QString, mods_mode, cxx_name = "modsMode", READ, NOTIFY)]
        #[qproperty(i32, mods_speed_percent, cxx_name = "modsSpeedPercent", READ, NOTIFY)]
        #[qproperty(bool, mods_adjust_pitch, cxx_name = "modsAdjustPitch", READ, NOTIFY)]
        #[qproperty(bool, mods_saving, cxx_name = "modsSaving", READ, NOTIFY)]
        #[qproperty(QString, mods_message, cxx_name = "modsMessage", READ, NOTIFY)]
        type SettingsViewModel = super::SettingsViewModelRust;
        #[qinvokable]
        #[cxx_name = "setDiscordRichPresence"]
        fn change_discord_rich_presence(self: Pin<&mut SettingsViewModel>, enabled: bool);
        #[qinvokable]
        #[cxx_name = "setUseUnicodeTitles"]
        fn change_unicode_titles(self: Pin<&mut SettingsViewModel>, enabled: bool);
        #[qinvokable]
        #[cxx_name = "setUseUnicodeArtists"]
        fn change_unicode_artists(self: Pin<&mut SettingsViewModel>, enabled: bool);
        #[qinvokable]
        #[cxx_name = "refreshFolders"]
        fn refresh_folders(self: Pin<&mut SettingsViewModel>);
        #[qinvokable]
        #[cxx_name = "changeVolume"]
        fn change_volume(self: Pin<&mut SettingsViewModel>, volume: f32);
        #[qinvokable]
        #[cxx_name = "setIndividualVolumeEnabled"]
        fn change_individual_volume(self: Pin<&mut SettingsViewModel>, enabled: bool);
        #[qinvokable]
        #[cxx_name = "setGlobalVolume"]
        fn change_global_volume(self: Pin<&mut SettingsViewModel>, percent: i32);
        #[qinvokable]
        #[cxx_name = "changeTrackVolume"]
        fn change_track_volume(self: Pin<&mut SettingsViewModel>, id: i32, percent: i32);
        #[qinvokable]
        #[cxx_name = "useGlobalVolume"]
        fn use_global_volume(self: Pin<&mut SettingsViewModel>);
        #[qinvokable]
        #[cxx_name = "retryAudioSettings"]
        fn retry_audio_settings(self: Pin<&mut SettingsViewModel>);
        #[qinvokable]
        #[cxx_name = "setIndividualModsEnabled"]
        fn change_individual_mods(self: Pin<&mut SettingsViewModel>, enabled: bool);
        #[qinvokable]
        #[cxx_name = "beginModsEdit"]
        fn begin_mods_edit(self: Pin<&mut SettingsViewModel>);
        #[qinvokable]
        #[cxx_name = "finishModsEdit"]
        fn finish_mods_edit(self: Pin<&mut SettingsViewModel>);
        #[qinvokable]
        #[cxx_name = "cycleDtMod"]
        fn cycle_dt_mod(self: Pin<&mut SettingsViewModel>);
        #[qinvokable]
        #[cxx_name = "toggleHtMod"]
        fn toggle_ht_mod(self: Pin<&mut SettingsViewModel>);
        #[qinvokable]
        #[cxx_name = "setModSpeed"]
        fn set_mod_speed(self: Pin<&mut SettingsViewModel>, percent: i32);
        #[qinvokable]
        #[cxx_name = "setModAdjustPitch"]
        fn set_mod_adjust_pitch(self: Pin<&mut SettingsViewModel>, adjust: bool);
        #[qinvokable]
        #[cxx_name = "clearMods"]
        fn clear_mods(self: Pin<&mut SettingsViewModel>);
        #[qinvokable]
        #[cxx_name = "retryMods"]
        fn retry_mods(self: Pin<&mut SettingsViewModel>);
        #[qinvokable]
        #[cxx_name = "selectFolder"]
        fn select_folder(self: Pin<&mut SettingsViewModel>, id: i32);
        #[qinvokable]
        #[cxx_name = "addFolder"]
        fn add_folder(self: Pin<&mut SettingsViewModel>);
        #[qinvokable]
        #[cxx_name = "closeFolderSelection"]
        fn close_folder_selection(self: Pin<&mut SettingsViewModel>);
        #[qinvokable]
        #[cxx_name = "browseFolderSelection"]
        fn browse_folder_selection(self: Pin<&mut SettingsViewModel>);
        #[qinvokable]
        #[cxx_name = "toggleFolderSelection"]
        fn toggle_folder_selection(self: Pin<&mut SettingsViewModel>, path: &QString);
        #[qinvokable]
        #[cxx_name = "refreshFolderSelection"]
        fn refresh_folder_selection(self: Pin<&mut SettingsViewModel>, path: &QString);
        #[qinvokable]
        #[cxx_name = "retryFolderCount"]
        fn retry_folder_count(self: Pin<&mut SettingsViewModel>, path: &QString);
        #[qinvokable]
        #[cxx_name = "applyFolderSelection"]
        fn apply_folder_selection(self: Pin<&mut SettingsViewModel>);
        #[qinvokable]
        #[cxx_name = "retryFolders"]
        fn retry_folders(self: Pin<&mut SettingsViewModel>);
        #[qobject]
        #[qml_element]
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
        #[qproperty(bool, selected_available, cxx_name = "selectedAvailable", READ, NOTIFY)]
        #[qproperty(bool, can_add_playlist, cxx_name = "canAddPlaylist", READ, NOTIFY)]
        #[qproperty(bool, can_play, cxx_name = "canPlay", READ, NOTIFY)]
        #[qproperty(
            i32,
            playback_speed_percent,
            cxx_name = "playbackSpeedPercent",
            READ,
            NOTIFY
        )]
        #[qproperty(u64, launch_token, cxx_name = "launchToken", READ, NOTIFY)]
        #[qproperty(
            i32,
            selected_playlist_item_id,
            cxx_name = "selectedPlaylistItemId",
            READ,
            NOTIFY
        )]
        type PlayerViewModel = super::PlayerViewModelRust;
        #[qinvokable]
        #[cxx_name = "togglePlayback"]
        fn toggle_playback(self: Pin<&mut PlayerViewModel>);
        #[qinvokable]
        #[cxx_name = "nextTrack"]
        fn next_track(self: Pin<&mut PlayerViewModel>);
        #[qinvokable]
        #[cxx_name = "previousTrack"]
        fn previous_track(self: Pin<&mut PlayerViewModel>);
        #[qinvokable]
        #[cxx_name = "seekPlayback"]
        fn seek_playback(self: Pin<&mut PlayerViewModel>, seconds: f64);
        #[qobject]
        #[qml_element]
        #[qproperty(*mut TrackListModel, tracks, READ = tracks_pointer, CONSTANT)]
        #[qproperty(i32, start_index, cxx_name = "startIndex", READ, NOTIFY)]
        #[qproperty(bool, queue_loading, cxx_name = "queueLoading", READ, NOTIFY)]
        #[qproperty(bool, queue_busy, cxx_name = "queueBusy", READ, NOTIFY)]
        #[qproperty(QString, queue_message, cxx_name = "queueMessage", READ, NOTIFY)]
        type QueueViewModel = super::QueueViewModelRust;
        fn tracks_pointer(self: &QueueViewModel) -> *mut TrackListModel;
        #[qinvokable]
        #[cxx_name = "playEntry"]
        fn play_entry(self: Pin<&mut QueueViewModel>, id: &QString);
        #[qinvokable]
        #[cxx_name = "removeEntry"]
        fn remove_entry(self: Pin<&mut QueueViewModel>, id: &QString);
        #[qinvokable]
        #[cxx_name = "moveEntry"]
        fn move_entry(self: Pin<&mut QueueViewModel>, id: &QString, before: &QVariant);
        #[qinvokable]
        #[cxx_name = "clearUpcoming"]
        fn clear_upcoming(self: Pin<&mut QueueViewModel>);
        #[qinvokable]
        #[cxx_name = "clearAll"]
        fn clear_all(self: Pin<&mut QueueViewModel>);
        #[qinvokable]
        #[cxx_name = "setVisibleQueueMedia"]
        fn set_visible_queue_media(self: Pin<&mut QueueViewModel>, ids: &QVariantList);
        #[qinvokable]
        #[cxx_name = "setQueueVisible"]
        fn set_queue_visible(self: Pin<&mut QueueViewModel>, visible: bool);
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        #[qproperty(i32, track_count, cxx_name = "trackCount", READ, NOTIFY)]
        type TrackListModel = super::TrackListModelRust;
        #[qinvokable]
        #[cxx_name = "entryIdAt"]
        fn entry_id_at(self: &TrackListModel, row: i32) -> QString;
        #[qinvokable]
        #[cxx_override]
        fn data(self: &TrackListModel, index: &QModelIndex, role: i32) -> QVariant;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &TrackListModel, parent: &QModelIndex) -> i32;
        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &TrackListModel) -> QHash_i32_QByteArray;
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows(
            self: Pin<&mut TrackListModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut TrackListModel>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        unsafe fn begin_remove_rows(
            self: Pin<&mut TrackListModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        unsafe fn end_remove_rows(self: Pin<&mut TrackListModel>);
        #[inherit]
        #[cxx_name = "beginMoveRows"]
        unsafe fn begin_move_rows(
            self: Pin<&mut TrackListModel>,
            source: &QModelIndex,
            first: i32,
            last: i32,
            destination: &QModelIndex,
            before: i32,
        ) -> bool;
        #[inherit]
        #[cxx_name = "endMoveRows"]
        unsafe fn end_move_rows(self: Pin<&mut TrackListModel>);
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut TrackListModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut TrackListModel>);
    }
    unsafe extern "RustQt" {
        #[inherit]
        fn index(self: &TrackListModel, row: i32, column: i32, parent: &QModelIndex)
        -> QModelIndex;
        #[inherit]
        #[qsignal]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut TrackListModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QVector_i32,
        );
    }
    impl cxx_qt::Threading for PlaylistsViewModel {}
}
macro_rules! property_setter {
    ($setter:ident, $field:ident, $notify:ident, $ty:ty) => {
        // Notify on exact property changes, including directly projected engine floats.
        #[allow(clippy::float_cmp)]
        pub(crate) fn $setter(mut self: Pin<&mut Self>, value: $ty) {
            if self.rust().$field != value {
                self.as_mut().rust_mut().$field = value;
                self.$notify();
            }
        }
    };
}
#[allow(clippy::struct_excessive_bools)]
pub struct LibraryViewModelRust {
    pub(crate) selected_audio_id: i32,
    pub(crate) track_sort_index: i32,
    pub(crate) library_message: QString,
    pub(crate) library_loading: bool,
    pub(crate) controller: Option<AppController>,
    pub(crate) model: cxx::UniquePtr<ffi::TrackListModel>,
}
impl Default for LibraryViewModelRust {
    fn default() -> Self {
        Self {
            selected_audio_id: -1,
            track_sort_index: 0,
            library_message: QString::default(),
            library_loading: Default::default(),
            controller: None,
            model: native::new_track_list_model(),
        }
    }
}
impl ffi::LibraryViewModel {
    pub(crate) fn replace_tracks(
        self: Pin<&mut Self>,
        tracks: Vec<Track>,
        epoch: u64,
        invalidate: bool,
    ) {
        self.rust_mut().model.pin_mut().replace_rows(
            tracks.into_iter().map(|track| (None, track)).collect(),
            epoch,
            invalidate,
        );
    }
    pub(crate) fn reorder_tracks(self: Pin<&mut Self>, tracks: Vec<Track>) {
        self.rust_mut()
            .model
            .pin_mut()
            .reorder_rows(tracks.into_iter().map(|track| (None, track)).collect());
    }
    pub(crate) fn change_track(self: Pin<&mut Self>, track: &Track) {
        self.rust_mut().model.pin_mut().change_track(None, track);
    }
    pub(crate) fn project_status(mut self: Pin<&mut Self>, status: OperationStatus) {
        self.as_mut().set_library_loading(status.loading);
        self.set_library_message(QString::from(status.message));
    }

    property_setter!(
        set_selected_audio_id,
        selected_audio_id,
        selected_audio_id_changed,
        i32
    );
    pub(crate) fn command(&self, command: AppCommand) {
        if let Some(controller) = &self.controller {
            controller.send(command);
        }
    }
    pub fn tracks_pointer(&self) -> *mut ffi::TrackListModel {
        std::ptr::from_ref(&*self.model).cast_mut()
    }
    property_setter!(
        project_track_sort,
        track_sort_index,
        track_sort_index_changed,
        i32
    );
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
    pub fn select_track(self: Pin<&mut Self>, id: i32) {
        self.command(AppCommand::SelectTrack((id >= 0).then_some(id)));
    }
    pub fn insert_track(self: Pin<&mut Self>, id: i32, next: bool) {
        if id > 0 {
            self.command(insert_item(
                QueueItemRef::Audio {
                    audio_source_id: id,
                },
                next,
            ));
        }
    }
}
#[allow(clippy::struct_excessive_bools)]
pub struct PlaylistsViewModelRust {
    pub(crate) track_sort_index: i32,
    pub(crate) playlist_state: PlaylistsState,
    pub(crate) playlist_cover_preparing: bool,
    pub(crate) playlist_editor_epoch: u64,
    pub(crate) cover_pick_serial: u64,
    pub(crate) draft_serial: u64,
    pub(crate) playlist_visible: bool,
    pub(crate) playlist_query: QString,
    pub(crate) playlist_editor_open: bool,
    pub(crate) playlist_editor_name: QString,
    pub(crate) playlist_editor_id: i32,
    pub(crate) playlist_editor_artwork_url: QString,
    pub(crate) filtered_playlists: VariantList,
    pub(crate) playlists: VariantList,
    pub(crate) playlist_candidates: VariantList,
    pub(crate) playlist_open: bool,
    pub(crate) playlist_adding: bool,
    pub(crate) playlist_busy: bool,
    pub(crate) playlist_loading: bool,
    pub(crate) playlist_message: QString,
    pub(crate) active_playlist_id: i32,
    pub(crate) active_playlist_name: QString,
    pub(crate) selected_playlist_item_id: i32,
    pub(crate) playlist_target_id: i32,
    pub(crate) playlist_artwork_urls: HashMap<(i32, Option<i64>, Option<i32>), QString>,
    pub(crate) controller: Option<AppController>,
    pub(crate) cache_epoch: u64,
    pub(crate) model: cxx::UniquePtr<ffi::TrackListModel>,
}
impl Default for PlaylistsViewModelRust {
    fn default() -> Self {
        Self {
            track_sort_index: 0,
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
            playlist_artwork_urls: HashMap::new(),
            controller: None,
            cache_epoch: 0,
            model: native::new_track_list_model(),
        }
    }
}
impl ffi::PlaylistsViewModel {
    pub(crate) fn replace_tracks(
        self: Pin<&mut Self>,
        tracks: Vec<PlaylistTrack>,
        epoch: u64,
        invalidate: bool,
    ) {
        self.rust_mut().model.pin_mut().replace_rows(
            tracks
                .into_iter()
                .map(|row| (Some(row.item_id), row.track))
                .collect(),
            epoch,
            invalidate,
        );
    }
    pub(crate) fn reorder_tracks(self: Pin<&mut Self>, tracks: Vec<PlaylistTrack>) {
        self.rust_mut().model.pin_mut().reorder_rows(
            tracks
                .into_iter()
                .map(|row| (Some(row.item_id), row.track))
                .collect(),
        );
    }
    pub(crate) fn change_track(self: Pin<&mut Self>, row: &PlaylistTrack) {
        self.rust_mut()
            .model
            .pin_mut()
            .change_track(Some(row.item_id), &row.track);
    }

    property_setter!(
        project_track_sort,
        track_sort_index,
        track_sort_index_changed,
        i32
    );
    pub fn set_track_sort(self: Pin<&mut Self>, index: i32) {
        if let Some(sort) = track_sort_from_index(index) {
            self.command(AppCommand::SetTrackSort(sort));
        }
    }

    pub(crate) fn project_playlists(mut self: Pin<&mut Self>, state: &PlaylistsState) {
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
        self.set_playlist_open(open);
    }
    pub(crate) fn project_playlist_editor(mut self: Pin<&mut Self>, state: &PlaylistsState) {
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
                let _ = affected;
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
    pub(crate) fn playlist_artwork(
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
    pub(crate) fn command(&self, command: AppCommand) {
        if let Some(controller) = &self.controller {
            controller.send(command);
        }
    }
    pub fn tracks_pointer(&self) -> *mut ffi::TrackListModel {
        std::ptr::from_ref(&*self.model).cast_mut()
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
    pub(crate) fn playlist_command(&self, action: PlaylistAction) {
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
            if let Some(revision) = row.custom_cover_revision {
                self.playlist_command(PlaylistAction::RequestCover { id, revision });
            } else if let Some(beatmap_id) = row.cover_beatmap_id {
                self.playlist_command(PlaylistAction::RequestArtwork { id, beatmap_id });
            }
        }
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
    pub fn insert_playlist_item(self: Pin<&mut Self>, item_id: i32, next: bool) {
        if self.active_playlist_id > 0 && item_id > 0 {
            self.command(insert_item(
                QueueItemRef::PlaylistItem {
                    playlist_id: self.active_playlist_id,
                    item_id,
                },
                next,
            ));
        }
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
}
#[allow(clippy::struct_excessive_bools)]
pub struct SettingsViewModelRust {
    pub(crate) folder_selection_rows: VariantList,
    pub(crate) folder_selection_open: bool,
    pub(crate) folder_selection_applying: bool,
    pub(crate) folder_selection_discovering: bool,
    pub(crate) folder_selection_picking: bool,
    pub(crate) folder_selection_message: QString,
    pub(crate) individual_volume_enabled: bool,
    pub(crate) global_volume_percent: i32,
    pub(crate) audio_settings_loaded: bool,
    pub(crate) volume_enabled: bool,
    pub(crate) volume_has_override: bool,
    pub(crate) audio_settings_message: QString,
    pub(crate) audio_settings_can_retry: bool,
    pub(crate) volume: f32,
    pub(crate) individual_mods_enabled: bool,
    pub(crate) mods_loaded: bool,
    pub(crate) mods_available: bool,
    pub(crate) mods_mode: QString,
    pub(crate) mods_speed_percent: i32,
    pub(crate) mods_adjust_pitch: bool,
    pub(crate) mods_saving: bool,
    pub(crate) mods_message: QString,
    /// Latest addressee from the client; an open editor captures it once and addresses every edit to it.
    pub(crate) mods_target: Option<ModsEditTarget>,
    pub(crate) mods_edit: Option<ModsEditTarget>,
    pub(crate) folders: VariantList,
    pub(crate) selected_folder_id: i32,
    pub(crate) folder_message: QString,
    pub(crate) folders_loading: bool,
    pub(crate) folder_busy: bool,
    pub(crate) folder_can_retry: bool,
    pub(crate) discord_rich_presence: bool,
    pub(crate) discord_preference_message: QString,
    pub(crate) use_unicode_titles: bool,
    pub(crate) use_unicode_artists: bool,
    pub(crate) track_name_preferences_message: QString,
    pub(crate) local_preferences_loaded: bool,
    pub(crate) controller: Option<AppController>,
    pub(crate) selected_audio_id: i32,
}
impl Default for SettingsViewModelRust {
    fn default() -> Self {
        Self {
            folder_selection_rows: VariantList::default(),
            folder_selection_open: false,
            folder_selection_applying: false,
            folder_selection_discovering: false,
            folder_selection_picking: false,
            folder_selection_message: QString::default(),
            individual_volume_enabled: false,
            global_volume_percent: 100,
            audio_settings_loaded: false,
            volume_enabled: false,
            volume_has_override: false,
            audio_settings_message: QString::default(),
            audio_settings_can_retry: false,
            volume: 1.0,
            individual_mods_enabled: false,
            mods_loaded: false,
            mods_available: false,
            mods_mode: QString::from("none"),
            mods_speed_percent: 100,
            mods_adjust_pitch: false,
            mods_saving: false,
            mods_message: QString::default(),
            mods_target: None,
            mods_edit: None,
            folders: VariantList::default(),
            selected_folder_id: -1,
            folder_message: QString::default(),
            folders_loading: Default::default(),
            folder_busy: Default::default(),
            folder_can_retry: Default::default(),
            discord_rich_presence: false,
            discord_preference_message: QString::default(),
            use_unicode_titles: false,
            use_unicode_artists: false,
            track_name_preferences_message: QString::default(),
            local_preferences_loaded: false,
            controller: None,
            selected_audio_id: -1,
        }
    }
}
impl ffi::SettingsViewModel {
    pub(crate) fn project_status(mut self: Pin<&mut Self>, status: OperationStatus) {
        self.as_mut().set_folders_loading(status.loading);
        self.as_mut().set_folder_busy(status.busy);
        self.as_mut().set_folder_can_retry(status.retry.is_some());
        self.set_folder_message(QString::from(status.message));
    }
    pub(crate) fn project_folders(
        self: Pin<&mut Self>,
        folders: Vec<osu_radio_client::models::OsuFolder>,
    ) {
        let rows = folders
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
        self.set_folders(rows);
    }

    pub(crate) fn project_preferences(
        mut self: Pin<&mut Self>,
        state: osu_radio_client::view_models::SettingsPreferences,
    ) {
        // All preference inputs come from storage or these controls; queued snapshots must not undo a newer input.
        if self.track_name_preferences() == state.name_preferences {
            self.as_mut()
                .set_track_name_preferences_message(QString::from(state.unicode_error));
        }
        if self.discord_rich_presence == state.discord_enabled {
            self.set_discord_preference_message(QString::from(state.discord_error));
        }
    }

    pub(crate) fn project_folder_selection(mut self: Pin<&mut Self>, selection: FolderSelection) {
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
    pub(crate) fn project_audio_settings(
        mut self: Pin<&mut Self>,
        state: osu_radio_client::view_models::AudioSettingsState,
    ) {
        self.as_mut().rust_mut().selected_audio_id = state.selected_audio_id.unwrap_or(-1);
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
    pub(crate) fn project_audio_mods(mut self: Pin<&mut Self>, state: AudioModsState) {
        // Retry/finish only bump the epoch of the same addressee; another addressee must not inherit an open editor.
        let edit = match (self.mods_edit, state.target) {
            (Some(open), Some(latest)) if open.audio_id == latest.audio_id => Some(latest),
            _ => None,
        };
        let profile = state.selected_profile;
        let mut this = self.as_mut();
        this.as_mut().rust_mut().mods_target = state.target;
        this.as_mut().rust_mut().mods_edit = edit;
        this.as_mut()
            .set_individual_mods_enabled(state.settings.individual_mods_enabled);
        this.as_mut().set_mods_loaded(state.loaded);
        this.as_mut()
            .set_mods_available(state.loaded && state.target.is_some());
        this.as_mut()
            .set_mods_mode(QString::from(match profile.mode {
                AudioModsMode::None => "none",
                AudioModsMode::Dt => "dt",
                AudioModsMode::Ht => "ht",
            }));
        this.as_mut()
            .set_mods_speed_percent(i32::from(profile.speed_percent));
        this.as_mut().set_mods_adjust_pitch(profile.adjust_pitch);
        // A failed write stays queued, which the client reports as saving; Retry must stay usable.
        this.as_mut()
            .set_mods_saving(state.saving && state.error.is_empty());
        this.set_mods_message(QString::from(state.error));
    }
    pub(crate) fn command(&self, command: AppCommand) {
        if let Some(controller) = &self.controller {
            controller.send(command);
        }
    }
    property_setter!(
        set_individual_mods_enabled,
        individual_mods_enabled,
        individual_mods_enabled_changed,
        bool
    );
    property_setter!(set_mods_loaded, mods_loaded, mods_loaded_changed, bool);
    property_setter!(
        set_mods_available,
        mods_available,
        mods_available_changed,
        bool
    );
    property_setter!(set_mods_mode, mods_mode, mods_mode_changed, QString);
    property_setter!(
        set_mods_speed_percent,
        mods_speed_percent,
        mods_speed_percent_changed,
        i32
    );
    property_setter!(
        set_mods_adjust_pitch,
        mods_adjust_pitch,
        mods_adjust_pitch_changed,
        bool
    );
    property_setter!(set_mods_saving, mods_saving, mods_saving_changed, bool);
    property_setter!(
        set_mods_message,
        mods_message,
        mods_message_changed,
        QString
    );
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
    property_setter!(set_folders, folders, folders_changed, VariantList);
    property_setter!(
        set_selected_folder_id,
        selected_folder_id,
        selected_folder_id_changed,
        i32
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
    pub fn change_discord_rich_presence(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().load_local_preferences();
        if self.discord_rich_presence == enabled {
            return;
        }
        self.as_mut().set_discord_rich_presence(enabled);
        self.command(AppCommand::SetDiscordRichPresence(enabled));
        if RuntimeContext::current().is_some() {
            let message = native::save_discord_rich_presence(enabled);
            self.command(AppCommand::ReportSettingsError {
                group: osu_radio_client::view_models::SettingsGroup::Discord,
                message: message.to_string(),
            });
            self.set_discord_preference_message(message);
        }
    }
    pub(crate) fn track_name_preferences(&self) -> TrackNamePreferences {
        TrackNamePreferences {
            use_unicode_titles: self.use_unicode_titles,
            use_unicode_artists: self.use_unicode_artists,
        }
    }
    pub(crate) fn load_local_preferences(mut self: Pin<&mut Self>) {
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
        let preferences = self.track_name_preferences();
        self.command(AppCommand::SetTrackNamePreferences(preferences));
        if RuntimeContext::current().is_some() {
            let error = native::save_track_name_preference(title, enabled);
            self.command(AppCommand::ReportSettingsError {
                group: osu_radio_client::view_models::SettingsGroup::Unicode,
                message: error.to_string(),
            });
            self.set_track_name_preferences_message(error);
        }
    }
    pub fn change_unicode_titles(self: Pin<&mut Self>, enabled: bool) {
        self.change_track_name_preference(true, enabled);
    }
    pub fn change_unicode_artists(self: Pin<&mut Self>, enabled: bool) {
        self.change_track_name_preference(false, enabled);
    }
    pub fn refresh_folders(self: Pin<&mut Self>) {
        self.command(AppCommand::RefreshFolders);
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
    pub fn change_individual_mods(self: Pin<&mut Self>, enabled: bool) {
        self.command(AppCommand::Mods(ModsAction::SetIndividual(enabled)));
    }
    /// The editor addresses its edits to the target visible when it opened.
    pub fn begin_mods_edit(mut self: Pin<&mut Self>) {
        let target = self.mods_target;
        self.as_mut().rust_mut().mods_edit = target;
    }
    pub fn finish_mods_edit(mut self: Pin<&mut Self>) {
        if let Some(target) = self.as_mut().rust_mut().mods_edit.take() {
            self.command(AppCommand::Mods(ModsAction::Finish(target)));
        }
    }
    fn mods_edit_command(&self, edit: ModsEdit) {
        if let Some(target) = self.mods_edit {
            self.command(AppCommand::Mods(ModsAction::Edit { target, edit }));
        }
    }
    pub fn cycle_dt_mod(self: Pin<&mut Self>) {
        self.mods_edit_command(ModsEdit::CycleDt);
    }
    pub fn toggle_ht_mod(self: Pin<&mut Self>) {
        self.mods_edit_command(ModsEdit::ToggleHt);
    }
    pub fn set_mod_speed(self: Pin<&mut Self>, percent: i32) {
        if let Ok(percent) = u16::try_from(percent) {
            self.mods_edit_command(ModsEdit::SetSpeed(percent));
        }
    }
    pub fn set_mod_adjust_pitch(self: Pin<&mut Self>, adjust: bool) {
        self.mods_edit_command(ModsEdit::SetAdjustPitch(adjust));
    }
    pub fn clear_mods(self: Pin<&mut Self>) {
        self.mods_edit_command(ModsEdit::Clear);
    }
    pub fn retry_mods(self: Pin<&mut Self>) {
        self.command(AppCommand::Mods(ModsAction::Retry));
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
    pub fn retry_folders(self: Pin<&mut Self>) {
        self.command(AppCommand::RetryFolders);
    }
}
#[allow(clippy::struct_excessive_bools)]
pub struct PlayerViewModelRust {
    pub(crate) selected_playlist_item_id: i32,
    pub(crate) can_play: bool,
    pub(crate) cache_epoch: u64,
    pub(crate) selected_track: Option<Track>,
    pub(crate) selected_available: bool,
    pub(crate) can_add_playlist: bool,
    pub(crate) current_audio_id: i32,
    pub(crate) loading_audio_id: i32,
    pub(crate) selected_is_playing: bool,
    pub(crate) can_seek: bool,
    pub(crate) can_next: bool,
    pub(crate) can_previous: bool,
    pub(crate) playback_position: f64,
    pub(crate) playback_duration: f64,
    pub(crate) playback_speed_percent: i32,
    pub(crate) launch_token: u64,
    pub(crate) playback_position_label: QString,
    pub(crate) playback_message: QString,
    pub(crate) selected_cover_id: Option<i32>,
    pub(crate) selected_audio_id: i32,
    pub(crate) selected_title: QString,
    pub(crate) selected_artist: QString,
    pub(crate) selected_subtitle: QString,
    pub(crate) selected_duration_label: QString,
    pub(crate) selected_artwork_url: QString,
    pub(crate) has_selection: bool,
    pub(crate) controller: Option<AppController>,
}
impl Default for PlayerViewModelRust {
    fn default() -> Self {
        Self {
            selected_playlist_item_id: -1,
            selected_available: Default::default(),
            can_add_playlist: Default::default(),
            cache_epoch: 0,
            selected_track: None,
            can_play: false,
            current_audio_id: -1,
            loading_audio_id: -1,
            selected_is_playing: false,
            can_seek: false,
            can_next: false,
            can_previous: false,
            playback_position: 0.0,
            playback_duration: 0.0,
            playback_speed_percent: 100,
            launch_token: 0,
            playback_position_label: QString::from("00:00"),
            playback_message: QString::default(),
            selected_cover_id: None,
            selected_audio_id: -1,
            selected_title: QString::default(),
            selected_artist: QString::default(),
            selected_subtitle: QString::default(),
            selected_duration_label: QString::from("--:--"),
            selected_artwork_url: QString::default(),
            has_selection: Default::default(),
            controller: None,
        }
    }
}
impl ffi::PlayerViewModel {
    property_setter!(
        set_selected_playlist_item_id,
        selected_playlist_item_id,
        selected_playlist_item_id_changed,
        i32
    );
    property_setter!(set_can_play, can_play, can_play_changed, bool);
    property_setter!(
        set_playback_speed_percent,
        playback_speed_percent,
        playback_speed_percent_changed,
        i32
    );
    property_setter!(set_launch_token, launch_token, launch_token_changed, u64);
    pub(crate) fn project_player(
        mut self: Pin<&mut Self>,
        state: osu_radio_client::view_models::PlayerPresentation,
    ) {
        if self.selected_track != state.selected_track {
            let track = state.selected_track.as_ref();
            let cover = track.and_then(|track| track.cover_beatmap_id);
            let artwork = cover
                .map(|id| native::artwork_url(self.cache_epoch, id))
                .unwrap_or_default();
            native::protect_artwork(self.cache_epoch, cover.unwrap_or(-1));
            self.as_mut().rust_mut().selected_cover_id = cover;
            self.as_mut()
                .set_selected_audio_id(track.map_or(-1, |track| track.audio_source_id));
            self.as_mut().set_has_selection(track.is_some());
            self.as_mut()
                .set_can_add_playlist(track.is_some_and(|track| !track.difficulties.is_empty()));
            self.as_mut().set_selected_title(QString::from(
                track.map_or("", |track| track.title.as_str()),
            ));
            self.as_mut().set_selected_artist(QString::from(
                track.map_or("", |track| track.artist.as_str()),
            ));
            self.as_mut().set_selected_subtitle(QString::from(
                track.map_or("", |track| track.subtitle.as_str()),
            ));
            self.as_mut().set_selected_artwork_url(artwork);
            self.as_mut()
                .rust_mut()
                .selected_track
                .clone_from(&state.selected_track);
        }
        self.as_mut()
            .set_selected_playlist_item_id(state.selected_playlist_item_id.unwrap_or(-1));
        self.as_mut().set_selected_available(state.can_play);
        self.as_mut().set_can_play(state.can_play);
        self.as_mut()
            .set_selected_is_playing(state.selected_is_playing);
        self.as_mut().set_can_seek(state.can_seek);
        self.as_mut().set_can_next(state.can_next);
        self.as_mut().set_can_previous(state.can_previous);
        self.as_mut()
            .set_playback_position(state.position.as_secs_f64());
        self.as_mut()
            .set_playback_duration(state.duration.map_or(0.0, |d| d.as_secs_f64()));
        self.as_mut()
            .set_playback_position_label(QString::from(format_time(state.position)));
        self.as_mut()
            .set_playback_speed_percent(i32::from(state.speed_percent));
        // Already in the UI timescale, including a selected track that is not sounding.
        self.as_mut().set_selected_duration_label(QString::from(
            state
                .selected_duration
                .map_or_else(|| "--:--".to_owned(), format_time),
        ));
        self.set_playback_message(QString::from(state.message));
    }

    pub(crate) fn command(&self, command: AppCommand) {
        if let Some(controller) = &self.controller {
            controller.send(command);
        }
    }
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
    pub fn toggle_playback(self: Pin<&mut Self>) {
        self.command(AppCommand::TogglePlayback);
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
}
pub struct QueueViewModelRust {
    pub(crate) cache_epoch: u64,
    pub(crate) model: cxx::UniquePtr<ffi::TrackListModel>,
    pub(crate) start_index: i32,
    pub(crate) queue_loading: bool,
    pub(crate) queue_busy: bool,
    pub(crate) queue_message: QString,
    pub(crate) queue_state: QueueView,
    pub(crate) controller: Option<AppController>,
}
impl Default for QueueViewModelRust {
    fn default() -> Self {
        Self {
            cache_epoch: 0,
            model: native::new_track_list_model(),
            start_index: 0,
            queue_loading: false,
            queue_busy: false,
            queue_message: QString::default(),
            queue_state: QueueView::default(),
            controller: None,
        }
    }
}
fn insert_item(item: QueueItemRef, next: bool) -> AppCommand {
    AppCommand::Queue(QueueAction::Insert {
        items: vec![item],
        placement: if next {
            QueuePlacement::Next
        } else {
            QueuePlacement::End
        },
    })
}
pub(crate) fn queue_start_index(view: &QueueView) -> i32 {
    i32::try_from(
        view.entries
            .iter()
            .position(|row| Some(row.id) == view.current_entry_id)
            .unwrap_or_else(|| view.history_len.min(view.entries.len().saturating_sub(1))),
    )
    .unwrap_or(i32::MAX)
}
pub(crate) fn parse_entry_id(value: &QString) -> Option<QueueEntryId> {
    QueueEntryId::try_from(String::from(value)).ok()
}
pub(crate) fn queue_move(id: &QString, before: &QVariant) -> Option<QueueAction> {
    let id = parse_entry_id(id)?;
    let before_entry_id = if before.is_null() || !before.is_valid() {
        None
    } else {
        if before.type_name() != "QString" {
            return None;
        }
        Some(parse_entry_id(&before.value::<QString>()?)?)
    };
    Some(QueueAction::Move {
        id,
        before_entry_id,
    })
}
impl ffi::QueueViewModel {
    pub fn tracks_pointer(&self) -> *mut ffi::TrackListModel {
        std::ptr::from_ref(&*self.model).cast_mut()
    }
    pub(crate) fn replace_queue(mut self: Pin<&mut Self>, state: QueueView) {
        self.as_mut().rust_mut().queue_state = state;
        self.project_queue();
    }
    pub(crate) fn project_queue(mut self: Pin<&mut Self>) {
        let (start, loading, busy, message) = {
            let mut pinned = self.as_mut().rust_mut();
            let state = &mut *pinned;
            state
                .model
                .pin_mut()
                .replace_queue_rows(&state.queue_state, state.cache_epoch);
            (
                queue_start_index(&state.queue_state),
                state.queue_state.loading,
                state.queue_state.busy,
                QString::from(
                    state
                        .queue_state
                        .operation_error
                        .as_deref()
                        .unwrap_or(&state.queue_state.message),
                ),
            )
        };
        self.as_mut().set_start_index(start);
        self.as_mut().set_queue_loading(loading);
        self.as_mut().set_queue_busy(busy);
        self.set_queue_message(message);
    }
    pub(crate) fn command(&self, command: AppCommand) {
        if let Some(controller) = &self.controller {
            controller.send(command);
        }
    }
    pub fn play_entry(self: Pin<&mut Self>, id: &QString) {
        if let Some(id) = parse_entry_id(id) {
            self.command(AppCommand::Queue(QueueAction::Play(id)));
        }
    }
    pub fn remove_entry(self: Pin<&mut Self>, id: &QString) {
        if let Some(id) = parse_entry_id(id) {
            self.command(AppCommand::Queue(QueueAction::Remove(id)));
        }
    }
    pub fn move_entry(self: Pin<&mut Self>, id: &QString, before: &QVariant) {
        if let Some(action) = queue_move(id, before) {
            self.command(AppCommand::Queue(action));
        }
    }
    pub fn clear_upcoming(self: Pin<&mut Self>) {
        self.command(AppCommand::Queue(QueueAction::ClearUpcoming));
    }
    pub fn clear_all(self: Pin<&mut Self>) {
        self.command(AppCommand::Queue(QueueAction::ClearAll));
    }
    pub fn set_visible_queue_media(self: Pin<&mut Self>, ids: &VariantList) {
        let visible = ids
            .0
            .iter()
            .filter_map(QVariant::value::<QString>)
            .filter_map(|id| parse_entry_id(&id))
            .filter_map(|id| {
                self.model
                    .rows
                    .iter()
                    .find(|row| row.queue.as_ref().is_some_and(|q| q.id == id))
                    .map(|row| (id, row.artwork.is_empty()))
            })
            .collect();
        self.command(AppCommand::SetVisibleQueueMedia(visible));
    }
    property_setter!(set_start_index, start_index, start_index_changed, i32);
    property_setter!(set_queue_busy, queue_busy, queue_busy_changed, bool);
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
    pub fn set_queue_visible(mut self: Pin<&mut Self>, visible: bool) {
        if visible {
            self.as_mut().set_queue_loading(true);
        }
        self.command(AppCommand::SetQueueVisible(visible));
    }
}
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct QueueRoles {
    pub(crate) id: QueueEntryId,
    pub(crate) current: bool,
    pub(crate) past: bool,
    pub(crate) availability: QueueAvailability,
}
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) queue: Option<QueueRoles>,
    pub(crate) playlist_item_id: Option<i32>,
    pub(crate) track: Track,
    pub(crate) artwork: QString,
}
pub(crate) const ROLES: [(i32, &str); 12] = [
    (256, "audioId"),
    (257, "title"),
    (258, "artist"),
    (259, "subtitle"),
    (260, "durationLabel"),
    (261, "artworkUrl"),
    (262, "playlistItemId"),
    (263, "available"),
    (264, "entryId"),
    (265, "isCurrent"),
    (266, "isPast"),
    (267, "availability"),
];
#[derive(Default)]
pub struct TrackListModelRust {
    pub(crate) rows: Vec<Row>,
    pub(crate) cover_rows: HashMap<i32, Vec<usize>>,
    pub(crate) audio_rows: HashMap<i32, usize>,
    pub(crate) track_count: i32,
}
impl ffi::TrackListModel {
    pub(crate) fn replace_queue_rows(mut self: Pin<&mut Self>, view: &QueueView, epoch: u64) {
        let ids: std::collections::HashSet<_> = view.entries.iter().map(|row| row.id).collect();
        self.as_mut().retain_queue_rows(&ids);
        let existing: std::collections::HashSet<_> = self
            .rows
            .iter()
            .filter_map(|row| row.queue.as_ref().map(|q| q.id))
            .collect();
        for (i, entry) in view.entries.iter().enumerate() {
            let Ok(index) = i32::try_from(i) else {
                return;
            };
            let found = self
                .rows
                .get(i)
                .filter(|row| row.queue.as_ref().is_some_and(|q| q.id == entry.id))
                .map(|_| i)
                .or_else(|| {
                    if !existing.contains(&entry.id) {
                        return None;
                    }
                    self.rows
                        .iter()
                        .position(|row| row.queue.as_ref().is_some_and(|q| q.id == entry.id))
                });
            if let Some(from) = found.filter(|from| *from != i) {
                let Ok(source) = i32::try_from(from) else {
                    return;
                };
                // Earlier positions are already reconciled, so this only moves upward.
                if unsafe {
                    self.as_mut().begin_move_rows(
                        &QModelIndex::default(),
                        source,
                        source,
                        &QModelIndex::default(),
                        index,
                    )
                } {
                    let row = self.as_mut().rust_mut().rows.remove(from);
                    self.as_mut().rust_mut().rows.insert(i, row);
                    unsafe {
                        self.as_mut().end_move_rows();
                    }
                } else {
                    return;
                }
            }
            let artwork = entry
                .track
                .cover_beatmap_id
                .map(|id| native::artwork_url(epoch, id))
                .filter(|url| !url.is_empty())
                .or_else(|| {
                    found
                        .and_then(|_| self.rows.get(i))
                        .filter(|row| row.track.cover_beatmap_id == entry.track.cover_beatmap_id)
                        .map(|row| row.artwork.clone())
                })
                .unwrap_or_default();
            let row = Row {
                queue: Some(QueueRoles {
                    id: entry.id,
                    current: Some(entry.id) == view.current_entry_id,
                    past: i < view.history_len,
                    availability: entry.availability,
                }),
                playlist_item_id: entry.playlist_item_id,
                track: entry.track.clone(),
                artwork,
            };
            if found.is_none() {
                unsafe {
                    self.as_mut()
                        .begin_insert_rows(&QModelIndex::default(), index, index);
                }
                self.as_mut().rust_mut().rows.insert(i, row);
                unsafe {
                    self.as_mut().end_insert_rows();
                }
            } else if self.rows.get(i) != Some(&row) {
                if let Some(existing) = self.as_mut().rust_mut().rows.get_mut(i) {
                    *existing = row;
                }
                self.as_mut()
                    .notify_row(i, &ROLES.iter().map(|(id, _)| *id).collect::<Vec<_>>());
            }
        }
        self.as_mut().index_rows();
        let count = i32::try_from(self.rows.len()).unwrap_or(i32::MAX);
        if self.track_count != count {
            self.as_mut().rust_mut().track_count = count;
            self.track_count_changed();
        }
    }
    fn retain_queue_rows(mut self: Pin<&mut Self>, ids: &std::collections::HashSet<QueueEntryId>) {
        for i in (0..self.rows.len()).rev() {
            let Ok(index) = i32::try_from(i) else {
                return;
            };
            if !self
                .rows
                .get(i)
                .and_then(|row| row.queue.as_ref())
                .is_some_and(|q| ids.contains(&q.id))
            {
                unsafe {
                    self.as_mut()
                        .begin_remove_rows(&QModelIndex::default(), index, index);
                }
                self.as_mut().rust_mut().rows.remove(i);
                unsafe {
                    self.as_mut().end_remove_rows();
                }
            }
        }
    }
    pub(crate) fn replace_rows(
        mut self: Pin<&mut Self>,
        tracks: Vec<(Option<i32>, Track)>,
        epoch: u64,
        invalidate: bool,
    ) {
        if !invalidate
            && tracks.len() == self.rows.len()
            && tracks
                .iter()
                .zip(&self.rows)
                .all(|((item, track), row)| *item == row.playlist_item_id && *track == row.track)
        {
            return;
        }
        // SAFETY: begin/end bracket the complete replacement on the QObject thread.
        unsafe {
            self.as_mut().begin_reset_model();
        }
        self.as_mut().rust_mut().rows = tracks
            .into_iter()
            .map(|(playlist_item_id, track)| {
                let artwork = track
                    .cover_beatmap_id
                    .map(|id| native::artwork_url(epoch, id))
                    .unwrap_or_default();
                Row {
                    queue: None,
                    playlist_item_id,
                    track,
                    artwork,
                }
            })
            .collect();
        self.as_mut().index_rows();
        let count = i32::try_from(self.rows.len()).unwrap_or(i32::MAX);
        self.as_mut().rust_mut().track_count = count;
        unsafe {
            self.as_mut().end_reset_model();
        }
        self.track_count_changed();
    }
    pub(crate) fn reorder_rows(mut self: Pin<&mut Self>, tracks: Vec<(Option<i32>, Track)>) {
        let reset = tracks.len() != self.rows.len();
        if reset {
            unsafe {
                self.as_mut().begin_reset_model();
            }
        }
        let mut previous: HashMap<_, _> = std::mem::take(&mut self.as_mut().rust_mut().rows)
            .into_iter()
            .map(|row| ((row.playlist_item_id, row.track.audio_source_id), row))
            .collect();
        self.as_mut().rust_mut().rows = tracks
            .into_iter()
            .map(|(playlist_item_id, mut track)| {
                let artwork = previous
                    .remove(&(playlist_item_id, track.audio_source_id))
                    .map_or_else(QString::default, |row| {
                        track.duration = track.duration.or(row.track.duration);
                        row.artwork
                    });
                Row {
                    queue: None,
                    playlist_item_id,
                    track,
                    artwork,
                }
            })
            .collect();
        self.as_mut().index_rows();
        if reset {
            let count = i32::try_from(self.rows.len()).unwrap_or(i32::MAX);
            self.as_mut().rust_mut().track_count = count;
            unsafe {
                self.as_mut().end_reset_model();
            }
            self.track_count_changed();
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
    pub(crate) fn change_track(mut self: Pin<&mut Self>, item_id: Option<i32>, track: &Track) {
        let indices: Vec<_> = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, row)| {
                (row.track.audio_source_id == track.audio_source_id
                    && item_id.is_none_or(|id| row.playlist_item_id == Some(id)))
                .then_some(i)
            })
            .collect();
        for i in indices {
            if let Some(row) = self.as_mut().rust_mut().rows.get_mut(i) {
                row.track.clone_from(track);
            }
            self.as_mut().notify_row(i, &[257, 258, 259, 260]);
        }
    }
    pub(crate) fn clear_artwork(mut self: Pin<&mut Self>) {
        for row in &mut self.as_mut().rust_mut().rows {
            row.artwork = QString::default();
        }
        if !self.rows.is_empty() {
            let first = self.index(0, 0, &QModelIndex::default());
            let last = self.index(
                self.track_count.saturating_sub(1),
                0,
                &QModelIndex::default(),
            );
            self.data_changed(&first, &last, &std::iter::once(261).collect());
        }
    }
    pub(crate) fn update_artwork(mut self: Pin<&mut Self>, epoch: u64, affected: &[i32]) {
        for cover in affected {
            let url = native::artwork_url(epoch, *cover);
            if url.is_empty() {
                continue;
            }
            for index in self.cover_rows.get(cover).cloned().unwrap_or_default() {
                if let Some(row) = self.as_mut().rust_mut().rows.get_mut(index) {
                    row.artwork = url.clone();
                }
                self.as_mut().notify_row(index, &[261]);
            }
        }
    }

    pub fn entry_id_at(&self, row: i32) -> QString {
        QString::from(
            usize::try_from(row)
                .ok()
                .and_then(|i| self.rows.get(i))
                .and_then(|row| row.queue.as_ref())
                .map(|q| q.id.to_string())
                .unwrap_or_default(),
        )
    }
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
            264 => QVariant::from(&QString::from(
                row.queue
                    .as_ref()
                    .map(|q| q.id.to_string())
                    .unwrap_or_default(),
            )),
            265 => QVariant::from(&row.queue.as_ref().is_some_and(|q| q.current)),
            266 => QVariant::from(&row.queue.as_ref().is_some_and(|q| q.past)),
            267 => QVariant::from(&QString::from(
                match row.queue.as_ref().map(|q| q.availability) {
                    Some(QueueAvailability::Checking) => "checking",
                    Some(QueueAvailability::Unavailable) => "unavailable",
                    _ => "available",
                },
            )),
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
    pub(crate) fn notify_row(mut self: Pin<&mut Self>, row: usize, roles: &[i32]) {
        let Ok(row) = i32::try_from(row) else {
            return;
        };
        let index = self.index(row, 0, &QModelIndex::default());
        let roles = roles.iter().copied().collect();
        self.as_mut().data_changed(&index, &index, &roles);
    }
    pub(crate) fn index_rows(mut self: Pin<&mut Self>) {
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
}

fn format_time(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}
