use super::*;

impl OxydeApp {
    /// A chevron's press: lists the folder before it, or closes its open or pending menu.
    pub(crate) fn toggle_breadcrumb_menu(&mut self, pane: String, path: String, index: usize, cx: &mut Context<Self>) {
        let request = (pane, path, index);
        let open = self.breadcrumb_menu.as_ref()
            .is_some_and(|(pane, path, current, _)| *pane == request.0 && *path == request.1 && *current == request.2);
        if open || self.breadcrumb_menu_request.as_ref() == Some(&request) {
            self.breadcrumb_menu = None;
            self.breadcrumb_menu_request = None;
            cx.notify();
            return;
        }
        self.breadcrumb_menu_request = Some(request.clone());
        let (show_hidden, show_system) = (self.show_hidden, self.show_system);
        cx.spawn(async move |this, cx| {
            let listed = oxyde_lib::commands::io::list_dir(request.1.clone(), Some(show_hidden), Some(show_system)).await;
            let _ = this.update(cx, |this, cx| {
                // A later press replaced or cancelled this request.
                if this.breadcrumb_menu_request.as_ref() != Some(&request) {
                    return;
                }
                this.breadcrumb_menu_request = None;
                match listed {
                    Ok(entries) => {
                        let (pane, path, index) = request;
                        let folders = entries.into_iter().filter(|entry| entry.is_dir).collect();
                        this.breadcrumb_menu = Some((pane, path, index, folders));
                        // The handle is shared by every chevron: each menu opens at its top.
                        this.breadcrumb_scroll = UniformListScrollHandle::new();
                    }
                    // A press that shows nothing says why.
                    Err(error) => {
                        log::warn!("Cannot list {} for its chevron: {error:?}", request.1);
                        let message = crate::ui::errors::path_load_message(&error);
                        this.toast(super::super::ToastKind::Error, message, String::new(), 0, cx);
                    }
                }
                cx.notify();
            });
        }).detach();
    }

    pub(crate) fn begin_breadcrumb_edit(
        &mut self,
        pane_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.inline_rename || self.editing_path.is_some() {
            return;
        }
        let Some(path) = self
            .state
            .layout
            .find_pane(&pane_id)
            .map(|pane| pane.active_tab().path.clone())
        else {
            return;
        };
        if path == "trash://" {
            return;
        }
        let Some(handle) = self.handle.clone() else {
            return;
        };
        self.invalidate_breadcrumb_validation();
        self.breadcrumb_editing = true;
        // A name that is not UTF-8 is edited with U+FFFD.
        let path = crate::ui::components::input::native_path_text(&oxyde_lib::utils::path_text::display(&path));
        self.breadcrumb_text = path.clone();
        self.breadcrumb_invalid = false;
        self.breadcrumb_pane_id = Some(pane_id);
        let input = cx.new(|cx| {
            TextInput::new(
                cx,
                cx.i18n().text(Message::PathPlaceholder).to_string(),
                path.clone(),
                {
                    let handle = handle.clone();
                    move |text, _, cx| {
                        handle.update(cx, |this, cx| {
                            this.breadcrumb_text = text.to_string();
                            this.invalidate_breadcrumb_validation();
                            this.breadcrumb_invalid = false;
                            cx.notify();
                        })
                    }
                },
                {
                    let handle = handle.clone();
                    move |_, cx| handle.update(cx, |this, cx| this.submit_breadcrumb_edit(cx))
                },
                {
                    let handle = handle.clone();
                    move |_, cx| handle.update(cx, |this, cx| this.cancel_breadcrumb_edit(cx))
                },
                {
                    let handle = handle.clone();
                    move |input, has_selection, position, window, cx| {
                        handle.update(cx, |this, cx| {
                            this.open_text_context_menu(input, has_selection, position, window, cx)
                        });
                    }
                },
                TextInputOptions {
                    appearance: InputAppearance::Inline,
                    submit_on_outside: true,
                    initial_selection: Some(0..path.len()),
                    ..Default::default()
                },
                window,
            )
            .with_path_submit(self.show_hidden, self.show_system)
        });
        let focus = input.read(cx).focus_handle.clone();
        self.editing_view = Some(input);
        window.focus(&focus, cx);
        cx.on_next_frame(window, move |this, window, cx| {
            if this.breadcrumb_editing {
                window.focus(&focus, cx);
            }
        });
        cx.notify();
    }

