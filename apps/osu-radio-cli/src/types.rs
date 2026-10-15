use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use radio_core::OsuKind;
use radio_scanner::discovery::DiscoveryDepth;

use crate::consts::{DEFAULT_IMPORT_LIMIT, PROJECT_HELP};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Development CLI for osu-radio.",
    long_about = PROJECT_HELP,
    disable_help_subcommand = true
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    #[command(
        about = "Explain what osu-radio is and why this CLI exists.",
        long_about = PROJECT_HELP
    )]
    Help,
    #[command(about = "Find local osu! installations that osu-radio can inspect.")]
    Scan(ScanArgs),
    #[command(about = "Import beatmap metadata from a discovered or explicit osu! source.")]
    Import(ImportArgs),
    #[command(about = "Import beatmaps from an osu! source into the configured database.")]
    Store(StoreArgs),
    #[command(about = "Connect to the database configured in .env and verify it responds.")]
    Database,
}

#[derive(Debug, Args)]
pub(crate) struct ScanArgs {
    #[command(flatten)]
    pub(crate) filters: MarkerFilters,
    #[arg(
        long,
        short = '1',
        conflicts_with = "limit",
        help = "Stop as soon as the first installation is found."
    )]
    pub(crate) first: bool,
    #[arg(
        long,
        value_name = "COUNT",
        help = "Stop after finding COUNT installations."
    )]
    pub(crate) limit: Option<usize>,
    #[arg(
        long,
        value_enum,
        default_value_t = DepthArg::Full,
        help = "How far discovery may search the filesystem."
    )]
    pub(crate) depth: DepthArg,
    #[arg(long, help = "Print machine-readable JSON instead of a table.")]
    pub(crate) json: bool,
    #[arg(short, long, help = "Print extra context for human-readable output.")]
    pub(crate) verbose: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum DepthArg {
    Known,
    Shallow,
    Full,
}

impl From<DepthArg> for DiscoveryDepth {
    fn from(depth: DepthArg) -> Self {
        match depth {
            DepthArg::Known => Self::Known,
            DepthArg::Shallow => Self::Shallow,
            DepthArg::Full => Self::Full,
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct ImportArgs {
    #[command(flatten)]
    pub(crate) filters: MarkerFilters,
    #[arg(
        long,
        value_name = "INDEX",
        help = "Select a discovered installation by the 1-based index shown by `scan`."
    )]
    pub(crate) index: Option<usize>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Import from an explicit marker file, such as client.realm or osu!.db."
    )]
    pub(crate) marker: Option<PathBuf>,
    #[arg(
        long,
        default_value_t = DEFAULT_IMPORT_LIMIT,
        value_name = "COUNT",
        help = "Maximum number of beatmaps to print."
    )]
    pub(crate) limit: usize,
    #[arg(long, help = "Print machine-readable JSON instead of a table.")]
    pub(crate) json: bool,
    #[arg(short, long, help = "Print extra context for human-readable output.")]
    pub(crate) verbose: bool,
}

#[derive(Debug, Args)]
pub(crate) struct StoreArgs {
    #[command(flatten)]
    pub(crate) filters: MarkerFilters,
    #[arg(
        long,
        value_name = "INDEX",
        help = "Select a discovered installation by the 1-based index shown by `scan`."
    )]
    pub(crate) index: Option<usize>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Import from an explicit marker file, such as client.realm or osu!.db."
    )]
    pub(crate) marker: Option<PathBuf>,
    #[arg(
        long,
        help = "Reset application tables before importing, discarding stored beatmaps and registered osu! folders."
    )]
    pub(crate) clear: bool,
    #[arg(long, help = "Print machine-readable JSON instead of a summary.")]
    pub(crate) json: bool,
    #[arg(short, long, help = "Print extra context for human-readable output.")]
    pub(crate) verbose: bool,
}

#[derive(Debug, Args)]
pub(crate) struct MarkerFilters {
    #[arg(
        long,
        value_enum,
        help = "Keep only discovered installations from this osu! source."
    )]
    pub(crate) source: Option<SourceArg>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Constrain discovery to PATH instead of searching every mounted drive."
    )]
    pub(crate) root: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum SourceArg {
    Stable,
    Lazer,
}

impl From<SourceArg> for OsuKind {
    fn from(source: SourceArg) -> Self {
        match source {
            SourceArg::Stable => Self::Stable,
            SourceArg::Lazer => Self::Lazer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, DepthArg, SourceArg};
    use clap::Parser;
    use radio_core::OsuKind;
    use radio_scanner::discovery::DiscoveryDepth;

    #[test]
    fn store_rejects_count_and_retains_explicit_clear() {
        assert!(Cli::try_parse_from(["osu-radio-cli", "store", "--count", "1"]).is_err());
        let cli = Cli::try_parse_from(["osu-radio-cli", "store", "--clear"])
            .expect("explicit reset should remain available");
        assert!(matches!(cli.command, Some(Command::Store(args)) if args.clear));
    }

    #[test]
    fn defaults_do_not_reset_storage_and_import_limits_only_preview_output() {
        assert!(
            Cli::try_parse_from(["osu-radio-cli"])
                .unwrap()
                .command
                .is_none()
        );
        let cli = Cli::try_parse_from(["osu-radio-cli", "store"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Store(args)) if !args.clear));
        let cli = Cli::try_parse_from(["osu-radio-cli", "import"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Import(args)) if args.limit == 20));
        let cli = Cli::try_parse_from(["osu-radio-cli", "import", "--limit", "0"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Import(args)) if args.limit == 0));
        let cli = Cli::try_parse_from(["osu-radio-cli", "scan"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Scan(args)) if
            !args.first && args.limit.is_none() && matches!(args.depth, DepthArg::Full)));
    }

    #[test]
    fn scan_rejects_conflicting_limits_unknown_sources_and_depths() {
        for args in [
            vec!["scan", "--first", "--limit", "2"],
            vec!["scan", "-1", "--limit", "2"],
            vec!["scan", "--depth", "recursive"],
            vec!["scan", "--source", "unknown"],
            vec!["scan", "--limit", "-1"],
        ] {
            assert!(Cli::try_parse_from(std::iter::once("osu-radio-cli").chain(args)).is_err());
        }
    }

    #[test]
    fn cli_enums_map_to_the_scanner_domain() {
        assert_eq!(OsuKind::from(SourceArg::Stable), OsuKind::Stable);
        assert_eq!(OsuKind::from(SourceArg::Lazer), OsuKind::Lazer);
        assert_eq!(DiscoveryDepth::from(DepthArg::Known), DiscoveryDepth::Known);
        assert_eq!(
            DiscoveryDepth::from(DepthArg::Shallow),
            DiscoveryDepth::Shallow
        );
        assert_eq!(DiscoveryDepth::from(DepthArg::Full), DiscoveryDepth::Full);
    }
}
