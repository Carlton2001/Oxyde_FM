//! The pieces every Properties page is built from: a titled section holding a card,
//! labelled rows inside it, and the header naming the item.
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;

/// A small uppercase title with its icon, above the card it introduces.
pub(super) fn section(label: SharedString, icon: &'static str, content: impl IntoElement, theme: OxydeTheme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(rems(0.5))
        .child(
            div()
                .flex()
                .items_center()
                .gap(rems(0.5))
                .px(rems(0.25))
                .text_size(rems(0.6875))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .child(svg().path(icon).size(rems(0.8125)).flex_none().text_color(theme.text))
                .child(label.to_uppercase()),
        )
        .child(content)
}

pub(super) fn card(theme: OxydeTheme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(rems(0.5))
        .px(rems(1.0))
        .py(rems(0.75))
        .rounded_lg()
        .bg(theme.header)
        .border(rems(0.0625))
        .border_color(theme.border)
}

/// A separator inside a card.
pub(super) fn rule(theme: OxydeTheme) -> Div {
    crate::ui::elements::separator::horizontal(theme.border)
}

/// A label and its value; `compact` stacks them in a narrow window.
pub(super) fn row(label: SharedString, value: impl IntoElement, label_width: Pixels, compact: bool, theme: OxydeTheme) -> Div {
    div()
        .flex()
        .gap(rems(0.75))
        .items_center()
        .min_h(rems(1.5))
        .min_w_0()
        .when(compact, |row| row.flex_col().items_stretch().gap(rems(0.25)))
        .child(
            div()
                .w(label_width)
                .flex_none()
                .text_color(theme.text_muted)
                .when(compact, |label| label.w_full())
                .when(!compact, |label| label.whitespace_nowrap())
                .child(label),
        )
        .child(div().min_w_0().when(!compact, |value| value.flex_1()).when(compact, |value| value.w_full()).child(value))
}

/// The item's graphic beside its name and a line describing it.
pub(super) fn header(graphic: impl IntoElement, title: impl IntoElement, subtitle: impl IntoElement, theme: OxydeTheme) -> Div {
    div()
        .flex()
        .items_center()
        .gap(rems(1.0))
        .px(rems(0.25))
        .child(div().flex_none().child(graphic))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(rems(0.25))
                .child(div().text_size(rems(1.0)).font_weight(FontWeight::MEDIUM).line_height(relative(1.3)).child(title))
                .child(div().text_color(theme.text_muted).child(subtitle)),
        )
}

/// A whole row toggling a switch.
pub(super) fn toggle_row(id: impl Into<ElementId>, label: SharedString, value: bool, enabled: bool, theme: OxydeTheme) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(rems(0.75))
        .min_h(rems(1.75))
        .min_w_0()
        .when(!enabled, |row| row.opacity(0.5))
        .child(div().flex_1().min_w_0().child(label))
        .child(crate::ui::elements::toggle::toggle(value, theme))
}
