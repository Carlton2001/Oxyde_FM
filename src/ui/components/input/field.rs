use super::{InputAppearance, TextInput};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;

/// Single source for standalone and composed standard field presentation.
fn standard_surface(input: Div, focused: bool, theme: &OxydeTheme) -> Div {
    input
        .flex()
        .items_center()
        .min_w_0()
        .flex_shrink(0.0)
        .h(rems(2.25))
        .text_size(rems(0.8125))
        .line_height(relative(1.25))
        .text_color(theme.text)
        .rounded_md()
        .border(rems(0.0625))
        .border_color(if focused { theme.accent } else { theme.border })
        .bg(theme.surface)
}

/// Frame for an Embedded editor and its prefix/suffix controls.
pub(crate) fn field_frame(input: &Entity<TextInput>, window: &Window, cx: &App) -> Div {
    standard_surface(
        div(),
        input.read(cx).focus_handle.is_focused(window),
        cx.theme(),
    )
}

/// Non-editable snapshot used while a form is busy; never installs input handlers.
pub(crate) fn read_only_field(value: impl Into<SharedString>, cx: &App) -> Div {
    standard_surface(div(), false, cx.theme())
        .px_3()
        .opacity(0.5)
        .overflow_hidden()
        .child(value.into())
}

pub(super) fn input_surface(
    input: Div,
    appearance: InputAppearance,
    focused: bool,
    theme: &OxydeTheme,
) -> Div {
    match appearance {
        InputAppearance::Standard => standard_surface(input, focused, theme).px_3(),
        InputAppearance::Inline => input
            .h_full()
            .px_3()
            .rounded_sm()
            .border(rems(0.0625))
            .border_color(if focused { theme.accent } else { theme.border })
            .bg(theme.surface),
        InputAppearance::Embedded => input.h_full().px_3(),
        InputAppearance::Bare => input.h_full(),
    }
}

#[cfg(test)]
#[path = "tests/field.rs"]
mod tests;
