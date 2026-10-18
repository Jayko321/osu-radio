use osu_radio_media_controls::{Event, LaunchKey, Snapshot};

// Pure transformations are also executable on hosts without a Linux runtime.
#[path = "../src/linux/conversion.rs"]
mod conversion;
#[path = "linux/conversions.rs"]
mod conversions;

#[cfg(target_os = "linux")]
#[path = "linux/protocol.rs"]
mod protocol;
