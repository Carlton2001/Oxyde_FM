//! Window frame: theme list beside the overview or editor, their actions, and Close.

use super::draft::builtin_label;
use super::{ThemeWindow, ICON};
use crate::ui::components::tool_window::{action_button, footer, header, pointer_drag};
use crate::ui::components::dialog::{self, DialogButton};
use crate::ui::elements::button::ButtonVariant;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::elements::segmented::{segment, segmented};
use crate::ui::theme::{ColorVision, ThemeChoice, ThemeContext, ThemeSeed};
use gpui::prelude::*;
use gpui::*;

type Action = fn(&mut ThemeWindow, &mut Window, &mut Context<ThemeWindow>);

impl ThemeWindow {
    /// A disabled button keeps its place and its focus stop, and does nothing.
    fn button(&self, id: &'static str, message: Message, variant: ButtonVariant, enabled: bool,
              focus: &FocusHandle, action: Action, cx: &mut Context<Self>) -> DialogButton {
        let handle = cx.entity().downgrade();
        dialog::button(id, cx.i18n().text(message), variant).focus(focus).enabled(enabled)
            .on_press(move |window, cx| {
                let _ = handle.update(cx, |this, cx| action(this, window, cx));
            })
    }

    /// Tab order: the name, the actions, the six codes, then the picker.
    fn tab_order(&self, cx: &App) -> Vec<FocusHandle> {
        let Some(draft) = &self.draft else {
            return vec![self.stops.create.clone()];
        };
        let mut order = vec![draft.name_input.read(cx).focus_handle.clone()];
        if draft.id.is_some() {
            order.push(self.stops.delete.clone());
        }
        order.extend([self.stops.cancel.clone(), self.stops.save.clone()]);
        order.extend(draft.hex_inputs.iter().map(|input| input.read(cx).focus_handle.clone()));
        order.extend([self.stops.field.clone(), self.stops.hue.clone()]);
        order
    }

    fn cycle(&mut self, backwards: bool, window: &mut Window, cx: &mut Context<Self>) {
        let order = self.tab_order(cx);
        dialog::cycle(&order, backwards, window, cx);
        cx.notify();
    }

    /// Applies to every theme, so it sits with the window rather than with one theme.
    fn render_color_vision(&self, current: ColorVision, cx: &mut Context<Self>) -> Div {
        let theme = *cx.theme();
        let mut group = segmented("theme-color-vision", None, theme);
        for (vision, message) in [
            (ColorVision::Typical, Message::ColorVisionTypical),
            (ColorVision::RedGreen, Message::ColorVisionRedGreen),
            (ColorVision::BlueYellow, Message::ColorVisionBlueYellow),
        ] {
            let owner = self.owner.clone();
            let handle = cx.entity().downgrade();
            group = group.child(segment(SharedString::from(format!("color-vision-{}", vision.as_str())), current == vision, true, theme)
                .child(cx.i18n().text(message))
                .on_click(move |_, _, cx| {
                    let _ = owner.update(cx, |app, cx| app.set_color_vision(vision, cx));
                    // A draft keeps showing its own colors under the new vision.
                    let _ = handle.update(cx, |this, cx| this.preview(cx));
                }));
        }
        div().flex().items_center().gap(rems(0.5)).min_w_0()
            .child(div().id("theme-color-vision-label").flex().items_center().gap(rems(0.375))
                .text_color(theme.text_muted).whitespace_nowrap()
                .child(svg().path("icons/info.svg").size(rems(0.875)).flex_none().text_color(theme.text_muted))
                .fixed_tooltip({
                    // First line as the title, one line per mode below it.
                    let text = cx.i18n().text(Message::ThemeColorVisionDescription);
                    let (title, modes) = text.split_once('\n').unwrap_or((&text, ""));
                    Tooltip::with_meta(title.to_string(), modes.to_string())
                })
                .child(cx.i18n().text(Message::ThemeColorVision)))
            .child(group)
    }

