//! The footer, below the entries: the name to save under, the application's type
//! filters and choices, and the buttons that answer it. Tab and Shift+Tab move the
//! keyboard from the entries through its controls and back, as GTK's file chooser
//! does; Enter or Space presses the control that has it, and Escape cancels.
use super::FileChooserWindow;
use crate::ui::elements::button::ButtonVariant;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

/// The footer's own keyboard stops; the name field and the choices hold theirs.
pub(super) struct FooterFocus {
    pub(super) types: FocusHandle,
    cancel: FocusHandle,
    accept: FocusHandle,
}

impl FooterFocus {
    pub(super) fn new(cx: &mut App) -> Self {
        Self { types: cx.focus_handle(), cancel: cx.focus_handle(), accept: cx.focus_handle() }
    }
}

impl FileChooserWindow {
    /// The keyboard's stops in order: the entries, then the footer's controls shown.
    fn tab_order(&self, cx: &App) -> Vec<FocusHandle> {
        let mut order = vec![self.focus.clone()];
        order.extend(self.name.input.as_ref().map(|input| input.read(cx).focus_handle.clone()));
        if self.shows_type_filter() {
            order.push(self.footer_focus.types.clone());
        }
        order.extend(self.choices.iter().map(|choice| choice.focus.clone()));
        order.push(self.footer_focus.cancel.clone());
        if self.can_accept(cx) {
            order.push(self.footer_focus.accept.clone());
        }
        order
    }

    /// Tab and Escape while a stop has the keyboard; a field typing a path or a filter,
    /// or a name edited in place, keeps them.
    pub(super) fn footer_key(&mut self, key: &Keystroke, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let m = key.modifiers;
        if m.control || m.alt || m.platform || m.function { return false; }
        let order = self.tab_order(cx);
        let Some(current) = order.iter().position(|focus| focus.is_focused(window)) else { return false };
        match key.key.as_str() {
            "tab" => {
                let count = order.len();
                let next = if m.shift { (current + count - 1) % count } else { (current + 1) % count };
                window.focus(&order[next], cx);
                cx.notify();
            }
            // The entries keep their own Escape, which also cancels.
            "escape" if current > 0 => self.finish(None, window, cx),
            _ => return false,
        }
        true
    }

    pub(super) fn render_footer(&self, theme: OxydeTheme, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.i18n();
        let default_label = if self.saving() { Message::FileChooserSave }
            else if self.directory { Message::FileChooserSelect }
            else { Message::Open };
        let accept = self.accept_label.clone().unwrap_or_else(|| i18n.text(default_label));
        let enabled = self.can_accept(cx);
        let (cancelled, accepted) = (cx.entity().downgrade(), cx.entity().downgrade());
        let cancel = crate::ui::components::dialog::button("file-chooser-cancel", i18n.text(Message::Cancel), ButtonVariant::Secondary)
            .focus(&self.footer_focus.cancel)
            .on_press(move |window, cx| { let _ = cancelled.update(cx, |this, cx| this.finish(None, window, cx)); });
        let open = crate::ui::components::dialog::button("file-chooser-open", accept, ButtonVariant::Primary)
            .focus(&self.footer_focus.accept)
            .enabled(enabled)
            .on_press(move |window, cx| { let _ = accepted.update(cx, |this, cx| this.accept(window, cx)); });
        let name = self.render_name_field(theme, cx);
        let named = name.is_some();
        let asked: Vec<AnyElement> = self.render_type_filter(window, cx).into_iter().chain(self.render_choices(theme, window, cx)).collect();
        // The name stretches; else a gap keeps the buttons at the end.
        let spaced = !named && !asked.is_empty();
        crate::ui::components::tool_window::footer(cx)
            .children(name)
            .children(asked)
            .when(spaced, |footer| footer.child(div().flex_1()))
            .child(cancel)
            .child(open)
    }
}
