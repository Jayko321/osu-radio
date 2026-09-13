use vizia::prelude::*;

use crate::app::UiState;
use crate::views::components::Artwork;

pub(crate) fn backdrop(cx: &mut Context, state: UiState) {
    Artwork::new(
        cx,
        state
            .selected
            .map(|track| track.as_ref().and_then(|t| t.cover_beatmap_id)),
        state.artwork_revision,
    )
    .class("backdrop");
    Element::new(cx).class("glow-warm");
    Element::new(cx).class("glow-cool");
}

pub(crate) fn style() -> CSS {
    include_style!("styles/background.css")
}
