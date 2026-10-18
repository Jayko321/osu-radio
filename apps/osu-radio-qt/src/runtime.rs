use osu_radio_client::controller::AppController;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::{
    runtime::{Handle, Runtime},
    task::JoinHandle,
};

#[cxx::bridge]
pub mod ffi {
    extern "Rust" {
        fn attach_window_media(hwnd: isize);
        fn begin_window_media_detach();
        fn window_media_detached() -> bool;
    }
    unsafe extern "C++" {
        include!("src/runtime.h");
        include!("src/native_tests.h");
        type AppBridge = crate::app_bridge::ffi::AppBridge;
        include!("src/view_models.h");
        type LibraryViewModel = crate::qt_view_models::ffi::LibraryViewModel;
        #[rust_name = "new_library_view_model"]
        #[must_use]
        fn newLibraryViewModel() -> UniquePtr<LibraryViewModel>;
        type PlaylistsViewModel = crate::qt_view_models::ffi::PlaylistsViewModel;
        #[rust_name = "new_playlists_view_model"]
        #[must_use]
        fn newPlaylistsViewModel() -> UniquePtr<PlaylistsViewModel>;
        type SettingsViewModel = crate::qt_view_models::ffi::SettingsViewModel;
        #[rust_name = "new_settings_view_model"]
        #[must_use]
        fn newSettingsViewModel() -> UniquePtr<SettingsViewModel>;
        type PlayerViewModel = crate::qt_view_models::ffi::PlayerViewModel;
        #[rust_name = "new_player_view_model"]
        #[must_use]
        fn newPlayerViewModel() -> UniquePtr<PlayerViewModel>;
        type QueueViewModel = crate::qt_view_models::ffi::QueueViewModel;
        #[rust_name = "new_queue_view_model"]
        #[must_use]
        fn newQueueViewModel() -> UniquePtr<QueueViewModel>;
        type TrackListModel = crate::qt_view_models::ffi::TrackListModel;
        #[rust_name = "new_track_list_model"]
        #[must_use]
        fn newTrackListModel() -> UniquePtr<TrackListModel>;

        #[rust_name = "new_app_bridge"]
        #[must_use]
        fn newAppBridge() -> UniquePtr<AppBridge>;
        #[rust_name = "check_artwork_cache"]
        #[must_use]
        fn checkArtworkCache() -> QString;
        #[rust_name = "check_playlist_cover_preparation"]
        #[must_use]
        fn checkPlaylistCoverPreparation() -> QString;
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        #[rust_name = "load_local_preferences"]
        #[must_use]
        fn loadLocalPreferences(titles: &mut bool, artists: &mut bool, discord: &mut bool) -> bool;
        #[rust_name = "save_track_name_preference"]
        #[must_use]
        fn saveTrackNamePreference(title: bool, enabled: bool) -> QString;
        #[rust_name = "save_discord_rich_presence"]
        #[must_use]
        fn saveDiscordRichPresence(enabled: bool) -> QString;
        include!("cxx-qt-lib/qbytearray.h");
        type QByteArray = cxx_qt_lib::QByteArray;
        include!("cxx-qt-lib/qvector.h");
        type QVector_i32 = cxx_qt_lib::QVector<i32>;
        #[rust_name = "configure_engine"]
        fn configureEngine(engine: Pin<&mut QQmlApplicationEngine>, smoke: bool);
        #[rust_name = "configure_probe"]
        fn configureProbe(engine: Pin<&mut QQmlApplicationEngine>, gallery: bool);
        type LiveWindowMedia;
        #[rust_name = "connect_live_window"]
        #[must_use]
        fn connectLiveWindow(engine: Pin<&mut QQmlApplicationEngine>)
        -> UniquePtr<LiveWindowMedia>;
        #[rust_name = "reset_artwork"]
        #[allow(clippy::must_use_candidate)]
        // Clearing is useful without retaining the epoch.
        fn resetArtwork() -> u64;
        #[rust_name = "artwork_url"]
        #[must_use]
        fn artworkUrl(generation: u64, id: i32) -> QString;
        #[rust_name = "protect_artwork"]
        fn protectArtwork(generation: u64, id: i32);
        #[rust_name = "install_artwork"]
        #[must_use]
        fn installArtwork(generation: u64, id: i32, bytes: &QByteArray) -> QVector_i32;
        #[rust_name = "playlist_artwork_url"]
        #[must_use]
        fn playlistArtworkUrl(generation: u64, id: i32, revision: i64, beatmap: i32) -> QString;
        #[rust_name = "install_playlist_artwork"]
        #[must_use]
        fn installPlaylistArtwork(
            generation: u64,
            id: i32,
            revision: i64,
            beatmap: i32,
            bytes: &QByteArray,
        ) -> QVector_i32;
        #[rust_name = "draft_artwork_url"]
        #[must_use]
        fn draftArtworkUrl(generation: u64, serial: u64) -> QString;
        #[rust_name = "install_draft_artwork"]
        #[must_use]
        fn installDraftArtwork(generation: u64, serial: u64, bytes: &QByteArray) -> QVector_i32;
        #[rust_name = "prepare_playlist_cover"]
        #[must_use]
        fn preparePlaylistCover(url: &QString, error: &mut QString) -> QByteArray;
    }
    impl UniquePtr<AppBridge> {}
    impl UniquePtr<LibraryViewModel> {}
    impl UniquePtr<PlaylistsViewModel> {}
    impl UniquePtr<SettingsViewModel> {}
    impl UniquePtr<PlayerViewModel> {}
    impl UniquePtr<QueueViewModel> {}
    impl UniquePtr<TrackListModel> {}
}

