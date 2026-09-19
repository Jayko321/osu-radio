use osu_radio_client::controller::AppController;
use std::sync::{Arc, Mutex};
use tokio::{
    runtime::{Handle, Runtime},
    task::JoinHandle,
};

#[cxx::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("src/runtime.h");
        include!("src/native_tests.h");
        type AppBridge = crate::app_bridge::ffi::AppBridge;
        #[rust_name = "new_app_bridge"]
        #[must_use]
        fn newAppBridge() -> UniquePtr<AppBridge>;
        #[rust_name = "check_artwork_cache"]
        #[must_use]
        fn checkArtworkCache() -> QString;
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qbytearray.h");
        type QByteArray = cxx_qt_lib::QByteArray;
        include!("cxx-qt-lib/qvector.h");
        type QVector_i32 = cxx_qt_lib::QVector<i32>;
        #[rust_name = "configure_engine"]
        fn configureEngine(engine: Pin<&mut QQmlApplicationEngine>, smoke: bool);
        #[rust_name = "configure_probe"]
        fn configureProbe(engine: Pin<&mut QQmlApplicationEngine>, gallery: bool);
        #[rust_name = "reset_artwork"]
        #[allow(clippy::must_use_candidate)]
        // Clearing is useful without retaining the epoch.
        fn resetArtwork() -> u64;
        #[rust_name = "artwork_url"]
        #[must_use]
        fn artworkUrl(generation: u64, id: i32) -> QString;
        #[rust_name = "install_artwork"]
        #[must_use]
        fn installArtwork(generation: u64, id: i32, bytes: &QByteArray) -> QVector_i32;
    }
    impl UniquePtr<AppBridge> {}
}

static CONTEXT: Mutex<Option<Arc<RuntimeContext>>> = Mutex::new(None);

pub struct RuntimeContext {
    pub handle: Handle,
    controllers: Mutex<Vec<AppController>>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}
impl RuntimeContext {
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
