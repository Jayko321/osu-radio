//! Typed Qt projection; workflows remain in osu-radio-client.
use crate::runtime::{RuntimeContext, ffi as native};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QByteArray, QList, QString, QVariant};
use osu_radio_client::{
    ServerOptions, Track,
    controller::{AppCommand, AppController, AppUpdate, ConnectionStatus, MediaTicket},
    view_models::TrackSort,
};
use std::{
    pin::Pin,
    sync::{Arc, Mutex},
};

/// Preserve Qt's canonical list name in generated QML metadata.
#[repr(transparent)]
#[derive(Clone, Default, PartialEq)]
pub struct VariantList(pub(crate) QList<QVariant>);
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
    unsafe extern "C++" {
        include!("src/qt_view_models.cxxqt.h");
        type LibraryViewModel = crate::qt_view_models::ffi::LibraryViewModel;
        type PlaylistsViewModel = crate::qt_view_models::ffi::PlaylistsViewModel;
        type SettingsViewModel = crate::qt_view_models::ffi::SettingsViewModel;
        type PlayerViewModel = crate::qt_view_models::ffi::PlayerViewModel;
        type QueueViewModel = crate::qt_view_models::ffi::QueueViewModel;
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
        #[qproperty(
            QString,
            connection_status,
            cxx_name = "connectionStatus",
            READ,
            NOTIFY
        )]
        #[qproperty(bool, connected, cxx_name = "connected", READ, NOTIFY)]
        #[qproperty(bool, connecting, cxx_name = "connecting", READ, NOTIFY)]
        #[qproperty(*mut LibraryViewModel, library, READ = library_pointer, CONSTANT)]
        #[qproperty(*mut PlaylistsViewModel, playlists, READ = playlists_pointer, CONSTANT)]
        #[qproperty(*mut SettingsViewModel, settings, READ = settings_pointer, CONSTANT)]
        #[qproperty(*mut PlayerViewModel, player, READ = player_pointer, CONSTANT)]
        #[qproperty(*mut QueueViewModel, queue, READ = queue_pointer, CONSTANT)]
        type AppBridge = super::AppBridgeRust;
        fn library_pointer(self: &AppBridge) -> *mut LibraryViewModel;
        fn playlists_pointer(self: &AppBridge) -> *mut PlaylistsViewModel;
        fn settings_pointer(self: &AppBridge) -> *mut SettingsViewModel;
        fn player_pointer(self: &AppBridge) -> *mut PlayerViewModel;
        fn queue_pointer(self: &AppBridge) -> *mut QueueViewModel;
        #[qinvokable]
        #[cxx_name = "connectSession"]
        fn connect_session(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "requestMedia"]
        fn request_media(self: Pin<&mut AppBridge>, id: i32);
        #[qinvokable]
        #[cxx_name = "setVisibleMedia"]
        fn set_visible_media(self: Pin<&mut AppBridge>, ids: &QVariantList);
        #[qsignal]
        #[cxx_name = "mediaViewChanged"]
        fn media_view_changed(self: Pin<&mut AppBridge>);
    }
    impl cxx_qt::Threading for AppBridge {}
}

