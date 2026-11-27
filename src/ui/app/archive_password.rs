//! Passwords of encrypted archives, asked when a listing, an opening or an extraction is
//! refused. The answer is remembered for the archive until Oxyde exits, then the refused
//! action runs again; a wrong password is forgotten and asked again.
use super::*;
use crate::ui::components::input::{InputAppearance, TextInput, TextInputOptions};
use oxyde_lib::models::error::ArchivePasswordFailure;
use oxyde_lib::utils::archive_password::{self, ArchiveSecret};
use std::path::{Path, PathBuf};

type Retry = Box<dyn FnOnce(&mut OxydeApp, &mut Context<OxydeApp>)>;

/// A password request that came while another prompt was open.
pub(crate) struct QueuedAsk {
    archive: PathBuf,
    wrong: bool,
    retry: Retry,
}

pub(crate) struct ArchivePasswordPrompt {
    pub archive: PathBuf,
    /// The previous password was refused.
    pub wrong: bool,
    /// The typed password is longer than an archive password may be.
    pub too_long: bool,
    pub input: Entity<TextInput>,
    /// Kept as typed: an input calling back cannot be read while it updates.
    /// Each replaced copy is wiped.
    pub value: zeroize::Zeroizing<String>,
    /// The typed password is being checked.
    pub checking: bool,
    /// The eye shows the typed characters.
    pub revealed: bool,
    /// Restarts the field's shake for each refusal.
    pub shake_id: u64,
    /// Identifies the running check: a cancelled one's result must not end a newer one.
    check: u64,
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

/// The archive an operation needs the password of: the one holding a source, else the
/// destination's, else a source archive itself (an extraction). An archive only copied
/// as a file needs none.
fn transfer_archive(sources: &[PathBuf], destination: Option<&Path>) -> Option<PathBuf> {
    sources
        .iter()
        .find_map(|path| {
            oxyde_lib::utils::archive::split_virtual_path(&path.to_string_lossy())
                .filter(|(_, internal)| !internal.is_empty())
                .map(|(archive, _)| archive)
        })
        .or_else(|| destination.map(|path| archive_of(&path.to_string_lossy())).filter(|archive| oxyde_lib::utils::archive::is_archive(archive)))
        .or_else(|| sources.iter().find(|path| oxyde_lib::utils::archive::is_archive(path)).cloned())
}

pub(crate) fn password_failure(detail: &str) -> Option<ArchivePasswordFailure> {
    ArchivePasswordFailure::from_message(detail)
}

/// A 7z or a RAR may encrypt its headers, hiding even its names: before showing it, its listing
/// is tried, so the password is asked over the current folder rather than an empty one.
pub(crate) fn locks_its_listing(path: &str) -> bool {
    let path = Path::new(path);
    matches!(oxyde_lib::utils::archive::ArchiveFormat::from_path(path), Some(oxyde_lib::utils::archive::ArchiveFormat::SevenZip | oxyde_lib::utils::archive::ArchiveFormat::Rar))
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
                oxyde_lib::commands::archive_probe::probe(probe)
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
                    oxyde_lib::commands::archive_probe::forget();
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
        // A refusal arriving while a new password is checked came from the old one:
        // forgetting it would drop the password being checked.
        let checking = self.archive_password.as_ref().is_some_and(|prompt| prompt.checking && prompt.archive == archive);
        if wrong && !checking {
            archive_password::forget(archive);
        }
        let archive = archive.to_path_buf();
        let retry: Retry = Box::new(retry);
        // One prompt at a time: the others wait for it.
        if self.archive_password.is_some() {
            self.archive_password_queue.push(QueuedAsk { archive, wrong, retry });
            return true;
        }
        let Some(window) = self.main_window else { return false };
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
            self.archive_password_queue.push(QueuedAsk { archive, wrong, retry });
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
                        prompt.value = zeroize::Zeroizing::new(text.to_string());
                        prompt.too_long = false;
                    }
                }),
                move |window, cx| submit.update(cx, |this, cx| this.answer_archive_password(false, window, cx)),
                move |window, cx| cancel.update(cx, |this, cx| this.answer_archive_password(true, window, cx)),
                move |input, selected, point, window, cx| {
                    menu.update(cx, |this, cx| this.open_text_context_menu(input, selected, point, window, cx))
                },
                TextInputOptions { appearance: InputAppearance::Embedded, ..Default::default() },
                window,
            )
            .password()
        });
        self.archive_password = Some(ArchivePasswordPrompt {
            archive,
            wrong,
            too_long: false,
            input,
            value: Default::default(),
            checking: false,
            revealed: false,
            shake_id: 0,
            check: 0,
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
            if let Some(prompt) = self.archive_password.take() {
                self.context_menus.clear();
                window.focus(&self.focus_handle, cx);
                // Declining an archive declines every request waiting for it.
                self.archive_password_queue.retain(|ask| ask.archive != prompt.archive);
                self.ask_next_queued_password(cx);
                cx.notify();
            }
            return;
        }
        let Some(prompt) = self.archive_password.as_mut().filter(|prompt| !prompt.checking) else { return };
        // Refused before any check, as a wrong one is: the field shakes and says why.
        if prompt.value.len() > archive_password::MAX_BYTES {
            prompt.too_long = true;
            prompt.shake_id = prompt.shake_id.wrapping_add(1);
            cx.notify();
            return;
        }
        let Some(secret) = ArchiveSecret::new(std::mem::take(&mut *prompt.value)) else { return };
        prompt.checking = true;
        static CHECKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let check = CHECKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        prompt.check = check;
        let archive = prompt.archive.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let path = archive.clone();
            let result = tokio::task::spawn_blocking(move || {
                // Resolving the archive's real path reads the disk.
                archive_password::remember(&path, secret);
                oxyde_lib::commands::archive::verify_archive_password(&path)
            })
            .await;
            let wrong = matches!(&result, Ok(Err(error))
                if password_failure(&error.to_string()) == Some(ArchivePasswordFailure::Wrong));
            let _ = this.update(cx, |this, cx| this.finish_password_check(&archive, check, wrong, cx));
        })
        .detach();
    }

    fn finish_password_check(&mut self, archive: &Path, check: u64, wrong: bool, cx: &mut Context<Self>) {
        if self.archive_password.as_ref().is_none_or(|prompt| prompt.check != check || !prompt.checking) {
            // Cancelled while checking: the password was remembered unconfirmed. A newer
            // check of the same archive owns what is remembered now.
            let newer = self.archive_password.as_ref().is_some_and(|prompt| prompt.checking && prompt.archive == archive);
            if !newer {
                archive_password::forget(archive);
            }
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
        self.ask_next_queued_password(cx);
    }

    /// Runs the waiting requests the new password unlocks, then prompts for the next archive.
    fn ask_next_queued_password(&mut self, cx: &mut Context<Self>) {
        while self.archive_password.is_none() && !self.archive_password_queue.is_empty() {
            let ask = self.archive_password_queue.remove(0);
            if archive_password::for_archive(&ask.archive).is_some() {
                (ask.retry)(self, cx);
                continue;
            }
            let Some(window) = self.main_window else { return };
            let QueuedAsk { archive, wrong, retry } = ask;
            // Shown by the task; a request arriving meanwhile queues itself behind it there.
            cx.spawn(async move |this, cx| {
                let _ = window.update(cx, |_, window, cx| {
                    let _ = this.update(cx, |this, cx| this.show_archive_password(archive, wrong, retry, window, cx));
                });
            })
            .detach();
            return;
        }
    }

    pub(crate) fn toggle_archive_password_prompt_reveal(&mut self, cx: &mut Context<Self>) {
        let Some(prompt) = &mut self.archive_password else { return };
        prompt.revealed = !prompt.revealed;
        let masked = !prompt.revealed;
        prompt.input.update(cx, |input, cx| input.set_masked(masked, cx));
        cx.notify();
    }

    /// The dialog could not open: the action stays refused. The requests waiting behind it
    /// are asked in turn instead of waiting for an unrelated prompt.
    pub(super) fn dismiss_archive_password(&mut self, cx: &mut Context<Self>) {
        self.archive_password = None;
        self.ask_next_queued_password(cx);
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
    /// it, deletion inside it) runs again for the sources it left untouched, with the
    /// same destination, conflict decisions and settings. When one may hold part of the
    /// work, the refusal is reported instead: running it again could do that part twice.
    pub(super) fn retry_operation_with_password(&mut self, op_id: &str, error: &str, cx: &mut Context<Self>) -> bool {
        if password_failure(error).is_none() {
            return false;
        }
        let Some(retry) = self.file_ops.untouched_retry(op_id) else { return false };
        let Some(archive) = transfer_archive(&retry.sources, retry.destination.as_deref()) else {
            return false;
        };
        // Files dropped from another application stay staged until their copy runs again.
        let staging = self.drop_stagings.remove(op_id);
        self.ask_archive_password(&archive, error, move |this, cx| {
            this.requeue_operation(retry, staging, cx);
        }, cx)
    }

    /// A copy or a move refused before it started, while its conflicts were looked for in
    /// an archive: once unlocked, it is prepared again, conflicts asked as usual.
    pub(super) fn retry_preparation_with_password(
        &mut self,
        paths: Vec<String>,
        target_dir: String,
        is_move: bool,
        staging: Option<std::rc::Rc<oxyde_lib::platform::DropStaging>>,
        error: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if password_failure(error).is_none() {
            return false;
        }
        let sources: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        let Some(archive) = transfer_archive(&sources, Some(Path::new(&target_dir))) else { return false };
        self.ask_archive_password(&archive, error, move |this, cx| {
            this.prepare_file_operation(paths, target_dir, is_move, staging, cx);
        }, cx)
    }

    fn requeue_operation(
        &mut self,
        operation: oxyde_lib::systems::file_ops::FileOperation,
        staging: Option<std::rc::Rc<oxyde_lib::platform::DropStaging>>,
        cx: &mut Context<Self>,
    ) {
        // As `queue_archive_operation`: the runtime is reached from a spawned task.
        cx.spawn(async move |this, cx| {
            let runtime = tokio::runtime::Handle::current();
            let _ = this.update(cx, |this, cx| {
                let id = this.file_ops.queue_operation_on(&runtime, this.history.clone(), operation);
                if let Some(staging) = staging {
                    this.drop_stagings.insert(id.clone(), staging);
                }
                this.watch_file_operation(id, cx);
            });
        })
        .detach();
    }
}

#[cfg(test)]
#[path = "archive_password/tests/archives.rs"]
mod tests;
