//! Standalone, memory-only examples. No `AppData`, runtime, session, or installation discovery.
use crate::views::components::ButtonVariant;
use crate::{
    assets,
    views::{
        self,
        components::{
            FilterTagState, MaterialKind, MenuItem, button, field, filter_tag, icon, icon_button,
            material, menu, modal, search_row, tabs, tag, toggle,
        },
    },
};
use vizia::prelude::*;

pub(crate) fn run() -> Result<(), ApplicationError> {
    Application::new(|cx| {
        crate::input::install(cx);
        assets::register(cx);
        views::styles(cx);
        gallery(cx);
    })
    .title("osu! radio · Components")
    .inner_size((1200_u32, 900_u32))
    .min_inner_size(Some((1024_u32, 640_u32)))
    .run()
}

fn section(cx: &mut Context, title: &'static str, content: impl FnOnce(&mut Context)) {
    VStack::new(cx, |cx| {
        Label::new(cx, title).class("gallery-heading");
        content(cx);
    })
    .class("gallery-section");
}

fn gallery(cx: &mut Context) {
    let disabled = Signal::new(false);
    let count = Signal::new(0_u32);
    let modal_open = Signal::new(false);
    ZStack::new(cx, move |cx| {
        ScrollView::new(cx, move |cx| {
            VStack::new(cx, move |cx| {
                Label::new(cx, "osu! radio / COMPONENTS").class("gallery-title");
                Label::new(cx, "Tab to focus · Enter / Space to activate · Arrow keys in menus · Escape to dismiss")
                    .class("field-hint");
                toggle(cx, "Disable demo controls", disabled, move |_| disabled.set(!disabled.get()));
                Label::new(cx, count.map(|value| format!("Button presses: {value}"))).class("field-hint");
                VStack::new(cx, move |cx| {
                    buttons(cx, count);
                    fields(cx);
                    switches_and_tabs(cx);
                    tags(cx);
                    menus(cx);
                    section(cx, "Modal", move |cx| {
                        button(cx, "Create playlist", ButtonVariant::Accent).on_press(move |_| modal_open.set(true));
                    });
                }).class("gallery-controls").disabled(disabled);
                materials(cx);
                icons(cx);
            }).class("gallery-page");
        }).show_horizontal_scrollbar(false);
        modal(cx, modal_open, "Create playlist", move |_| modal_open.set(false), move |cx| {
            Label::new(cx, "Collect your favourite songs in one place. This example stays in memory.").class("modal-description");
            let name = Signal::new(String::new());
            field(cx, Some("Name"), Some("Try Tab / Shift+Tab, Escape and clicking the backdrop."), name, "My playlist", move |_, text| name.set(text));
            button(cx, "Create", ButtonVariant::Accent).on_press(move |_| modal_open.set(false));
        });
    }).class("gallery-root");
}

fn buttons(cx: &mut Context, count: Signal<u32>) {
    section(cx, "Buttons", move |cx| {
        HStack::new(cx, move |cx| {
            for (label, variant) in [
                ("Light", ButtonVariant::Light),
                ("Alternate", ButtonVariant::Alternate),
                ("Accent", ButtonVariant::Accent),
                ("Link", ButtonVariant::Link),
            ] {
                button(cx, label, variant)
                    .on_press(move |_| count.set(count.get().saturating_add(1)));
            }
            icon_button(cx, assets::ADD)
                .name("Add")
                .on_press(move |_| count.set(count.get().saturating_add(1)));
            button(cx, "Disabled", ButtonVariant::Accent)
                .disabled(true)
                .on_press(move |_| count.set(count.get().saturating_add(1)));
        })
        .class("gallery-row");
    });
}

fn fields(cx: &mut Context) {
    section(cx, "Fields & search", |cx| {
        let empty = Signal::new(String::new());
        let filled =
            Signal::new("A very long playlist title · 夜に駆ける · Музыка для вечера".to_owned());
        let query = Signal::new(String::new());
        field(
            cx,
            Some("Playlist name"),
            Some("Give your collection a name."),
            empty,
            "New playlist",
            move |_, text| empty.set(text),
        );
        field(cx, None, None, filled, "Name", move |_, text| {
            filled.set(text);
        });
        search_row(cx, query, "Search songs...");
        let disabled_text = Signal::new("Unavailable field".to_owned());
        field(
            cx,
            Some("Disabled"),
            None,
            disabled_text,
            "Disabled field",
            move |_, text| disabled_text.set(text),
        )
        .disabled(true);
    });
}

