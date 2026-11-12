use super::{InputAppearance, TextInput};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;

/// Height of every standalone field, the height of the bars and settings rows.
pub(crate) const FIELD_HEIGHT: Rems = rems(2.25);
/// Height of a field placed in a bar: a list row, leaving 0.25 rem above and below in the
/// 2.25 rem bars.
pub(crate) const BAR_FIELD_HEIGHT: Rems = crate::ui::style::ROW_HEIGHT;
/// Horizontal text margin, also the gap between a leading or trailing icon and the frame.
pub(crate) const FIELD_PADDING: Rems = rems(0.75);
/// Width of a button cell inside a field (reveal, clear, submit, toggles, calendar).
pub(crate) const FIELD_BUTTON_WIDTH: Rems = rems(2.0);
/// Size of every icon drawn inside a field.
pub(crate) const FIELD_ICON_SIZE: Rems = rems(1.0);
const FIELD_BORDER: Rems = rems(0.0625);
/// Corner of every field frame.
pub(crate) const FIELD_RADIUS: Rems = rems(0.25);
/// Frame radius less its border, so a cell at an edge follows the frame corner.
const FIELD_INNER_RADIUS: Rems = rems(0.1875);

/// Side of the frame a button cell sits against, which decides its rounded corners.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FieldEdge {
    Leading,
    Trailing,
}

fn framed(input: Div, focused: bool, invalid: bool, theme: &OxydeTheme) -> Div {
    input
        .rounded(FIELD_RADIUS)
        .border(FIELD_BORDER)
        .border_color(if invalid {
            theme.error
        } else if focused {
            theme.accent
        } else {
            theme.border
        })
        .bg(theme.surface)
}

/// Single source for standalone and composed standard field presentation.
fn standard_surface(input: Div, focused: bool, theme: &OxydeTheme) -> Div {
    surface(input, focused, false, theme)
}

fn surface(input: Div, focused: bool, invalid: bool, theme: &OxydeTheme) -> Div {
    framed(
        input
            .flex()
            .items_center()
            .min_w_0()
            .flex_shrink(0.0)
            .h(FIELD_HEIGHT)
            .text_size(rems(0.8125))
            .line_height(relative(1.25))
            .text_color(theme.text),
        focused,
        invalid,
        theme,
    )
}

/// Decorative icon at the start or end of a field frame, `FIELD_PADDING` from the border; the
/// editor's own padding gives the same gap on the text side.
pub(crate) fn field_icon(path: &'static str, edge: FieldEdge, theme: &OxydeTheme) -> Svg {
    let icon = svg().path(path).size(FIELD_ICON_SIZE).flex_none().text_color(theme.text_muted);
    match edge {
        FieldEdge::Leading => icon.ml(FIELD_PADDING),
        FieldEdge::Trailing => icon.mr(FIELD_PADDING),
    }
}

/// The one button style inside a field: a full-height cell against an edge of the frame, its
/// corners following the frame, highlighted on hover or filled with the accent while active.
pub(crate) fn field_button(
    id: impl Into<ElementId>,
    edge: FieldEdge,
    active: bool,
    theme: &OxydeTheme,
) -> Stateful<Div> {
    let theme = *theme;
    let cell = div()
        .id(id)
        .flex_none()
        .w(FIELD_BUTTON_WIDTH)
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .occlude();
    let cell = match edge {
        FieldEdge::Leading => cell.rounded_l(FIELD_INNER_RADIUS),
        FieldEdge::Trailing => cell.rounded_r(FIELD_INNER_RADIUS),
    };
    if active {
        cell.bg(theme.accent)
    } else {
        cell.hover(move |style| style.bg(theme.control_hover))
            .focus(move |style| style.bg(theme.control_hover))
    }
}

/// A field button for a lasting option of the field (regex switch, show password): the same
/// cell set apart on the `header` background behind a border on its inner side, so it reads
/// as part of the frame rather than as a passing action.
pub(crate) fn field_option(
    id: impl Into<ElementId>,
    edge: FieldEdge,
    active: bool,
    theme: &OxydeTheme,
) -> Stateful<Div> {
    let cell = field_button(id, edge, active, theme).border_color(theme.border);
    let cell = match edge {
        FieldEdge::Leading => cell.border_r(FIELD_BORDER),
        FieldEdge::Trailing => cell.border_l(FIELD_BORDER),
    };
    if active { cell } else { cell.bg(theme.header) }
}

/// Icon of a `field_button`, contrasted on the accent while the button is active.
pub(crate) fn field_button_icon(path: &'static str, active: bool, theme: &OxydeTheme) -> Svg {
    svg()
        .path(path)
        .size(FIELD_ICON_SIZE)
        .flex_none()
        .text_color(if active { theme.text_on_accent } else { theme.text })
}

/// Frame for an Embedded editor and its prefix/suffix controls.
pub(crate) fn field_frame(input: &Entity<TextInput>, window: &Window, cx: &App) -> Div {
    control_frame(input.read(cx).focus_handle.is_focused(window), cx)
}

/// An Embedded password editor in the standard frame, with its show/hide eye as a trailing
/// field button. The caller owns the
/// revealed state and its action.
pub(crate) fn password_field(
    input: &Entity<TextInput>,
    id: impl Into<ElementId>,
    revealed: bool,
    on_toggle: impl Fn(&mut Window, &mut App) + 'static,
    window: &Window,
    cx: &App,
) -> Div {
    let theme = *cx.theme();
    field_frame(input, window, cx)
        .w_full()
        .child(div().flex_1().min_w_0().h_full().child(input.clone()))
        .child(
            field_option(id, FieldEdge::Trailing, false, &theme)
                .child(field_button_icon(
                    if revealed { "icons/eye-off.svg" } else { "icons/eye.svg" },
                    false,
                    &theme,
                ))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    on_toggle(window, cx);
                }),
        )
}

/// Matching frame for non-editable form controls such as selection pickers.
pub(crate) fn control_frame(focused: bool, cx: &App) -> Div {
    standard_surface(div(), focused, cx.theme())
}

/// Non-editable snapshot used while a form is busy; never installs input handlers.
pub(crate) fn read_only_field(value: impl Into<SharedString>, cx: &App) -> Div {
    standard_surface(div(), false, cx.theme())
        .px(FIELD_PADDING)
        .opacity(0.5)
        .overflow_hidden()
        .child(value.into())
}

pub(super) fn input_surface(
    input: Div,
    appearance: InputAppearance,
    focused: bool,
    invalid: bool,
    theme: &OxydeTheme,
) -> Div {
    match appearance {
        InputAppearance::Standard => surface(input, focused, invalid, theme).px(FIELD_PADDING),
        InputAppearance::Inline => framed(input.h_full().px(FIELD_PADDING), focused, invalid, theme),
        InputAppearance::Embedded => input.h_full().px(FIELD_PADDING),
        InputAppearance::Bare => input.h_full(),
    }
}

#[cfg(test)]
#[path = "tests/field.rs"]
mod tests;
