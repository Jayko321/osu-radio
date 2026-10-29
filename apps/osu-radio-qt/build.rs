use cxx_qt_build::{CxxQtBuilder, QResource, QResources, QmlFile, QmlModule};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn files_under(root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            files.extend(files_under(&path)?);
        } else {
            // QML module entries and resource aliases need URL separators on Windows too.
            files.push(PathBuf::from(path.to_string_lossy().replace('\\', "/")));
        }
    }
    files.sort();
    Ok(files)
}

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::var("CARGO_CFG_TARGET_OS")?.as_str() == "windows" {
        println!("cargo:rerun-if-changed=src/windows.rc");
        println!("cargo:rerun-if-changed=../../packaging/icons/osu-radio.ico");
        let resource = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("missing OUT_DIR")?)
            .join("osu-radio.res");
        let status = Command::new("rc.exe")
            .arg("/nologo")
            .arg(format!("/fo{}", resource.display()))
            .arg("src/windows.rc")
            .status()?;
        if !status.success() {
            return Err("Windows resource compiler failed".into());
        }
        println!(
            "cargo:rustc-link-arg-bin=osu-radio-qt={}",
            resource.display()
        );
    }
    println!("cargo:rerun-if-changed=qml");
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=src/runtime.h");
    println!("cargo:rerun-if-changed=src/view_models.h");
    println!("cargo:rerun-if-changed=src/artwork.h");
    println!("cargo:rerun-if-changed=src/native_tests.h");
    let qml = files_under(Path::new("qml"))?;
    let module = QmlModule::new("OsuRadio")
        .depend("QtQml")
        .depend("QtQml.Models")
        .version(1, 0)
        .qml_files(
            qml.iter()
                .filter(|p| p.extension().is_some_and(|e| e == "qml"))
                .map(|p| {
                    QmlFile::from(p).singleton(p.file_name().is_some_and(|n| n == "Theme.qml"))
                }),
        );
    let builder = CxxQtBuilder::new_qml_module(module)
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .qt_module("Network")
        .files([
            "src/bridge.rs",
            "src/runtime.rs",
            "src/app_bridge.rs",
            "src/qt_view_models.rs",
        ])
        .qrc_resources(["tests/AdapterProbe.qml", "tests/VisualProbe.qml"])
        .qrc_resources(
            QResources::new().resource(
                QResource::new()
                    .prefix("/")
                    .files(files_under(Path::new("assets"))?),
            ),
        )
        .qrc_resources(
            QResources::new().resource(
                QResource::new()
                    .prefix("/qt/qml/OsuRadio")
                    .files(qml.iter().filter(|p| p.extension().is_none())),
            ),
        );
    // SAFETY: only adds our header directory; no ABI/compiler settings are changed.
    unsafe {
        builder.cc_builder(|cc| {
            cc.include(".");
        })
    }
    .build();
    Ok(())
}
