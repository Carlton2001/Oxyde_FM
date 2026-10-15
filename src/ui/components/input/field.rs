use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

/// Shared frame for the compact TextInput used in advanced-search forms.
pub(crate) fn field_frame(cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .flex()
        .items_center()
        .gap(rems(0.0))
        .h(rems(2.25))
        .rounded_md()
        .overflow_hidden()
        .border(rems(0.0625))
        .border_color(theme.border)
        .bg(theme.surface)
}
