use cxx_qt_build::{CxxQtBuilder, QResource, QResources, QmlFile, QmlModule};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

fn files_under(root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            files.extend(files_under(&path)?);
        } else {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=qml");
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=src/runtime.h");
    println!("cargo:rerun-if-changed=src/artwork.h");
    println!("cargo:rerun-if-changed=src/native_tests.h");
    let qml = files_under(Path::new("qml"))?;
    let module = QmlModule::new("OsuRadio")
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
        .files(["src/bridge.rs", "src/runtime.rs", "src/app_bridge.rs"])
        .qrc_resources(["tests/AdapterProbe.qml"])
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