    /// Actions on the theme shown on the right: create from it, or manage the user theme.
    pub(super) fn render_actions(&self, cx: &mut Context<Self>) -> Div {
        let row = div().flex().flex_none().items_center().gap(rems(0.5));
        let Some(draft) = &self.draft else {
            return row.child(self.button("theme-create", Message::ThemeCreate, ButtonVariant::Primary, true,
                &self.stops.create, |this, window, cx| this.start_draft(false, window, cx), cx));
        };
        let modified = draft.is_modified();
        row.when(draft.id.is_some(), |row| row.child(self.button("theme-delete", Message::ThemeDelete,
                ButtonVariant::Secondary, true, &self.stops.delete, |this, _, cx| this.delete_draft(cx), cx)))
            .child(self.button("theme-cancel", Message::Cancel, ButtonVariant::Secondary, modified,
                &self.stops.cancel, |this, window, cx| this.revert_draft(window, cx), cx))
            .child(self.button("theme-save", Message::ThemeSave, ButtonVariant::Primary, modified,
                &self.stops.save, |this, _, cx| this.save_draft(cx), cx))
    }
}

impl Render for ThemeWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let title = cx.i18n().text(Message::ThemeWindowTitle);
        window.set_window_title(title.as_ref());
        let Some(owner) = self.owner.upgrade() else {
            return div().into_any_element();
        };
        let app = owner.read(cx);
        window.set_rem_size(px(app.ui_rem_size() as f32));
        let choice = app.theme_choice.clone();
        let vision = app.color_vision();
        let custom = app.custom_themes().to_vec();
        // A dropped draft takes its focused field with it, and the picker's stops leave
        // with the editor; keep Escape working.
        let picker_left = self.draft.is_none()
            && [&self.stops.field, &self.stops.hue, &self.stops.delete, &self.stops.cancel, &self.stops.save]
                .iter().any(|stop| stop.is_focused(window));
        if picker_left || window.focused(cx).is_none() {
            window.focus(&self.focus, cx);
        }
        let (name, colors) = match &choice {
            ThemeChoice::Builtin(id) => (builtin_label(*id, cx), ThemeSeed::for_id(*id).colors()),
            ThemeChoice::Custom(id) => custom.iter().find(|saved| &saved.id == id)
                .map(|saved| (saved.name.clone().into(), saved.colors(ThemeSeed::for_id(saved.base()).colors())))
                .unwrap_or_else(|| (SharedString::default(), [0; 6])),
        };
        let detail = if self.draft.is_some() { self.render_editor(window, cx) } else { self.render_overview(name, colors, cx) };
        let list = self.render_list(&custom, &choice, cx);
        let list = crate::ui::components::scroll_area::scroll_area("theme-list-scroll-state",
            div().w(rems(18.5)).flex_none().h_full().border_r(rems(0.0625)).border_color(theme.border).bg(theme.sidebar),
            div().id("theme-list-scroll").size_full().px(rems(0.625)).py(rems(0.5)).child(list), false, window, cx);
        let detail = crate::ui::components::scroll_area::scroll_area("theme-detail-scroll-state",
            div().flex_1().min_w_0().h_full(),
            div().id("theme-detail-scroll").size_full().p(rems(1.25)).child(detail), false, window, cx);
        let close = cx.entity().downgrade();
        let keys = cx.entity().downgrade();
        let frame = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.active_background)
            .text_color(theme.text)
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .text_size(rems(0.8125))
            .line_height(relative(1.25))
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let modifiers = event.keystroke.modifiers;
                if event.keystroke.key == "tab" && !(modifiers.control || modifiers.alt || modifiers.platform) {
                    this.cycle(modifiers.shift, window, cx);
                    cx.stop_propagation();
                }
            }))
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "escape" {
                    let _ = keys.update(cx, |this, cx| {
                        if this.draft.as_ref().is_some_and(|draft| draft.id.is_none()) { this.discard_draft(cx) } else { this.close(window, cx) }
                    });
                    cx.stop_propagation();
                }
            })
            .child(header(ICON, title, window, false, cx, move |window, cx| {
                let _ = close.update(cx, |this, cx| this.close(window, cx));
            }))
            .child(div().flex_1().min_h_0().overflow_hidden().flex().child(list).child(detail))
            .child(footer(cx)
                .child(self.render_color_vision(vision, cx))
                .child(div().flex_1())
                .child(action_button("theme-close", cx.i18n().text(Message::Close), false, cx)
                    .on_click(cx.listener(|this, _, window, cx| this.close(window, cx)))))
            .when(self.drag.is_some(), |frame| frame.child(pointer_drag(cx.entity(),
                |this: &mut ThemeWindow, event: &MouseMoveEvent, cx| this.drag_picker(event.position, cx),
                |this: &mut ThemeWindow, cx| { this.drag = None; cx.notify(); })));
        crate::ui::components::client_decorations::frame(frame, theme.border, window).into_any_element()
    }
}
