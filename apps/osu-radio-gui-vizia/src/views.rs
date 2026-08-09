use vizia::prelude::*;

use crate::app::{AppEvent, Tab, UiState};
use crate::assets::{self, icon};
use crate::sample;

pub fn shell(cx: &mut Context, state: UiState) {
    VStack::new(cx, move |cx| {
        top_bar(cx, state);

        ZStack::new(cx, move |cx| {
            Element::new(cx)
                .class("backdrop")
                .background_image(state.playing.map(|playing| sample::TRACKS[*playing].cover));
            Element::new(cx).class("glow-warm");
            Element::new(cx).class("glow-cool");

            HStack::new(cx, move |cx| {
                songs_pane(cx, state).display(state.tab.map(|tab| *tab == Tab::Songs));
                settings_pane(cx, state).display(state.tab.map(|tab| *tab == Tab::Settings));
                stage(cx, state);
            })
            .class("panes");
        })
        .class("body");
    })
    .class("app");
}

fn top_bar(cx: &mut Context, state: UiState) {
    HStack::new(cx, move |cx| {
        nav_tab(cx, state, Tab::Songs, assets::MUSIC, "Songs");
        nav_tab(cx, state, Tab::Settings, assets::SETTINGS, "Settings");

        Element::new(cx).class("hspacer");

        Label::new(cx, state.status)
            .class("server-status")
            .display(state.status.map(|status| !status.is_empty()));

        icon(cx, assets::STACK).class("icon-button");
    })
    .class("top-bar");
}

fn nav_tab(cx: &mut Context, state: UiState, tab: Tab, glyph: &'static [u8], text: &'static str) {
    HStack::new(cx, move |cx| {
        icon(cx, glyph);
        Label::new(cx, text).class("nav-label");
    })
    .class("nav-tab")
    .toggle_class("active", state.tab.map(move |current| *current == tab))
    .on_press(move |cx| cx.emit(AppEvent::SelectTab(tab)));
}

fn songs_pane(cx: &mut Context, state: UiState) -> Handle<'_, VStack> {
    VStack::new(cx, move |cx| {
        search_row(cx, state.song_query, "Type to search songs...");
        gap(cx, 16.0);

        HStack::new(cx, |cx| {
            for filter in sample::FILTERS {
                HStack::new(cx, move |cx| {
                    icon(cx, assets::CHEVRON);
                    Label::new(cx, filter);
                })
                .class("chip");
            }
        })
        .class("chip-row");
        gap(cx, 32.0);

        ScrollView::new(cx, move |cx| {
            for (index, track) in sample::TRACKS.iter().enumerate() {
                track_card(cx, state, index, track);
            }
        })
        .show_horizontal_scrollbar(false)
        .class("track-list");
    })
    .class("sidebar")
}

fn track_card(cx: &mut Context, state: UiState, index: usize, track: &'static sample::Track) {
    ZStack::new(cx, move |cx| {
        Element::new(cx).class("track-card-tint").class(track.tint);

        VStack::new(cx, move |cx| {
            Label::new(cx, track.title).class("track-card-title");
            Label::new(cx, track.meta()).class("track-card-meta");
        })
        .class("track-card-text");
    })
    .class("track-card")
    .background_image(track.cover)
    .toggle_class(
        "playing",
        state.playing.map(move |playing| *playing == index),
    )
    .on_press(move |cx| cx.emit(AppEvent::SelectTrack(index)));
}

fn settings_pane(cx: &mut Context, state: UiState) -> Handle<'_, VStack> {
    VStack::new(cx, move |cx| {
        search_row(cx, state.settings_query, "Type to search settings...");
        gap(cx, 40.0);

        ScrollView::new(cx, |cx| {
            for section in sample::SECTIONS {
                VStack::new(cx, move |cx| {
                    HStack::new(cx, move |cx| {
                        icon(cx, section.glyph);
                        Label::new(cx, section.title);
                    })
                    .class("section-header");

                    for field in section.fields {
                        VStack::new(cx, move |cx| {
                            Label::new(cx, field.label).class("field-label");

                            HStack::new(cx, move |cx| {
                                Label::new(cx, field.value);

                                if field.pickable {
                                    icon(cx, assets::CHEVRON);
                                }
                            })
                            .class("field-box")
                            .toggle_class("pickable", field.pickable);
                        })
                        .class("field");
                    }
                })
                .class("settings-section");
            }
        })
        .show_horizontal_scrollbar(false)
        .class("settings-list");
    })
    .class("sidebar")
}

/// Vertical spacing between siblings. A child's CSS `top` is ignored inside a stack, so gaps that
/// differ per sibling have to be their own element.
fn gap(cx: &mut Context, height: f32) {
    Element::new(cx).width(Stretch(1.0)).height(Pixels(height));
}

/// The placeholder is drawn as a label rather than through `Textbox::placeholder`, which panics
/// with a subtract overflow in vizia 0.4.0's accessibility pass whenever the box is empty.
fn search_row(cx: &mut Context, query: Signal<String>, placeholder: &'static str) {
    HStack::new(cx, move |cx| {
        ZStack::new(cx, move |cx| {
            Label::new(cx, placeholder)
                .class("search-placeholder")
                .display(query.map(|query| query.is_empty()));

            Textbox::new(cx, query).on_edit(move |_, text| query.set(text));
        })
        .class("search-field");

        icon(cx, assets::SEARCH);
    })
    .class("search-row");
}

fn stage(cx: &mut Context, state: UiState) {
    VStack::new(cx, move |cx| {
        Element::new(cx)
            .class("cover")
            .background_image(state.playing.map(|playing| sample::TRACKS[*playing].cover));

        Element::new(cx).class("vspacer");

        Label::new(
            cx,
            state
                .playing
                .map(|playing| sample::TRACKS[*playing].title.to_owned()),
        )
        .class("now-title");
        gap(cx, 6.0);

        Label::new(
            cx,
            state
                .playing
                .map(|playing| sample::TRACKS[*playing].artist.to_owned()),
        )
        .class("now-artist");
        gap(cx, 14.0);

        ZStack::new(cx, |cx| {
            Element::new(cx).class("progress-track");
            Element::new(cx).class("progress-fill");
            Element::new(cx).class("progress-knob");
        })
        .class("progress-row");

        HStack::new(cx, move |cx| {
            Label::new(cx, sample::ELAPSED);
            Element::new(cx).class("hspacer");
            Label::new(
                cx,
                state
                    .playing
                    .map(|playing| sample::TRACKS[*playing].duration.to_owned()),
            );
        })
        .class("time-row");
        gap(cx, 8.0);

        controls(cx);
    })
    .class("stage");
}

fn controls(cx: &mut Context) {
    HStack::new(cx, |cx| {
        icon(cx, assets::VOLUME).class("icon-button");

        Element::new(cx).class("hspacer");

        HStack::new(cx, |cx| {
            icon(cx, assets::SHUFFLE).class("icon-button");
            icon(cx, assets::SKIP_BACK).class("icon-button");

            HStack::new(cx, |cx| {
                icon(cx, assets::PLAY);
            })
            .class("play-button");

            icon(cx, assets::SKIP_FORWARD).class("icon-button");
            icon(cx, assets::REPEAT).class("icon-button");
        })
        .class("transport");

        Element::new(cx).class("hspacer");

        icon(cx, assets::ADD_CIRCLE).class("icon-button");
    })
    .class("controls-row");
}
