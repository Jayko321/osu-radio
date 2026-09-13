use osu_radio_client::Track;
use vizia::prelude::*;

use crate::app::UiState;
use crate::views::components::hspacer;

pub(crate) fn progress_bar(cx: &mut Context) {
    ZStack::new(cx, |cx| {
        Element::new(cx).class("progress-track");
        ZStack::new(cx, |cx| {
            Element::new(cx).class("progress-knob");
        })
        .class("progress-fill")
        .width(Pixels(0.0));
    })
    .class("progress-row");
}

pub(crate) fn time_row(cx: &mut Context, state: UiState) {
    HStack::new(cx, move |cx| {
        Label::new(cx, "00:00");
        hspacer(cx);
        Label::new(
            cx,
            state.selected.map(|track| {
                track
                    .as_ref()
                    .map_or_else(|| "--:--".to_owned(), Track::duration_label)
            }),
        );
    })
    .class("time-row");
}
