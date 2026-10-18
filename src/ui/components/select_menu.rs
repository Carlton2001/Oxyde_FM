//! Shared appearance for compact choice menus. Callers own placement and actions.
use crate::ui::theme::OxydeTheme;
use gpui::{prelude::*, *};

pub(crate) fn surface(theme: OxydeTheme) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .p(rems(0.3125))
        .rounded_md()
        .bg(theme.menu_surface)
        .border(rems(0.0625))
        .border_color(theme.menu_border)
        .text_size(rems(0.8125))
        .text_color(theme.text)
        .shadow(crate::ui::style::rem_shadow(false))
        .occlude()
}

pub(crate) fn row(input: Stateful<Div>, theme: OxydeTheme, selected: bool) -> Stateful<Div> {
    input
        .h(rems(1.75))
        .flex_none()
        .px(rems(0.75))
        .flex()
        .items_center()
        .gap(rems(0.5))
        .rounded_sm()
        .text_size(rems(0.8125))
        .text_color(theme.text)
        .when(selected, |row| row.bg(theme.selected))
        .hover(|row| row.bg(theme.menu_hover))
}
