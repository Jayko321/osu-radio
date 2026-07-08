use serde_json::json;

use crate::{
    commands::helpers::{discover_markers, markers_to_json, print_json, print_markers_table},
    types::ScanArgs,
};

pub(crate) fn scan(args: ScanArgs) -> anyhow::Result<()> {
    let markers = discover_markers(&args.filters)?;

    if args.json {
        print_json(&json!({
            "markers": markers_to_json(&markers),
        }))?;
        return Ok(());
    }

    if markers.is_empty() {
        println!("No osu! installations found.");
        println!(
            "If osu! is installed in an unusual location, scanner support for targeted roots is still pending."
        );
        return Ok(());
    }

    println!("Found {} osu! installation(s).", markers.len());
    if args.verbose {
        println!(
            "Indexes are 1-based and can be passed to `osu-radio-cli import --index <INDEX>`."
        );
    }

    print_markers_table(&markers);
    Ok(())
}
