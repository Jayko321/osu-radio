use std::{
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use radio_core::{OsuKind, OsuMarker, import_types::ImportedBeatmapSet};
use radio_scanner::{
    UnsupportedSourceError,
    discovery::{DiscoveryDepth, DiscoveryOptions, discover},
};
use serde_json::{Value, json};

use crate::{
    consts::MARKER_TABLE_WIDTHS,
    types::{MarkerFilters, SourceArg},
};

pub(crate) const MARKER_TABLE_HEADERS: &[&str] = &["Index", "Source", "Root", "Marker"];

pub(crate) fn discovery_options(
    filters: &MarkerFilters,
    depth: DiscoveryDepth,
    limit: Option<usize>,
) -> Result<DiscoveryOptions> {
    if limit == Some(0) {
        bail!("`--limit` must be at least 1.");
    }

    let roots = filters
        .root
        .as_deref()
        .map(normalize_root_filter)
        .transpose()?
        .into_iter()
        .collect();

    Ok(DiscoveryOptions {
        roots,
        kind: filters.source.map(OsuKind::from),
        limit: limit.and_then(NonZeroUsize::new),
        depth,
        ..DiscoveryOptions::default()
    })
}

pub(crate) async fn discover_markers(
    filters: &MarkerFilters,
    depth: DiscoveryDepth,
    limit: Option<usize>,
) -> Result<Vec<OsuMarker>> {
    Ok(discover(discovery_options(filters, depth, limit)?)
        .collect()
        .await)
}

pub(crate) async fn select_marker(
    filters: &MarkerFilters,
    index: Option<usize>,
    marker: Option<&Path>,
) -> Result<OsuMarker> {
    if index.is_some() && marker.is_some() {
        bail!("Use either `--index` or `--marker`, not both.");
    }

    if let Some(marker_path) = marker {
        return marker_from_path(marker_path, filters.source);
    }

    let limit = index.is_none().then_some(2);
    let markers = discover_markers(filters, DiscoveryDepth::Full, limit).await?;

    if let Some(index) = index {
        if index == 0 {
            bail!("`--index` is 1-based. Use an index from `osu-radio-cli scan`.");
        }

        return markers
            .get(index.saturating_sub(1))
            .cloned()
            .with_context(|| {
            format!(
                "No discovered osu! installation has index {index}. Run `osu-radio-cli scan` to see available indexes."
            )
        });
    }

    match markers.as_slice() {
        [] => bail!(
            "No osu! installations found. Run `osu-radio-cli scan` to inspect discovery results."
        ),
        [marker] => Ok(marker.clone()),
        _ => {
            print_markers_table(&markers);
            bail!("Multiple osu! installations found. Choose one with `--index <INDEX>`.")
        }
    }
}

pub(crate) fn import_error(marker: &OsuMarker, error: anyhow::Error) -> anyhow::Error {
    match error.downcast_ref::<UnsupportedSourceError>() {
        Some(UnsupportedSourceError { kind }) => {
            anyhow::anyhow!("The selected osu! source is not supported yet: {kind:?}.")
        }
        None => error.context(format!(
            "Failed to import beatmaps from {} marker `{}`",
            source_name(marker.kind),
            marker.marker_path.display()
        )),
    }
}

pub(crate) fn beatmap_count(beatmap_sets: &[ImportedBeatmapSet]) -> usize {
    beatmap_sets
        .iter()
        .map(|beatmap_set| beatmap_set.beatmaps.len())
        .sum()
}

pub(crate) fn marker_from_path(path: &Path, source: Option<SourceArg>) -> Result<OsuMarker> {
    let kind = match source {
        Some(source) => source.into(),
        None => infer_source_from_marker(path).with_context(|| {
            format!(
                "Could not infer osu! source from `{}`. Pass `--source lazer` or `--source stable`.",
                path.display()
            )
        })?,
    };

    let Some(root_path) = path.parent().map(Path::to_path_buf) else {
        bail!(
            "Marker path `{}` does not have a parent directory.",
            path.display()
        );
    };

    Ok(OsuMarker {
        kind,
        marker_path: path.to_path_buf(),
        root_path,
    })
}

pub(crate) fn print_json(value: &Value) -> Result<()> {
    serde_json::to_writer_pretty(std::io::stdout(), value)
        .context("Failed to write JSON output")?;
    println!();
    Ok(())
}

pub(crate) fn markers_to_json(markers: &[OsuMarker]) -> Vec<Value> {
    markers
        .iter()
        .enumerate()
        .map(|(index, marker)| marker_to_json(index.saturating_add(1), marker))
        .collect()
}

pub(crate) fn marker_to_json(index: usize, marker: &OsuMarker) -> Value {
    json!({
        "index": index,
        "source": source_name(marker.kind),
        "root_path": marker.root_path.display().to_string(),
        "marker_path": marker.marker_path.display().to_string(),
    })
}

pub(crate) fn print_markers_table(markers: &[OsuMarker]) {
    let rows = markers
        .iter()
        .enumerate()
        .map(|(index, marker)| marker_row(index.saturating_add(1), marker))
        .collect::<Vec<_>>();

    print_table(MARKER_TABLE_HEADERS, &rows, MARKER_TABLE_WIDTHS);
}

pub(crate) fn marker_row(index: usize, marker: &OsuMarker) -> Vec<String> {
    vec![
        index.to_string(),
        source_name(marker.kind).to_string(),
        marker.root_path.display().to_string(),
        marker.marker_path.display().to_string(),
    ]
}

pub(crate) fn print_table_header(headers: &[&str], widths: &[usize]) {
    print_table_row(
        &headers.iter().map(ToString::to_string).collect::<Vec<_>>(),
        widths,
    );
    print_table_separator(widths);
}

pub(crate) fn print_table(headers: &[&str], rows: &[Vec<String>], max_widths: &[usize]) {
    let widths = headers
        .iter()
        .enumerate()
        .map(|(column, header)| {
            let content_width = rows
                .iter()
                .filter_map(|row| row.get(column))
                .map(|value| value.chars().count())
                .max()
                .unwrap_or(0);

            let max_width = max_widths.get(column).copied().unwrap_or(usize::MAX);

            header.chars().count().max(content_width).min(max_width)
        })
        .collect::<Vec<_>>();

    print_table_header(headers, &widths);

    for row in rows {
        print_table_row(row, &widths);
    }
}

pub(crate) const fn source_name(kind: OsuKind) -> &'static str {
    kind.as_str()
}

