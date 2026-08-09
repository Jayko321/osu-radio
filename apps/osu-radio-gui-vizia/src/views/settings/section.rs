use vizia::prelude::*;

use super::field::field;
use crate::sample::Section;
use crate::views::components::icon;

pub(crate) fn section(cx: &mut Context, section: &'static Section) {
    VStack::new(cx, move |cx| {
        section_header(cx, section);

        for entry in section.fields {
            field(cx, entry);
        }
    })
    .class("settings-section");
}

fn section_header(cx: &mut Context, section: &'static Section) {
    HStack::new(cx, move |cx| {
        icon(cx, section.glyph);
        Label::new(cx, section.title);
    })
    .class("section-header");
}
