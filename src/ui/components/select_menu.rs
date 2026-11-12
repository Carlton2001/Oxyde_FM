//! Shared appearance for choice lists, matching `GenericDropdown`'s surface and
//! entries so every list reads alike. Callers own placement and actions.
use crate::ui::theme::OxydeTheme;
use gpui::{prelude::*, *};

const INNER_PADDING: Rems = rems(0.25);
/// The menus' corner, also `GenericDropdown`'s.
const RADIUS: Rems = rems(0.5);

/// Keep the rail against all three inner edges, independently of row padding.
pub(crate) fn scrollbar_overlay() -> Div {
    div()
        .absolute()
        .top(-INNER_PADDING)
        .bottom(-INNER_PADDING)
        .left_0()
        .right(-INNER_PADDING)
}

/// Height a surface adds around its entries: padding and outline on both sides.
pub(crate) fn chrome() -> Rems {
    (INNER_PADDING + rems(0.0625)) * 2.0
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

/// Narrowest button opening a list.
pub(crate) const TRIGGER_MIN_WIDTH: Rems = rems(6.0);
/// Narrowest list; a list is otherwise as wide as the button that opened it.
pub(crate) const LIST_MIN_WIDTH: Rems = rems(10.0);

/// Space between a button and the list it opens, as an owned popup leaves it.
pub(crate) const LIST_GAP: Rems = rems(0.25);

/// Where a list opens under the button at `trigger`, in the button's coordinates.
pub(crate) fn list_origin(trigger: Bounds<Pixels>) -> Point<Pixels> {
    point(trigger.left(), trigger.bottom() + crate::ui::scale::rem_pixels(LIST_GAP))
}

/// Width of a list opened from a button `trigger` wide.
pub(crate) fn list_width(trigger: Pixels) -> Pixels {
    trigger.max(crate::ui::scale::rem_pixels(LIST_MIN_WIDTH))
}

/// The button opening a list: a field's frame (2.25 rem, `surface`, the accent
/// outline while `focused`), the value chosen truncated, then a chevron. The
/// caller sets its width, measures its bounds and owns the list.
pub(crate) fn trigger(id: impl Into<ElementId>, value: impl IntoElement, focused: bool, enabled: bool, cx: &App) -> Stateful<Div> {
    let theme = *crate::ui::theme::ThemeContext::theme(cx);
    crate::ui::components::input::control_frame(focused, cx)
        .id(id)
        .relative()
        .min_w(TRIGGER_MIN_WIDTH)
        .px_3()
        .gap(rems(0.5))
        .justify_between()
        .when(!enabled, |trigger| trigger.opacity(0.5))
        .when(enabled, |trigger| trigger.hover(move |style| style.bg(theme.control_hover)))
        .child(div().flex_1().min_w_0().truncate().child(value))
        .child(svg().path("icons/chevron-down.svg").size(rems(0.875)).flex_none().text_color(theme.text))
}

pub(crate) fn surface(theme: OxydeTheme) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .p(INNER_PADDING)
        .rounded(RADIUS)
        .bg(theme.menu_surface)
        .border(rems(0.0625))
        .border_color(theme.menu_border)
        .text_size(rems(0.8125))
        .text_color(theme.text)
        .shadow(crate::ui::style::rem_shadow(true))
        .occlude()
}

/// An entry, as a menu's: `style::MENU_ITEM_HEIGHT` high, its fill under the
/// pointer or while the keyboard holds it (`highlighted`).
pub(crate) fn row(input: Stateful<Div>, theme: OxydeTheme, highlighted: bool) -> Stateful<Div> {
    input
        .h(crate::ui::style::MENU_ITEM_HEIGHT)
        .flex_none()
        .px(rems(0.5))
        .flex()
        .items_center()
        .gap(rems(0.5))
        .rounded(rems(0.3125))
        .text_size(rems(0.8125))
        .text_color(theme.text)
        .when(highlighted, |row| row.bg(theme.menu_hover))
        .hover(|row| row.bg(theme.menu_hover))
}

/// An entry of a single choice: a menu's icon column holds the check of the
/// value chosen. A disabled entry is dimmed, keeps no hover and must omit its action.
pub(crate) fn choice(input: Stateful<Div>, theme: OxydeTheme, chosen: bool, enabled: bool) -> Stateful<Div> {
    let row = if enabled {
        row(input, theme, false)
    } else {
        row(input, theme, false).hover(|row| row.bg(gpui::transparent_black())).opacity(0.42)
    };
    row.child(div().size(rems(1.0)).flex_none().when(chosen, |slot| {
        slot.child(svg().path("icons/check.svg").size(rems(1.0)).text_color(theme.text))
    }))
}
