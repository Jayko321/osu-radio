use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};

pub(super) fn relative_path(value: &str) -> Result<String> {
    let value = value.replace('\\', "/");
    ensure!(
        !value.starts_with('/') && !value.contains([':', '\0']),
        "absolute or invalid file reference `{value}`"
    );
    let mut parts = Vec::new();
    for part in value.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                ensure!(parts.pop().is_some(), "escaping file reference `{value}`");
            }
            _ => parts.push(part),
        }
    }
    ensure!(!parts.is_empty(), "empty file reference `{value}`");
    Ok(parts.join("/"))
}

pub(super) fn songs_directory(installation: &Path) -> Result<PathBuf> {
    let mut configs = Vec::new();
    for entry in fs::read_dir(installation)
        .with_context(|| format!("failed to list `{}`", installation.display()))?
    {
        let entry = entry?;
        if entry.file_name().to_str().is_some_and(|name| {
            name.starts_with("osu!.")
                && Path::new(name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("cfg"))
        }) {
            configs.push(entry.path());
        }
    }
    configs.sort();
    let mut configured: Option<PathBuf> = None;
    for config in configs {
        let text = fs::read_to_string(&config)
            .with_context(|| format!("failed to read configuration `{}`", config.display()))?;
        for line in text.trim_start_matches('\u{feff}').lines() {
            let Some((key, value)) = line.trim().split_once('=') else {
                continue;
            };
            if !key.trim().eq_ignore_ascii_case("BeatmapDirectory") {
                continue;
            }
            let value = value.trim().trim_matches('"');
            if value.is_empty() {
                continue;
            }
            let directory = configured_directory(installation, value)
                .with_context(|| format!("invalid BeatmapDirectory in `{}`", config.display()))?;
            if let Some(previous) = &configured {
                ensure!(
                    previous == &directory,
                    "conflicting BeatmapDirectory configurations: `{}` and `{}` (in `{}`)",
                    previous.display(),
                    directory.display(),
                    config.display()
                );
            } else {
                configured = Some(directory);
            }
        }
    }
    Ok(configured.unwrap_or_else(|| installation.join("Songs")))
}

fn configured_directory(installation: &Path, value: &str) -> Result<PathBuf> {
    let normalized = value.replace('\\', "/");
    ensure!(!normalized.contains('\0'), "invalid directory path");
    let path = if normalized.as_bytes().get(1) == Some(&b':') {
        ensure!(
            normalized
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic)
                && normalized.as_bytes().get(2) == Some(&b'/'),
            "drive-relative directory cannot be resolved: `{value}`"
        );
        #[cfg(target_os = "linux")]
        {
            wine_directory(installation, &normalized)?
        }
        #[cfg(windows)]
        {
            PathBuf::from(&normalized)
        }
        #[cfg(not(any(target_os = "linux", windows)))]
        {
            bail!("Windows directory cannot be resolved on this platform: `{value}`");
        }
    } else {
        #[cfg(not(windows))]
        ensure!(
            !normalized.starts_with("//"),
            "Windows network directory cannot be resolved: `{value}`"
        );
        let path = Path::new(&normalized);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            installation.join(path)
        }
    };
    let mut cleaned = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                cleaned.pop();
            }
            _ => cleaned.push(component),
        }
    }
    // Canonicalize existing directories so equivalent configurations agree.
    Ok(fs::canonicalize(&cleaned).unwrap_or(cleaned))
}

#[cfg(target_os = "linux")]
fn wine_directory(installation: &Path, value: &str) -> Result<PathBuf> {
    let installation = fs::canonicalize(installation)
        .context("failed to resolve installation for Wine mapping")?;
    let prefix = installation
        .ancestors()
        .find(|path| path.join("dosdevices").is_dir())
        .with_context(|| format!("no enclosing Wine prefix for `{value}`"))?;
    let drive = value
        .get(..2)
        .context("invalid drive prefix")?
        .to_ascii_lowercase();
    let mapping = prefix.join("dosdevices").join(drive);
    // Canonicalization follows both relative and absolute dosdevices links.
    let root = fs::canonicalize(&mapping)
        .with_context(|| format!("unresolvable Wine drive `{}`", mapping.display()))?;
    ensure!(
        root.is_dir(),
        "Wine drive is not a directory: `{}`",
        mapping.display()
    );
    Ok(root.join(value.get(3..).context("invalid drive path")?))
}

pub(super) fn background(path: &Path) -> Result<Option<String>> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read `{}`", path.display()))?;
    background_event(&text)
}

pub(super) fn background_event(text: &str) -> Result<Option<String>> {
    let mut in_events = false;
    for line in text.trim_start_matches('\u{feff}').lines().map(str::trim) {
        if line.starts_with('[') {
            in_events = line == "[Events]";
            continue;
        }
        if !in_events || line.is_empty() || line.starts_with("//") {
            continue;
        }
        let mut fields = line.splitn(3, ',');
        if !matches!(fields.next().map(str::trim), Some("0" | "Background")) {
            continue;
        }
        fields
            .next()
            .context("background event has no start time")?;
        let filename = fields
            .next()
            .context("background event has no filename")?
            .trim();
        let Some(quoted) = filename.strip_prefix('"') else {
            return Ok(Some(
                filename
                    .split(',')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_owned(),
            ));
        };
        let mut chars = quoted.chars().peekable();
        let mut result = String::new();
        while let Some(character) = chars.next() {
            if character == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    result.push('"');
                } else {
                    let suffix = chars.collect::<String>();
                    ensure!(
                        suffix.trim().is_empty() || suffix.trim_start().starts_with(','),
                        "invalid quoted background filename"
                    );
                    return Ok(Some(result));
                }
            } else {
                result.push(character);
            }
        }
        bail!("unterminated quoted background filename");
    }
    Ok(None)
}
