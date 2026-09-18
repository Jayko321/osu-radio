#[cxx::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("src/runtime.h");
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;
        #[rust_name = "configure_engine"]
        fn configureEngine(engine: Pin<&mut QQmlApplicationEngine>, smoke: bool);
    }
}
