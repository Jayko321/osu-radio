mod artwork;
mod chip;
mod controls;
mod field;
mod icon;
mod layout;
mod menu;
mod modal;
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

pub(crate) use controls::{
    ButtonVariant, FilterTagState, MaterialKind, button, filter_tag, material, tabs, tag, toggle,
};
pub(crate) use field::{field, text_input};
pub(crate) use menu::{MenuItem, menu};
pub(crate) use modal::modal;
