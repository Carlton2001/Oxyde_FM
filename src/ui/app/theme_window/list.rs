//! Built-in and user themes; choosing one applies it at once.

use super::draft::builtin_label;
use super::ThemeWindow;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::{is_light_background, CustomTheme, OxydeTheme, ThemeChoice, ThemeContext, ThemeId, ThemeSeed};
use gpui::prelude::*;
use gpui::*;

fn heading(label: SharedString, theme: OxydeTheme) -> Div {
    div()
        .px(rems(0.75))
        .pt(rems(1.0))
        .pb(rems(0.25))
        .text_size(rems(0.6875))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.text_muted)
        .child(label.to_uppercase())
}

/// The six seeds in a row, so themes compare at a glance.
pub(super) fn swatches(colors: [u32; 6], theme: OxydeTheme) -> Div {
    // Corners on the end squares: a clipping parent let them paint past the scrolled list.
    let radius = rems(0.125);
    div().flex().flex_none().rounded(rems(0.1875))
        .border(rems(0.0625)).border_color(theme.border)
        .children(colors.into_iter().enumerate().map(move |(index, color)| div().size(rems(0.625)).bg(rgb(color))
            .when(index == 0, |square| square.rounded_l(radius))
            .when(index == 5, |square| square.rounded_r(radius))))
}

fn row(
    id: impl Into<ElementId>,
    label: SharedString,
    colors: [u32; 6],
    selected: bool,
    theme: OxydeTheme,
) -> Stateful<Div> {
    let light = is_light_background(rgb(colors[0]).into());
    div()
        .id(id)
        .h(rems(2.25))
        .flex_none()
        .px(rems(0.75))
        .flex()
        .items_center()
        .gap(rems(0.625))
        .rounded_md()
        .when(selected, |row| row.bg(theme.selected))
        .hover(|row| row.bg(theme.menu_hover))
        .child(svg().path(if light { "icons/sun.svg" } else { "icons/moon.svg" })
            .size(rems(0.875)).flex_none().text_color(theme.text))
        .child(div().flex_1().min_w_0().truncate().child(label))
        .child(swatches(colors, theme))
}

impl ThemeWindow {
    pub(super) fn render_list(&self, custom: &[CustomTheme], choice: &ThemeChoice, cx: &mut Context<Self>) -> Stateful<Div> {
        let theme = *cx.theme();
        let handle = cx.entity().downgrade();
        let choose = move |choice: ThemeChoice| {
            let handle = handle.clone();
            move |_: &MouseDownEvent, window: &mut Window, cx: &mut App| {
                let choice = choice.clone();
                let _ = handle.update(cx, |this, cx| this.choose(choice, window, cx));
            }
        };
        let builtin = ThemeId::ALL.into_iter().map(|id| {
            let selected = *choice == ThemeChoice::Builtin(id);
            row(SharedString::from(format!("theme-{}", id.as_str())), builtin_label(id, cx),
                ThemeSeed::for_id(id).colors(), selected, theme)
                .on_mouse_down(MouseButton::Left, choose(ThemeChoice::Builtin(id)))
        }).collect::<Vec<_>>();
        let users = custom.iter().map(|saved| {
            let current = ThemeChoice::Custom(saved.id.clone());
            let colors = saved.colors(ThemeSeed::for_id(saved.base()).colors());
            row(SharedString::from(format!("custom-{}", saved.id)), saved.name.clone().into(),
                colors, *choice == current, theme)
                .on_mouse_down(MouseButton::Left, choose(current))
        }).collect::<Vec<_>>();
        div()
            .id("theme-list")
            .flex()
            .flex_col()
            .gap(rems(0.1875))
            .child(heading(cx.i18n().text(Message::ThemeBuiltin), theme))
            .children(builtin)
            .child(heading(cx.i18n().text(Message::ThemeCustom), theme))
            .when(users.is_empty(), |list| list.child(div().px(rems(0.75)).py(rems(0.25))
                .text_color(theme.text_muted).child(cx.i18n().text(Message::ThemeCustomEmpty))))
            .children(users)
    }
}