// Independent operation flags are exposed as individual typed QML properties.
#[allow(clippy::struct_excessive_bools)]
pub struct AppBridgeRust {
    connection_status: QString,
    connected: bool,
    connecting: bool,
    controller: Option<AppController>,
    generation: u64,
    cache_epoch: u64,
    library_object: cxx::UniquePtr<crate::qt_view_models::ffi::LibraryViewModel>,
    playlists_object: cxx::UniquePtr<crate::qt_view_models::ffi::PlaylistsViewModel>,
    settings_object: cxx::UniquePtr<crate::qt_view_models::ffi::SettingsViewModel>,
    player_object: cxx::UniquePtr<crate::qt_view_models::ffi::PlayerViewModel>,
    queue_object: cxx::UniquePtr<crate::qt_view_models::ffi::QueueViewModel>,
}
impl Default for AppBridgeRust {
    fn default() -> Self {
        Self {
            connection_status: QString::default(),
            connected: Default::default(),
            connecting: Default::default(),
            controller: None,
            generation: 0,
            cache_epoch: 0,
            library_object: native::new_library_view_model(),
            playlists_object: native::new_playlists_view_model(),
            settings_object: native::new_settings_view_model(),
            player_object: native::new_player_view_model(),
            queue_object: native::new_queue_view_model(),
        }
    }
}
macro_rules! adapter {
    ($name:ident, $read:ident, $write:ident, $field:ident, $ty:ident) => {
        pub fn $name(&self) -> *mut crate::qt_view_models::ffi::$ty {
            std::ptr::from_ref(&*self.$field).cast_mut()
        }
        pub(crate) fn $read(&self) -> &crate::qt_view_models::ffi::$ty {
            &self.$field
        }
        pub(crate) fn $write(self: Pin<&mut Self>) -> Pin<&mut crate::qt_view_models::ffi::$ty> {
            self.rust_mut().get_mut().$field.pin_mut()
        }
    };
}
impl ffi::AppBridge {
    adapter!(
        library_pointer,
        library_ref,
        library_mut,
        library_object,
        LibraryViewModel
    );
    adapter!(
        playlists_pointer,
        playlists_ref,
        playlists_mut,
        playlists_object,
        PlaylistsViewModel
    );
    adapter!(
        settings_pointer,
        settings_ref,
        settings_mut,
        settings_object,
        SettingsViewModel
    );
    adapter!(
        player_pointer,
        player_ref,
        player_mut,
        player_object,
        PlayerViewModel
    );
    adapter!(
        queue_pointer,
        queue_ref,
        queue_mut,
        queue_object,
        QueueViewModel
    );

