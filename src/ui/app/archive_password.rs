//! Passwords of encrypted archives, asked when a listing, an opening or an extraction is
//! refused. The answer is remembered for the archive until Oxyde exits, then the refused
//! action runs again; a wrong password is forgotten and asked again.
use super::*;
use crate::ui::components::input::{TextInput, TextInputOptions};
use oxyde_lib::models::error::ArchivePasswordFailure;
use oxyde_lib::utils::archive_password::{self, ArchiveSecret};
use std::path::{Path, PathBuf};

type Retry = Box<dyn FnOnce(&mut OxydeApp, &mut Context<OxydeApp>)>;

pub(crate) struct ArchivePasswordPrompt {
    pub archive: PathBuf,
    /// The previous password was refused.
    pub wrong: bool,
    pub input: Entity<TextInput>,
    /// Kept as typed: an input calling back cannot be read while it updates.
    pub value: String,
    pub focus: FocusHandle,
    /// Cancel and unlock.
    pub controls: Vec<FocusHandle>,
    retry: Option<Retry>,
}

/// The archive holding `path`: the path itself, or the archive of a virtual path.
pub(crate) fn archive_of(path: &str) -> PathBuf {
    oxyde_lib::utils::archive::split_virtual_path(path)
        .map_or_else(|| PathBuf::from(path), |(archive, _)| archive)
}

pub(crate) fn password_failure(detail: &str) -> Option<ArchivePasswordFailure> {
    ArchivePasswordFailure::from_message(detail)
}

impl OxydeApp {
    /// When `detail` is a password refusal for `archive`, asks for one and returns true;
    /// `retry` then runs once the user has answered.
    pub(crate) fn ask_archive_password(
        &mut self,
        archive: &Path,
        detail: &str,
        retry: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(failure) = password_failure(detail) else { return false };
        let wrong = failure == ArchivePasswordFailure::Wrong;
        if wrong {
            archive_password::forget(archive);
        }
        // One prompt at a time; a refusal meanwhile keeps its error.
        if self.archive_password.is_some() {
            return true;
        }
        let Some(window) = self.main_window else { return false };
        let archive = archive.to_path_buf();
        let retry: Retry = Box::new(retry);
        cx.spawn(async move |this, cx| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| this.show_archive_password(archive, wrong, retry, window, cx));
            });
        })
        .detach();
        true
    }

    fn show_archive_password(
        &mut self,
        archive: PathBuf,
        wrong: bool,
        retry: Retry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.archive_password.is_some() {
            return;
        }
        let handle = cx.entity();
        let (change, submit, cancel, menu) = (handle.clone(), handle.clone(), handle.clone(), handle);
        let input = cx.new(|cx| {
            TextInput::new(
                cx,
                "",
                String::new(),
                move |text, _, cx| change.update(cx, |this, _| {
                    if let Some(prompt) = &mut this.archive_password {
                        prompt.value = text.to_string();
                    }
                }),
                move |window, cx| submit.update(cx, |this, cx| this.answer_archive_password(false, window, cx)),
                move |window, cx| cancel.update(cx, |this, cx| this.answer_archive_password(true, window, cx)),
                move |input, selected, point, window, cx| {
                    menu.update(cx, |this, cx| this.open_text_context_menu(input, selected, point, window, cx))
                },
                TextInputOptions::default(),
                window,
            )
            .password()
        });
        self.archive_password = Some(ArchivePasswordPrompt {
            archive,
            wrong,
            input,
            value: String::new(),
            focus: cx.focus_handle(),
            controls: (0..2).map(|_| cx.focus_handle()).collect(),
            retry: Some(retry),
        });
        cx.notify();
    }

    pub(crate) fn answer_archive_password(&mut self, cancel: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !cancel && self.archive_password.as_ref().is_some_and(|prompt| prompt.value.is_empty()) {
            return;
        }
        let Some(mut prompt) = self.archive_password.take() else { return };
        let value = std::mem::take(&mut prompt.value);
        prompt.input.update(cx, |input, cx| input.set_content(String::new(), cx));
        self.context_menus.clear();
        window.focus(&self.focus_handle, cx);
        cx.notify();
        if cancel {
            return;
        }
        let Some(secret) = ArchiveSecret::new(value) else { return };
        archive_password::remember(&prompt.archive, secret);
        if let Some(retry) = prompt.retry.take() {
            retry(self, cx);
        }
    }

    /// The dialog could not open: the action stays refused.
    pub(super) fn dismiss_archive_password(&mut self) {
        self.archive_password = None;
    }

    /// A tab whose archive refused its listing lists it again once unlocked.
    pub(super) fn ask_listing_password(&mut self, tab_id: &str, path: &str, detail: &str, cx: &mut Context<Self>) {
        let (tab_id, path) = (tab_id.to_string(), path.to_string());
        let archive = archive_of(&path);
        self.ask_archive_password(&archive, detail, move |this, cx| {
            let mut target = None;
            this.state.layout.visit_panes(&mut |pane| {
                if let Some(index) = pane.tabs.iter().position(|tab| tab.id == tab_id && tab.path == path) {
                    target = Some((pane.id.clone(), index));
                }
            });
            if let Some((pane, index)) = target {
                if let Some(tab) = this.state.find_tab_mut(&tab_id) {
                    tab.load_status = crate::ui::workspace::TabLoadStatus::Loading;
                }
                this.request_tab_files(&pane, index, path, false, cx);
            }
        }, cx);
    }

    /// An extraction or a copy out of an archive refused for its password runs again,
    /// with the same sources and destination, once unlocked.
    pub(super) fn retry_operation_with_password(&mut self, op_id: &str, error: &str, cx: &mut Context<Self>) -> bool {
        if password_failure(error).is_none() {
            return false;
        }
        let Some((kind, sources, destination)) = self.file_ops.operation_request(op_id) else { return false };
        let Some(archive) = sources.iter().find_map(|source| {
            oxyde_lib::utils::archive::split_virtual_path(&source.to_string_lossy())
                .map(|(archive, _)| archive)
                .or_else(|| oxyde_lib::utils::archive::is_archive(source).then(|| source.clone()))
        }) else {
            return false;
        };
        let Some(destination) = destination else { return false };
        self.ask_archive_password(&archive, error, move |this, cx| {
            this.queue_archive_operation(kind, sources, destination, cx);
        }, cx)
    }
}
