pub(crate) const PROJECT_HELP: &str = "\
osu-radio is a development CLI for exploring osu! beatmap audio outside of the game.

The long-term app will use a backend server to discover beatmaps, resolve audio
references, and expose them to a frontend UI. This CLI exists as a small testing
harness while that backend takes shape.

Right now, the CLI helps validate local osu! discovery, scanner behavior, and
database connectivity.
More focused commands will be added as those workflows become concrete.";

pub(crate) const DEFAULT_IMPORT_LIMIT: usize = 20;
pub(crate) const MARKER_TABLE_WIDTHS: &[usize] = &[5, 6, 48, 64];
pub(crate) const IMPORT_TABLE_WIDTHS: &[usize] = &[5, 6, 24, 32, 24, 7, 10];
