use crate::ui::theme::OxydeTheme;
use gpui::{prelude::*, *};

/// Shared visual only; callers retain focus, hit targets and actions.
pub(crate) fn checkbox(theme: OxydeTheme, checked: bool, partial: bool) -> Div {
    let marked = checked || partial;
    div()
        .size(rems(1.0))
        .flex_none()
        .rounded(rems(0.25))
        .border(rems(0.0625))
        .border_color(if marked { theme.accent } else { theme.checkbox_border })
        .bg(if partial {
            theme.surface.blend(theme.accent.opacity(0.20))
        } else if checked {
            theme.accent
        } else {
            theme.surface
        })
        .flex()
        .items_center()
        .justify_center()
        .when(checked, |d| d.child(svg().path("icons/check.svg")
            .size(rems(0.75)).text_color(theme.text_on_accent)))
        .when(partial, |d| d.child(div().w(rems(0.5)).h(rems(0.125)).bg(theme.text)))
}
