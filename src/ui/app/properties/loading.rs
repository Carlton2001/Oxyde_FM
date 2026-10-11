use super::super::*;
use super::PropertiesDialog;

fn same_root(a: &str, b: &str) -> bool {
    a.trim_end_matches(['\\', '/'])
        .eq_ignore_ascii_case(b.trim_end_matches(['\\', '/']))
}

impl OxydeApp {
    pub(crate) fn open_file_properties(
        &mut self,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if path == "trash://" {
            self.open_trash_settings(window, cx);
            return;
        }
        self.context_menus.clear();
        if path == "__network_vincinity__" {
            cx.spawn(async move |_, _| {
                let _ = tokio::task::spawn_blocking(move || {
                    oxyde_lib::platform::platform().show_properties_dialog(&path)
                })
                .await;
            })
            .detach();
            cx.notify();
            return;
        }
        self.properties_revision += 1;
        let revision = self.properties_revision;
        let drive = self
            .state
            .drives
            .iter()
            .find(|drive| same_root(&drive.path, &path))
            .cloned();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.properties_dialog = Some(PropertiesDialog {
            path: path.clone(),
            revision,
            properties: None,
            drive,
            calculating: false,
            error: None,
            focus,
            ok_focus: cx.focus_handle(),
            shortcut_tab: false,
            shortcut_inputs: Vec::new(),
            run_window: 1,
            run_menu: None,
            run_focus: cx.focus_handle(),
            run_bounds: None,
            saving: false,
            validation_error: None,
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || {
                oxyde_lib::commands::io::get_file_properties(path)
            })
            .await;
            let scan = this
                .update(cx, |this, cx| {
                    let dialog = this
                        .properties_dialog
                        .as_mut()
                        .filter(|dialog| dialog.revision == revision)?;
                    let scan = match result {
                        Ok(Ok(properties)) => {
                            let scan = (properties.is_dir && dialog.drive.is_none())
                                .then(|| properties.path.clone());
                            dialog.calculating = scan.is_some();
                            dialog.properties = Some(properties);
                            scan
                        }
                        Ok(Err(error)) => {
                            dialog.error = Some(error.to_string());
                            None
                        }
                        Err(error) => {
                            dialog.error = Some(error.to_string());
                            None
                        }
                    };
                    cx.notify();
                    scan
                })
                .ok()
                .flatten();
            if let Some(path) = scan {
                let result = oxyde_lib::commands::io::calculate_folder_size(path).await;
                let _ = this.update(cx, |this, cx| {
                    let Some(dialog) = this
                        .properties_dialog
                        .as_mut()
                        .filter(|dialog| dialog.revision == revision)
                    else {
                        return;
                    };
                    dialog.calculating = false;
                    match result {
                        Ok(size) => {
                            if let Some(properties) = &mut dialog.properties {
                                properties.size = size.size;
                                properties.files_count = Some(size.files_count);
                                properties.folders_count = Some(size.folders_count);
                                properties.is_calculated = true;
                            }
                        }
                        Err(error) => dialog.error = Some(error.to_string()),
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn close_file_properties(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .properties_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.saving)
        {
            return;
        }
        self.properties_dialog = None;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    pub(super) fn system_file_properties(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = &self.properties_dialog else {
            return;
        };
        let path = dialog.path.clone();
        cx.spawn(async move |_, _| {
            let _ = tokio::task::spawn_blocking(move || {
                oxyde_lib::platform::platform().show_properties_dialog(&path)
            })
            .await;
        })
        .detach();
    }
}