fn normalize_root_filter(root: &Path) -> Result<PathBuf> {
    if root.is_absolute() {
        return Ok(root.to_path_buf());
    }

    std::env::current_dir()
        .map(|cwd| cwd.join(root))
        .with_context(|| format!("Failed to resolve --root path `{}`", root.display()))
}

fn infer_source_from_marker(path: &Path) -> Option<OsuKind> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("client.realm") => Some(OsuKind::Lazer),
        Some("osu!.db") => Some(OsuKind::Stable),
        _ => None,
    }
}

pub(crate) fn print_table_row(row: &[String], widths: &[usize]) {
    let last = widths.len().saturating_sub(1);

    for (column, width) in widths.iter().enumerate() {
        if column > 0 {
            print!("  ");
        }

        let value = truncate(row.get(column).map_or("", String::as_str), *width);
        if column == last {
            print!("{value}");
        } else {
            print!("{value:<width$}");
        }
    }
    println!();
}

fn print_table_separator(widths: &[usize]) {
    for (column, width) in widths.iter().enumerate() {
        if column > 0 {
            print!("  ");
        }
        print!("{}", "-".repeat(*width));
    }
    println!();
}

fn truncate(value: &str, width: usize) -> String {
    let length = value.chars().count();
    if length <= width {
        return value.to_string();
    }

    if width <= 1 {
        return ".".to_string();
    }

    let mut truncated = value
        .chars()
        .take(width.saturating_sub(1))
        .collect::<String>();
    truncated.push('.');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_options_preserve_explicit_scope_and_resolve_relative_roots() {
        let filters = MarkerFilters {
            source: Some(SourceArg::Stable),
            root: Some(PathBuf::from("fixture/osu")),
        };
        let options = discovery_options(&filters, DiscoveryDepth::Shallow, Some(3)).unwrap();
        assert_eq!(
            options.roots,
            vec![std::env::current_dir().unwrap().join("fixture/osu")]
        );
        assert_eq!(options.kind, Some(OsuKind::Stable));
        assert_eq!(options.depth, DiscoveryDepth::Shallow);
        assert_eq!(options.limit.map(NonZeroUsize::get), Some(3));

        let absolute = std::env::temp_dir().join("osu-cli-options");
        let options = discovery_options(
            &MarkerFilters {
                source: None,
                root: Some(absolute.clone()),
            },
            DiscoveryDepth::Known,
            None,
        )
        .unwrap();
        assert_eq!(options.roots, vec![absolute]);
        assert_eq!(options.kind, None);
        assert_eq!(options.limit, None);
    }

    #[test]
    fn zero_discovery_limit_is_rejected_before_discovery() {
        let error = discovery_options(
            &MarkerFilters {
                source: None,
                root: None,
            },
            DiscoveryDepth::Full,
            Some(0),
        )
        .unwrap_err();
        assert!(error.to_string().contains("`--limit` must be at least 1"));
    }

    #[test]
    fn marker_inference_is_exact_and_source_override_is_explicit() {
        for (filename, kind) in [
            ("osu!.db", OsuKind::Stable),
            ("client.realm", OsuKind::Lazer),
        ] {
            let path = Path::new("fixture").join(filename);
            let marker = marker_from_path(&path, None).unwrap();
            assert_eq!(marker.kind, kind);
            assert_eq!(marker.marker_path, path);
            assert_eq!(marker.root_path, Path::new("fixture"));
        }
        for filename in ["CLIENT.REALM", "osu!.db.bak", "unknown"] {
            assert!(marker_from_path(Path::new(filename), None).is_err());
        }
        let marker =
            marker_from_path(Path::new("fixture/custom.db"), Some(SourceArg::Lazer)).unwrap();
        assert_eq!(marker.kind, OsuKind::Lazer);
        assert!(marker_from_path(Path::new(""), Some(SourceArg::Stable)).is_err());
    }

    #[test]
    fn marker_output_uses_one_based_indexes_and_display_paths() {
        let markers = [
            marker_from_path(Path::new("fixture/曲/osu!.db"), None).unwrap(),
            marker_from_path(Path::new("fixture/lazer/client.realm"), None).unwrap(),
        ];
        let values = markers_to_json(&markers);
        assert_eq!(
            values[0],
            json!({
                "index": 1, "source": "stable",
                "root_path": Path::new("fixture/曲").display().to_string(),
                "marker_path": markers[0].marker_path.display().to_string(),
            })
        );
        assert_eq!(values[1]["index"], 2);
        assert_eq!(values[1]["source"], "lazer");
        assert_eq!(
            marker_row(2, &markers[1]),
            vec![
                "2".to_string(),
                "lazer".to_string(),
                markers[1].root_path.display().to_string(),
                markers[1].marker_path.display().to_string(),
            ]
        );
        assert!(markers_to_json(&[]).is_empty());
    }

    #[test]
    fn import_errors_preserve_the_cause_and_identify_the_source() {
        let marker = marker_from_path(Path::new("fixture/osu!.db"), None).unwrap();
        let error = import_error(&marker, anyhow::anyhow!("broken fixture"));
        let message = format!("{error:#}");
        assert!(message.contains("stable marker"));
        assert!(message.contains(&marker.marker_path.display().to_string()));
        assert!(message.contains("broken fixture"));
        let error = import_error(
            &marker,
            UnsupportedSourceError {
                kind: OsuKind::Lazer,
            }
            .into(),
        );
        assert!(error.to_string().contains("not supported yet: Lazer"));
    }

    #[test]
    fn truncation_counts_characters_instead_of_utf8_bytes() {
        for (value, width, expected) in [
            ("", 4, ""),
            ("abc", 3, "abc"),
            ("abc", 2, "a."),
            ("abc", 1, "."),
            ("abc", 0, "."),
            ("曲目🎵", 3, "曲目🎵"),
            ("曲目🎵", 2, "曲."),
        ] {
            assert_eq!(truncate(value, width), expected);
        }
    }
}
