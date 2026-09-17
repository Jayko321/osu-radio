use std::{
    ops::ControlFlow,
    path::{Path, PathBuf},
};

use ignore::{DirEntry, Error, WalkBuilder, WalkState};
use radio_core::{OsuKind, OsuMarker};
#[cfg(windows)]
use sysinfo::Disks;

use super::Collector;

const IGNORED_DIR_NAMES: &[&str] = &["node_modules", ".git"];

#[cfg(windows)]
const WINDOWS_IGNORED_DIR_NAMES: &[&str] = &[
    "$recycle.bin",
    "system volume information",
    "recovery",
    "$winreagent",
    "$windows.~bt",
    "$windows.~ws",
    "perflogs",
    "winsxs",
    "windowsapps",
];

#[cfg(not(windows))]
const UNIX_IGNORED_ROOT_DIRS: &[&str] = &[
    "/proc",
    "/sys",
    "/dev",
    "/run",
    "/tmp",
    "/snap",
    "/nix/store",
    "/var/tmp",
    "/var/cache",
    "/var/log",
    "/var/lib/docker",
    "/lost+found",
];

#[must_use]
pub fn system_scan_roots() -> Vec<PathBuf> {
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

pub(crate) fn sweep<F>(roots: &[PathBuf], max_depth: Option<usize>, collector: &Collector<F>)
where
    F: FnMut(OsuMarker) -> ControlFlow<()> + Send,
{
    for root in roots {
        if collector.stopped() {
            return;
        }

        sweep_root(root, roots, max_depth, collector);
    }
}

fn walk_builder(root: &Path, roots: &[PathBuf], max_depth: Option<usize>) -> WalkBuilder {
    // Each explicit subtree gets its own full walk. Keep all roots for shallow
    // walks, whose depth limit is relative to each root.
    let separate_roots = if max_depth.is_none() {
        roots.to_vec()
    } else {
        Vec::new()
    };
    let mut builder = WalkBuilder::new(root);
    builder
        .standard_filters(false)
        .follow_links(false)
        .max_depth(max_depth)
        .filter_entry(move |entry| {
            entry.depth() == 0
                || (should_scan_entry(entry)
                    && !separate_roots.iter().any(|root| root == entry.path()))
        });
    builder
}

fn sweep_root<F>(root: &Path, roots: &[PathBuf], max_depth: Option<usize>, collector: &Collector<F>)
where
    F: FnMut(OsuMarker) -> ControlFlow<()> + Send,
{
    let walker = walk_builder(root, roots, max_depth).build_parallel();

    walker.run(|| {
        Box::new(|entry: Result<DirEntry, Error>| {
            if collector.stopped() {
                return WalkState::Quit;
            }

            let Ok(entry) = entry else {
                return WalkState::Continue;
            };
            if !entry
                .file_type()
                .is_some_and(|file_type| file_type.is_file())
            {
                return WalkState::Continue;
            }

            let Some(kind) = marker_kind(entry.path()) else {
                return WalkState::Continue;
            };

            match collector.offer(kind, entry.path().to_path_buf()) {
                ControlFlow::Continue(()) => WalkState::Continue,
                ControlFlow::Break(()) => WalkState::Quit,
            }
        })
    });
}

pub(crate) fn marker_kind(path: &Path) -> Option<OsuKind> {
    match path.file_name()?.to_str()? {
        "client.realm" => Some(OsuKind::Lazer),
        "osu!.db" => Some(OsuKind::Stable),
        _ => None,
    }
}

fn should_scan_entry(entry: &DirEntry) -> bool {
    if !entry
        .file_type()
        .is_some_and(|file_type| file_type.is_dir())
    {
        return true;
    }

    !is_ignored_scan_dir(entry.path())
}

fn has_ignored_dir_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| IGNORED_DIR_NAMES.contains(&name))
}

#[cfg(windows)]
fn is_ignored_scan_dir(path: &Path) -> bool {
    if has_ignored_dir_name(path) {
        return true;
    }

    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    let name = name.to_ascii_lowercase();
    if WINDOWS_IGNORED_DIR_NAMES.contains(&name.as_str()) {
        return true;
    }

    if name == "windows" && is_directly_under_drive_root(path) {
        return true;
    }

    path_ends_with_ci(path, &["windows", "temp"])
        || path_ends_with_ci(path, &["appdata", "local", "temp"])
        || path_ends_with_ci(path, &["programdata", "package cache"])
}

