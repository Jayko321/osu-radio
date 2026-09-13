use vizia::prelude::*;

use super::track_card::track_card;
use crate::app::{AppEvent, UiState};

pub(crate) fn track_list(cx: &mut Context, state: UiState) {
    HStack::new(cx, move |cx| {
        Label::new(cx, state.library_message).class("load-message");
        Button::new(cx, |cx| Label::new(cx, "Refresh / Retry"))
            .on_press(|cx| cx.emit(AppEvent::RefreshLibrary))
            .disabled(state.library_loading);
    })
    .height(Auto);
    Binding::new(cx, state.library_revision, move |cx| {
        // Old row bindings can run while a refreshed list shrinks. Never index unchecked.
        VirtualList::new_generic(
            cx,
            state.tracks,
            Vec::len,
            |tracks, index| tracks.get(index).cloned(),
            106.0,
            move |cx, index, track| track_card(cx, state, index, track),
        )
        .class("track-list");
    });
}
