use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use radio_core::OsuKind;

use super::sweep::marker_kind;

const STORAGE_INI_MAX_BYTES: u64 = 64 * 1024;

pub(crate) fn known_candidates(roots: &[PathBuf]) -> Vec<(OsuKind, PathBuf)> {
    let (lazer_dirs, stable_dirs) = if roots.is_empty() {
        (default_lazer_dirs(), default_stable_dirs())
    } else {
        (lazer_dirs_under(roots), stable_dirs_under(roots))
    };

    let mut candidates = Vec::new();
    for dir in lazer_dirs {
        candidates.push(dir.join("client.realm"));

        if let Some(relocated) = read_storage_ini_full_path(&dir.join("storage.ini")) {
            candidates.push(relocated.join("client.realm"));
        }
    }
    for dir in stable_dirs {
        candidates.push(dir.join("osu!.db"));
    }

    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .filter(|path| path.is_file())
        .filter_map(|path| Some((marker_kind(&path)?, path)))
        .collect()
}

pub(crate) fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    const HOME_VAR: &str = "USERPROFILE";
    #[cfg(not(windows))]
    const HOME_VAR: &str = "HOME";

    env_path(HOME_VAR)
}

fn env_path(key: &str) -> Option<PathBuf> {
    let value = std::env::var_os(key)?;
    (!value.is_empty()).then(|| PathBuf::from(value))
}

fn lazer_dirs_under(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .flat_map(|root| [root.clone(), root.join("osu")])
        .collect()
}

fn stable_dirs_under(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots
        .iter()
        .flat_map(|root| [root.clone(), root.join("osu!")])
        .collect()
}

#[cfg(windows)]
fn default_lazer_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(appdata) = env_path("APPDATA") {
        dirs.push(appdata.join("osu"));
    }

    for root in super::system_scan_roots() {
        dirs.push(root.join("osu"));
        dirs.push(root.join("Games").join("osu"));
    }

    dirs
}

#[cfg(windows)]
fn default_stable_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    for key in [
        "LOCALAPPDATA",
        "USERPROFILE",
        "ProgramFiles",
        "ProgramW6432",
    ] {
        if let Some(base) = env_path(key) {
            dirs.push(base.join("osu!"));
        }
    }
    if let Some(base) = env_path("ProgramFiles(x86)") {
        dirs.push(base.join("osu!"));
    }

    for root in super::system_scan_roots() {
        dirs.push(root.join("osu!"));
        dirs.push(root.join("Games").join("osu!"));
    }

    dirs
}

#[cfg(target_os = "macos")]
fn default_lazer_dirs() -> Vec<PathBuf> {
    home_dir()
        .map(|home| vec![home.join("Library/Application Support/osu")])
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn default_stable_dirs() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn default_lazer_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    match env_path("XDG_DATA_HOME") {
        Some(data_home) => dirs.push(data_home.join("osu")),
        None => {
            if let Some(home) = home_dir() {
                dirs.push(home.join(".local/share/osu"));
            }
        }
    }

    if let Some(home) = home_dir() {
        dirs.push(home.join(".var/app/sh.ppy.osu/data/osu"));
    }

    dirs
}

#[cfg(all(unix, not(target_os = "macos")))]
fn default_stable_dirs() -> Vec<PathBuf> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };

    let mut dirs = vec![home.join(".local/share/osu-wine/osu!")];
    dirs.extend(wine_prefix_stable_dirs(&home.join(".wine")));

    dirs
}

#[cfg(all(unix, not(target_os = "macos")))]
fn wine_prefix_stable_dirs(prefix: &Path) -> Vec<PathBuf> {
    let users = prefix.join("drive_c/users");
    let Ok(entries) = fs::read_dir(&users) else {
        return Vec::new();
    };

    entries
        .flatten()
        .map(|entry| entry.path().join("AppData/Local/osu!"))
        .collect()
}

fn read_storage_ini_full_path(path: &Path) -> Option<PathBuf> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > STORAGE_INI_MAX_BYTES {
        return None;
    }

    parse_storage_ini_full_path(&fs::read_to_string(path).ok()?)
}

