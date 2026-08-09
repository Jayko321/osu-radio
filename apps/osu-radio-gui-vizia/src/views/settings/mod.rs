mod field;
mod section;

use vizia::prelude::*;

use section::section;

use crate::app::UiState;
use crate::sample;
use crate::views::components::{gap, search_row, sidebar};

pub(crate) fn settings_pane(cx: &mut Context, state: UiState) -> Handle<'_, VStack> {
    sidebar(cx, move |cx| {
        search_row(cx, state.settings_query, "Type to search settings...");
        gap(cx, 40.0);

        ScrollView::new(cx, |cx| {
            for entry in sample::SECTIONS {
                section(cx, entry);
            }
        })
        .show_horizontal_scrollbar(false)
        .class("settings-list");
    })
}

pub(crate) fn style() -> CSS {
    include_style!("styles/settings.css")
}
