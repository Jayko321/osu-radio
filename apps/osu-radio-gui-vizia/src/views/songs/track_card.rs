use osu_radio_client::Track;
use vizia::prelude::*;

use crate::app::{AppEvent, UiState};
use crate::assets;

pub(crate) fn track_card(cx: &mut Context, state: UiState, index: usize, track: &Track) {
    let title = track.title.clone();
    let meta = track.meta();

    ZStack::new(cx, move |cx| {
        Element::new(cx)
            .class("track-card-tint")
            .class(assets::tint(index));

        VStack::new(cx, move |cx| {
            Label::new(cx, title).class("track-card-title");
            Label::new(cx, meta).class("track-card-meta");
        })
        .class("track-card-text");
    })
    .class("track-card")
    .background_image(assets::cover_image(index))
    .toggle_class(
        "playing",
        state.playing.map(move |playing| *playing == index),
    )
    .on_press(move |cx| cx.emit(AppEvent::SelectTrack(index)));
}
