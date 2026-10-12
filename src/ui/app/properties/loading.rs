use super::PropertiesWindow;
use gpui::*;
impl PropertiesWindow {
    pub(super) fn load(&mut self, path: String, cx: &mut Context<Self>) {
        let revision = 0;
        let cancellation = self.cancellation.clone();
        self._calculation = Some(cx.spawn(async move |this, cx| {
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
                let result =
                    oxyde_lib::commands::io::calculate_folder_size_cancellable(path, cancellation)
                        .await;
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
        }));
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
        window.remove_window();
        cx.notify();
    }

    pub(super) fn system_file_properties(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &self.properties_dialog else {
            return;
        };
        if dialog.saving {
            return;
        }
        let path = dialog.path.clone();
        self.close_file_properties(window, cx);
        cx.spawn(async move |_, _| {
            let _ = tokio::task::spawn_blocking(move || {
                oxyde_lib::platform::platform().show_properties_dialog(&path)
            })
            .await;
        })
        .detach();
    }
}
