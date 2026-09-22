use osu_radio_client::Track;
use vizia::prelude::*;

use crate::app::{AppEvent, UiState};
use crate::assets;
use crate::views::components::Artwork;

pub(crate) fn track_card(
    cx: &mut Context,
    state: UiState,
    index: usize,
    track: Memo<Option<Track>>,
) -> Handle<'_, VStack> {
    VStack::new(cx, move |cx| {
        Binding::new(
            cx,
            track.map(|track| track.as_ref().map(|track| track.audio_source_id)),
            move |cx| {
                if let Some(track) = track.get() {
                    cx.emit(AppEvent::RequestMedia(track.audio_source_id));
                }
            },
        );
        ZStack::new(cx, move |cx| {
            Artwork::new(
                cx,
                track.map(|t| t.as_ref().and_then(|t| t.cover_beatmap_id)),
                state.artwork_revision,
            )
            .class("track-card-artwork");
            Element::new(cx)
                .class("track-card-tint")
                .class(assets::tint(index));
            VStack::new(cx, move |cx| {
                Label::new(
                    cx,
                    track.map(|t| t.as_ref().map_or_else(String::new, |t| t.title.clone())),
                )
                .class("track-card-title");
                Label::new(
                    cx,
                    track.map(|t| t.as_ref().map_or_else(String::new, Track::meta)),
                )
                .class("track-card-meta");
            })
            .class("track-card-text");
            Element::new(cx).class("track-card-border");
        })
        .class("track-card")
        .toggle_class(
            "playing",
            state
                .selected_audio_id
                .map(move |id| *id == track.get().as_ref().map(|t| t.audio_source_id)),
        )
        .on_press(move |cx| {
            if let Some(track) = track.get() {
                cx.emit(AppEvent::SelectTrack(track.audio_source_id));
            }
        });
    })
    .class("track-row")
}
