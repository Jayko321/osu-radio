mod artwork;
mod chip;
mod icon;
mod layout;
mod search_row;
mod sidebar;

use vizia::prelude::*;

pub(crate) use artwork::Artwork;
pub(crate) use chip::chip_row;
pub(crate) use icon::{icon, icon_button};
pub(crate) use layout::{gap, hspacer};
pub(crate) use search_row::search_row;
pub(crate) use sidebar::sidebar;

pub(crate) fn style() -> CSS {
    include_style!("styles/components.css")
}
