use vizia::prelude::*;

use super::PlayerGeometry;
use crate::app::UiState;
use crate::views::components::Artwork;

pub(super) fn cover(cx: &mut Context, state: UiState, geometry: Signal<PlayerGeometry>) {
    VStack::new(cx, move |cx| {
        Artwork::new(
            cx,
            state
                .selected
                .map(|track| track.as_ref().and_then(|t| t.cover_beatmap_id)),
            state.artwork_revision,
        )
        .class("cover")
        .size(geometry.map(|size| Pixels(size.artwork)));
    })
    .class("artwork-region");
}
