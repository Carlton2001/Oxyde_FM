//! Asking before saving over files already there, in a prompt over the file chooser
//! that blocks it, as the main window's confirmations look (`confirmation::body`,
//! `Modal`): Cancel has the keyboard first, Tab moves it, Enter or Space presses it,
//! Escape and the title bar's button cancel.
use super::FileChooserWindow;
use crate::ui::components::confirmation::{self, Tone};
use crate::ui::components::modal::Modal;
use crate::ui::elements::button::ButtonVariant;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

const ICON: &str = "icons/copy-x.svg";
/// The first names listed when several files are replaced.
const SHOWN: usize = 3;

pub(super) struct OverwritePrompt {
    chooser: WindowHandle<FileChooserWindow>,
    existing: Vec<PathBuf>,
    cancel: FocusHandle,
    replace: FocusHandle,
}

impl FileChooserWindow {
    /// Asks before `existing` are replaced; Replace answers `targets`.
    pub(super) fn confirm_overwrite(&mut self, existing: Vec<PathBuf>, targets: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        self.close_overwrite(cx);
        let Some(chooser) = window.window_handle().downcast::<Self>() else { return };
        let rem = window.rem_size();
        let size = size(rem * 27.5, rem * 10.0);
        let bounds = Bounds::new(window.bounds().center() - point(size.width / 2.0, size.height / 2.0), size);
        // The prompt opens once this update has ended, parented to the chooser, which
        // has the keyboard.
        cx.defer(move |cx| {
            let opened = cx.open_window(
                WindowOptions {
                    app_id: Some(crate::ui::APP_ID.into()),
                    window_decorations: crate::ui::window_decorations(),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    is_resizable: false,
                    is_minimizable: false,
                    titlebar: Some(TitlebarOptions { appears_transparent: true, ..Default::default() }),
                    #[cfg(target_os = "linux")]
                    kind: WindowKind::Dialog,
                    ..Default::default()
                },
                move |window, cx| {
                    window.set_rem_size(rem);
                    window.on_window_should_close(cx, move |_, cx| {
                        answered(chooser, false, cx);
                        true
                    });
                    cx.new(|cx| {
                        let cancel = cx.focus_handle();
                        window.focus(&cancel, cx);
                        OverwritePrompt { chooser, existing, cancel, replace: cx.focus_handle() }
                    })
                },
            );
            match opened {
                Ok(prompt) => {
                    let _ = chooser.update(cx, |this, _, _| this.overwrite = Some((prompt, targets)));
                }
                Err(error) => log::error!("Cannot ask before replacing files: {error}"),
            }
        });
    }

    /// Closes the prompt, unanswered.
    pub(super) fn close_overwrite(&mut self, cx: &mut App) {
        if let Some((prompt, _)) = self.overwrite.take() {
            let _ = prompt.update(cx, |_, window, cx| {
                crate::ui::components::tool_window::close_transients(window, cx);
                window.remove_window();
            });
        }
    }

    /// The prompt's answer: the files are chosen, or the user is back where they typed.
    fn overwrite_answered(&mut self, replace: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, targets)) = self.overwrite.take() else { return };
        if replace {
            self.finish(Some(targets), window, cx);
        } else {
            self.refocus_name(window, cx);
        }
    }
}

/// Hands the answer to the chooser once the prompt's own update has ended.
fn answered(chooser: WindowHandle<FileChooserWindow>, replace: bool, cx: &mut App) {
    cx.defer(move |cx| {
        let _ = chooser.update(cx, |this, window, cx| this.overwrite_answered(replace, window, cx));
    });
}

impl OverwritePrompt {
    fn answer(&mut self, replace: bool, window: &mut Window, cx: &mut Context<Self>) {
        answered(self.chooser, replace, cx);
        window.remove_window();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "escape" => self.answer(false, window, cx),
            "tab" => {
                let order = [self.cancel.clone(), self.replace.clone()];
                crate::ui::components::dialog::cycle(&order, event.keystroke.modifiers.shift, window, cx);
                cx.notify();
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    /// The question, the folder, the first names when there are several, and the warning.
    fn body(&self, cx: &App) -> Div {
        let theme = *cx.theme();
        let i18n = cx.i18n();
        let language = i18n.language();
        let count = self.existing.len();
        let name = |path: &PathBuf| path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        let folder = self.existing.first().and_then(|path| path.parent()).map(|folder| folder.to_string_lossy().into_owned()).unwrap_or_default();
        let mut args = FluentArgs::new();
        args.set("count", count as i64);
        args.set("number", crate::ui::format::format_number(count as u64, language));
        args.set("name", self.existing.first().map(name).unwrap_or_default());
        args.set("folder", folder);
        let question = i18n.text_with_args(Message::FileChooserReplacePrompt, &args);
        let mut details = vec![confirmation::detail(i18n.text_with_args(Message::FileChooserReplaceLocation, &args), theme)];
        if count > 1 {
            details.extend(self.existing.iter().take(SHOWN).map(|path| div().min_w_0().truncate().child(name(path)).into_any_element()));
        }
        if count > SHOWN {
            let mut more = FluentArgs::new();
            more.set("count", (count - SHOWN) as i64);
            more.set("number", crate::ui::format::format_number((count - SHOWN) as u64, language));
            details.push(confirmation::detail(i18n.text_with_args(Message::DeleteMore, &more), theme));
        }
        let warning = Some(i18n.text_with_args(Message::FileChooserReplaceWarning, &args));
        confirmation::body(ICON, Tone::Danger, question, details, warning, theme)
    }
}

impl Render for OverwritePrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.cancel.is_focused(window) && !self.replace.is_focused(window) {
            window.focus(&self.cancel, cx);
        }
        let theme = *cx.theme();
        let mut args = FluentArgs::new();
        args.set("count", self.existing.len() as i64);
        let title = cx.i18n().text_with_args(Message::FileChooserReplaceTitle, &args);
        let replace_label = cx.i18n().text(Message::ConflictReplace);
        let (cancel, replace) = (cx.entity().downgrade(), cx.entity().downgrade());
        let close = cx.entity().downgrade();
        let modal = Modal::new("file-chooser-replace", title, self.body(cx))
            .title_icon(ICON)
            .on_close(move |window, cx| { let _ = close.update(cx, |this, cx| this.answer(false, window, cx)); })
            .action(
                crate::ui::components::dialog::button("file-chooser-replace-cancel", cx.i18n().text(Message::Cancel), ButtonVariant::Secondary)
                    .focus(&self.cancel)
                    .on_press(move |window, cx| { let _ = cancel.update(cx, |this, cx| this.answer(false, window, cx)); }),
            )
            .action(
                crate::ui::components::dialog::button("file-chooser-replace-confirm", replace_label, Tone::Danger.variant())
                    .focus(&self.replace)
                    .on_press(move |window, cx| { let _ = replace.update(cx, |this, cx| this.answer(true, window, cx)); }),
            );
        let root = div()
            .size_full()
            .relative()
            .bg(theme.active_background)
            .text_color(theme.text)
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .on_key_down(cx.listener(Self::key_down))
            .child(modal);
        crate::ui::components::client_decorations::frame(root, theme.border, window, cx)
    }
}
