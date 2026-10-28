use crate::ui::theme::OxydeTheme;
use gpui::{prelude::*, *};

/// Shared visual only; callers retain focus, hit targets and actions.
pub(crate) fn radio(theme: OxydeTheme, selected: bool) -> Div {
    div()
        .size(rems(1.0))
        .flex_none()
        .rounded_full()
        .border(rems(0.0625))
        .border_color(if selected { theme.accent } else { theme.checkbox_border })
        .bg(if selected { theme.accent } else { theme.surface })
        .flex()
        .items_center()
        .justify_center()
        .when(selected, |d| d.child(div().size(rems(0.375)).rounded_full().bg(theme.text_on_accent)))
}
