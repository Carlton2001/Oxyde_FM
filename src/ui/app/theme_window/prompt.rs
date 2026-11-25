//! Asking before a user theme goes: deleted, or its unsaved changes left for another
//! theme. The question covers the window, as the Trash's confirmations do theirs.

use super::ThemeWindow;
use crate::ui::components::confirmation::{self, Tone};
use crate::ui::components::dialog;
use crate::ui::components::modal::Modal;
use crate::ui::elements::button::ButtonVariant;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::{ThemeChoice, ThemeContext};
use gpui::*;

pub(super) enum ThemePrompt {
    Delete,
    /// Another theme chosen while the draft has unsaved changes.
    Leave(ThemeChoice),
}

impl ThemeWindow {
    pub(super) fn ask_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ask(ThemePrompt::Delete, window, cx);
    }

    fn ask(&mut self, prompt: ThemePrompt, window: &mut Window, cx: &mut Context<Self>) {
        self.prompt = Some(prompt);
        self.drag = None;
        // Cancel holds the keyboard first.
        window.focus(&self.stops.prompt_cancel, cx);
        cx.notify();
    }

    /// Chooses `choice`, after asking when the draft has changes it would drop.
    pub(super) fn choose(&mut self, choice: ThemeChoice, window: &mut Window, cx: &mut Context<Self>) {
        if self.draft.as_ref().is_some_and(|draft| draft.is_modified()) {
            self.ask(ThemePrompt::Leave(choice), window, cx);
        } else {
            self.choose_now(choice, window, cx);
        }
    }

    pub(super) fn answer_prompt(&mut self, confirm: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(prompt) = self.prompt.take() else { return };
        window.focus(&self.focus, cx);
        if confirm {
            match prompt {
                ThemePrompt::Delete => self.delete_draft(cx),
                ThemePrompt::Leave(choice) => self.choose_now(choice, window, cx),
            }
        }
        cx.notify();
    }

    /// Tab moves between the question's two buttons while it is shown.
    pub(super) fn prompt_order(&self) -> Option<Vec<FocusHandle>> {
        self.prompt.as_ref().map(|_| vec![self.stops.prompt_cancel.clone(), self.stops.prompt_confirm.clone()])
    }

    pub(super) fn render_prompt(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let prompt = self.prompt.as_ref()?;
        let theme = *cx.theme();
        let mut args = FluentArgs::new();
        args.set("name", self.draft.as_ref().map(|draft| draft.name.clone()).unwrap_or_default());
        let (icon, tone, title, question, confirm) = match prompt {
            ThemePrompt::Delete => ("icons/trash-2.svg", Tone::Danger, Message::ThemeDelete,
                Message::ThemeDeleteQuestion, Message::ThemeDelete),
            ThemePrompt::Leave(_) => ("icons/palette.svg", Tone::Neutral, Message::ThemeUnsavedTitle,
                Message::ThemeLeaveQuestion, Message::ThemeDiscard),
        };
        let question = cx.i18n().text_with_args(question, &args);
        let body = confirmation::body(icon, tone, question, Vec::new(), None, theme);
        let handle = cx.entity().downgrade();
        let answer = move |confirm: bool| {
            let handle = handle.clone();
            move |window: &mut Window, cx: &mut App| {
                let _ = handle.update(cx, |this, cx| this.answer_prompt(confirm, window, cx));
            }
        };
        let modal = Modal::new("theme-prompt", cx.i18n().text(title), body)
            .title_icon(icon)
            .on_close(answer(false))
            .action(dialog::button("theme-prompt-cancel", cx.i18n().text(Message::Cancel), ButtonVariant::Secondary)
                .focus(&self.stops.prompt_cancel)
                .on_press(answer(false)))
            .action(dialog::button("theme-prompt-confirm", cx.i18n().text(confirm), tone.variant())
                .focus(&self.stops.prompt_confirm)
                .on_press(answer(true)));
        Some(div().absolute().inset_0().child(modal).into_any_element())
    }
}
