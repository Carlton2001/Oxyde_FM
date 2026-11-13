//! The six seed colors: explained for the selected theme, editable for a draft.

use super::{PickerPart, ThemeWindow};
use crate::ui::elements::color_picker::{color_field, hue_strip, Hsv};
use crate::ui::elements::segmented::outline;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::{contrast, OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::custom_theme::format_hex_color;

/// Label and role of each seed, in `ThemeSeed::colors` order.
const SEEDS: [(Message, Message); 6] = [
    (Message::ThemeColorBackground, Message::ThemeColorBackgroundDescription),
    (Message::ThemeColorActiveBackground, Message::ThemeColorActiveBackgroundDescription),
    (Message::ThemeColorSidebar, Message::ThemeColorSidebarDescription),
    (Message::ThemeColorSurface, Message::ThemeColorSurfaceDescription),
    (Message::ThemeColorText, Message::ThemeColorTextDescription),
    (Message::ThemeColorAccent, Message::ThemeColorAccentDescription),
];

/// Below this ratio against a pane, text stops meeting WCAG AA.
const TEXT_CONTRAST: f32 = 4.5;

/// Outlined as the segmented groups, so a swatch of the window's own background still shows.
fn swatch(color: u32, theme: OxydeTheme) -> Div {
    div().size(rems(1.75)).flex_none().rounded_md().bg(rgb(color))
        .border(rems(0.0625)).border_color(outline(theme))
}

fn described(label: SharedString, description: SharedString, theme: OxydeTheme) -> Div {
    div().flex_1().min_w_0().flex().flex_col()
        .child(div().font_weight(FontWeight::MEDIUM).child(label))
        .child(div().text_size(rems(0.75)).text_color(theme.text_muted).child(description))
}

/// Worst contrast of the text seed against the four pane seeds.
fn text_contrast(colors: [u32; 6]) -> f32 {
    let text: Hsla = rgb(colors[4]).into();
    colors[..4].iter().map(|pane| contrast(text, rgb(*pane).into())).fold(f32::INFINITY, f32::min)
}

impl ThemeWindow {
    pub(super) fn render_overview(&self, title: SharedString, colors: [u32; 6], cx: &mut Context<Self>) -> Div {
        let actions = self.render_actions(cx);
        let theme = *cx.theme();
        let i18n = cx.i18n();
        div()
            .flex()
            .flex_col()
            .gap(rems(0.75))
            .child(div().flex().items_center().gap(rems(0.75))
                .child(div().flex_1().min_w_0().text_size(rems(1.0)).font_weight(FontWeight::SEMIBOLD).child(title))
                .child(actions))
            .child(div().text_color(theme.text_muted).child(i18n.text(Message::ThemeIntro)))
            .child(div().flex().flex_col().gap(rems(0.5)).children(SEEDS.iter().enumerate().map(|(index, (label, description))| {
                div().flex().items_center().gap(rems(0.75))
                    .child(swatch(colors[index], theme))
                    .child(described(i18n.text(*label), i18n.text(*description), theme))
                    .child(div().flex_none().text_color(theme.text_muted).font_family("JetBrains Mono")
                        .child(format_hex_color(colors[index])))
            })))
            .child(div().text_color(theme.text_muted).child(i18n.text(Message::ThemeCreateHint)))
    }

    /// One line per seed beside the picker; only the edited seed shows its role, above the picker.
    pub(super) fn render_editor(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let Some(draft) = &self.draft else {
            return div();
        };
        let actions = self.render_actions(cx);
        let theme = *cx.theme();
        let i18n = cx.i18n();
        let handle = cx.entity().downgrade();
        let rows = SEEDS.iter().enumerate().map(|(index, (label, _))| {
            let select = handle.clone();
            div()
                .id(("theme-seed", index))
                .flex()
                .items_center()
                .gap(rems(0.75))
                .px(rems(0.5))
                .py(rems(0.375))
                .rounded_md()
                .when(draft.active == index, |row| row.bg(theme.selected))
                .when(draft.active != index, |row| row.hover(|row| row.bg(theme.menu_hover)))
                .child(swatch(draft.colors[index], theme))
                .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::MEDIUM).child(i18n.text(*label)))
                .child(div().w(rems(6.5)).flex_none().font_family("JetBrains Mono")
                    .child(draft.hex_inputs[index].clone()))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let _ = select.update(cx, |this, cx| this.select_color(index, cx));
                })
        });
        let (field_press, field_key) = (handle.clone(), handle.clone());
        let (hue_press, hue_key) = (handle.clone(), handle.clone());
        let ratio = text_contrast(draft.colors);
        let (label, description) = SEEDS[draft.active];
        let picker = div()
            .w(rems(14.0))
            .flex_none()
            .flex()
            .flex_col()
            .gap(rems(0.625))
            .child(described(i18n.text(label), i18n.text(description), theme))
            .child(color_field("theme-picker-field", draft.hsv, self.field_bounds.clone(), &self.stops.field, window, theme,
                move |position, _, cx| {
                    let _ = field_press.update(cx, |this, cx| this.press_picker(PickerPart::Field, position, cx));
                },
                move |hsv, _, cx| {
                    let _ = field_key.update(cx, |this, cx| this.set_hsv(hsv, cx));
                }).h(rems(11.0)).w_full())
            .child(hue_strip("theme-picker-hue", draft.hsv.h, self.hue_bounds.clone(), &self.stops.hue, window, theme,
                move |position, _, cx| {
                    let _ = hue_press.update(cx, |this, cx| this.press_picker(PickerPart::Hue, position, cx));
                },
                move |h, _, cx| {
                    let _ = hue_key.update(cx, |this, cx| {
                        let hsv = this.draft.as_ref().map(|draft| draft.hsv);
                        if let Some(hsv) = hsv {
                            this.set_hsv(Hsv { h, ..hsv }, cx);
                        }
                    });
                }).w_full())
            .child(div().text_size(rems(0.75)).text_color(theme.text_muted).child(i18n.text(Message::ThemePickerHint)))
            .when(ratio < TEXT_CONTRAST, |picker| {
                let mut args = FluentArgs::new();
                args.set("ratio", format!("{ratio:.1}"));
                picker.child(div().text_size(rems(0.75)).text_color(theme.warning_text)
                    .child(i18n.text_with_args(Message::ThemeContrastLow, &args)))
            });
        div()
            .flex()
            .flex_col()
            .gap(rems(0.75))
            .child(div().flex().items_center().gap(rems(0.75))
                .child(div().flex_none().w(rems(3.5)).child(i18n.text(Message::ThemeName)))
                .child(div().flex_1().min_w_0().child(draft.name_input.clone()))
                .child(actions))
            .child(div().flex().items_start().gap(rems(1.0))
                .child(div().flex_1().min_w_0().flex().flex_col().gap(rems(0.125)).children(rows))
                .child(picker))
    }
}
