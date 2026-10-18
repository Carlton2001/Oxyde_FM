use super::*;

impl OxydeApp {
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
        let path = crate::ui::components::input::native_path_text(&path);
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
        self.invalidate_breadcrumb_validation();
        let Some(owner) = self.breadcrumb_owner() else {
            return;
        };
        let Some(handle) = self.handle.clone() else {
            return;
        };
        let path = crate::ui::components::input::navigation_path_text(&owner.text);
        self.breadcrumb_validation_task = Some(cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let check = path.clone();
                let valid =
                    validate_path_in_background(check, |path| std::path::Path::new(path).is_dir())
                        .await;
                handle.update(&mut cx, |this, cx| {
                    if this.breadcrumb_owner().as_ref() != Some(&owner) {
                        return;
                    }
                    this.breadcrumb_validation_task = None;
                    if !valid {
                        this.breadcrumb_shake_id = this.breadcrumb_shake_id.wrapping_add(1);
                        this.breadcrumb_invalid = true;
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
async fn validate_path_in_background(
    path: String,
    is_dir: impl FnOnce(&str) -> bool + Send + 'static,
) -> bool {
    tokio::task::spawn_blocking(move || {
        path == "__network_vincinity__"
            || (cfg!(target_os = "windows") && oxyde_lib::commands::io::is_network_server(&path))
            || is_dir(&path)
    })
    .await
    .unwrap_or(false)
}
#[cfg(test)]
#[path = "breadcrumb/tests/validation.rs"]
mod tests;
