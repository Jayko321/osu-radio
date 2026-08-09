use vizia::prelude::*;

use super::track_card::track_card;
use crate::app::UiState;
use crate::sample;

pub(crate) fn track_list(cx: &mut Context, state: UiState) {
    ScrollView::new(cx, move |cx| {
        for (index, track) in sample::TRACKS.iter().enumerate() {
            track_card(cx, state, index, track);
        }
    })
    .show_horizontal_scrollbar(false)
    .class("track-list");
}
