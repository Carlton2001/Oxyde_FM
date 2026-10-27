use crate::ui::components::input::TextInputOptions;
use super::*;
use oxyde_lib::models::DriveInfo;
use std::collections::HashSet;

pub(crate) struct NetworkForceConfirmation {
    pub paths: Vec<String>,
    pub revision: u64,
    pub direct: bool,
    pub focus: FocusHandle,
    pub confirm_focus: FocusHandle,
}

enum NetworkOperationError {
    Busy(Vec<String>),
    Other(String),
}
impl From<String> for NetworkOperationError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}

fn disconnect_safely(
    paths: &[String],
    force: bool,
    mut disconnect: impl FnMut(&str, bool) -> Result<(), oxyde_lib::platform::NetworkDisconnectError>,
) -> Result<(), NetworkOperationError> {
    for (index, path) in paths.iter().enumerate() {
        let force_current = force && index == 0;
        match disconnect(path, force_current) {
            Ok(()) => {}
            Err(oxyde_lib::platform::NetworkDisconnectError::Busy) if !force_current => {
                return Err(NetworkOperationError::Busy(paths[index..].to_vec()))
            }
            Err(oxyde_lib::platform::NetworkDisconnectError::Busy) => {
                return Err(NetworkOperationError::Other(format!(
                    "{path}: connection remains in use"
                )))
            }
            Err(oxyde_lib::platform::NetworkDisconnectError::Other(error)) => {
                return Err(NetworkOperationError::Other(format!("{path}: {error}")))
            }
        }
    }
    Ok(())
}

pub(crate) struct NetworkDialog {
    pub revision: u64,
    pub disconnect: bool,
    pub inputs: Vec<Entity<TextInput>>,
    pub values: [String; 3],
    pub drives: Vec<DriveInfo>,
    pub selected: HashSet<String>,
    pub letter: Option<String>,
    pub reconnect: bool,
    pub loading: bool,
    pub busy: bool,
    pub letter_open: bool,
    pub letter_bounds: Option<Bounds<Pixels>>,
    pub letter_scroll: ScrollHandle,
    pub letter_scroll_drag: Option<(String, f32, f32)>,
    pub error: Option<(Message, String)>,
    pub focus: FocusHandle,
    pub controls: Vec<FocusHandle>,
    pub drive_focus: Vec<FocusHandle>,
    pub preferred_drive: Option<String>,
}

pub(crate) fn mappable_network_path(path: &str) -> bool {
    path.strip_prefix("\\\\").is_some_and(|tail| {
        let mut parts = tail.split('\\');
        parts
            .next()
            .is_some_and(|server| !server.is_empty() && server != "?" && server != ".")
            && parts.next().is_some_and(|share| !share.is_empty())
    })
}
pub(crate) fn network_drive(drive: &DriveInfo) -> bool {
    matches!(drive.drive_type.as_str(), "remote" | "network") || drive.path.starts_with("\\\\")
}
pub(crate) fn drive_letter_used(letter: &str, drives: &[DriveInfo]) -> bool {
    drives
        .iter()
        .any(|drive| drive.path.to_ascii_uppercase().starts_with(letter))
}
pub(crate) fn free_letter(drives: &[DriveInfo]) -> Option<String> {
    ('A'..='Z')
        .rev()
        .map(|letter| format!("{letter}:"))
        .find(|letter| !drive_letter_used(letter, drives))
}
pub(crate) fn normalized_share(path: &str) -> Option<String> {
    let path = path.trim().replace('/', "\\");
    if path.contains('\0') {
        return None;
    }
    let path = format!(
        "\\\\{}",
        path.trim_start_matches('\\').trim_end_matches('\\')
    );
    let server = path
        .trim_start_matches('\\')
        .split('\\')
        .next()
        .unwrap_or_default();
    (mappable_network_path(&path) && !server.contains([':', '*', '?', '<', '>', '"', '|']))
        .then_some(path)
}

