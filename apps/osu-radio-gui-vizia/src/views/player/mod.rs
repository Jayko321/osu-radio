mod controls;
mod cover;
mod progress;

use vizia::prelude::*;

use controls::controls;
use cover::cover;
use progress::{progress_bar, time_row};

use crate::app::UiState;
use crate::views::components::gap;

pub(crate) fn player(cx: &mut Context, state: UiState) {
    let geometry = Signal::new(PlayerGeometry::new(960.0, 902.0));
    VStack::new(cx, move |cx| {
        cover(cx, state, geometry);
        VStack::new(cx, move |cx| {
            Label::new(
                cx,
                state.selected.map(|track| {
                    track
                        .as_ref()
                        .map_or_else(String::new, |track| track.title.clone())
                }),
            )
            .class("now-title");
            gap(cx, 6.0);

            Label::new(
                cx,
                state.selected.map(|track| {
                    track
                        .as_ref()
                        .map_or_else(String::new, |track| track.artist.clone())
                }),
            )
            .class("now-artist");
            gap(cx, 14.0);

            progress_bar(cx, state);
            time_row(cx, state);
            gap(cx, 8.0);

            controls(cx, state);
            controls::status(cx, state);
        })
        .class("player-info");
    })
    .class("player")
    .padding_left(geometry.map(|size| Pixels(size.left)))
    .padding_right(geometry.map(|size| Pixels(size.right)))
    .on_geo_changed(move |cx, _| {
        let bounds = cx.bounds();
        let size = PlayerGeometry::new(bounds.w / cx.scale_factor(), bounds.h / cx.scale_factor());
        if geometry.get() != size {
            geometry.set(size);
        }
    });
}

pub(crate) fn style() -> CSS {
    include_style!("styles/player.css")
}

/// All dimensions here are logical pixels; Vizia scales the bound styles once.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PlayerGeometry {
    artwork: f32,
    left: f32,
    right: f32,
}

impl PlayerGeometry {
    fn new(width: f32, height: f32) -> Self {
        let width = width.max(0.0);
        let scale = (width / 960.0).max(1.0);
        let content = (650.0 * scale).min(960.0).min(width * 0.85);
        let spare = width - content;
        // Preserve the reference's 138:172 margins without fixing them at smaller sizes.
        let left = spare * (138.0 / 310.0);
        Self {
            artwork: (340.0 * scale)
                .min(640.0)
                .min(content)
                .min((height - 225.0).max(0.0)),
            left,
            right: spare - left,
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // Exact reference geometry is intentional.
mod tests {
    use super::*;

    #[test]
    fn player_dimensions_across_window_sizes() {
        for (width, height, artwork, content) in [
            (1024.0, 640.0, 340.0, 462.4),
            (1440.0, 952.0, 340.0, 650.0),
            (1600.0, 900.0, 396.6667, 758.3333),
            (1920.0, 1080.0, 510.0, 960.0),
            (2560.0, 1440.0, 640.0, 960.0),
        ] {
            let pane = width - 480.0;
            let size = PlayerGeometry::new(pane, height - 50.0);
            assert!(
                (size.artwork - artwork).abs() < 0.01,
                "{width}x{height}: {size:?}"
            );
            assert!((pane - size.left - size.right - content).abs() < 0.01);
            assert!(size.artwork <= height - 275.0);
        }
        let reference = PlayerGeometry::new(960.0, 902.0);
        assert_eq!(480.0 + reference.left, 618.0);
        assert_eq!(1440.0 - reference.right, 1268.0);
    }

    #[test]
    fn artwork_respects_short_and_empty_bounds() {
        assert_eq!(PlayerGeometry::new(960.0, 400.0).artwork, 175.0);
        assert_eq!(PlayerGeometry::new(0.0, 0.0).artwork, 0.0);
    }
}