fn switches_and_tabs(cx: &mut Context) {
    section(cx, "Switches & tabs", |cx| {
        let off = Signal::new(false);
        let on = Signal::new(true);
        toggle(cx, "Off / On", off, move |_| off.set(!off.get()));
        toggle(cx, "On / Off", on, move |_| on.set(!on.get()));
        toggle(cx, "Disabled switch", true, |_| {}).disabled(true);
        let selected = Signal::new(0_usize);
        tabs(
            cx,
            &["Songs", "Playlists", "Settings"],
            selected,
            move |_, index| selected.set(index),
        );
        Label::new(cx, selected.map(|value| format!("Selected tab: {value}"))).class("field-hint");
    });
}

fn tags(cx: &mut Context) {
    section(cx, "Tags", |cx| {
        HStack::new(cx, |cx| {
            let selected = Signal::new(false);
            let chosen = Signal::new(true);
            tag(cx, "Electronic", selected).on_press(move |_| selected.set(!selected.get()));
            tag(cx, "Selected", chosen).on_press(move |_| chosen.set(!chosen.get()));
            tag(cx, "Disabled", true).disabled(true);
        })
        .class("gallery-row");
        HStack::new(cx, |cx| {
            for (label, initial) in [
                ("Ambient", FilterTagState::Neutral),
                ("Jazz", FilterTagState::Included),
                ("Rock", FilterTagState::Excluded),
            ] {
                let state = Signal::new(initial);
                filter_tag(cx, label, state).on_press(move |_| state.set(state.get().next()));
            }
        })
        .class("gallery-row");
    });
}

fn menus(cx: &mut Context) {
    section(cx, "Menus", |cx| {
        let selected = Signal::new(Some(0));
        let tags = vec![
            MenuItem {
                id: 0,
                label: "All tags".into(),
            },
            MenuItem {
                id: 1,
                label: "Electronic".into(),
            },
            MenuItem {
                id: 2,
                label: "Instrumental".into(),
            },
        ];
        menu(
            cx,
            Signal::new(tags),
            selected,
            "Tags",
            false,
            move |_, id| selected.set(Some(id)),
        );
        let selected = Signal::new(Some(0));
        let playlists: Vec<_> = (0..30).map(|id| MenuItem { id, label: if id == 0 {
                            "A very long playlist name — favourites for a quiet evening / お気に入り / Избранное".into()
                        } else { format!("Playlist {id:02} · late night radio") }}).collect();
        menu(
            cx,
            Signal::new(playlists),
            selected,
            "Playlists",
            true,
            move |_, id| selected.set(Some(id)),
        );
        menu(
            cx,
            Signal::new(Vec::new()),
            Signal::new(None),
            "Empty menu",
            true,
            |_, _| {},
        );
    });
}

fn materials(cx: &mut Context) {
    section(cx, "Materials", |cx| {
        ZStack::new(cx, |cx| {
            HStack::new(cx, |cx| {
                for color in ["swatch-accent", "swatch-green", "swatch-red"]
                    .into_iter()
                    .cycle()
                    .take(12)
                {
                    Element::new(cx).class("color-band").class(color);
                }
            })
            .class("material-backdrop");
            HStack::new(cx, |cx| {
                for (kind, title) in [
                    (MaterialKind::Regular, "Regular · 50px"),
                    (MaterialKind::Thick, "Thick · 60px"),
                    (MaterialKind::Thin, "Thin · 60px"),
                ] {
                    material(cx, kind, |cx| {
                        Label::new(cx, title).class("gallery-heading");
                        Label::new(cx, "Sharp text above a blurred, coloured backdrop")
                            .class("field-hint");
                        icon(cx, assets::MUSIC);
                    })
                    .class("material-sample");
                }
            })
            .class("material-samples");
        })
        .class("material-stage");
    });
}

fn icons(cx: &mut Context) {
    section(cx, "Lucide · 24px / window controls · 16px", |cx| {
        HStack::new(cx, |cx| {
            for glyph in [
                assets::SEARCH,
                assets::CHEVRON,
                assets::PENCIL,
                assets::ADD,
                assets::PLAY,
                assets::SKIP_BACK,
                assets::SKIP_FORWARD,
                assets::SHUFFLE,
                assets::REPEAT,
                assets::VOLUME,
                assets::ADD_CIRCLE,
                assets::STACK,
                assets::MUSIC,
                assets::SETTINGS,
            ] {
                icon(cx, glyph);
            }
        })
        .class("gallery-row");
        HStack::new(cx, |cx| {
            for glyph in [
                assets::MINIMIZE,
                assets::MAXIMIZE,
                assets::RESTORE,
                assets::CLOSE,
            ] {
                icon(cx, glyph).size(Pixels(16.0));
            }
        })
        .class("gallery-row");
    });
}
