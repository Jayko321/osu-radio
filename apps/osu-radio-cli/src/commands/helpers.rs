use std::{
    io,
    path::{Path, PathBuf},
};

use radio_core::{OsuKind, OsuMarker};
use serde_json::{Value, json};

use crate::{
    consts::MARKER_TABLE_WIDTHS,
    types::{MarkerFilters, SourceArg},
};

pub(crate) fn discover_markers(filters: &MarkerFilters) -> Result<Vec<OsuMarker>, String> {
    let root = filters
        .root
        .as_deref()
        .map(normalize_root_filter)
        .transpose()?;
    let source = filters.source.map(OsuKind::from);

    Ok(radio_scanner::helpers::find_osu_markers()
        .into_iter()
        .filter(|marker| source.is_none_or(|source| marker.kind == source))
        .filter(|marker| {
            root.as_ref().is_none_or(|root| {
                marker.root_path.starts_with(root) || marker.marker_path.starts_with(root)
            })
        })
        .collect())
}

pub(crate) fn marker_from_path(
    path: &Path,
    source: Option<SourceArg>,
) -> Result<OsuMarker, String> {
    let kind = match source {
        Some(source) => source.into(),
        None => infer_source_from_marker(path).ok_or_else(|| {
            format!(
                "Could not infer osu! source from `{}`. Pass `--source lazer` or `--source stable`.",
                path.display()
            )
        })?,
    };

    let root_path = path.parent().map(Path::to_path_buf).ok_or_else(|| {
        format!(
            "Marker path `{}` does not have a parent directory.",
            path.display()
        )
    })?;

    Ok(OsuMarker {
        kind,
        marker_path: path.to_path_buf(),
        root_path,
    })
}

pub(crate) fn print_json(value: &Value) -> Result<(), String> {
    serde_json::to_writer_pretty(io::stdout(), value)
        .map_err(|error| format!("Failed to write JSON output: {error}"))?;
    println!();
    Ok(())
}

pub(crate) fn markers_to_json(markers: &[OsuMarker]) -> Vec<Value> {
    markers
        .iter()
        .enumerate()
        .map(|(index, marker)| marker_to_json(index + 1, marker))
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
        .map(|(index, marker)| {
            vec![
                (index + 1).to_string(),
                source_name(marker.kind).to_string(),
                marker.root_path.display().to_string(),
                marker.marker_path.display().to_string(),
            ]
        })
        .collect::<Vec<_>>();

    print_table(
        &["Index", "Source", "Root", "Marker"],
        &rows,
        MARKER_TABLE_WIDTHS,
    );
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

            header
                .chars()
                .count()
                .max(content_width)
                .min(max_widths[column])
        })
        .collect::<Vec<_>>();

    print_table_row(
        &headers
            .iter()
            .map(|header| header.to_string())
            .collect::<Vec<_>>(),
        &widths,
    );
    print_table_separator(&widths);

    for row in rows {
        print_table_row(row, &widths);
    }
}

pub(crate) fn source_name(kind: OsuKind) -> &'static str {
    match kind {
        OsuKind::Stable => "stable",
        OsuKind::Lazer => "lazer",
    }
}

fn normalize_root_filter(root: &Path) -> Result<PathBuf, String> {
    if root.is_absolute() {
        return Ok(root.to_path_buf());
    }

    std::env::current_dir()
        .map(|cwd| cwd.join(root))
        .map_err(|error| {
            format!(
                "Failed to resolve --root path `{}`: {error}",
                root.display()
            )
        })
}

fn infer_source_from_marker(path: &Path) -> Option<OsuKind> {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("client.realm") => Some(OsuKind::Lazer),
        Some("osu!.db") => Some(OsuKind::Stable),
        _ => None,
    }
}

fn print_table_row(row: &[String], widths: &[usize]) {
    for (column, width) in widths.iter().enumerate() {
        if column > 0 {
            print!("  ");
        }

        let value = row.get(column).map(String::as_str).unwrap_or("");
        print!("{:<width$}", truncate(value, *width), width = width);
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

    let mut truncated = value.chars().take(width - 1).collect::<String>();
    truncated.push('.');
    truncated
}
