use vizia::prelude::*;

/// The pane frame shared by the songs and settings tabs. Both panes are built every frame and
/// toggled with `display`, so this returns its handle for the caller to bind that to.
pub(crate) fn sidebar(cx: &mut Context, content: impl FnOnce(&mut Context)) -> Handle<'_, VStack> {
    VStack::new(cx, content).class("sidebar")
}
