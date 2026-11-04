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
    /// The typed password is being checked.
    pub checking: bool,
    /// Restarts the field's shake for each refusal.
    pub shake_id: u64,
    /// The field takes the focus back at the next frame, after a refusal.
    pub refocus: std::cell::Cell<bool>,
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

/// A 7z may encrypt its header, hiding even its names: before showing it, its listing
/// is tried, so the password is asked over the current folder rather than an empty one.
pub(crate) fn locks_its_listing(path: &str) -> bool {
    let path = Path::new(path);
    oxyde_lib::utils::archive::ArchiveFormat::from_path(path) == Some(oxyde_lib::utils::archive::ArchiveFormat::SevenZip)
        && archive_password::for_archive(path).is_none()
}

impl OxydeApp {
    pub(super) fn open_locked_archive(&mut self, pane_id: &str, path: String, new_tab: bool, cx: &mut Context<Self>) {
        let pane = pane_id.to_string();
        let origin = self.state.find_pane(pane_id)
            .map(|pane| (pane.active_tab().id.clone(), pane.active_tab().path.clone()));
        cx.spawn(async move |this, cx| {
            let probe = path.clone();
            let refusal = tokio::task::spawn_blocking(move || {
                oxyde_lib::commands::archive::list_archive_contents(probe, String::new())
                    .err()
                    .map(|error| error.to_string())
                    .filter(|detail| password_failure(detail).is_some())
            })
            .await
            .ok()
            .flatten();
            let _ = this.update(cx, |this, cx| {
                // The user went elsewhere meanwhile.
                let current = this.state.find_pane(&pane)
                    .map(|pane| (pane.active_tab().id.clone(), pane.active_tab().path.clone()));
                if !new_tab && current != origin {
                    return;
                }
                let archive = PathBuf::from(&path);
                let open = move |this: &mut Self, cx: &mut Context<Self>| {
                    if new_tab {
                        this.add_tab(&pane, path, false, cx);
                    } else {
                        this.navigate_to(&pane, path, cx);
                    }
                };
                match refusal {
                    Some(detail) => {
                        this.ask_archive_password(&archive, &detail, open, cx);
                    }
                    None => open(this, cx),
                }
            });
        })
        .detach();
    }

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
            checking: false,
            shake_id: 0,
            refocus: Default::default(),
            focus: cx.focus_handle(),
            controls: (0..2).map(|_| cx.focus_handle()).collect(),
            retry: Some(retry),
        });
        cx.notify();
    }

    /// Cancels, or checks the typed password off the UI thread. A wrong one keeps the
    /// dialog open: the field shakes, empties and says so, as the path bar does.
    pub(crate) fn answer_archive_password(&mut self, cancel: bool, window: &mut Window, cx: &mut Context<Self>) {
        if cancel {
            if self.archive_password.take().is_some() {
                self.context_menus.clear();
                window.focus(&self.focus_handle, cx);
                cx.notify();
            }
            return;
        }
        let Some(prompt) = self.archive_password.as_mut().filter(|prompt| !prompt.checking) else { return };
        let Some(secret) = ArchiveSecret::new(std::mem::take(&mut prompt.value)) else { return };
        prompt.checking = true;
        let archive = prompt.archive.clone();
        archive_password::remember(&archive, secret);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let check = archive.clone();
            let result = tokio::task::spawn_blocking(move || {
                oxyde_lib::commands::archive::verify_archive_password(&check)
            })
            .await;
            let wrong = matches!(&result, Ok(Err(error))
                if password_failure(&error.to_string()) == Some(ArchivePasswordFailure::Wrong));
            let _ = this.update(cx, |this, cx| this.finish_password_check(&archive, wrong, cx));
        })
        .detach();
    }

    fn finish_password_check(&mut self, archive: &Path, wrong: bool, cx: &mut Context<Self>) {
        if self.archive_password.as_ref().is_none_or(|prompt| prompt.archive != archive || !prompt.checking) {
            return;
        }
        if wrong {
            archive_password::forget(archive);
            if let Some(prompt) = &mut self.archive_password {
                prompt.checking = false;
                prompt.wrong = true;
                prompt.shake_id = prompt.shake_id.wrapping_add(1);
                // The Unlock button had the focus: typing again starts in the field.
                prompt.refocus.set(true);
                // A task of its own: the field is not updating itself here.
                prompt.input.update(cx, |input, cx| input.set_content(String::new(), cx));
            }
            cx.notify();
            return;
        }
        // Any other outcome is the action's to report.
        let Some(mut prompt) = self.archive_password.take() else { return };
        self.context_menus.clear();
        if let Some(window) = self.main_window {
            let focus = self.focus_handle.clone();
            cx.defer(move |cx| {
                let _ = window.update(cx, |_, window, cx| window.focus(&focus, cx));
            });
        }
        cx.notify();
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

    /// An operation refused for an archive's password (extraction, copy out of or into
    /// it, deletion inside it) runs again with the same sources and destination.
    pub(super) fn retry_operation_with_password(&mut self, op_id: &str, error: &str, cx: &mut Context<Self>) -> bool {
        if password_failure(error).is_none() {
            return false;
        }
        let Some((kind, sources, destination)) = self.file_ops.operation_request(op_id) else { return false };
        let Some(archive) = sources.iter().chain(destination.iter()).find_map(|source| {
            oxyde_lib::utils::archive::split_virtual_path(&source.to_string_lossy())
                .map(|(archive, _)| archive)
                .or_else(|| oxyde_lib::utils::archive::is_archive(source).then(|| source.clone()))
        }) else {
            return false;
        };
        self.ask_archive_password(&archive, error, move |this, cx| {
            this.requeue_operation(kind, sources, destination, cx);
        }, cx)
    }

    fn requeue_operation(
        &mut self,
        kind: oxyde_lib::systems::file_ops::FileOpType,
        sources: Vec<PathBuf>,
        destination: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let quality_path = destination.clone().or_else(|| sources.first().cloned());
        let quality = quality_path.map(|path| self.archive_quality_for_path(&path));
        let (qualities, turbo) = (self.archive_qualities(), self.config.default_turbo_mode);
        // As `queue_archive_operation`: the runtime is reached from a spawned task.
        cx.spawn(async move |this, cx| {
            let runtime = tokio::runtime::Handle::current();
            let _ = this.update(cx, |this, cx| {
                let mut operation = oxyde_lib::systems::file_ops::FileOperation::new(kind, sources, destination, None);
                if let Some(quality) = quality {
                    operation.compression_quality = quality;
                }
                operation.archive_qualities = qualities;
                operation.set_turbo(turbo);
                let id = this.file_ops.queue_operation_on(&runtime, this.history.clone(), operation);
                this.watch_file_operation(id, cx);
            });
        })
        .detach();
    }
}
