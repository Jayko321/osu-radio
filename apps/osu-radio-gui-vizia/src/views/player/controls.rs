use osu_radio_client::playback::PlayerState;
use vizia::prelude::*;

use crate::views::components::{hspacer, icon, icon_button};
use crate::{
    app::{AppEvent, UiState},
    assets,
};

pub(crate) fn controls(cx: &mut Context, state: UiState) {
    HStack::new(cx, move |cx| {
        Dropdown::new(
            cx,
            |cx| {
                icon_button(cx, assets::VOLUME)
                    .name("Volume")
                    .on_press(|cx| cx.emit(PopupEvent::Switch));
            },
            move |cx| {
                VStack::new(cx, move |cx| {
                    Label::new(
                        cx,
                        state.playback.map(|playback| {
                            format!("Volume {:.0}%", playback.snapshot.volume * 100.0)
                        }),
                    );
                    Slider::new(cx, state.playback.map(|playback| playback.snapshot.volume))
                        .name("Volume")
                        .class("audio-slider")
                        .on_change(|cx, volume| cx.emit(AppEvent::SetVolume(volume)));
                })
                .class("volume-popup");
            },
        )
        .show_arrow(false)
        .placement(Placement::TopStart)
        .class("volume-dropdown");
        hspacer(cx);
        transport(cx, state);
        hspacer(cx);
        icon_button(cx, assets::ADD_CIRCLE)
            .name("Add to playlist (unavailable)")
            .disabled(true);
    })
    .class("controls-row");
}

pub(crate) fn status(cx: &mut Context, state: UiState) {
    let message = Memo::new(move |_| {
        let playback = state.playback.get();
        if let Some(error) = playback.error {
            error
        } else if playback.loading_audio_id.is_some() {
            "Loading audio…".to_owned()
        } else {
            String::new()
        }
    });
    Label::new(cx, message).class("playback-status");
}

fn transport(cx: &mut Context, state: UiState) {
    HStack::new(cx, move |cx| {
        icon_button(cx, assets::SHUFFLE)
            .name("Shuffle (unavailable)")
            .disabled(true);
        icon_button(cx, assets::SKIP_BACK)
            .name("Previous track (unavailable)")
            .disabled(true);
        play_button(cx, state);
        icon_button(cx, assets::SKIP_FORWARD)
            .name("Next track (unavailable)")
            .disabled(true);
        icon_button(cx, assets::REPEAT)
            .name("Repeat (unavailable)")
            .disabled(true);
    })
    .class("transport");
}

fn play_button(cx: &mut Context, state: UiState) {
    let playing = Memo::new(move |_| {
        let playback = state.playback.get();
        state.selected_audio_id.get().is_some()
            && state.selected_audio_id.get() == playback.current_audio_id
            && playback.snapshot.state == PlayerState::Playing
    });
    Button::new(cx, move |cx| {
        ZStack::new(cx, move |cx| {
            Binding::new(cx, playing, move |cx| {
                icon(
                    cx,
                    if playing.get() {
                        assets::PAUSE
                    } else {
                        assets::PLAY
                    },
                );
            });
        })
        .width(Pixels(24.0))
        .height(Pixels(24.0))
        .hoverable(false)
    })
    .class("ui-button")
    .class("icon-button")
    .class("play-button")
    .name(playing.map(|playing| if *playing { "Pause" } else { "Play" }))
    .disabled(state.selected_audio_id.map(Option::is_none))
    .on_press(|cx| cx.emit(AppEvent::PlaySelected));
}
