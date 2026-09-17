mod track_card;
mod track_list;

use vizia::prelude::*;

use track_list::track_list;

use crate::app::{AppEvent, UiState};
use crate::assets;
use crate::views::components::{chip_row, gap, icon, search_row, sidebar};

pub(crate) fn songs_pane(cx: &mut Context, state: UiState) -> Handle<'_, VStack> {
    sidebar(cx, move |cx| {
        search_row(cx, state.song_query, "Type to search songs...");
        gap(cx, 16.0);

        chip_row(cx, &["Title", "All musics", "Tags"]);
        gap(cx, 32.0);

        track_list(cx, state);

        HStack::new(cx, move |cx| {
            Button::new(cx, |cx| {
                HStack::new(cx, |cx| {
                    icon(cx, assets::REFRESH);
                    Label::new(cx, "Refresh library");
                })
                .class("library-refresh-content")
            })
            .class("ui-button")
            .class("button-alternate")
            .class("library-refresh")
            .name("Refresh library")
            .on_press(|cx| cx.emit(AppEvent::RefreshLibrary))
            .disabled(state.library_loading);
        })
        .class("songs-footer");
    })
}

pub(crate) fn style() -> CSS {
    include_style!("styles/songs.css")
}
