use vizia::prelude::*;

use crate::assets;
use crate::views::components::{hspacer, icon, icon_button};

pub(crate) fn controls(cx: &mut Context) {
    HStack::new(cx, |cx| {
        icon_button(cx, assets::VOLUME);

        hspacer(cx);
        transport(cx);
        hspacer(cx);

        icon_button(cx, assets::ADD_CIRCLE);
    })
    .class("controls-row");
}

fn transport(cx: &mut Context) {
    HStack::new(cx, |cx| {
        icon_button(cx, assets::SHUFFLE);
        icon_button(cx, assets::SKIP_BACK);

        play_button(cx);

        icon_button(cx, assets::SKIP_FORWARD);
        icon_button(cx, assets::REPEAT);
    })
    .class("transport");
}

fn play_button(cx: &mut Context) {
    HStack::new(cx, |cx| {
        icon(cx, assets::PLAY);
    })
    .class("play-button");
}
