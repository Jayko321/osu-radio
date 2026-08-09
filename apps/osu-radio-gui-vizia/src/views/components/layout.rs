use vizia::prelude::*;

/// Vertical spacing between siblings. A child's CSS `top` is ignored inside a stack, so gaps that
/// differ per sibling have to be their own element.
pub(crate) fn gap(cx: &mut Context, height: f32) {
    Element::new(cx).width(Stretch(1.0)).height(Pixels(height));
}

pub(crate) fn hspacer(cx: &mut Context) {
    Element::new(cx).class("hspacer");
}

pub(crate) fn vspacer(cx: &mut Context) {
    Element::new(cx).class("vspacer");
}