static CONTEXT: Mutex<Option<Arc<RuntimeContext>>> = Mutex::new(None);

pub struct RuntimeContext {
    pub handle: Handle,
    controllers: Mutex<Vec<AppController>>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
    window_media: Mutex<WindowMedia>,
}

#[derive(Default)]
struct WindowMedia {
    attach: Option<JoinHandle<()>>,
    detached: Option<Arc<AtomicBool>>,
}

fn attach_window_media(hwnd: isize) {
    let Some(runtime) = RuntimeContext::current() else {
        return;
    };
    #[cfg(target_os = "windows")]
    let context = {
        let Some(hwnd) = std::num::NonZeroIsize::new(hwnd) else {
            eprintln!("System media controls: no native Windows window");
            return;
        };
        osu_radio_client::media_controls::NativeContext::Windows { hwnd }
    };
    #[cfg(target_os = "linux")]
    let context = {
        let _ = hwnd;
        osu_radio_client::media_controls::NativeContext::Linux
    };
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let controllers = runtime.controllers();
        let task = runtime.handle.spawn(async move {
            for controller in controllers {
                if let Err(error) = controller.connect_system_media(context).await {
                    eprintln!("System media controls: {error}");
                }
                controller.publish_system_media();
            }
        });
        runtime
            .window_media
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .attach = Some(task);
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    let _ = (runtime, hwnd);
}

