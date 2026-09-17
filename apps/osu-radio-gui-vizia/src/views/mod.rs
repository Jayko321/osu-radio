pub(crate) mod background;
pub(crate) mod components;
pub(crate) mod player;
pub(crate) mod settings;
pub(crate) mod songs;
pub(crate) mod top_bar;

use vizia::prelude::*;

use background::backdrop;
use player::player;
use settings::settings_pane;
use songs::songs_pane;
use top_bar::top_bar;

use crate::app::{Tab, UiState};

/// Theme and shared controls load before area-specific geometry.
pub(crate) fn styles(cx: &mut Context) {
    let sheets = [
        include_style!("styles/base.css"),
        background::style(),
        components::style(),
        top_bar::style(),
        songs::style(),
        settings::style(),
        player::style(),
        include_style!("styles/gallery.css"),
    ];

    for sheet in sheets {
        if let Err(error) = cx.add_stylesheet(sheet) {
            eprintln!("a bundled stylesheet could not be loaded: {error:?}");
        }
    }
}

pub(crate) fn shell(cx: &mut Context, state: UiState) {
    VStack::new(cx, move |cx| {
        top_bar(cx, state);

        ZStack::new(cx, move |cx| {
            backdrop(cx, state);

            HStack::new(cx, move |cx| {
                songs_pane(cx, state).display(state.tab.map(|tab| *tab == Tab::Songs));
                settings_pane(cx, state).display(state.tab.map(|tab| *tab == Tab::Settings));
                player(cx, state);
            })
            .class("panes");
        })
        .class("body");
    })
    .class("app");
}

#[cfg(test)]
mod style_tests {
    use std::sync::{Arc, RwLock};
    use vizia_style::{CssRule, ParserOptions, Property, StyleSheet, TokenOrValue};

    #[test]
    #[allow(clippy::arc_with_non_send_sync)] // ParserOptions requires Arc<RwLock<_>>.
    fn all_bundled_css_parses_without_errors_or_recovery_warnings() {
        let sheets = [
            ("base", include_str!("../../styles/base.css")),
            ("background", include_str!("../../styles/background.css")),
            ("top-bar", include_str!("../../styles/top-bar.css")),
            ("components", include_str!("../../styles/components.css")),
            ("songs", include_str!("../../styles/songs.css")),
            ("settings", include_str!("../../styles/settings.css")),
            ("player", include_str!("../../styles/player.css")),
            ("gallery", include_str!("../../styles/gallery.css")),
        ];
        for (name, css) in sheets {
            let warnings = Arc::new(RwLock::new(Vec::new()));
            let mut options = ParserOptions::default();
            options.filename = name.into();
            options.warnings = Some(warnings.clone());
            let parsed = StyleSheet::parse(css, options);
            assert!(parsed.is_ok(), "{name}: {parsed:?}");
            if let Ok(sheet) = parsed {
                for rule in sheet.rules.0 {
                    if let CssRule::Style(rule) = rule {
                        for declaration in rule.declarations.declarations {
                            match declaration {
                                Property::Custom(property) => assert!(
                                    property.name.starts_with("--"),
                                    "{name}: unknown property {property:?}"
                                ),
                                // Vizia only resolves a standalone var() for these colour properties.
                                Property::Unparsed(property) => {
                                    assert!(
                                        matches!(
                                            property.name.as_ref(),
                                            "color"
                                                | "fill"
                                                | "background-color"
                                                | "border-color"
                                                | "outline-color"
                                                | "caret-color"
                                                | "selection-color"
                                        ),
                                        "{name}: unsupported value {property:?}"
                                    );
                                    assert!(
                                        matches!(
                                            property.value.0.as_slice(),
                                            [TokenOrValue::Var(_)]
                                        ),
                                        "{name}: unsupported variable expression {property:?}"
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            assert!(
                warnings.read().is_ok_and(|warnings| warnings.is_empty()),
                "{name}: {warnings:?}"
            );
        }
    }
}
