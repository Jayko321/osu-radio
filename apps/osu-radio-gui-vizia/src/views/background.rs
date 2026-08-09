use vizia::prelude::*;

use crate::app::UiState;
use crate::assets;

pub(crate) fn backdrop(cx: &mut Context, state: UiState) {
    Element::new(cx)
        .class("backdrop")
        .background_image(state.playing.map(|playing| assets::cover_image(*playing)));
    Element::new(cx).class("glow-warm");
    Element::new(cx).class("glow-cool");
}

pub(crate) fn style() -> CSS {
    include_style!("styles/background.css")
}
