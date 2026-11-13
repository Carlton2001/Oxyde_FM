//! The footer buttons and keyboard of modal prompts: one look, a focus ring only when
//! the keyboard brought the focus, a press on release as GTK does, or on Enter and Space
//! while the button has the keyboard. The calling dialog keeps its state, its order of
//! focus stops and what Escape and Enter mean elsewhere.
use crate::ui::elements::button::{action, button_style, ButtonVariant};
use crate::ui::elements::focus_ring::{focus_ring, keyboard_focused};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use std::rc::Rc;

type Press = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
pub(crate) struct DialogButton {
    id: ElementId,
    label: SharedString,
    variant: ButtonVariant,
    focus: Option<FocusHandle>,
    enabled: bool,
    on_press: Option<Press>,
}

/// A footer action: `Secondary` for Cancel and neutral choices, `Primary` for the
/// action asked for, `Danger` when it destroys.
pub(crate) fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, variant: ButtonVariant) -> DialogButton {
    DialogButton { id: id.into(), label: label.into(), variant, focus: None, enabled: true, on_press: None }
}

impl DialogButton {
    /// A focus stop of the dialog's Tab order.
    pub(crate) fn focus(mut self, focus: &FocusHandle) -> Self {
        self.focus = Some(focus.clone());
        self
    }

    /// A disabled button keeps its place and its focus stop, and does nothing.
    pub(crate) fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub(crate) fn on_press(mut self, press: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_press = Some(Rc::new(press));
        self
    }
}

impl RenderOnce for DialogButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let ring = self.focus.as_ref().is_some_and(|focus| keyboard_focused(focus, window));
        let press = self.on_press.filter(|_| self.enabled);
        let key_press = press.clone();
        // The ring says where the keyboard is: the border keeps its resting color.
        let border = match self.variant {
            ButtonVariant::Secondary => theme.border,
            ButtonVariant::Primary | ButtonVariant::Danger => transparent_black(),
        };
        let button = action(button_style(div().id(self.id), theme, self.variant, self.enabled))
            .focus_visible(move |style| style.border_color(border))
            .when_some(self.focus, |button, focus| {
                button.track_focus(&focus).on_key_down(move |event, window, cx| {
                    let key = event.keystroke.key.as_str();
                    if matches!(key, "enter" | "space") && !event.keystroke.modifiers.modified() {
                        if let Some(press) = &key_press {
                            press(window, cx);
                        }
                        cx.stop_propagation();
                    }
                })
            })
            // Keys are handled above, on press, as the dialogs' other controls.
            .when_some(press, |button, press| {
                button.on_click(move |event, window, cx| {
                    if !event.is_keyboard() {
                        press(window, cx);
                    }
                })
            })
            .child(self.label);
        focus_ring(button, ring, false, theme)
    }
}

/// Moves the keyboard to the next stop of `order`, or the previous one with Shift.
pub(crate) fn cycle(order: &[FocusHandle], backwards: bool, window: &mut Window, cx: &mut App) {
    let current = order.iter().position(|focus| focus.contains_focused(window, cx));
    if let Some(next) = next_stop(current, order.len(), backwards) {
        window.focus(&order[next], cx);
    }
}

/// From outside the order, Tab starts at its first stop and Shift+Tab at its last.
fn next_stop(current: Option<usize>, count: usize, backwards: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match current {
        Some(index) if backwards => (index + count - 1) % count,
        Some(index) => (index + 1) % count,
        None if backwards => count - 1,
        None => 0,
    })
}

#[cfg(test)]
#[path = "dialog/tests/cycle.rs"]
mod tests;
