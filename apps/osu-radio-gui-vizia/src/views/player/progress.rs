use osu_radio_client::Track;
use vizia::prelude::*;

use crate::app::UiState;
use crate::sample;
use crate::views::components::hspacer;

pub(crate) fn progress_bar(cx: &mut Context) {
    ZStack::new(cx, |cx| {
        Element::new(cx).class("progress-track");
        Element::new(cx).class("progress-fill");
        Element::new(cx).class("progress-knob");
    })
    .class("progress-row");
}

pub(crate) fn time_row(cx: &mut Context, state: UiState) {
    HStack::new(cx, move |cx| {
        Label::new(cx, sample::ELAPSED);
        hspacer(cx);
        Label::new(
            cx,
            state.playing.map(|playing| {
                sample::TRACKS
                    .get(*playing)
                    .map_or_else(String::new, Track::duration_label)
            }),
        );
    })
    .class("time-row");
}
