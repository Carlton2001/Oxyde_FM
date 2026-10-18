//! Shared appearance for compact choice menus. Callers own placement and actions.
use crate::ui::theme::OxydeTheme;
use gpui::{prelude::*, *};

const INNER_PADDING: Rems = rems(0.3125);

/// Keep the rail against all three inner edges, independently of row padding.
pub(crate) fn scrollbar_overlay() -> Div {
    div()
        .absolute()
        .top(-INNER_PADDING)
        .bottom(-INNER_PADDING)
        .left_0()
        .right(-INNER_PADDING)
}

pub(crate) fn scrollbar_track_size(viewport: f32) -> f32 {
    viewport + crate::ui::scale::rem_pixels(INNER_PADDING).as_f32() * 2.0
}

pub(crate) fn scrollbar_track_shape() -> super::scrollbar::TrackShape {
    super::scrollbar::TrackShape::RoundedRight(INNER_PADDING)
}

pub(crate) fn scrollbar_content_inset() -> Rems {
    rems(super::scrollbar::ZED_TRACK_WIDTH.0 - INNER_PADDING.0)
}

pub(crate) fn surface(theme: OxydeTheme) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .p(INNER_PADDING)
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
