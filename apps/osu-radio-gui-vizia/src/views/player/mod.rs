mod controls;
mod cover;
mod progress;

use vizia::prelude::*;

use controls::controls;
use cover::cover;
use progress::{progress_bar, time_row};

use crate::app::UiState;
use crate::sample;
use crate::views::components::{gap, vspacer};

pub(crate) fn player(cx: &mut Context, state: UiState) {
    VStack::new(cx, move |cx| {
        cover(cx, state);
        vspacer(cx);

        Label::new(
            cx,
            state.playing.map(|playing| {
                sample::TRACKS
                    .get(*playing)
                    .map_or_else(String::new, |track| track.title.clone())
            }),
        )
        .class("now-title");
        gap(cx, 6.0);

        Label::new(
            cx,
            state.playing.map(|playing| {
                sample::TRACKS
                    .get(*playing)
                    .map_or_else(String::new, |track| track.artist.clone())
            }),
        )
        .class("now-artist");
        gap(cx, 14.0);

        progress_bar(cx);
        time_row(cx, state);
        gap(cx, 8.0);

        controls(cx);
    })
    .class("player");
}

pub(crate) fn style() -> CSS {
    include_style!("styles/player.css")
}
