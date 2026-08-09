use vizia::prelude::*;

use super::icon;
use crate::assets;

pub(crate) fn chip_row(cx: &mut Context, labels: &'static [&'static str]) {
    HStack::new(cx, move |cx| {
        for &label in labels {
            chip(cx, label);
        }
    })
    .class("chip-row");
}

fn chip(cx: &mut Context, label: &'static str) {
    HStack::new(cx, move |cx| {
        icon(cx, assets::CHEVRON);
        Label::new(cx, label);
    })
    .class("chip");
}
