//! Remote locations opened by URI, such as `smb://server/share` on Linux: the share is
//! mounted with the credentials its server asks for, as Nautilus does, then browsed at
//! its local path; servers and the network list stay listed by URI.
use super::*;
use crate::ui::components::input::{TextInput, TextInputOptions};
use oxyde_lib::platform::{RemoteAnswer, RemoteError, RemoteLocation, RemotePrompt};
use tokio::sync::{mpsc, oneshot};

pub(crate) struct RemoteLogin {
    pub prompt: RemotePrompt,
    /// User, domain and password.
    pub inputs: Vec<Entity<TextInput>>,
    /// Kept as typed: an input calling back cannot be read while it updates.
    pub values: [String; 3],
    pub anonymous: bool,
    pub remember: bool,
    pub focus: FocusHandle,
    /// The first field to type in.
    pub initial: FocusHandle,
    /// Anonymous, remember, cancel and connect.
    pub controls: Vec<FocusHandle>,
    reply: Option<oneshot::Sender<RemoteAnswer>>,
}

enum Progress {
    Prompt(RemotePrompt, oneshot::Sender<RemoteAnswer>),
    Done(Result<RemoteLocation, RemoteError>),
}

impl OxydeApp {
    /// Connects before listing; a share then opens at its local path.
    pub(super) fn connect_remote_tab(&mut self, tab_id: String, request_id: u64, uri: String, cx: &mut Context<Self>) {
        let Some(handle) = self.handle.clone() else { return };
        cx.spawn(async move |_, cx| {
            let result = connect(&handle, cx, uri.clone()).await;
            let _ = handle.update(cx, |this, cx| {
                if this.pending_file_requests.get(&tab_id) != Some(&request_id) {
                    return;
                }
                this.pending_file_requests.remove(&tab_id);
                this.finish_remote_connection(&tab_id, &uri, result, cx);
            });
        })
        .detach();
    }

    fn finish_remote_connection(
        &mut self, tab_id: &str, uri: &str, result: Result<RemoteLocation, RemoteError>, cx: &mut Context<Self>,
    ) {
        let mut target = None;
        self.state.layout.visit_panes_mut(&mut |pane| {
            if let Some(index) = pane.tabs.iter().position(|tab| tab.id == tab_id && tab.path == uri) {
                target = Some((pane.id.clone(), index));
            }
        });
        let Some((pane_id, index)) = target else { return };
        let path = match result {
            Ok(RemoteLocation::Local(path)) => path.to_string_lossy().into_owned(),
            Ok(RemoteLocation::Browse(_)) => {
                self.browsable_remotes.insert(uri.to_string());
                uri.to_string()
            }
            // Dismissed: back where the user came from, as Nautilus stays there.
            Err(RemoteError::Cancelled) => self.state.find_tab_mut(tab_id)
                .and_then(|tab| tab.back_stack.pop())
                .unwrap_or_else(crate::ui::workspace::WorkspaceState::home_location),
            Err(error) => {
                log::warn!("Cannot connect to {uri}: {error}");
                if let Some(tab) = self.state.find_tab_mut(tab_id) {
                    tab.load_status = crate::ui::workspace::TabLoadStatus::Error(Message::RemoteConnectFailed);
                }
                self.toast(ToastKind::Error, Message::RemoteConnectFailedToast, error.to_string(), 0, cx);
                cx.notify();
                return;
            }
        };
        let Some(tab) = self.state.find_tab_mut(tab_id) else { return };
        tab.path = path.clone();
        self.request_tab_files(&pane_id, index, path, true, cx);
        self.reconcile_watchers(cx);
        self.save_workspace(cx);
        cx.notify();
    }

