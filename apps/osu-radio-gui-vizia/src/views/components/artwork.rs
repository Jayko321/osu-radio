use vizia::{prelude::*, vg};

use crate::assets;

/// Shared cover rendering, bypassing Vizia 0.4's distorted raster background transform.
pub(crate) struct Artwork {
    index: Signal<Option<i32>>,
}

impl Artwork {
    pub(crate) fn new(
        cx: &mut Context,
        source: impl Res<Option<i32>>,
        revision: Signal<u64>,
    ) -> Handle<'_, Self> {
        let index = Signal::new(source.get_value(cx));
        let mut handle = Self { index }.build(cx, |_| {}).hoverable(false);
        let entity = handle.entity();
        source.set_or_bind(handle.context(), move |cx, source| {
            index.set(source.get_value(cx));
            cx.needs_redraw(entity);
        });
        handle.bind(revision, |mut handle| {
            let entity = handle.entity();
            handle.context().needs_redraw(entity);
        })
    }
}

impl View for Artwork {
    #[allow(clippy::as_conversions, clippy::cast_precision_loss)] // Skia draw coordinates are f32.
    fn draw(&self, cx: &mut DrawContext, canvas: &Canvas) {
        let Some(image) = self.index.get().and_then(assets::cover_image) else {
            return;
        };
        let bounds = cx.bounds();
        let Some(source) = centered_crop(
            image.width() as f32,
            image.height() as f32,
            bounds.w,
            bounds.h,
        ) else {
            return;
        };

        cx.draw_shadows(canvas);
        canvas.save();
        canvas.clip_path(&cx.path(), vg::ClipOp::Intersect, true);
        canvas.draw_image_rect_with_sampling_options(
            &image,
            Some((&source, vg::canvas::SrcRectConstraint::Strict)),
            vg::Rect::from_xywh(bounds.x, bounds.y, bounds.w, bounds.h),
            vg::SamplingOptions::new(vg::FilterMode::Linear, vg::MipmapMode::None),
            &vg::Paint::default(),
        );
        canvas.restore();
    }
}

/// Crop equally from opposite sides; one scale factor fills both destination dimensions.
fn centered_crop(image_width: f32, image_height: f32, width: f32, height: f32) -> Option<vg::Rect> {
    if [image_width, image_height, width, height]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0)
    {
        return None;
    }
    let scale = (width / image_width).max(height / image_height);
    let crop_width = width / scale;
    let crop_height = height / scale;
    Some(vg::Rect::from_xywh(
        (image_width - crop_width) / 2.0,
        (image_height - crop_height) / 2.0,
        crop_width,
        crop_height,
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn crops_landscape_portrait_and_matching_ratios_with_uniform_scale() {
        for (iw, ih, w, h) in [
            (1920.0, 1080.0, 340.0, 340.0),
            (1080.0, 1920.0, 440.0, 90.0),
            (1920.0, 1080.0, 960.0, 540.0),
        ] {
            let crop = centered_crop(iw, ih, w, h).unwrap();
            assert!((crop.center_x() - iw / 2.0).abs() < 0.001);
            assert!((crop.center_y() - ih / 2.0).abs() < 0.001);
            assert!((w / crop.width() - h / crop.height()).abs() < 0.001);
            assert!(crop.left >= 0.0 && crop.top >= 0.0 && crop.right <= iw && crop.bottom <= ih);
            assert!((crop.width() - iw).abs() < 0.001 || (crop.height() - ih).abs() < 0.001);
        }
    }

    #[test]
    fn rejects_empty_and_invalid_bounds() {
        for dimensions in [
            (0.0, 100.0, 40.0, 40.0),
            (100.0, 0.0, 40.0, 40.0),
            (100.0, 100.0, 0.0, 40.0),
            (100.0, 100.0, 40.0, 0.0),
            (100.0, 100.0, -1.0, 40.0),
            (100.0, 100.0, f32::NAN, 40.0),
        ] {
            assert!(
                centered_crop(dimensions.0, dimensions.1, dimensions.2, dimensions.3).is_none()
            );
        }
    }

    #[test]
    fn decodes_real_artwork_and_rejects_corrupt_bytes() {
        let bytes = include_bytes!("../../../assets/covers/karakara.jpg");
        let image = assets::decode_cover(bytes).expect("artwork must decode");
        assert_eq!(image.width(), 1280);
        assets::cache_cover(1, image);
        assert!(assets::cover_image(1).is_some());
        assert!(assets::decode_cover(b"corrupt").is_none());
        assets::clear_covers();
        assert!(assets::cover_image(1).is_none());
    }
}
