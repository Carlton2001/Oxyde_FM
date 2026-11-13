//! What some applications ask beyond the portal's questions, as Windows dialogs let
//! them (`FileChooserExtras`): text fields and notes in the footer's form, questions
//! shown but disabled, a menu of ways to accept beside the button, and the button held
//! until the user has looked around.
use super::menus::FooterMenu;
use super::FileChooserWindow;
use crate::ui::components::input::{TextInput, TextInputOptions};
use crate::ui::elements::button::{action, button_style, menu_chevron, ButtonVariant};
use crate::ui::elements::focus_ring::{focus_ring, keyboard_focused};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::platform::FileChooserField;

/// A text the application asks for, with its label.
pub(super) struct Field {
    id: String,
    label: SharedString,
    input: Entity<TextInput>,
    focus: FocusHandle,
}

impl Field {
    pub(super) fn new(field: &FileChooserField, window: &mut Window, cx: &mut Context<FileChooserWindow>) -> Self {
        let this = cx.weak_entity();
        let (entered, escaped) = (this.clone(), this);
        let input = cx.new(|cx| TextInput::new(
            cx,
            "",
            field.text.clone(),
            |_, _, _| {},
            // Enter accepts, as in the name to save under.
            move |window, cx| {
                let entered = entered.clone();
                window.defer(cx, move |window, cx| {
                    let _ = entered.update(cx, |this, cx| if this.can_accept(cx) { this.accept(window, cx) });
                });
            },
            move |window, cx| { let _ = escaped.update(cx, |this, cx| this.finish(None, window, cx)); },
            |_, _, _, _, _| {},
            TextInputOptions::default(),
            window,
        ));
        let focus = input.read(cx).focus_handle.clone();
        Self { id: field.id.clone(), label: field.label.clone().into(), input, focus }
    }
}

/// A text answered on one line, as the dialogs' protocol carries it.
pub(super) fn one_line(text: &str) -> String {
    text.replace(['\t', '\r', '\n'], " ")
}

impl FileChooserWindow {
    /// Whether the application shows the question or field `id` without letting it change.
    pub(super) fn disabled(&self, id: &str) -> bool {
        self.extras.disabled.iter().any(|disabled| disabled == id)
    }

    /// The fields' texts and the way to accept, after the choices' answers.
    pub(super) fn addition_answers(&self, cx: &App) -> Vec<(String, String)> {
        let fields = self.fields.iter().map(|field| (field.id.clone(), one_line(field.input.read(cx).content())));
        let accept = self.extras.accept_menu.as_ref().zip(self.accept_item.clone()).map(|(menu, item)| (menu.id.clone(), item));
        fields.chain(accept).collect()
    }

    /// The fields that take the keyboard, in the form's order.
    pub(super) fn field_focus(&self) -> impl Iterator<Item = FocusHandle> + '_ {
        self.fields.iter().filter(|field| !self.disabled(&field.id)).map(|field| field.focus.clone())
    }

    /// The fields' rows of the form, by their keyboard stops.
    pub(super) fn field_rows(&self) -> impl Iterator<Item = Vec<FocusHandle>> + '_ {
        self.fields.iter().map(|field| vec![field.focus.clone()])
    }

    /// The fields, each with its label: rows of the form, a disabled one read only.
    pub(super) fn render_fields(&self, cx: &App) -> Vec<(SharedString, AnyElement)> {
        self.fields.iter().map(|field| {
            let control = div().flex_1().min_w(rems(10.0)).max_w(rems(40.0));
            let control = if self.disabled(&field.id) {
                control.child(crate::ui::components::input::read_only_field(field.input.read(cx).content().to_owned(), cx))
            } else {
                control.child(field.input.clone())
            };
            (field.label.clone(), control.into_any_element())
        }).collect()
    }

    /// The application's notes, below the form's controls.
    pub(super) fn render_notes(&self, theme: OxydeTheme) -> Vec<AnyElement> {
        self.extras.notes.iter().map(|note| div().text_color(theme.text_muted).child(note.clone()).into_any_element()).collect()
    }

    /// The user moved in the folders, selected or typed: a button that waits for it
    /// comes alive.
    pub(super) fn note_interaction(&mut self, cx: &mut Context<Self>) {
        if !self.interacted {
            self.interacted = true;
            if self.extras.needs_interaction {
                cx.notify();
            }
        }
    }

    /// Whether the button still waits for the user to do something.
    pub(super) fn awaits_interaction(&self) -> bool {
        self.extras.needs_interaction && !self.interacted
    }

    /// The ways to accept, and the one the button stands for.
    pub(super) fn accept_entries(&self) -> (Vec<String>, Option<usize>) {
        let Some(menu) = &self.extras.accept_menu else { return Default::default() };
        let current = menu.options.iter().position(|(id, _)| Some(id) == self.accept_item.as_ref());
        (menu.options.iter().map(|(_, label)| label.clone()).collect(), current)
    }

    /// A way to accept picked in the menu: the button stands for it, and accepts.
    pub(super) fn choose_accept(&mut self, position: usize, cx: &mut Context<Self>) {
        let Some((id, _)) = self.extras.accept_menu.as_ref().and_then(|menu| menu.options.get(position)) else { return };
        self.accept_item = Some(id.clone());
        if !self.can_accept(cx) {
            return;
        }
        // From the menu's own window, the chooser's is reached once it is let go.
        let (handle, this) = (self.window_handle, cx.weak_entity());
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| { let _ = this.update(cx, |this, cx| this.accept(window, cx)); });
        });
    }

    /// The button that accepts, with the chevron opening its menu when the application
    /// offers several ways to accept.
    pub(super) fn render_accept(&self, open: AnyElement, enabled: bool, theme: OxydeTheme, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        if self.extras.accept_menu.is_none() {
            return open;
        }
        let focus = &self.footer.accept_menu;
        let chevron = action(button_style(div().id("file-chooser-accept-menu"), theme, ButtonVariant::Primary, enabled))
            .px(rems(0.5))
            .track_focus(focus)
            .child(menu_chevron(false, theme.text_on_accent))
            .when(enabled, |chevron| chevron
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.toggle_menu(FooterMenu::Accept, cx)))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "space" | "enter" | "down") && !event.keystroke.modifiers.modified() {
                        this.toggle_menu(FooterMenu::Accept, cx);
                        cx.stop_propagation();
                    }
                })));
        let chevron = self.with_menu(FooterMenu::Accept, focus_ring(chevron, keyboard_focused(focus, window), false, theme), cx);
        div().flex().gap(rems(0.125)).child(open).child(chevron).into_any_element()
    }
}

#[cfg(test)]
#[path = "tests/additions.rs"]
mod tests;