    fn command(&self, command: AppCommand) {
        if let Some(controller) = &self.controller {
            controller.send(command);
        }
    }
    pub fn connect_session(mut self: Pin<&mut Self>) {
        self.as_mut().settings_mut().load_local_preferences();
        if self.controller.is_none() {
            let Some(context) = RuntimeContext::current() else {
                return;
            };
            let thread = self.qt_thread();
            let slot = Arc::new(Mutex::new(None::<AppController>));
            let callback_slot = slot.clone();
            let options = ServerOptions {
                remote_url: std::env::var_os("OSU_RADIO_SERVER_URL")
                    .map(|url| url.to_string_lossy().into_owned()),
                ..ServerOptions::default()
            };
            let controller = AppController::spawn(&context.handle, options, move |update| {
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
                        // Undelivered closures release the media slot after QObject destruction.
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
            self.as_mut().library_mut().rust_mut().controller = Some(controller.clone());
            self.as_mut().playlists_mut().rust_mut().controller = Some(controller.clone());
            self.as_mut().settings_mut().rust_mut().controller = Some(controller.clone());
            self.as_mut().player_mut().rust_mut().controller = Some(controller.clone());
            self.as_mut().queue_mut().rust_mut().controller = Some(controller.clone());
            self.as_mut().rust_mut().controller = Some(controller);
            let unicode_error = self
                .settings_ref()
                .track_name_preferences_message
                .to_string();
            let discord_error = self.settings_ref().discord_preference_message.to_string();
            self.command(AppCommand::SetDiscordRichPresence(
                self.settings_ref().discord_rich_presence,
            ));
            self.command(AppCommand::SetTrackNamePreferences(
                self.settings_ref().track_name_preferences(),
            ));
            if !unicode_error.is_empty() {
                self.command(AppCommand::ReportSettingsError {
                    group: osu_radio_client::view_models::SettingsGroup::Unicode,
                    message: unicode_error,
                });
            }
            if !discord_error.is_empty() {
                self.command(AppCommand::ReportSettingsError {
                    group: osu_radio_client::view_models::SettingsGroup::Discord,
                    message: discord_error,
                });
            }
        }
        self.command(AppCommand::Connect);
    }
    fn media_generation(mut self: Pin<&mut Self>, generation: u64, invalidate: bool) {
        self.as_mut().rust_mut().generation = generation;
        if invalidate {
            let epoch = native::reset_artwork();
            self.as_mut().rust_mut().cache_epoch = epoch;
            self.as_mut().playlists_mut().rust_mut().cache_epoch = epoch;
            self.as_mut()
                .playlists_mut()
                .rust_mut()
                .playlist_artwork_urls
                .clear();
            self.as_mut().queue_mut().rust_mut().cache_epoch = epoch;
            self.as_mut()
                .queue_mut()
                .rust_mut()
                .model
                .pin_mut()
                .clear_artwork();
            self.as_mut().player_mut().rust_mut().cache_epoch = epoch;
            native::protect_artwork(epoch, self.player_ref().selected_cover_id.unwrap_or(-1));
            self.player_mut()
                .set_selected_artwork_url(QString::default());
        }
    }
    fn update_artwork(mut self: Pin<&mut Self>, affected: &[i32]) {
        let epoch = self.cache_epoch;
        self.as_mut()
            .library_mut()
            .rust_mut()
            .model
            .pin_mut()
            .update_artwork(epoch, affected);
        self.as_mut()
            .playlists_mut()
            .rust_mut()
            .model
            .pin_mut()
            .update_artwork(epoch, affected);
        if self
            .player_ref()
            .selected_cover_id
            .is_some_and(|cover| affected.contains(&cover))
        {
            let cover = self.player_ref().selected_cover_id.unwrap_or(-1);
            let url = native::artwork_url(epoch, cover);
            if !url.is_empty() {
                self.as_mut().player_mut().set_selected_artwork_url(url);
            }
        }
        self.queue_mut()
            .rust_mut()
            .model
            .pin_mut()
            .update_artwork(epoch, affected);
    }
    fn media_track(&self, id: i32) -> Option<&Track> {
        if self
            .player_ref()
            .selected_track
            .as_ref()
            .is_some_and(|track| track.audio_source_id == id)
        {
            return self.player_ref().selected_track.as_ref();
        }
        let library = &self.library_ref().model;
        let playlists = &self.playlists_ref().model;
        let (first, second) = if self.playlists_ref().playlist_state.showing_detail() {
            (playlists, library)
        } else {
            (library, playlists)
        };
        first
            .audio_rows
            .get(&id)
            .and_then(|index| first.rows.get(*index))
            .map(|row| &row.track)
            .or_else(|| {
                second
                    .audio_rows
                    .get(&id)
                    .and_then(|index| second.rows.get(*index))
                    .map(|row| &row.track)
            })
            .or_else(|| {
                self.queue_ref()
                    .queue_state
                    .entries
                    .iter()
                    .find(|row| row.audio_source_id == Some(id))
                    .map(|row| &row.track)
            })
    }
    pub fn request_media(self: Pin<&mut Self>, id: i32) {
        let Some(track) = self.media_track(id) else {
            return;
        };
        let missing = track
            .cover_beatmap_id
            .is_some_and(|cover| native::artwork_url(self.cache_epoch, cover).is_empty());
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
                let track = self.media_track(id)?;
                let missing = track
                    .cover_beatmap_id
                    .is_some_and(|cover| native::artwork_url(self.cache_epoch, cover).is_empty());
                Some((id, missing))
            })
            .collect();
        self.command(AppCommand::SetVisibleMedia(visible));
    }
    fn project_connection(mut self: Pin<&mut Self>, status: ConnectionStatus) {
        let connected = matches!(status, ConnectionStatus::Connected);
        let connecting = matches!(status, ConnectionStatus::Connecting);
        let text = match status {
            ConnectionStatus::Disconnected => "Disconnected".to_owned(),
            ConnectionStatus::Connecting => "Connecting…".to_owned(),
            ConnectionStatus::Connected => "Connected".to_owned(),
            ConnectionStatus::Failed(error) => error,
        };
        if self.connected != connected {
            self.as_mut().rust_mut().connected = connected;
            self.as_mut().connected_changed();
        }
        if self.connecting != connecting {
            self.as_mut().rust_mut().connecting = connecting;
            self.as_mut().connecting_changed();
        }
        let text = QString::from(text);
        if self.connection_status != text {
            self.as_mut().rust_mut().connection_status = text;
            self.connection_status_changed();
        }
    }
    fn apply(mut self: Pin<&mut Self>, update: AppUpdate) {
        match update {
            AppUpdate::MediaGeneration {
                generation,
                invalidate_artwork,
            } => self.media_generation(generation, invalidate_artwork),
            AppUpdate::Player(state) => {
                let changed = self.player_ref().selected_track != state.selected_track;
                let id = state
                    .selected_track
                    .as_ref()
                    .map(|track| track.audio_source_id);
                self.as_mut().player_mut().project_player(state);
                if changed && let Some(id) = id {
                    self.request_media(id);
                }
            }
            AppUpdate::Queue(state) => self.queue_mut().replace_queue(state),
            AppUpdate::AudioSettings(state) => self.settings_mut().project_audio_settings(state),
            AppUpdate::AudioMods(state) => self.settings_mut().project_audio_mods(state),
            AppUpdate::SettingsPreferences(state) => self.settings_mut().project_preferences(state),
            AppUpdate::Playlists(state) => self.playlists_mut().project_playlists(&state),
            AppUpdate::PlaylistCover {
                id,
                revision,
                bytes,
            } => self.install_playlist_image(id, revision, -1, &bytes),
            AppUpdate::PlaylistArtwork {
                id,
                beatmap_id,
                bytes,
            } => self.install_playlist_image(id, -1, beatmap_id, &bytes),
            AppUpdate::FolderSelection(state) => {
                self.settings_mut().project_folder_selection(state);
            }
            AppUpdate::FolderSelectionPickerRequested(epoch) => self.pick_folder(Some(epoch)),
            AppUpdate::Playback(state) => {
                self.as_mut()
                    .player_mut()
                    .set_current_audio_id(state.current_audio_id.unwrap_or(-1));
                self.as_mut()
                    .player_mut()
                    .set_loading_audio_id(state.loading_audio_id.unwrap_or(-1));
                self.player_mut().set_launch_token(state.playback_token);
            }
            AppUpdate::Connection(status) => self.project_connection(status),
            AppUpdate::LibraryStatus(status) => self.library_mut().project_status(status),
            AppUpdate::FolderStatus(status) => self.settings_mut().project_status(status),
            AppUpdate::LibraryTracksReplaced {
                tracks,
                invalidate_artwork,
            } => {
                let epoch = self.cache_epoch;
                self.as_mut()
                    .library_mut()
                    .replace_tracks(tracks, epoch, invalidate_artwork);
                self.media_view_changed();
            }
            AppUpdate::LibraryTracksReordered(tracks) => self.library_mut().reorder_tracks(tracks),
            AppUpdate::LibraryTrackChanged(track) => self.library_mut().change_track(&track),
            AppUpdate::PlaylistTracksReplaced {
                tracks,
                invalidate_artwork,
            } => {
                let epoch = self.cache_epoch;
                self.as_mut()
                    .playlists_mut()
                    .replace_tracks(tracks, epoch, invalidate_artwork);
                self.media_view_changed();
            }
            AppUpdate::PlaylistTracksReordered(tracks) => {
                self.playlists_mut().reorder_tracks(tracks);
            }
            AppUpdate::PlaylistTrackChanged(row) => self.playlists_mut().change_track(&row),
            AppUpdate::LibrarySelected(id) => {
                self.library_mut().set_selected_audio_id(id.unwrap_or(-1));
            }
            AppUpdate::TrackSort(sort) => {
                self.as_mut()
                    .library_mut()
                    .project_track_sort(track_sort_index(sort));
                self.playlists_mut()
                    .project_track_sort(track_sort_index(sort));
            }
            AppUpdate::TrackSelected(_) | AppUpdate::Artwork { .. } => {}
            AppUpdate::FoldersReplaced(folders) => self.settings_mut().project_folders(folders),
            AppUpdate::FolderSelected(id) => {
                self.settings_mut().set_selected_folder_id(id.unwrap_or(-1));
            }
            AppUpdate::FolderPickerRequested => self.pick_folder(None),
        }
    }
    fn install_playlist_image(
        mut self: Pin<&mut Self>,
        id: i32,
        revision: i64,
        beatmap: i32,
        bytes: &[u8],
    ) {
        if !self
            .playlists_ref()
            .playlist_state
            .playlists
            .iter()
            .any(|row| {
                row.id == id
                    && if revision >= 0 {
                        row.custom_cover_revision == Some(revision)
                    } else {
                        row.cover_beatmap_id == Some(beatmap)
                    }
            })
        {
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
        let state = self.playlists_ref().playlist_state.clone();
        self.playlists_mut().project_playlists(&state);
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
        let epoch = self.cache_epoch;
        let thread = self.qt_thread();
        let task = context.handle.spawn_blocking(move || {
            let affected: Vec<i32> = if ack.ticket.generation == generation {
                native::install_artwork(epoch, cover_id, &QByteArray::from(bytes.as_slice()))
                    .iter()
                    .copied()
                    .collect()
            } else {
                Vec::new()
            };
            ack.available = !affected.is_empty();
            let _ = thread.queue(move |object| {
                if object.cache_epoch == epoch {
                    object.update_artwork(&affected);
                }
            });
            drop(ack);
        });
        context.track(task);
    }
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
    use cxx_qt_lib::QModelIndex;
    use osu_radio_client::view_models::{PlayerPresentation, PlaylistTrack, PlaylistsState};
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

    #[test]
    fn native_model_cache_and_destroyed_object_contracts() {
        let cover_error = native::check_playlist_cover_preparation();
        assert!(cover_error.is_empty(), "{cover_error}");
        independent_models_keep_item_identity_and_media();
        queue_model_preserves_entry_identity_and_updates_roles();
        player_projection_does_not_reset_lists();
        warmed_rows_and_queue_keep_artwork_after_eviction();
        playlist_images_validate_revision_and_reuse_resident_keys();
        let cache_error = native::check_artwork_cache();
        assert!(cache_error.is_empty(), "{cache_error}");
        let mut object = native::new_app_bridge();
        let bridge = object.pin_mut();
        let thread = bridge.qt_thread();
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

    fn track(id: i32, cover: i32, title: &str) -> Track {
        let mut track = Track::new(title, "Artist", Duration::from_secs(91));
        track.audio_source_id = id;
        track.cover_beatmap_id = Some(cover);
        track
    }
    fn independent_models_keep_item_identity_and_media() {
        let mut object = native::new_app_bridge();
        let mut bridge = object.pin_mut();
        bridge.as_mut().apply(AppUpdate::MediaGeneration {
            generation: 1,
            invalidate_artwork: true,
        });
        let song = track(72, 17, "Song");
        bridge.as_mut().apply(AppUpdate::LibraryTracksReplaced {
            tracks: vec![song.clone()],
            invalidate_artwork: true,
        });
        let first = PlaylistTrack {
            item_id: 100,
            track: song.clone(),
        };
        let mut second = first.clone();
        second.item_id = 200;
        second.track.subtitle = "Hard".into();
        let mut unavailable = first.clone();
        unavailable.item_id = 300;
        unavailable.track.audio_source_id = -1;
        bridge.as_mut().apply(AppUpdate::PlaylistTracksReplaced {
            tracks: vec![first.clone(), second.clone(), unavailable.clone()],
            invalidate_artwork: false,
        });
        assert_ne!(
            bridge.library_ref().tracks_pointer(),
            bridge.playlists_ref().tracks_pointer()
        );
        let library = &bridge.library_ref().model;
        assert_eq!(library.row_count(&QModelIndex::default()), 1);
        assert_eq!(library.rows[0].playlist_item_id, None);
        let playlist = &bridge.playlists_ref().model;
        assert_eq!(playlist.row_count(&QModelIndex::default()), 3);
        let index = playlist.index(0, 0, &QModelIndex::default());
        assert_eq!(playlist.data(&index, 262), QVariant::from(&100));
        let missing = playlist.index(2, 0, &QModelIndex::default());
        assert_eq!(playlist.data(&missing, 263), QVariant::from(&false));
        assert_eq!(playlist.row_count(&index), 0);
        assert!(!playlist.data(&QModelIndex::default(), 257).is_valid());
        bridge
            .as_mut()
            .playlists_mut()
            .rust_mut()
            .model
            .pin_mut()
            .rust_mut()
            .rows[0]
            .artwork = QString::from("first");
        bridge
            .as_mut()
            .playlists_mut()
            .rust_mut()
            .model
            .pin_mut()
            .rust_mut()
            .rows[1]
            .artwork = QString::from("second");
        first_media_reorder(&mut bridge, first, second, unavailable);
        let playlist = &bridge.playlists_ref().model;
        assert_eq!(playlist.rows[0].playlist_item_id, Some(200));
        assert_eq!(playlist.rows[0].artwork, QString::from("second"));
        assert_eq!(playlist.rows[1].artwork, QString::from("first"));
        assert_eq!(
            playlist.rows[0].track.duration,
            Some(Duration::from_secs(91))
        );
        assert_eq!(bridge.library_ref().model.rows[0].track, song);
        bridge.as_mut().apply(AppUpdate::LibrarySelected(Some(72)));
        bridge.as_mut().apply(AppUpdate::Player(PlayerPresentation {
            selected_track: Some(track(42, 52, "Playlist selection")),
            selected_playlist_item_id: Some(200),
            can_play: true,
            ..Default::default()
        }));
        assert_eq!(bridge.library_ref().selected_audio_id, 72);
        assert_eq!(bridge.player_ref().selected_audio_id, 42);
        assert_eq!(bridge.player_ref().selected_playlist_item_id, 200);
        assert_eq!(bridge.generation, 1);
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
    fn first_media_reorder(
        bridge: &mut Pin<&mut ffi::AppBridge>,
        mut first: PlaylistTrack,
        mut second: PlaylistTrack,
        unavailable: PlaylistTrack,
    ) {
        first.track.duration = None;
        second.track.duration = None;
        bridge
            .as_mut()
            .apply(AppUpdate::PlaylistTracksReordered(vec![
                second,
                first,
                unavailable,
            ]));
    }
    fn player_projection_does_not_reset_lists() {
        let mut object = native::new_app_bridge();
        let mut bridge = object.pin_mut();
        bridge.as_mut().apply(AppUpdate::LibraryTracksReplaced {
            tracks: vec![track(7, 17, "A")],
            invalidate_artwork: false,
        });
        let rows = bridge.library_ref().model.rows.as_ptr();
        let selected = track(42, 52, "B");
        bridge.as_mut().apply(AppUpdate::Player(PlayerPresentation {
            selected_track: Some(selected),
            can_play: true,
            position: Duration::ZERO,
            selected_duration: Some(Duration::from_secs(91)),
            ..Default::default()
        }));
        assert_eq!(bridge.player_ref().selected_audio_id, 42);
        assert_eq!(
            bridge.player_ref().selected_duration_label,
            QString::from("01:31")
        );
        assert!(!bridge.player_ref().selected_is_playing && !bridge.player_ref().can_seek);
        bridge.as_mut().apply(AppUpdate::Player(PlayerPresentation {
            selected_track: Some(track(7, 17, "A")),
            selected_is_playing: true,
            can_play: true,
            can_seek: true,
            position: Duration::from_secs(23),
            duration: Some(Duration::from_secs(95)),
            selected_duration: Some(Duration::from_secs(95)),
            ..Default::default()
        }));
        assert!(bridge.player_ref().selected_is_playing && bridge.player_ref().can_seek);
        assert_eq!(
            bridge.player_ref().playback_position_label,
            QString::from("00:23")
        );
        assert_eq!(
            bridge.player_ref().selected_duration_label,
            QString::from("01:35")
        );
        assert_eq!(bridge.library_ref().model.rows.as_ptr(), rows);
        bridge
            .as_mut()
            .apply(AppUpdate::Player(PlayerPresentation::default()));
        assert!(!bridge.player_ref().has_selection);
        assert_eq!(bridge.library_ref().model.track_count, 1);
    }
    fn queue_model_preserves_entry_identity_and_updates_roles() {
        use crate::qt_view_models::{parse_entry_id, queue_move};
        use osu_radio_client::models::{QueueAvailability, QueueEntryId};
        let maximum = QueueEntryId::new(i64::MAX).unwrap();
        let large = QueueEntryId::new(9_007_199_254_740_993).unwrap();
        let mut view = osu_radio_client::mock::MockState::default().queue.view;
        view.entries.truncate(2);
        let mut first = view.entries[0].clone();
        first.id = large;
        first.playlist_item_id = Some(88);
        first.track = track(42, 52, "Duplicate");
        let mut second = first.clone();
        second.id = maximum;
        second.position = 1;
        view.entries = vec![first, second];
        view.current_entry_id = None;
        view.history_len = 1;
        assert_eq!(
            crate::qt_view_models::queue_start_index(&view),
            1,
            "first upcoming without current"
        );
        view.history_len = 2;
        assert_eq!(
            crate::qt_view_models::queue_start_index(&view),
            1,
            "end of history without current"
        );
        view.history_len = 0;
        assert_eq!(crate::qt_view_models::queue_start_index(&view), 0);
        view.current_entry_id = Some(maximum);
        view.history_len = 1;
        assert_eq!(
            crate::qt_view_models::queue_start_index(&view),
            1,
            "current entry"
        );
        let mut model = native::new_track_list_model();
        model.pin_mut().replace_queue_rows(&view, 0);
        let index = model.index(1, 0, &QModelIndex::default());
        assert_eq!(
            model.data(&index, 264).value::<QString>().unwrap(),
            QString::from(i64::MAX.to_string())
        );
        assert!(model.data(&index, 265).value::<bool>().unwrap());
        assert_eq!(
            model.rows[0].playlist_item_id,
            model.rows[1].playlist_item_id
        );
        model.pin_mut().rust_mut().rows[0].artwork = QString::from("resident-cover");
        view.entries[0].track.title = "Unicode 日本語".into();
        view.entries[0].availability = QueueAvailability::Checking;
        model.pin_mut().replace_queue_rows(&view, 0);
        assert_eq!(model.rows[0].artwork, QString::from("resident-cover"));
        view.entries.swap(0, 1);
        view.history_len = 0;
        model.pin_mut().replace_queue_rows(&view, 0);
        assert_eq!(model.entry_id_at(0), QString::from(i64::MAX.to_string()));
        assert_eq!(model.rows[1].artwork, QString::from("resident-cover"));
        let moved = model.index(1, 0, &QModelIndex::default());
        assert_eq!(
            model.data(&moved, 267).value::<QString>().unwrap(),
            QString::from("checking")
        );
        assert_eq!(
            model.data(&moved, 257).value::<QString>().unwrap(),
            QString::from("Unicode 日本語")
        );
        view.entries[1].track.cover_beatmap_id = None;
        model.pin_mut().replace_queue_rows(&view, 0);
        assert!(
            model.rows[1].artwork.is_empty(),
            "changed cover source drops old image"
        );
        view.entries.remove(0);
        model.pin_mut().replace_queue_rows(&view, 0);
        assert_eq!(model.entry_id_at(0), QString::from(large.to_string()));
        assert_eq!(model.track_count, 1);
        view.entries.clear();
        model.pin_mut().replace_queue_rows(&view, 0);
        assert_eq!(model.track_count, 0);
        assert!(model.entry_id_at(-1).is_empty());
        for invalid in ["", "-1", "0", "1.5", "1e3", "9223372036854775808", " 1"] {
            assert!(parse_entry_id(&QString::from(invalid)).is_none());
        }
        let id = QString::from(i64::MAX.to_string());
        assert!(queue_move(&id, &QVariant::from(&42_f64)).is_none());
        assert!(queue_move(&id, &QVariant::from(&QString::from("bad"))).is_none());
        assert!(
            matches!(queue_move(&id, &QVariant::default()), Some(osu_radio_client::view_models::QueueAction::Move { id, before_entry_id: None }) if id == maximum)
        );
    }
    fn warmed_rows_and_queue_keep_artwork_after_eviction() {
        let mut object = native::new_app_bridge();
        let mut bridge = object.pin_mut();
        bridge.as_mut().media_generation(1, true);
        let selected = track(7, 17, "Selected");
        let other = track(42, 52, "Other");
        bridge.as_mut().apply(AppUpdate::LibraryTracksReplaced {
            tracks: vec![selected.clone(), other.clone()],
            invalidate_artwork: true,
        });
        bridge.as_mut().apply(AppUpdate::Player(PlayerPresentation {
            selected_track: Some(selected.clone()),
            ..Default::default()
        }));
        let epoch = bridge.cache_epoch;
        let mut pixels = b"P6\n1280 1280\n255\n".to_vec();
        pixels.resize(pixels.len() + 1280 * 1280 * 3, 100);
        let large = QByteArray::from(pixels.as_slice());
        assert!(!native::install_artwork(epoch, 17, &large).is_empty());
        assert!(!native::install_artwork(epoch, 52, &large).is_empty());
        bridge.as_mut().update_artwork(&[17, 52]);
        let pinned = bridge.player_ref().selected_artwork_url.clone();
        let displayed = bridge.library_ref().model.rows[1].artwork.clone();
        bridge.as_mut().queue_mut().rust_mut().queue_state.entries = (1..=4)
            .map(|id| {
                let mut track = if id == 4 {
                    selected.clone()
                } else {
                    other.clone()
                };
                if id > 2 {
                    track.audio_source_id = -1;
                }
                osu_radio_client::view_models::QueueRow {
                    id: osu_radio_client::models::QueueEntryId::new(id).unwrap(),
                    position: usize::try_from(id.saturating_sub(1)).unwrap(),
                    audio_source_id: (id <= 2).then_some(other.audio_source_id),
                    playlist_item_id: None,
                    availability: osu_radio_client::models::QueueAvailability::Available,
                    track,
                }
            })
            .collect();
        bridge.as_mut().queue_mut().project_queue();
        let queue = bridge.queue_ref().model.rows.clone();
        assert_eq!(queue.len(), 4);
        assert_ne!(
            queue.first().unwrap().queue.as_ref().unwrap().id,
            queue.get(1).unwrap().queue.as_ref().unwrap().id,
            "duplicate audio retains entry identity"
        );
        bridge.as_mut().apply(AppUpdate::PlaylistTracksReplaced {
            tracks: vec![],
            invalidate_artwork: false,
        });
        bridge.as_mut().apply(AppUpdate::LibraryTracksReplaced {
            tracks: vec![selected.clone(), other],
            invalidate_artwork: false,
        });
        assert_eq!(bridge.player_ref().selected_artwork_url, pinned);
        assert_eq!(bridge.library_ref().model.rows[1].artwork, displayed);
        for id in 100..115 {
            assert_eq!(native::install_artwork(epoch, id, &large).len(), 1);
        }
        assert!(native::artwork_url(epoch, 52).is_empty());
        assert_eq!(native::artwork_url(epoch, 17), pinned);
        bridge.as_mut().update_artwork(&[52]);
        assert_eq!(bridge.library_ref().model.rows[1].artwork, displayed);
        assert!(bridge.queue_ref().model.rows == queue);
        assert!(!native::install_artwork(epoch, 52, &large).is_empty());
        bridge.as_mut().update_artwork(&[52]);
        assert_ne!(bridge.library_ref().model.rows[1].artwork, displayed);
        bridge.as_mut().media_generation(2, true);
        assert!(
            bridge
                .queue_ref()
                .model
                .rows
                .iter()
                .all(|row| row.artwork.is_empty())
        );
        assert!(native::install_artwork(epoch, 17, &large).is_empty());
        assert!(bridge.player_ref().selected_artwork_url.is_empty());
        bridge.as_mut().apply(AppUpdate::Player(PlayerPresentation {
            selected_track: Some(selected),
            ..Default::default()
        }));
        assert!(bridge.player_ref().selected_artwork_url.is_empty());
        let fresh_epoch = bridge.cache_epoch;
        assert!(
            native::install_artwork(fresh_epoch, 17, &QByteArray::from(b"bad image".as_slice()))
                .is_empty()
        );
        bridge.as_mut().update_artwork(&[17]);
        assert!(bridge.player_ref().selected_artwork_url.is_empty());
        assert!(!native::install_artwork(fresh_epoch, 17, &large).is_empty());
        bridge.as_mut().update_artwork(&[17]);
        assert!(!bridge.player_ref().selected_artwork_url.is_empty());
        assert_ne!(bridge.player_ref().selected_artwork_url, pinned);
    }
    fn playlist_images_validate_revision_and_reuse_resident_keys() {
        let mut object = native::new_app_bridge();
        let mut bridge = object.pin_mut();
        bridge.as_mut().media_generation(1, true);
        let epoch = bridge.cache_epoch;
        bridge.as_mut().apply(AppUpdate::Playlists(PlaylistsState {
            playlists: vec![osu_radio_client::models::PlaylistSummary {
                id: 11,
                name: "Cover".into(),
                item_count: 1,
                cover_beatmap_id: Some(17),
                custom_cover_revision: Some(1),
            }],
            ..Default::default()
        }));
        let bytes = b"P6\n1 1\n255\n\x50\x64\x78".to_vec();
        bridge.as_mut().apply(AppUpdate::PlaylistCover {
            id: 11,
            revision: 2,
            bytes: bytes.clone(),
        });
        assert!(native::playlist_artwork_url(epoch, 11, 2, -1).is_empty());
        bridge.as_mut().apply(AppUpdate::PlaylistCover {
            id: 11,
            revision: 1,
            bytes: bytes.clone(),
        });
        let resident = native::playlist_artwork_url(epoch, 11, 1, -1);
        assert!(!resident.is_empty());
        bridge.as_mut().apply(AppUpdate::PlaylistCover {
            id: 11,
            revision: 1,
            bytes,
        });
        assert_eq!(native::playlist_artwork_url(epoch, 11, 1, -1), resident);
    }
}
