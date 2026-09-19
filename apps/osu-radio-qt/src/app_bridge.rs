//! Typed Qt projection; workflows remain in osu-radio-client.
use crate::runtime::{RuntimeContext, ffi as native};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{
    QByteArray, QHash, QHashPair_i32_QByteArray, QList, QMap, QMapPair_QString_QVariant,
    QModelIndex, QString, QVariant,
};
use osu_radio_client::{
    ServerOptions, Track,
    controller::{AppCommand, AppController, AppUpdate, ConnectionStatus, MediaTicket},
};
use std::{
    collections::HashMap,
    pin::Pin,
    sync::{Arc, Mutex},
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
        #[qproperty(i32, track_count, cxx_name = "trackCount", READ, NOTIFY)]
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
        type AppBridge = super::AppBridgeRust;
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
        #[cxx_name = "connectSession"]
        fn connect_session(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "searchLibrary"]
        fn search_library(self: Pin<&mut AppBridge>, query: &QString);

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
        #[cxx_name = "selectFolder"]
        fn select_folder(self: Pin<&mut AppBridge>, id: i32);
        #[qinvokable]
        #[cxx_name = "addFolder"]
        fn add_folder(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "retryFolders"]
        fn retry_folders(self: Pin<&mut AppBridge>);
        #[qinvokable]
        #[cxx_name = "requestMedia"]
        fn request_media(self: Pin<&mut AppBridge>, id: i32);
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
    track_count: i32,
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
    controller: Option<AppController>,
    generation: u64,
    cache_epoch: u64,
}
impl Default for AppBridgeRust {
    fn default() -> Self {
        Self {
            track_count: Default::default(),
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
            controller: None,
            generation: 0,
            cache_epoch: 0,
        }
    }
}
struct Row {
    track: Track,
    artwork: QString,
}
const ROLES: [(i32, &str); 6] = [
    (256, "audioId"),
    (257, "title"),
    (258, "artist"),
    (259, "subtitle"),
    (260, "durationLabel"),
    (261, "artworkUrl"),
];
macro_rules! property_setter {
    ($setter:ident, $field:ident, $notify:ident, $ty:ty) => {
        fn $setter(mut self: Pin<&mut Self>, value: $ty) {
            if self.rust().$field != value {
                self.as_mut().rust_mut().$field = value;
                self.$notify();
            }
        }
    };
}
impl ffi::AppBridge {
    property_setter!(set_track_count, track_count, track_count_changed, i32);
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
        }
        self.command(AppCommand::Connect);
    }
    pub fn search_library(self: Pin<&mut Self>, query: &QString) {
        self.command(AppCommand::SearchLibrary(query.to_string()));
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
    pub fn select_folder(self: Pin<&mut Self>, id: i32) {
        self.command(AppCommand::SelectFolder((id >= 0).then_some(id)));
    }
    pub fn add_folder(self: Pin<&mut Self>) {
        self.command(AppCommand::BeginFolderPick);
    }
    pub fn retry_folders(self: Pin<&mut Self>) {
        self.command(AppCommand::RetryFolders);
    }
    pub fn request_media(self: Pin<&mut Self>, id: i32) {
        let Some(row) = self
            .audio_rows
            .get(&id)
            .and_then(|index| self.rows.get(*index))
        else {
            return;
        };
        let missing = row
            .track
            .cover_beatmap_id
            .is_some_and(|cover| native::artwork_url(self.cache_epoch, cover).is_empty());
        self.command(AppCommand::RequestMedia {
            audio_id: id,
            artwork_missing: missing,
        });
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
        let artwork = track
            .as_ref()
            .and_then(|t| t.cover_beatmap_id)
            .map(|id| native::artwork_url(self.cache_epoch, id))
            .unwrap_or_default();
        self.as_mut()
            .set_selected_audio_id(track.as_ref().map_or(-1, |t| t.audio_source_id));
        self.as_mut().set_has_selection(track.is_some());
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
        if let Some(track) = track {
            self.request_media(track.audio_source_id);
        }
    }
    fn update_artwork(mut self: Pin<&mut Self>, affected: &[i32]) {
        for cover in affected {
            let url = native::artwork_url(self.cache_epoch, *cover);
            let indices = self.cover_rows.get(cover).cloned().unwrap_or_default();
            for index in indices {
                if let Some(row) = self.as_mut().rust_mut().rows.get_mut(index) {
                    row.artwork = url.clone();
                }
                self.as_mut().notify_row(index, &[261]);
            }
        }
        let selected = self
            .audio_rows
            .get(&self.selected_audio_id)
            .and_then(|index| self.rows.get(*index))
            .map(|r| r.artwork.clone())
            .unwrap_or_default();
        self.set_selected_artwork_url(selected);
    }
    fn replace_tracks(mut self: Pin<&mut Self>, tracks: Vec<Track>) {
        let generation = self.generation.saturating_add(1);
        let cache_epoch = native::reset_artwork();
        self.as_mut().rust_mut().cache_epoch = cache_epoch;
        // SAFETY: matching begin/end bracket the complete row replacement.
        unsafe {
            self.as_mut().begin_reset_model();
        }
        self.as_mut().rust_mut().generation = generation;
        self.as_mut().rust_mut().rows = tracks
            .into_iter()
            .map(|track| Row {
                track,
                artwork: QString::default(),
            })
            .collect();
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
        unsafe {
            self.as_mut().end_reset_model();
        }
        let count = i32::try_from(self.rows.len()).unwrap_or(i32::MAX);
        self.as_mut().set_track_count(count);
        self.set_selected_artwork_url(QString::default());
    }
    fn apply(mut self: Pin<&mut Self>, update: AppUpdate) {
        match update {
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
            AppUpdate::TracksReplaced(tracks) => self.replace_tracks(tracks),
            AppUpdate::TrackChanged(track) => {
                if let Some(index) = self.audio_rows.get(&track.audio_source_id).copied() {
                    if let Some(row) = self.as_mut().rust_mut().rows.get_mut(index) {
                        row.track = track;
                    }
                    self.notify_row(index, &[257, 258, 259, 260]);
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
            AppUpdate::FolderPickerRequested => {
                if let (Some(context), Some(controller)) =
                    (RuntimeContext::current(), self.controller.clone())
                {
                    let task = context.handle.spawn(async move {
                        let path = rfd::AsyncFileDialog::new()
                            .set_title("Select osu! installation")
                            .pick_folder()
                            .await
                            .map(|file| file.path().to_path_buf());
                        controller.send(AppCommand::CompleteFolderPick(path));
                    });
                    context.track(task);
                }
            }
            AppUpdate::Artwork { .. } => {} // Dispatched with its acknowledgement guard.
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

    #[test]
    fn native_model_cache_and_destroyed_object_contracts() {
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
        model
            .as_mut()
            .apply(AppUpdate::TracksReplaced(vec![track.clone()]));
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
        model.as_mut().apply(AppUpdate::TracksReplaced(Vec::new()));
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
}