fn begin_window_media_detach() {
    let Some(runtime) = RuntimeContext::current() else {
        return;
    };
    let mut media = runtime
        .window_media
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if media.detached.is_some() {
        return;
    }
    let controllers = runtime.controllers();
    let attach = media.attach.take();
    let detached = Arc::new(AtomicBool::new(false));
    media.detached = Some(detached.clone());
    drop(media);
    runtime.track(runtime.handle.spawn(async move {
        // Close can arrive before the queued attach has reached the controller.
        if let Some(attach) = attach {
            let _ = attach.await;
        }
        if std::env::var_os("OSU_RADIO_QT_SMOKE_TEST").is_some()
            && std::env::var("OSU_RADIO_QT_PROBE_CASE")
                .is_ok_and(|case| case.starts_with("system-media"))
        {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
        for controller in controllers {
            controller.disconnect_system_media().await;
        }
        detached.store(true, Ordering::Release);
    }));
}

fn window_media_detached() -> bool {
    RuntimeContext::current().is_none_or(|runtime| {
        runtime
            .window_media
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .detached
            .as_ref()
            .is_some_and(|done| done.load(Ordering::Acquire))
    })
}

impl RuntimeContext {
    fn controllers(&self) -> Vec<AppController> {
        self.controllers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    pub fn current() -> Option<Arc<Self>> {
        CONTEXT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
    pub fn retain(&self, controller: AppController) {
        self.controllers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(controller);
    }
    pub fn track(&self, task: JoinHandle<()>) {
        let mut tasks = self
            .tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        tasks.retain(|task| !task.is_finished());
        tasks.push(task);
    }
}

/// Launcher ownership outlives all `QObjects`, child cleanup and noncancelable image work.
pub struct LiveRuntime {
    runtime: Runtime,
    context: Arc<RuntimeContext>,
}
impl LiveRuntime {
    pub fn new() -> std::io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let context = Arc::new(RuntimeContext {
            handle: runtime.handle().clone(),
            controllers: Mutex::new(Vec::new()),
            tasks: Mutex::new(Vec::new()),
            window_media: Mutex::new(WindowMedia::default()),
        });
        *CONTEXT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(context.clone());
        Ok(Self { runtime, context })
    }
    pub fn shutdown(self) {
        *CONTEXT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        let controllers = std::mem::take(
            &mut *self
                .context
                .controllers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        let tasks = std::mem::take(
            &mut *self
                .context
                .tasks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        self.runtime.block_on(async {
            for controller in controllers {
                controller.shutdown().await;
            }
            for task in &tasks {
                task.abort();
            }
            for task in tasks {
                let _ = task.await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        time::{Duration, Instant},
    };

    struct Dropped(Arc<AtomicBool>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    fn early_detach_waits_for_pending_attach(live: &LiveRuntime, context: &RuntimeContext) {
        let (release_attach, blocked_attach) = tokio::sync::oneshot::channel();
        context.window_media.lock().unwrap().attach = Some(context.handle.spawn(async move {
            let _ = blocked_attach.await;
        }));
        begin_window_media_detach();
        assert!(!window_media_detached());
        begin_window_media_detach();
        release_attach.send(()).unwrap();
        live.runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), async {
                while !window_media_detached() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        });
        begin_window_media_detach();
        assert!(window_media_detached());
    }

    #[test]
    fn runtime_prunes_completed_work_and_waits_for_controller_and_task_cleanup() {
        assert!(RuntimeContext::current().is_none());
        let live = LiveRuntime::new().unwrap();
        let context = RuntimeContext::current().unwrap();
        assert!(Arc::ptr_eq(&context, &live.context));

        let finished = context.handle.spawn(async {});
        live.runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), async {
                while !finished.is_finished() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        });
        context.track(finished);
        let controller_stopped = Arc::new(AtomicBool::new(false));
        let controller_guard = Dropped(controller_stopped.clone());
        context.retain(AppController::spawn(
            &context.handle,
            osu_radio_client::ServerOptions::default(),
            move |_| {
                let _ = &controller_guard;
            },
        ));

        // An immediate close must await even an attach that has not run yet.
        early_detach_waits_for_pending_attach(&live, &context);

        let cancelled = Arc::new(AtomicBool::new(false));
        let pending_guard = Dropped(cancelled.clone());
        let (started, ready) = tokio::sync::oneshot::channel();
        let pending = context.handle.spawn(async move {
            let _guard = pending_guard;
            let _ = started.send(());
            std::future::pending::<()>().await;
        });
        live.runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(5), ready)
                .await
                .unwrap()
                .unwrap();
        });
        let fallback_abort = pending.abort_handle();
        context.track(pending);
        assert_eq!(
            context.tasks.lock().unwrap().len(),
            1,
            "completed work must not accumulate"
        );

        let (release, blocked) = mpsc::channel();
        let (started, ready) = mpsc::channel();
        let completed = Arc::new(AtomicBool::new(false));
        let flag = completed.clone();
        context.track(context.handle.spawn_blocking(move || {
            started.send(()).unwrap();
            blocked.recv().unwrap();
            flag.store(true, Ordering::SeqCst);
        }));
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let ordered = loop {
                if cancelled.load(Ordering::SeqCst)
                    && controller_stopped.load(Ordering::SeqCst)
                    && RuntimeContext::current().is_none()
                {
                    break true;
                }
                if Instant::now() >= deadline {
                    break false;
                }
                std::thread::yield_now();
            };
            // Release both tasks even on failure so a broken shutdown cannot hang this test.
            fallback_abort.abort();
            release.send(()).unwrap();
            ordered
        });
        live.shutdown();
        assert!(
            worker.join().unwrap(),
            "shutdown must clear context and stop controllers before completing tracked work"
        );
        assert!(
            completed.load(Ordering::SeqCst),
            "started blocking work must finish before shutdown returns"
        );
        assert!(RuntimeContext::current().is_none());
        assert!(context.tasks.lock().unwrap().is_empty());
        assert!(context.controllers.lock().unwrap().is_empty());
    }
}
