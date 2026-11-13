//! The application's questions beside the files, such as an encoding, as the portal
//! asks them: a checkbox, or a label and a menu of answers. They sit in the footer's
//! form, menus first, and their answers go back with the choice, as GTK's file
//! chooser offers them.
use super::menus::FooterMenu;
use super::FileChooserWindow;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::platform::FileChooserChoice;

pub(super) struct Choice {
    choice: FileChooserChoice,
    /// The answer's id; `"true"` or `"false"` for a checkbox.
    answer: String,
    /// Its keyboard stop in the footer.
    pub(super) focus: FocusHandle,
}

impl Choice {
    pub(super) fn new(choice: FileChooserChoice, focus: FocusHandle) -> Self {
        Self { answer: choice.initial.clone(), choice, focus }
    }

    fn checkbox(&self) -> bool {
        self.choice.options.is_empty()
    }

    /// The answers' labels, and the one given.
    pub(super) fn entries(&self) -> (Vec<String>, Option<usize>) {
        let options = &self.choice.options;
        (options.iter().map(|(_, label)| label.clone()).collect(), options.iter().position(|(id, _)| *id == self.answer))
    }
}

impl FileChooserWindow {
    pub(super) fn answer_choice(&mut self, choice: usize, position: usize) {
        if let Some(choice) = self.choices.get_mut(choice) {
            if let Some((id, _)) = choice.choice.options.get(position) {
                choice.answer = id.clone();
            }
        }
    }

    fn toggle_choice(&mut self, choice: usize, cx: &mut Context<Self>) {
        if let Some(choice) = self.choices.get_mut(choice).filter(|choice| choice.checkbox()) {
            choice.answer = if choice.answer == "true" { "false" } else { "true" }.into();
            cx.notify();
        }
    }

    /// Each choice's id and answer, as the portal sends them back.
    pub(super) fn choice_answers(&self) -> Vec<(String, String)> {
        self.choices.iter().map(|choice| (choice.choice.id.clone(), choice.answer.clone())).collect()
    }

    /// The choices offering a list of answers, each with its label: rows of the form.
    pub(super) fn render_choice_menus(&self, window: &Window, cx: &mut Context<Self>) -> Vec<(SharedString, AnyElement)> {
        self.choices.iter().enumerate().filter(|(_, choice)| !choice.checkbox()).map(|(position, choice)| {
            let (labels, answered) = choice.entries();
            let shown = answered.and_then(|answered| labels.get(answered).cloned()).unwrap_or_default();
            (choice.choice.label.clone().into(), self.render_menu(FooterMenu::Choice(position), shown, &choice.focus, window, cx))
        }).collect()
    }

    /// The checkboxes, which carry their own label below the form's menus.
    pub(super) fn render_choice_checkboxes(&self, theme: OxydeTheme, window: &Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        use crate::ui::elements::focus_ring::{focus_ring, keyboard_focused};
        self.choices.iter().enumerate().filter(|(_, choice)| choice.checkbox()).map(|(position, choice)| {
            div()
                .id(("file-chooser-choice", position))
                .track_focus(&choice.focus)
                .flex_none()
                .flex()
                .items_center()
                .gap(rems(0.5))
                .child(focus_ring(crate::ui::elements::checkbox::checkbox(theme, choice.answer == "true", false),
                    keyboard_focused(&choice.focus, window), false, theme))
                .child(choice.choice.label.clone())
                // Keys are handled below, on press, as the dialogs' buttons take them.
                .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| if !event.is_keyboard() { this.toggle_choice(position, cx) }))
                // Space or Enter while it has the keyboard, as the dialogs' checkboxes.
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                    if matches!(event.keystroke.key.as_str(), "space" | "enter") && !event.keystroke.modifiers.modified() {
                        this.toggle_choice(position, cx);
                        cx.stop_propagation();
                    }
                }))
                .into_any_element()
        }).collect()
    }

    /// The keyboard's stops of the choices' menus, then of their checkboxes: the form's
    /// reading order.
    pub(super) fn choice_menu_focus(&self) -> impl Iterator<Item = FocusHandle> + '_ {
        self.choices.iter().filter(|choice| !choice.checkbox()).map(|choice| choice.focus.clone())
    }

    pub(super) fn choice_checkbox_focus(&self) -> impl Iterator<Item = FocusHandle> + '_ {
        self.choices.iter().filter(|choice| choice.checkbox()).map(|choice| choice.focus.clone())
    }
}
