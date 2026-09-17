use vizia::prelude::*;

#[derive(Clone, Copy, Debug)]
pub(crate) enum ButtonVariant {
    Light,
    Alternate,
    Accent,
    Link,
}
impl ButtonVariant {
    const fn class(self) -> &'static str {
        match self {
            Self::Light => "button-light",
            Self::Alternate => "button-alternate",
            Self::Accent => "button-accent",
            Self::Link => "button-link",
        }
    }
}

pub(crate) fn button<T: ToString + 'static>(
    cx: &mut Context,
    text: impl Res<T> + Clone + 'static,
    variant: ButtonVariant,
) -> Handle<'_, Button> {
    Button::new(cx, move |cx| Label::new(cx, text))
        .class("ui-button")
        .class(variant.class())
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum MaterialKind {
    Regular,
    Thick,
    Thin,
}
impl MaterialKind {
    pub(crate) const fn class(self) -> &'static str {
        match self {
            Self::Regular => "material-regular",
            Self::Thick => "material-thick",
            Self::Thin => "material-thin",
        }
    }
}

pub(crate) fn material(
    cx: &mut Context,
    kind: MaterialKind,
    content: impl FnOnce(&mut Context),
) -> Handle<'_, VStack> {
    VStack::new(cx, content).class(kind.class())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum FilterTagState {
    #[default]
    Neutral,
    Included,
    Excluded,
}
impl FilterTagState {
    pub(crate) const fn next(self) -> Self {
        match self {
            Self::Neutral => Self::Included,
            Self::Included => Self::Excluded,
            Self::Excluded => Self::Neutral,
        }
    }
    pub(crate) const fn description(self) -> &'static str {
        match self {
            Self::Neutral => "Neutral",
            Self::Included => "Included",
            Self::Excluded => "Excluded",
        }
    }
}

pub(crate) fn tag<'a>(
    cx: &'a mut Context,
    text: &'static str,
    selected: impl Res<bool>,
) -> Handle<'a, Button> {
    button(cx, text, ButtonVariant::Alternate)
        .class("tag")
        .checked(selected)
}

pub(crate) fn filter_tag<'a>(
    cx: &'a mut Context,
    text: &'static str,
    state: Signal<FilterTagState>,
) -> Handle<'a, Button> {
    button(
        cx,
        state.map(move |value| format!("{text} · {}", value.description())),
        ButtonVariant::Alternate,
    )
    .class("tag")
    .checked(state.map(|value| *value == FilterTagState::Included))
    .toggle_class(
        "excluded",
        state.map(|value| *value == FilterTagState::Excluded),
    )
}

pub(crate) fn toggle<'a>(
    cx: &'a mut Context,
    label: &'static str,
    value: impl Res<bool>,
    action: impl Fn(&mut EventContext) + 'static,
) -> Handle<'a, HStack> {
    HStack::new(cx, move |cx| {
        Switch::new(cx, value)
            .class("ui-switch")
            .name(label)
            .on_toggle(action);
        Label::new(cx, label);
    })
    .class("toggle-row")
}

/// A single selected index is the complete tab state; invalid indices are never selected.
pub(crate) const fn selected_tab(selected: usize, candidate: usize, count: usize) -> bool {
    selected < count && selected == candidate
}

pub(crate) fn tabs<'a>(
    cx: &'a mut Context,
    labels: &'static [&'static str],
    selected: Signal<usize>,
    on_select: impl Fn(&mut EventContext, usize) + Send + Sync + 'static,
) -> Handle<'a, HStack> {
    let on_select = std::sync::Arc::new(on_select);
    HStack::new(cx, move |cx| {
        for (index, label) in labels.iter().enumerate() {
            let action = on_select.clone();
            button(cx, *label, ButtonVariant::Alternate)
                .class("tab")
                .checked(selected.map(move |current| selected_tab(*current, index, labels.len())))
                .on_press(move |cx| action(cx, index));
        }
    })
    .class("tabs")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filter_cycles_back_to_neutral() {
        let state = FilterTagState::Neutral;
        assert_eq!(state.next(), FilterTagState::Included);
        assert_eq!(state.next().next(), FilterTagState::Excluded);
        assert_eq!(state.next().next().next(), state);
    }
    #[test]
    fn tabs_have_only_one_selection_after_each_change() {
        for selected in [0, 2, 1, 0] {
            let active: Vec<_> = (0..3)
                .filter(|index| selected_tab(selected, *index, 3))
                .collect();
            assert_eq!(active, vec![selected]);
        }
        assert!(!selected_tab(3, 3, 3));
    }
}