fn parse_storage_ini_full_path(contents: &str) -> Option<PathBuf> {
    contents.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        if !key.trim().eq_ignore_ascii_case("FullPath") {
            return None;
        }

        let value = value.trim();
        (!value.is_empty()).then(|| PathBuf::from(value))
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use radio_core::OsuKind;
    use tempfile::TempDir;

    use super::{
        STORAGE_INI_MAX_BYTES, known_candidates, parse_storage_ini_full_path,
        read_storage_ini_full_path,
    };

    #[test]
    fn storage_ini_requires_a_small_utf8_regular_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("storage.ini");
        assert_eq!(read_storage_ini_full_path(&path), None);
        assert_eq!(read_storage_ini_full_path(temp.path()), None);
        fs::write(&path, b"FullPath=\xff").unwrap();
        assert_eq!(read_storage_ini_full_path(&path), None);

        let mut contents = "FullPath=relative=folder\n".to_owned();
        contents.extend(std::iter::repeat_n(
            ' ',
            usize::try_from(STORAGE_INI_MAX_BYTES).unwrap() - contents.len(),
        ));
        fs::write(&path, &contents).unwrap();
        assert_eq!(
            read_storage_ini_full_path(&path),
            Some(PathBuf::from("relative=folder"))
        );
        contents.push(' ');
        fs::write(&path, contents).unwrap();
        assert_eq!(read_storage_ini_full_path(&path), None);
    }

    #[test]
    fn repeated_roots_and_relocation_keep_each_marker_once() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().to_path_buf();
        let nested = root.join("osu");
        fs::create_dir(&nested).unwrap();
        for dir in [&root, &nested] {
            fs::write(dir.join("client.realm"), "").unwrap();
            fs::write(
                dir.join("storage.ini"),
                format!("FullPath={}\n", nested.display()),
            )
            .unwrap();
        }
        fs::write(root.join("osu!.db"), "").unwrap();
        // A directory with a marker's name must never become an installation.
        fs::create_dir(nested.join("osu!.db")).unwrap();
        assert_eq!(
            known_candidates(&[root.clone(), root.clone(), nested.clone()]),
            vec![
                (OsuKind::Lazer, root.join("client.realm")),
                (OsuKind::Lazer, nested.join("client.realm")),
                (OsuKind::Stable, root.join("osu!.db")),
            ]
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn linux_defaults_use_only_the_isolated_home_and_xdg_locations() {
        use std::process::Command;

        const FIXTURE: &str = "OSU_RADIO_TEST_KNOWN_PATHS";
        if let Some(root) = std::env::var_os(FIXTURE) {
            let root = PathBuf::from(root);
            let home_present = std::env::var_os("HOME").is_some();
            assert_eq!(super::home_dir(), home_present.then(|| root.join("home")));
            let mut actual = known_candidates(&[]);
            actual.sort_by(|left, right| left.1.cmp(&right.1));
            let mut expected = Vec::new();
            if home_present {
                expected.extend([
                    (
                        OsuKind::Lazer,
                        root.join("home/.var/app/sh.ppy.osu/data/osu/client.realm"),
                    ),
                    (
                        OsuKind::Stable,
                        root.join("home/.local/share/osu-wine/osu!/osu!.db"),
                    ),
                    (
                        OsuKind::Stable,
                        root.join("home/.wine/drive_c/users/player/AppData/Local/osu!/osu!.db"),
                    ),
                ]);
            }
            if std::env::var_os("XDG_DATA_HOME").is_some_and(|value| !value.is_empty()) {
                expected.push((OsuKind::Lazer, root.join("data/osu/client.realm")));
            } else if home_present {
                expected.push((
                    OsuKind::Lazer,
                    root.join("home/.local/share/osu/client.realm"),
                ));
            }
            expected.sort_by(|left, right| left.1.cmp(&right.1));
            assert_eq!(actual, expected);
            return;
        }

        let temp = TempDir::new().unwrap();
        for marker in [
            "data/osu/client.realm",
            "home/.local/share/osu/client.realm",
            "home/.var/app/sh.ppy.osu/data/osu/client.realm",
            "home/.local/share/osu-wine/osu!/osu!.db",
            "home/.wine/drive_c/users/player/AppData/Local/osu!/osu!.db",
        ] {
            let path = temp.path().join(marker);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        for (home, xdg) in [
            (true, Some("data")),
            (true, None),
            (true, Some("")),
            (false, None),
            (false, Some("data")),
        ] {
            let mut child = Command::new(std::env::current_exe().unwrap());
            child.env_clear().args([
                "--exact",
                "discovery::known::tests::linux_defaults_use_only_the_isolated_home_and_xdg_locations",
                "--nocapture",
            ]).env(FIXTURE, temp.path()).current_dir(temp.path());
            if home {
                child.env("HOME", temp.path().join("home"));
            }
            if let Some(xdg) = xdg {
                child.env(
                    "XDG_DATA_HOME",
                    if xdg.is_empty() {
                        PathBuf::new()
                    } else {
                        temp.path().join(xdg)
                    },
                );
            }
            if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
                child.env("LLVM_PROFILE_FILE", profile);
            }
            let output = child.output().unwrap();
            assert!(
                output.status.success(),
                "home={home} xdg={xdg:?}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    fn parses_the_full_path_key() {
        assert_eq!(
            parse_storage_ini_full_path("[General]\nFullPath = D:\\osu-data\n"),
            Some(PathBuf::from("D:\\osu-data"))
        );
        assert_eq!(
            parse_storage_ini_full_path("fullpath=/mnt/games/osu"),
            Some(PathBuf::from("/mnt/games/osu"))
        );
    }

    #[test]
    fn rejects_missing_malformed_and_commented_entries() {
        assert_eq!(parse_storage_ini_full_path(""), None);
        assert_eq!(parse_storage_ini_full_path("[General]\nOther = x"), None);
        assert_eq!(parse_storage_ini_full_path("FullPath"), None);
        assert_eq!(parse_storage_ini_full_path("FullPath =   "), None);
        assert_eq!(parse_storage_ini_full_path("; FullPath = D:\\osu"), None);
    }

    #[test]
    fn follows_a_relocated_lazer_data_directory() {
        let temp = TempDir::new().expect("temp dir");
        let relocated = temp.path().join("elsewhere");
        fs::create_dir_all(&relocated).expect("create relocated dir");
        fs::write(relocated.join("client.realm"), b"").expect("write marker");

        let default_dir = temp.path().join("osu");
        fs::create_dir_all(&default_dir).expect("create default dir");
        fs::write(
            default_dir.join("storage.ini"),
            format!("[General]\nFullPath = {}\n", relocated.display()),
        )
        .expect("write storage.ini");

        let candidates = known_candidates(&[temp.path().to_path_buf()]);

        assert_eq!(
            candidates,
            vec![(OsuKind::Lazer, relocated.join("client.realm"))]
        );
    }

    #[test]
    fn ignores_known_candidates_that_do_not_exist() {
        let temp = TempDir::new().expect("temp dir");

        assert!(known_candidates(&[temp.path().to_path_buf()]).is_empty());
    }
}
