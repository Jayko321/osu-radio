use vizia::prelude::*;

pub const SEARCH: &[u8] = include_bytes!("../assets/icons/search-line.svg");
pub const CHEVRON: &[u8] = include_bytes!("../assets/icons/arrow-down-s-line.svg");
pub const PENCIL: &[u8] = include_bytes!("../assets/icons/pencil-line.svg");
pub const ADD: &[u8] = include_bytes!("../assets/icons/add-line.svg");
pub const PLAY: &[u8] = include_bytes!("../assets/icons/play-fill.svg");
pub const SKIP_FORWARD: &[u8] = include_bytes!("../assets/icons/skip-forward-mini-fill.svg");
pub const SKIP_BACK: &[u8] = include_bytes!("../assets/icons/skip-back-mini-fill.svg");
pub const SHUFFLE: &[u8] = include_bytes!("../assets/icons/shuffle-line.svg");
pub const REPEAT: &[u8] = include_bytes!("../assets/icons/repeat-2-line.svg");
pub const VOLUME: &[u8] = include_bytes!("../assets/icons/volume-up-fill.svg");
pub const ADD_CIRCLE: &[u8] = include_bytes!("../assets/icons/add-circle-line.svg");
pub const STACK: &[u8] = include_bytes!("../assets/icons/stack-line.svg");
pub const MUSIC: &[u8] = include_bytes!("../assets/icons/music-fill.svg");
pub const SETTINGS: &[u8] = include_bytes!("../assets/icons/settings-4-line.svg");
pub const MINIMIZE: &[u8] = include_bytes!("../assets/icons/subtract-line.svg");
pub const MAXIMIZE: &[u8] = include_bytes!("../assets/icons/checkbox-blank-line.svg");
pub const RESTORE: &[u8] = include_bytes!("../assets/icons/file-copy-line.svg");
pub const CLOSE: &[u8] = include_bytes!("../assets/icons/close-line.svg");

const NUNITO: &[u8] = include_bytes!("../assets/fonts/Nunito-Variable.ttf");

const CACHE_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Default)]
struct CoverCache {
    images: std::collections::VecDeque<(i32, vizia::vg::Image, usize)>,
    bytes: usize,
}
thread_local! { static COVERS: std::cell::RefCell<CoverCache> = std::cell::RefCell::default(); }

pub fn decode_cover(bytes: &[u8]) -> Option<vizia::vg::Image> {
    let image = vizia::vg::Image::from_encoded(vizia::vg::Data::new_copy(bytes))?;
    let bytes = image_bytes(&image)?;
    if bytes > CACHE_LIMIT {
        return None;
    }
    let image = image.make_raster_image(None, None)?;
    // Keep visible rows in the bounded cache; the player cover grows to 640 logical pixels.
    let longest = image.width().max(image.height());
    if longest <= 1280 {
        return Some(image);
    }
    let scaled = |size: i32| {
        i32::try_from(
            i64::from(size)
                .checked_mul(1280)?
                .checked_div(i64::from(longest))?,
        )
        .ok()
        .map(|size| size.max(1))
    };
    let info = image
        .image_info()
        .with_dimensions((scaled(image.width())?, scaled(image.height())?));
    image.make_scaled(
        &info,
        vizia::vg::SamplingOptions::new(vizia::vg::FilterMode::Linear, vizia::vg::MipmapMode::None),
    )
}
fn image_bytes(image: &vizia::vg::Image) -> Option<usize> {
    usize::try_from(image.width())
        .ok()?
        .checked_mul(usize::try_from(image.height()).ok()?)?
        .checked_mul(image.image_info().bytes_per_pixel())
}
pub fn cache_cover(id: i32, image: vizia::vg::Image) {
    let Some(bytes) = image_bytes(&image).filter(|bytes| *bytes <= CACHE_LIMIT) else {
        return;
    };
    COVERS.with_borrow_mut(|cache| {
        if cache.images.iter().any(|(key, _, _)| *key == id) {
            return;
        }
        while cache.bytes.saturating_add(bytes) > CACHE_LIMIT {
            if let Some((_, _, size)) = cache.images.pop_front() {
                cache.bytes = cache.bytes.saturating_sub(size);
            }
        }
        cache.bytes = cache.bytes.saturating_add(bytes);
        cache.images.push_back((id, image, bytes));
    });
}
pub fn clear_covers() {
    COVERS.with_borrow_mut(|cache| *cache = CoverCache::default());
}
pub fn has_cover(id: i32) -> bool {
    COVERS.with_borrow(|cache| cache.images.iter().any(|(key, _, _)| *key == id))
}
pub fn cover_image(id: i32) -> Option<vizia::vg::Image> {
    COVERS.with_borrow_mut(|cache| {
        let index = cache.images.iter().position(|(key, _, _)| *key == id)?;
        let entry = cache.images.remove(index)?;
        let image = entry.1.clone();
        cache.images.push_back(entry);
        Some(image)
    })
}

const TINTS: [&str; 4] = ["tint-navy", "tint-olive", "tint-plum", "tint-maroon"];

pub fn tint(index: usize) -> &'static str {
    pick(&TINTS, index)
}

fn pick(options: &[&'static str], index: usize) -> &'static str {
    index
        .checked_rem(options.len())
        .and_then(|position| options.get(position))
        .copied()
        .unwrap_or_default()
}

pub fn register(cx: &mut Context) {
    cx.add_font_mem(NUNITO);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn cache_is_bounded_and_evicts_least_recent_images() {
        clear_covers();
        let image = decode_cover(include_bytes!("../assets/covers/karakara.jpg")).unwrap();
        assert!(image.width().max(image.height()) <= 1280);
        for id in 0..100 {
            cache_cover(id, image.clone());
        }
        COVERS.with_borrow(|cache| assert!(cache.bytes <= CACHE_LIMIT));
        assert!(!has_cover(0));
        assert!(has_cover(99));
        assert!(cover_image(99).is_some());
        cache_cover(100, image);
        assert!(has_cover(99));
        clear_covers();
    }
}