impl OxydeApp {
    pub(crate) fn resolve_network_force(
        &mut self,
        force: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(confirmation) = self.network_force_confirmation.take() else {
            return;
        };
        if force {
            if let Some(dialog) = &mut self.network_dialog {
                dialog.busy = true;
            }
            window.focus(&self.focus_handle, cx);
            self.run_network_operation(
                confirmation.revision,
                true,
                false,
                confirmation.paths,
                None,
                confirmation.direct,
                true,
                cx,
            );
        } else {
            if let Some(dialog) = &self.network_dialog {
                window.focus(&dialog.focus, cx);
            } else {
                window.focus(&self.focus_handle, cx);
            }
            cx.notify();
        }
    }
    pub(crate) fn network_drive_dialog(
        &mut self,
        disconnect: bool,
        path: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.context_menus.clear();
        self.settings_open = false;
        if self.ejecting_drive || self.network_dialog.is_some() {
            cx.notify();
            return;
        }
        self.network_dialog_revision = self.network_dialog_revision.wrapping_add(1);
        let revision = self.network_dialog_revision;
        let handle = cx.entity();
        let mut inputs = Vec::new();
        let values = [
            if disconnect {
                String::new()
            } else {
                path.clone().unwrap_or_default()
            },
            String::new(),
            String::new(),
        ];
        if !disconnect {
            for (index, initial) in values.iter().cloned().enumerate() {
                let submit = handle.clone();
                let change = handle.clone();
                let cancel = handle.clone();
                let menu = handle.clone();
                let dropdown = handle.clone();
                let input = cx.new(|cx| {
                    let input = TextInput::new(
                        cx,
                        if index == 0 { "\\\\server\\share" } else { "" },
                        initial,
                        move |text, _, cx| {
                            change.update(cx, |view, cx| {
                                if let Some(dialog) = &mut view.network_dialog {
                                    if dialog.busy {
                                        return;
                                    }
                                    dialog.values[index] = text.to_string();
                                    dialog.error = None;
                                    cx.notify();
                                }
                            })
                        },
                        move |_, cx| submit.update(cx, |this, cx| this.submit_network_dialog(cx)),
                        move |window, cx| {
                            cancel.update(cx, |this, cx| this.cancel_network_dialog(window, cx))
                        },
                        move |input, selected, point, window, cx| {
                            menu.update(cx, |this, cx| {
                                this.open_text_context_menu(input, selected, point, window, cx)
                            })
                        },
                        TextInputOptions {
                            clearable: true,
                            ..Default::default()
                        },
                        window,
                    );
                    cx.on_focus(&input.focus_handle, window, move |_, _, cx| {
                        dropdown.update(cx, |view, cx| {
                            if let Some(dialog) = &mut view.network_dialog {
                                dialog.letter_open = false;
                                cx.notify();
                            }
                        });
                    })
                    .detach();
                    if index == 2 {
                        input.password()
                    } else {
                        input
                    }
                });
                inputs.push(input);
            }
        }
        let focus = cx.focus_handle();
        let drives = self.state.drives.clone();
        let letter = free_letter(&drives);
        let drive_focus = drives
            .iter()
            .filter(|drive| network_drive(drive))
            .map(|_| cx.focus_handle())
            .collect();
        let selected = if disconnect {
            path.clone().into_iter().collect()
        } else {
            HashSet::new()
        };
        self.network_dialog = Some(NetworkDialog {
            revision,
            disconnect,
            inputs,
            values,
            drives,
            selected,
            letter,
            reconnect: true,
            loading: true,
            busy: false,
            letter_open: false,
            letter_bounds: None,
            letter_scroll: ScrollHandle::new(),
            letter_scroll_drag: None,
            error: None,
            focus: focus.clone(),
            controls: (0..4).map(|_| cx.focus_handle()).collect(),
            drive_focus,
            preferred_drive: if disconnect { path } else { None },
        });
        if let Some(input) = self
            .network_dialog
            .as_ref()
            .and_then(|dialog| dialog.inputs.first())
        {
            let input_focus = input.read(cx).focus_handle.clone();
            window.focus(&input_focus, cx);
        } else {
            window.focus(&focus, cx);
        }
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result =
                tokio::task::spawn_blocking(|| oxyde_lib::platform::platform().volumes().list(false))
                    .await;
            let _ = this.update(cx, |this, cx| {
                let Some(dialog) = this
                    .network_dialog
                    .as_mut()
                    .filter(|dialog| dialog.revision == revision)
                else {
                    return;
                };
                dialog.loading = false;
                match result {
                    Ok(drives) => {
                        dialog.drive_focus = drives
                            .iter()
                            .filter(|drive| network_drive(drive))
                            .map(|_| cx.focus_handle())
                            .collect();
                        dialog.drives = drives;
                        if dialog
                            .letter
                            .as_ref()
                            .is_none_or(|letter| drive_letter_used(letter, &dialog.drives))
                        {
                            dialog.letter = free_letter(&dialog.drives);
                        }
                        dialog.selected.retain(|path| {
                            dialog.drives.iter().any(|drive| {
                                network_drive(drive)
                                    && crate::ui::components::sidebar::sidebar_paths_equal(
                                        path,
                                        &drive.path,
                                    )
                            })
                        });
                        if let Some(path) = &dialog.preferred_drive {
                            if let Some(drive) = dialog.drives.iter().find(|drive| {
                                network_drive(drive)
                                    && crate::ui::components::sidebar::sidebar_paths_equal(
                                        path,
                                        &drive.path,
                                    )
                            }) {
                                dialog.selected.insert(drive.path.clone());
                            }
                        }
                    }
                    Err(error) => {
                        dialog.error = Some((Message::NetworkDriveFailed, error.to_string()))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn cancel_network_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .network_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.busy)
        {
            return;
        }
        self.network_dialog = None;
        self.context_menus.clear();
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    pub(crate) fn submit_network_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.network_dialog.as_mut() else {
            return;
        };
        if dialog.busy || dialog.loading {
            return;
        }
        let revision = dialog.revision;
        let disconnect = dialog.disconnect;
        let reconnect = dialog.reconnect;
        let paths: Vec<_> = dialog
            .drives
            .iter()
            .filter(|drive| network_drive(drive) && dialog.selected.contains(&drive.path))
            .map(|drive| drive.path.clone())
            .collect();
        let request = if disconnect {
            if paths.is_empty() {
                return;
            }
            None
        } else {
            let Some(path) = normalized_share(&dialog.values[0]) else {
                dialog.error = Some((Message::NetworkInvalidPath, String::new()));
                cx.notify();
                return;
            };
            let Some(letter) = dialog
                .letter
                .clone()
                .filter(|letter| !drive_letter_used(letter, &dialog.drives))
            else {
                dialog.error = Some((Message::NetworkNoLetter, String::new()));
                cx.notify();
                return;
            };
            let username = dialog.values[1].trim().to_string();
            let password = dialog.values[2].clone();
            Some((
                letter,
                path,
                (!username.is_empty()).then_some(username),
                (!password.is_empty()).then_some(password),
            ))
        };
        dialog.busy = true;
        dialog.letter_open = false;
        dialog.error = None;
        self.run_network_operation(
            revision, disconnect, reconnect, paths, request, false, false, cx,
        );
    }

    pub(crate) fn disconnect_network_drive_direct(&mut self, path: &str, cx: &mut Context<Self>) {
        self.context_menus.clear();
        if self.ejecting_drive || self.network_dialog.is_some() {
            cx.notify();
            return;
        }
        let Some(drive) = self.state.drives.iter().find(|drive| {
            network_drive(drive)
                && (crate::ui::components::sidebar::sidebar_paths_equal(&drive.path, path)
                    || path.replace('/', "\\").to_lowercase().starts_with(&format!(
                        "{}\\",
                        drive.path.trim_end_matches(['/', '\\']).to_lowercase()
                    )))
        }) else {
            cx.notify();
            return;
        };
        let paths = vec![drive.path.clone()];
        self.network_dialog_revision = self.network_dialog_revision.wrapping_add(1);
        self.run_network_operation(
            self.network_dialog_revision,
            true,
            false,
            paths,
            None,
            true,
            false,
            cx,
        );
    }

    fn run_network_operation(
        &mut self,
        revision: u64,
        disconnect: bool,
        reconnect: bool,
        paths: Vec<String>,
        request: Option<(String, String, Option<String>, Option<String>)>,
        direct: bool,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        self.context_menus.clear();
        self.ejecting_drive = true;
        self.drive_refresh_epoch = self.drive_refresh_epoch.wrapping_add(1);
        self.drive_details_task = None;
        self.sidebar_follow_task = None;
        self.pending_sidebar_requests.clear();
        self.pending_watchers.clear();
        let watchers = if disconnect {
            std::mem::take(&mut self._path_watchers)
        } else {
            std::collections::HashMap::new()
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                for (watched_path, watcher) in watchers {
                    watcher.release(&watched_path)?;
                }
                if let Some((letter, path, user, password)) = request {
                    oxyde_lib::platform::platform()
                        .map_network_drive(letter, path, reconnect, user, password)
                        .map_err(NetworkOperationError::Other)
                } else {
                    disconnect_safely(&paths, force, |path, force| {
                        oxyde_lib::platform::platform()
                            .disconnect_network_drive(path.to_string(), force)
                    })
                }
            })
            .await;
            let roots =
                tokio::task::spawn_blocking(|| oxyde_lib::platform::platform().volumes().roots())
                    .await;
            let _ = this.update(cx, |this, cx| {
                this.ejecting_drive = false;
                let error = match result {
                    Ok(Ok(())) => None,
                    Ok(Err(error)) => Some(error),
                    Err(error) => Some(NetworkOperationError::Other(error.to_string())),
                };
                if let Ok(roots) = roots {
                    if let Some(dialog) = this.network_dialog.as_mut() {
                        dialog.drives = roots
                            .iter()
                            .map(|root| {
                                dialog
                                    .drives
                                    .iter()
                                    .find(|drive| {
                                        crate::ui::components::sidebar::sidebar_paths_equal(
                                            &drive.path,
                                            &root.path,
                                        )
                                    })
                                    .cloned()
                                    .unwrap_or_else(|| root.clone())
                            })
                            .collect();
                        dialog.drive_focus = dialog
                            .drives
                            .iter()
                            .filter(|drive| network_drive(drive))
                            .map(|_| cx.focus_handle())
                            .collect();
                        dialog.selected.retain(|path| {
                            roots
                                .iter()
                                .any(|drive| network_drive(drive) && drive.path == *path)
                        });
                    }
                    this.apply_drive_roots(roots, cx);
                }
                if direct
                    || this
                        .network_dialog
                        .as_ref()
                        .is_some_and(|dialog| dialog.revision == revision)
                {
                    if let Some(dialog) = this.network_dialog.as_mut() {
                        dialog.busy = false;
                    }
                    if let Some(error) = error {
                        match error {
                            NetworkOperationError::Busy(paths) => {
                                let focus = cx.focus_handle();
                                this.network_force_confirmation = Some(NetworkForceConfirmation {
                                    paths,
                                    revision,
                                    direct,
                                    focus,
                                    confirm_focus: cx.focus_handle(),
                                });
                                this.network_force_focus_pending = true;
                            }
                            NetworkOperationError::Other(error) => {
                                if direct {
                                    this.toast(super::ToastKind::Error, Message::NetworkDriveFailed, error, 0, cx);
                                } else if let Some(dialog) = this.network_dialog.as_mut() {
                                    dialog.error = Some((Message::NetworkDriveFailed, error));
                                }
                            }
                        }
                    } else {
                        this.network_dialog = None;
                        this.network_focus_restore = true;
                        this.toast(super::ToastKind::Success, if disconnect { Message::NetworkDisconnected } else { Message::NetworkConnected }, String::new(), 0, cx);
                    }
                }
                this.request_drive_details(cx);
                this.reconcile_watchers(cx);
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        disconnect_safely, free_letter, mappable_network_path, network_drive, normalized_share,
        NetworkOperationError,
    };
    use oxyde_lib::models::DriveInfo;
    #[test]
    fn only_unc_shares_can_be_mapped() {
        for path in [
            "\\\\server\\share",
            "\\\\server\\share\\folder",
            "\\\\192.168.1.2\\data$",
        ] {
            assert!(mappable_network_path(path));
        }
        for path in [
            "\\\\server",
            "\\\\server\\",
            "C:\\folder",
            "::{device}",
            "\\\\?\\C:\\",
            "__network_vincinity__",
        ] {
            assert!(!mappable_network_path(path));
        }
        assert_eq!(
            normalized_share(" server/share/ "),
            Some("\\\\server\\share".into())
        );
        assert!(normalized_share("server").is_none());
        assert!(normalized_share("C:\\folder").is_none());
        assert!(normalized_share("\\\\server\\share\0hidden").is_none());
    }
    fn drive(path: &str, kind: &str) -> DriveInfo {
        DriveInfo {
            path: path.into(),
            drive_type: kind.into(),
            label: String::new(),
            total_bytes: 0,
            free_bytes: 0,
            media_type: None,
            physical_id: None,
            remote_path: None,
            file_system: None,
            volume_bytes: 0,
        }
    }
    #[test]
    fn letter_selection_excludes_every_occupied_drive_and_handles_exhaustion() {
        assert_eq!(
            free_letter(&[drive("z:\\", "remote"), drive("Y:\\", "fixed")]),
            Some("X:".into())
        );
        let drives: Vec<_> = ('A'..='Z')
            .map(|letter| drive(&format!("{letter}:\\"), "fixed"))
            .collect();
        assert!(free_letter(&drives).is_none());
        assert!(network_drive(&drive("Z:\\", "remote")));
        assert!(network_drive(&drive("Y:\\", "network")));
        assert!(!network_drive(&drive("C:\\", "fixed")));
    }
    #[test]
    fn multi_disconnect_stops_on_failure_and_identifies_the_failed_drive() {
        let paths: Vec<_> = ["Z:\\", "Y:\\", "X:\\"]
            .into_iter()
            .map(String::from)
            .collect();
        let mut calls = Vec::new();
        let result = disconnect_safely(&paths, false, |path, force| {
            assert!(!force);
            calls.push(path.to_string());
            if path == "Y:\\" {
                Err(oxyde_lib::platform::NetworkDisconnectError::Other(
                    "offline".into(),
                ))
            } else {
                Ok(())
            }
        });
        assert_eq!(calls, paths[..2]);
        assert!(
            matches!(result, Err(NetworkOperationError::Other(error)) if error == "Y:\\: offline")
        );
        let mut completed = Vec::new();
        assert!(disconnect_safely(&paths, false, |path, force| {
            assert!(!force);
            completed.push(path.to_string());
            Ok(())
        })
        .is_ok());
        assert_eq!(completed, paths);
    }

    #[test]
    fn busy_connection_pauses_batch_and_only_confirmed_drive_is_forced() {
        let paths = vec!["Z:\\".to_string(), "Y:\\".to_string(), "X:\\".to_string()];
        let result = disconnect_safely(&paths, false, |path, force| {
            assert!(!force);
            if path == "Y:\\" {
                Err(oxyde_lib::platform::NetworkDisconnectError::Busy)
            } else {
                Ok(())
            }
        });
        let Err(NetworkOperationError::Busy(remaining)) = result else {
            panic!("expected confirmation");
        };
        assert_eq!(remaining, paths[1..]);
        let mut calls = Vec::new();
        let result = disconnect_safely(&remaining, true, |path, force| {
            calls.push((path.to_string(), force));
            if path == "X:\\" {
                Err(oxyde_lib::platform::NetworkDisconnectError::Busy)
            } else {
                Ok(())
            }
        });
        assert_eq!(calls, vec![("Y:\\".into(), true), ("X:\\".into(), false)]);
        assert!(matches!(result, Err(NetworkOperationError::Busy(paths)) if paths == vec!["X:\\"]));
    }
}
