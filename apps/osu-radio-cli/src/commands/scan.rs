use anyhow::Result;
use radio_scanner::discovery::discover;
use serde_json::json;

use crate::{
    commands::helpers::{
        MARKER_TABLE_HEADERS, discovery_options, marker_row, markers_to_json, print_json,
        print_table_header, print_table_row,
    },
    consts::MARKER_TABLE_WIDTHS,
    types::ScanArgs,
};

pub(crate) async fn scan(args: ScanArgs) -> Result<()> {
    let options = discovery_options(&args.filters, args.depth.into(), scan_limit(&args))?;

    if args.json {
        let markers = discover(options).collect().await;
        print_json(&json!({
            "markers": markers_to_json(&markers),
        }))?;
        return Ok(());
    }

    eprintln!("Searching for osu! installations...");

    let mut discovery = discover(options);
    let mut found = 0usize;
    while let Some(marker) = discovery.next().await {
        found = found.saturating_add(1);
        if found == 1 {
            print_table_header(MARKER_TABLE_HEADERS, MARKER_TABLE_WIDTHS);
        }

        print_table_row(&marker_row(found, &marker), MARKER_TABLE_WIDTHS);
    }

    if found == 0 {
        println!("No osu! installations found.");
        println!(
            "If osu! is installed in an unusual location, point discovery at it with `--root <PATH>`."
        );
        return Ok(());
    }

    println!("Found {found} osu! installation(s).");
    if args.verbose {
        println!(
            "Indexes are 1-based and can be passed to `osu-radio-cli import --index <INDEX>`."
        );
    }

    Ok(())
}

const fn scan_limit(args: &ScanArgs) -> Option<usize> {
    if args.first {
        return Some(1);
    }

    args.limit
}
