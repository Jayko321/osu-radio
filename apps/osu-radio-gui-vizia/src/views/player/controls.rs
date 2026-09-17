use vizia::prelude::*;

use crate::assets;
use crate::views::components::{hspacer, icon_button};

pub(crate) fn controls(cx: &mut Context) {
    HStack::new(cx, |cx| {
        icon_button(cx, assets::VOLUME)
            .name("Volume (unavailable)")
            .disabled(true);

        hspacer(cx);
        transport(cx);
        hspacer(cx);

        icon_button(cx, assets::ADD_CIRCLE)
            .name("Add to playlist (unavailable)")
            .disabled(true);
    })
    .class("controls-row");
}

fn transport(cx: &mut Context) {
    HStack::new(cx, |cx| {
        icon_button(cx, assets::SHUFFLE)
            .name("Shuffle (unavailable)")
            .disabled(true);
        icon_button(cx, assets::SKIP_BACK)
            .name("Previous track (unavailable)")
            .disabled(true);

        play_button(cx);

        icon_button(cx, assets::SKIP_FORWARD)
            .name("Next track (unavailable)")
            .disabled(true);
        icon_button(cx, assets::REPEAT)
            .name("Repeat (unavailable)")
            .disabled(true);
    })
    .class("transport");
}

fn play_button(cx: &mut Context) {
    icon_button(cx, assets::PLAY)
        .class("play-button")
        .name("Play (unavailable)")
        .disabled(true);
}