    pub(crate) fn submit_breadcrumb_edit(&mut self, cx: &mut Context<Self>) {
        if !self.breadcrumb_editing || !self.context_menus.is_empty() {
            return;
        }
        // The same text is being checked already: a repeated Enter starts no other check.
        if self.breadcrumb_validation_task.is_some() {
            return;
        }
        self.invalidate_breadcrumb_validation();
        let Some(owner) = self.breadcrumb_owner() else {
            return;
        };
        let Some(handle) = self.handle.clone() else {
            return;
        };
        let typed = crate::ui::components::input::navigation_path_text(&owner.text);
        // A name relative to the folder shown, not to Oxyde's own working folder.
        let path = resolved_path(&typed, &owner.location);
        let timer = cx.background_executor().timer(VALIDATION_TIMEOUT);
        self.breadcrumb_validation_task = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let Some(path) = path else {
                    handle.update(&mut cx, |this, cx| {
                        if this.breadcrumb_owner().as_ref() == Some(&owner) {
                            this.breadcrumb_validation_task = None;
                            this.reject_breadcrumb_text(cx);
                        }
                    });
                    return;
                };
                let check = path.clone();
                let validation = validate_path_in_background(check, |path| std::path::Path::new(path).is_dir());
                // A share that does not answer: the field says no rather than waiting.
                let valid = match futures::future::select(Box::pin(validation), timer).await {
                    futures::future::Either::Left((valid, _)) => valid,
                    futures::future::Either::Right(_) => false,
                };
                handle.update(&mut cx, |this, cx| {
                    if this.breadcrumb_owner().as_ref() != Some(&owner) {
                        return;
                    }
                    this.breadcrumb_validation_task = None;
                    if !valid {
                        this.reject_breadcrumb_text(cx);
                    } else {
                        this.breadcrumb_pane_id = None;
                        this.breadcrumb_editing = false;
                        this.breadcrumb_invalid = false;
                        this.editing_view = None;
                        if !path.is_empty() {
                            this.navigate_to(&owner.pane, path, cx);
                        }
                    }
                    cx.notify();
                });
            }
        }));
    }

    fn reject_breadcrumb_text(&mut self, cx: &mut Context<Self>) {
        self.breadcrumb_shake_id = self.breadcrumb_shake_id.wrapping_add(1);
        self.breadcrumb_invalid = true;
        if let Some(input) = self.editing_view.clone() {
            input.update(cx, |input, cx| input.mark_invalid(cx));
        }
        cx.notify();
    }

    pub(crate) fn cancel_breadcrumb_edit(&mut self, cx: &mut Context<Self>) {
        self.invalidate_breadcrumb_validation();
        self.breadcrumb_editing = false;
        self.breadcrumb_invalid = false;
        self.breadcrumb_pane_id = None;
        self.editing_view = None;
        self.context_menus.clear();
        cx.notify();
    }

    fn invalidate_breadcrumb_validation(&mut self) {
        self.breadcrumb_validation_revision = self.breadcrumb_validation_revision.wrapping_add(1);
        self.breadcrumb_validation_task = None;
    }
    fn breadcrumb_owner(&self) -> Option<Owner> {
        if !self.breadcrumb_editing {
            return None;
        }
        let pane = self.breadcrumb_pane_id.as_ref()?;
        let tab = self.state.find_pane(pane)?.active_tab();
        Some(Owner {
            revision: self.breadcrumb_validation_revision,
            pane: pane.clone(),
            tab: tab.id.clone(),
            location: tab.path.clone(),
            text: self.breadcrumb_text.clone(),
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Owner {
    revision: u64,
    pane: String,
    tab: String,
    location: String,
    text: String,
}
/// How long a typed path may take to be found before the field refuses it.
const VALIDATION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// The path `typed` names from the folder shown at `location`: special locations and
/// absolute paths as they are, a bare drive at its root, a relative name under
/// `location` without `.` or `..`. None when `location` is no folder to start from.
fn resolved_path(typed: &str, location: &str) -> Option<String> {
    use std::path::{Component, Path, PathBuf};
    let special = typed.is_empty()
        || typed == "__network_vincinity__"
        || typed.contains("://")
        || typed.starts_with(r"\\")
        || typed.starts_with("//");
    if special {
        return Some(typed.to_owned());
    }
    let path = Path::new(typed);
    if path.is_absolute() {
        return Some(typed.to_owned());
    }
    let base = Path::new(location);
    let joined = match path.components().next() {
        // `D:` or `D:name`: from that drive's root, never from its current folder.
        Some(Component::Prefix(_)) => PathBuf::from(typed),
        _ if base.is_absolute() && !location.contains("://") => base.join(path),
        _ => return None,
    };
    let mut resolved = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::Prefix(prefix) => {
                resolved.push(prefix.as_os_str());
                resolved.push(std::path::MAIN_SEPARATOR_STR);
            }
            Component::RootDir => resolved.push(std::path::MAIN_SEPARATOR_STR),
            Component::Normal(name) => resolved.push(name),
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => {}
        }
    }
    Some(resolved.to_string_lossy().into_owned())
}

async fn validate_path_in_background(
    path: String,
    is_dir: impl FnOnce(&str) -> bool + Send + 'static,
) -> bool {
    tokio::task::spawn_blocking(move || {
        path == "__network_vincinity__"
            || oxyde_lib::commands::io::is_network_server(&path)
            || oxyde_lib::platform::platform().is_remote_uri(&path)
            || is_dir(&path)
    })
    .await
    .unwrap_or(false)
}
#[cfg(test)]
#[path = "breadcrumb/tests/validation.rs"]
mod tests;
