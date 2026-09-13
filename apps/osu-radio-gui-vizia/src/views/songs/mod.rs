mod track_card;
mod track_list;

use vizia::prelude::*;

use track_list::track_list;

use crate::app::UiState;
use crate::views::components::{chip_row, gap, search_row, sidebar};

pub(crate) fn songs_pane(cx: &mut Context, state: UiState) -> Handle<'_, VStack> {
    sidebar(cx, move |cx| {
        search_row(cx, state.song_query, "Type to search songs...");
        gap(cx, 16.0);

        chip_row(cx, &["Title", "All musics", "Tags"]);
        gap(cx, 32.0);

        track_list(cx, state);
    })
}

pub(crate) fn style() -> CSS {
    include_style!("styles/songs.css")
}
