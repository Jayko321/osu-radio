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

    use super::{known_candidates, parse_storage_ini_full_path};

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
