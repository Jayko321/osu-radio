use vizia::prelude::*;

use crate::app::UiState;
use crate::assets;

pub(crate) fn cover(cx: &mut Context, state: UiState) {
    Element::new(cx)
        .class("cover")
        .background_image(state.playing.map(|playing| assets::cover_image(*playing)));
}
