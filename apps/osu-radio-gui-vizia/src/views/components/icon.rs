use vizia::prelude::*;

pub(crate) fn icon<'a>(cx: &'a mut Context, glyph: &'static [u8]) -> Handle<'a, Svg> {
    Svg::new(cx, glyph).class("icon")
}

pub(crate) fn icon_button<'a>(cx: &'a mut Context, glyph: &'static [u8]) -> Handle<'a, Button> {
    Button::new(cx, move |cx| icon(cx, glyph))
        .class("ui-button")
        .class("icon-button")
}