#[cfg(windows)]
fn is_directly_under_drive_root(path: &Path) -> bool {
    path.parent()
        .is_some_and(|parent| parent.parent().is_none())
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
    if has_ignored_dir_name(path) {
        return true;
    }

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
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::Mutex,
    };

    use ignore::{WalkBuilder, WalkState};
    use radio_core::OsuKind;
    use tempfile::TempDir;

    use super::{is_ignored_scan_dir, marker_kind, walk_builder};

    fn visited_paths(builder: &WalkBuilder) -> Vec<PathBuf> {
        let paths = Mutex::new(Vec::new());
        builder.build_parallel().run(|| {
            Box::new(|entry| {
                paths
                    .lock()
                    .unwrap()
                    .push(entry.expect("walk entry").into_path());
                WalkState::Continue
            })
        });
        paths.into_inner().unwrap()
    }

    #[test]
    fn full_sweep_visits_overlapping_roots_once_in_either_order() {
        let temp = TempDir::new().expect("temp dir");
        let child = temp.path().join("home/user");
        fs::create_dir_all(child.join("library")).unwrap();
        let marker = child.join("library/client.realm");
        fs::write(&marker, "").unwrap();
        let mut roots = vec![temp.path().to_path_buf(), child];

        for _ in 0..2 {
            let paths: Vec<_> = roots
                .iter()
                .flat_map(|root| visited_paths(&walk_builder(root, &roots, None)))
                .collect();
            assert_eq!(paths.iter().filter(|path| *path == &marker).count(), 1);
            let unique: std::collections::HashSet<_> = paths.iter().collect();
            assert_eq!(paths.len(), unique.len(), "visited a subtree twice");
            roots.reverse();
        }
    }

    #[test]
    fn shallow_sweep_preserves_depth_relative_to_each_root() {
        let temp = TempDir::new().expect("temp dir");
        let child = temp.path().join("home/user");
        fs::create_dir_all(child.join("library")).unwrap();
        let marker = child.join("library/client.realm");
        fs::write(&marker, "").unwrap();
        let roots = vec![temp.path().to_path_buf(), child.clone()];

        let parent_paths = visited_paths(&walk_builder(temp.path(), &roots, Some(2)));
        let child_paths = visited_paths(&walk_builder(&child, &roots, Some(2)));
        assert!(parent_paths.contains(&child));
        assert!(!parent_paths.contains(&marker));
        assert!(child_paths.contains(&marker));
    }

    #[test]
    fn full_sweep_preserves_explicit_roots_inside_pruned_directories() {
        let temp = TempDir::new().expect("temp dir");
        let child = temp.path().join("node_modules/library");
        fs::create_dir_all(&child).unwrap();
        let marker = child.join("client.realm");
        fs::write(&marker, "").unwrap();
        let roots = vec![temp.path().to_path_buf(), child.clone()];

        assert!(!visited_paths(&walk_builder(temp.path(), &roots, None)).contains(&marker));
        assert!(visited_paths(&walk_builder(&child, &roots, None)).contains(&marker));
    }

    #[test]
    fn git_exclude_rules_do_not_hide_markers() {
        let temp = TempDir::new().expect("temp dir");
        fs::create_dir_all(temp.path().join(".git/info")).unwrap();
        fs::write(temp.path().join(".git/info/exclude"), "library/\n").unwrap();
        fs::create_dir_all(temp.path().join("library")).unwrap();
        let marker = temp.path().join("library/client.realm");
        fs::write(&marker, "").unwrap();
        let mut builder = walk_builder(temp.path(), &[], None);

        assert!(visited_paths(&builder).contains(&marker));
        // Prove the fixture would hide the installation with Git excludes enabled.
        assert!(!visited_paths(builder.git_exclude(true)).contains(&marker));
    }

    #[test]
    fn git_global_ignores_do_not_hide_markers() {
        const FIXTURE: &str = "OSU_RADIO_TEST_GLOBAL_IGNORE_ROOT";
        if let Some(root) = std::env::var_os(FIXTURE) {
            let root = PathBuf::from(root);
            let marker = root.join("library/client.realm");
            let mut builder = walk_builder(&root, &[], None);
            assert!(visited_paths(&builder).contains(&marker));
            assert!(!visited_paths(builder.git_global(true).git_exclude(true)).contains(&marker));
            return;
        }

        let temp = TempDir::new().expect("temp dir");
        fs::create_dir_all(temp.path().join("repo/.git")).unwrap();
        fs::create_dir_all(temp.path().join("repo/library")).unwrap();
        fs::write(temp.path().join("repo/library/client.realm"), "").unwrap();
        fs::create_dir_all(temp.path().join("config/git")).unwrap();
        fs::write(temp.path().join("config/git/ignore"), "library/\n").unwrap();
        fs::create_dir_all(temp.path().join("home")).unwrap();

        // Isolate global Git configuration without mutating this test process's environment.
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "discovery::sweep::tests::git_global_ignores_do_not_hide_markers",
                "--nocapture",
            ])
            .env(FIXTURE, temp.path().join("repo"))
            .env("HOME", temp.path().join("home"))
            .env("USERPROFILE", temp.path().join("home"))
            .env("XDG_CONFIG_HOME", temp.path().join("config"))
            .current_dir(temp.path().join("repo"))
            .output()
            .expect("run isolated global-ignore test");
        assert!(
            output.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn recognizes_marker_file_names() {
        assert_eq!(
            marker_kind(Path::new("some/dir/client.realm")),
            Some(OsuKind::Lazer)
        );
        assert_eq!(
            marker_kind(Path::new("some/dir/osu!.db")),
            Some(OsuKind::Stable)
        );
        assert_eq!(marker_kind(Path::new("some/dir/scores.db")), None);
    }

    #[test]
    fn ignores_dependency_and_vcs_dirs() {
        assert!(is_ignored_scan_dir(Path::new(
            "/home/user/code/app/node_modules"
        )));
        assert!(is_ignored_scan_dir(Path::new("/home/user/code/app/.git")));
    }

    #[cfg(windows)]
    #[test]
    fn ignores_windows_system_dirs() {
        assert!(is_ignored_scan_dir(Path::new(r"C:\$Recycle.Bin")));
        assert!(is_ignored_scan_dir(Path::new(
            r"D:\System Volume Information"
        )));
        assert!(is_ignored_scan_dir(Path::new(r"C:\PerfLogs")));
    }

    #[cfg(windows)]
    #[test]
    fn ignores_windows_temp_dirs() {
        assert!(is_ignored_scan_dir(Path::new(r"C:\Windows\Temp")));
        assert!(is_ignored_scan_dir(Path::new(
            r"C:\Users\test\AppData\Local\Temp"
        )));
    }

    #[cfg(windows)]
    #[test]
    fn ignores_the_system_root_but_not_similarly_named_dirs() {
        assert!(is_ignored_scan_dir(Path::new(r"C:\Windows")));
        assert!(is_ignored_scan_dir(Path::new(r"c:\windows")));
        assert!(!is_ignored_scan_dir(Path::new(r"D:\Games\windows")));
    }

    #[cfg(windows)]
    #[test]
    fn ignores_windows_package_stores() {
        assert!(is_ignored_scan_dir(Path::new(r"C:\Windows\WinSxS")));
        assert!(is_ignored_scan_dir(Path::new(
            r"C:\Program Files\WindowsApps"
        )));
        assert!(is_ignored_scan_dir(Path::new(
            r"C:\ProgramData\Package Cache"
        )));
    }

    #[cfg(windows)]
    #[test]
    fn keeps_windows_plausible_osu_locations() {
        assert!(!is_ignored_scan_dir(Path::new(
            r"C:\Users\test\AppData\Roaming\osu"
        )));
        assert!(!is_ignored_scan_dir(Path::new(r"D:\Games\osu!")));
        assert!(!is_ignored_scan_dir(Path::new(r"C:\Program Files\osu!")));
    }

    #[cfg(not(windows))]
    #[test]
    fn ignores_unix_system_dirs() {
        assert!(is_ignored_scan_dir(Path::new("/proc")));
        assert!(is_ignored_scan_dir(Path::new("/sys")));
        assert!(is_ignored_scan_dir(Path::new("/var/cache")));
        assert!(is_ignored_scan_dir(Path::new("/nix/store")));
        assert!(is_ignored_scan_dir(Path::new("/snap")));
    }

    #[cfg(not(windows))]
    #[test]
    fn ignores_unix_cache_and_trash_dirs() {
        assert!(is_ignored_scan_dir(Path::new("/home/test/.cache")));
        assert!(is_ignored_scan_dir(Path::new("/home/test/.Trash")));
        assert!(is_ignored_scan_dir(Path::new("/home/test/.Trash-1000")));
        assert!(is_ignored_scan_dir(Path::new(
            "/home/test/.local/share/Trash"
        )));
    }

    #[cfg(not(windows))]
    #[test]
    fn keeps_unix_plausible_osu_locations() {
        assert!(!is_ignored_scan_dir(Path::new(
            "/home/test/.local/share/osu"
        )));
        assert!(!is_ignored_scan_dir(Path::new("/mnt/games/osu!")));
        assert!(!is_ignored_scan_dir(Path::new("/media/games/osu")));
    }
}
