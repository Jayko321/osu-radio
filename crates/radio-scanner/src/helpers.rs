use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use ignore::{DirEntry, Error, WalkBuilder, WalkState};
use radio_core::{OsuKind, OsuMarker};
#[cfg(windows)]
use sysinfo::Disks;

#[cfg(windows)]
const WINDOWS_IGNORED_DIR_NAMES: &[&str] = &[
    "$recycle.bin",
    "system volume information",
    "recovery",
    "$winreagent",
    "$windows.~bt",
    "$windows.~ws",
    "perflogs",
];

#[cfg(not(windows))]
const UNIX_IGNORED_ROOT_DIRS: &[&str] = &[
    "/proc",
    "/sys",
    "/dev",
    "/run",
    "/tmp",
    "/var/tmp",
    "/var/cache",
    "/var/log",
    "/lost+found",
];

fn system_scan_roots() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let disks = Disks::new_with_refreshed_list();

        disks
            .list()
            .iter()
            .map(|disk| disk.mount_point().to_path_buf())
            .collect()
    }

    #[cfg(not(windows))]
    {
        vec![PathBuf::from("/")]
    }
}

pub fn find_osu_markers() -> Vec<OsuMarker> {
    let mut res = Vec::new();
    for root in system_scan_roots() {
        res.append(&mut find_osu_markers_in(&root));
    }

    res
}

fn find_osu_markers_in(root: &Path) -> Vec<OsuMarker> {
    let result = Arc::new(Mutex::new(Vec::new()));

    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(false)
        .ignore(false)
        .parents(false)
        .follow_links(false)
        .filter_entry(should_scan_entry)
        .build_parallel();

    walker.run(|| {
        let result = Arc::clone(&result);
        Box::new(move |entry: Result<DirEntry, Error>| {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => return WalkState::Continue,
            };
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                return WalkState::Continue;
            }

            let file_name = entry.file_name().to_string_lossy();

            let kind = match file_name.as_ref() {
                "client.realm" => OsuKind::Lazer,
                "osu!.db" => OsuKind::Stable,
                _ => return WalkState::Continue,
            };

            let Some(parent) = entry.path().parent() else {
                return WalkState::Continue;
            };

            let res = OsuMarker {
                kind,
                marker_path: entry.path().to_path_buf(),
                root_path: parent.to_path_buf(),
            };
            result.lock().unwrap().push(res);

            WalkState::Continue
        })
    });

    Arc::try_unwrap(result).unwrap().into_inner().unwrap()
}

fn should_scan_entry(entry: &DirEntry) -> bool {
    if !entry.file_type().is_some_and(|ft| ft.is_dir()) {
        return true;
    }

    !is_ignored_scan_dir(entry.path())
}

#[cfg(windows)]
fn is_ignored_scan_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    let name = name.to_ascii_lowercase();
    if WINDOWS_IGNORED_DIR_NAMES.contains(&name.as_str()) {
        return true;
    }

    path_ends_with_ci(path, &["windows", "temp"])
        || path_ends_with_ci(path, &["appdata", "local", "temp"])
}

#[cfg(windows)]
fn path_ends_with_ci(path: &Path, suffix: &[&str]) -> bool {
    let mut components = path
        .components()
        .rev()
        .filter_map(|component| component.as_os_str().to_str());

    suffix.iter().rev().all(|expected| {
        components
            .next()
            .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
    })
}

#[cfg(not(windows))]
fn is_ignored_scan_dir(path: &Path) -> bool {
    if UNIX_IGNORED_ROOT_DIRS
        .iter()
        .any(|ignored| path == Path::new(ignored))
    {
        return true;
    }

    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    name == ".cache"
        || name == ".Trash"
        || name.starts_with(".Trash-")
        || path.ends_with(".local/share/Trash")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn ignores_windows_system_dirs() {
        assert!(is_ignored_scan_dir(Path::new(r"C:\$Recycle.Bin")));
        assert!(is_ignored_scan_dir(Path::new(
            r"C:\System Volume Information"
        )));
        assert!(is_ignored_scan_dir(Path::new(r"C:\Recovery")));
        assert!(is_ignored_scan_dir(Path::new(r"C:\$WinREAgent")));
        assert!(is_ignored_scan_dir(Path::new(r"C:\$WINDOWS.~BT")));
        assert!(is_ignored_scan_dir(Path::new(r"C:\$WINDOWS.~WS")));
        assert!(is_ignored_scan_dir(Path::new(r"C:\PerfLogs")));
    }

    #[cfg(windows)]
    #[test]
    fn ignores_windows_temp_dirs() {
        assert!(is_ignored_scan_dir(Path::new(r"C:\Windows\Temp")));
        assert!(is_ignored_scan_dir(Path::new(
            r"C:\Users\me\AppData\Local\Temp"
        )));
    }

    #[cfg(windows)]
    #[test]
    fn keeps_windows_plausible_osu_locations() {
        assert!(!is_ignored_scan_dir(Path::new(r"C:\Program Files")));
        assert!(!is_ignored_scan_dir(Path::new(
            r"C:\Users\me\AppData\Local\osu!"
        )));
        assert!(!is_ignored_scan_dir(Path::new(r"D:\Games")));
    }

    #[cfg(not(windows))]
    #[test]
    fn ignores_unix_system_dirs() {
        assert!(is_ignored_scan_dir(Path::new("/proc")));
        assert!(is_ignored_scan_dir(Path::new("/sys")));
        assert!(is_ignored_scan_dir(Path::new("/dev")));
        assert!(is_ignored_scan_dir(Path::new("/run")));
        assert!(is_ignored_scan_dir(Path::new("/tmp")));
        assert!(is_ignored_scan_dir(Path::new("/var/tmp")));
        assert!(is_ignored_scan_dir(Path::new("/var/cache")));
        assert!(is_ignored_scan_dir(Path::new("/var/log")));
        assert!(is_ignored_scan_dir(Path::new("/lost+found")));
    }

    #[cfg(not(windows))]
    #[test]
    fn ignores_unix_cache_and_trash_dirs() {
        assert!(is_ignored_scan_dir(Path::new("/home/me/.cache")));
        assert!(is_ignored_scan_dir(Path::new("/home/me/.Trash")));
        assert!(is_ignored_scan_dir(Path::new("/home/me/.Trash-1000")));
        assert!(is_ignored_scan_dir(Path::new(
            "/home/me/.local/share/Trash"
        )));
    }

    #[cfg(not(windows))]
    #[test]
    fn keeps_unix_plausible_osu_locations() {
        assert!(!is_ignored_scan_dir(Path::new("/mnt")));
        assert!(!is_ignored_scan_dir(Path::new("/media")));
        assert!(!is_ignored_scan_dir(Path::new("/snap")));
        assert!(!is_ignored_scan_dir(Path::new("/flatpak")));
        assert!(!is_ignored_scan_dir(Path::new("/home/me/.local/share/osu")));
    }
}