    fn show_remote_login(
        &mut self, prompt: RemotePrompt, reply: oneshot::Sender<RemoteAnswer>, window: &mut Window, cx: &mut Context<Self>,
    ) {
        // One prompt at a time; a second connection asking meanwhile gives up.
        if self.remote_login.is_some() {
            let _ = reply.send(RemoteAnswer::Cancel);
            return;
        }
        let handle = cx.entity();
        let values = [prompt.user.clone(), prompt.domain.clone(), String::new()];
        let inputs: Vec<_> = values.iter().cloned().enumerate().map(|(index, value)| {
            let change = handle.clone();
            let submit = handle.clone();
            let cancel = handle.clone();
            let menu = handle.clone();
            cx.new(|cx| {
                let input = TextInput::new(
                    cx, "", value,
                    move |text, _, cx| change.update(cx, |this, _| {
                        if let Some(login) = &mut this.remote_login {
                            login.values[index] = text.to_string();
                        }
                    }),
                    move |window, cx| submit.update(cx, |this, cx| this.answer_remote_login(false, window, cx)),
                    move |window, cx| cancel.update(cx, |this, cx| this.answer_remote_login(true, window, cx)),
                    move |input, selected, point, window, cx| {
                        menu.update(cx, |this, cx| this.open_text_context_menu(input, selected, point, window, cx))
                    },
                    TextInputOptions { clearable: index < 2, ..Default::default() },
                    window,
                );
                if index == 2 { input.password() } else { input }
            })
        }).collect();
        // The password is what a known user still has to type.
        let first = if prompt.needs_user && prompt.user.is_empty() { 0 } else { 2 };
        let initial = inputs[first].read(cx).focus_handle.clone();
        self.remote_login = Some(RemoteLogin {
            prompt,
            initial,
            inputs,
            values,
            anonymous: false,
            remember: false,
            focus: cx.focus_handle(),
            controls: (0..4).map(|_| cx.focus_handle()).collect(),
            reply: Some(reply),
        });
        cx.notify();
    }

    pub(crate) fn answer_remote_login(&mut self, cancel: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut login) = self.remote_login.take() else { return };
        let answer = if cancel {
            RemoteAnswer::Cancel
        } else if login.anonymous {
            RemoteAnswer::Anonymous
        } else {
            let [user, domain, password] = std::mem::take(&mut login.values);
            RemoteAnswer::Login {
                user: user.trim().to_string(),
                domain: domain.trim().to_string(),
                password,
                remember: login.remember && login.prompt.saving,
            }
        };
        if let Some(reply) = login.reply.take() {
            let _ = reply.send(answer);
        }
        self.context_menus.clear();
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    /// The dialog could not open: the connection gives up.
    pub(super) fn dismiss_remote_login(&mut self) {
        if let Some(reply) = self.remote_login.take().and_then(|mut login| login.reply.take()) {
            let _ = reply.send(RemoteAnswer::Cancel);
        }
    }

    pub(crate) fn toggle_remote_login(&mut self, remember: bool, cx: &mut Context<Self>) {
        let Some(login) = &mut self.remote_login else { return };
        if remember {
            login.remember = !login.remember;
        } else if login.prompt.anonymous {
            login.anonymous = !login.anonymous;
        }
        cx.notify();
    }
}

/// Mounts on a blocking worker; each prompt waits for the dialog's answer.
async fn connect(handle: &Entity<OxydeApp>, cx: &mut AsyncApp, uri: String) -> Result<RemoteLocation, RemoteError> {
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let target = uri.clone();
    tokio::task::spawn_blocking(move || {
        let result = oxyde_lib::platform::platform().connect_remote(&target, &mut |prompt| {
            let (reply, answer) = oneshot::channel();
            if sender.send(Progress::Prompt(prompt.clone(), reply)).is_err() {
                return RemoteAnswer::Cancel;
            }
            answer.blocking_recv().unwrap_or(RemoteAnswer::Cancel)
        });
        let _ = sender.send(Progress::Done(result));
    });
    while let Some(progress) = receiver.recv().await {
        match progress {
            Progress::Prompt(prompt, reply) => {
                // A dropped reply answers Cancel.
                let Some(window) = handle.update(cx, |this, _| this.main_window) else { continue };
                let handle = handle.clone();
                let _ = window.update(cx, |_, window, cx| {
                    handle.update(cx, |this, cx| this.show_remote_login(prompt, reply, window, cx));
                });
            }
            Progress::Done(result) => return result,
        }
    }
    Err(RemoteError::Other(format!("The connection to {uri} ended")))
}
