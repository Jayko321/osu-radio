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
        Some(UnsupportedSourceError {
            kind: OsuKind::Stable,
        }) => {
            anyhow::anyhow!(
                "Stable osu! installations are detected, but stable importing is not supported yet. Try a lazer source or run `osu-radio-cli scan --source lazer`."
            )
        }
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
