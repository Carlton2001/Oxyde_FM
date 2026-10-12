use super::PropertiesWindow;
use crate::ui::components::input::TextInput;
use crate::ui::i18n::Message;
use gpui::*;

impl PropertiesWindow {
    pub(super) fn select_properties_tab(
        &mut self,
        shortcut: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(dialog) = &self.properties_dialog else {
            return;
        };
        if dialog.saving {
            return;
        }
        let values = dialog
            .properties
            .as_ref()
            .and_then(|properties| properties.shortcut.as_ref())
            .map(|info| {
                (
                    vec![
                        info.target.clone(),
                        info.arguments.clone(),
                        info.working_dir.clone(),
                        info.description.clone(),
                    ],
                    info.run_window,
                )
            });
        if shortcut && dialog.shortcut_inputs.is_empty() {
            let Some((values, run_window)) = values else {
                return;
            };
            let handle = cx.entity();
            let mut inputs = Vec::new();
            for value in values {
                let submit = handle.clone();
                let cancel = handle.clone();
                let context = handle.clone();
                inputs.push(cx.new(|cx| {
                    TextInput::new(
                        cx,
                        "",
                        value,
                        |_, _, _| {},
                        move |window, cx| {
                            submit.update(cx, |this, cx| this.accept_properties(window, cx))
                        },
                        move |window, cx| {
                            cancel.update(cx, |this, cx| this.close_file_properties(window, cx))
                        },
                        move |input, selected, point, window, cx| {
                            context.update(cx, |this, cx| {
                                this.open_text_context_menu(input, selected, point, window, cx)
                            })
                        },
                        false,
                        false,
                        false,
                        None,
                        window,
                    )
                }));
            }
            let dialog = self.properties_dialog.as_mut().unwrap();
            dialog.shortcut_inputs = inputs;
            dialog.run_window = run_window;
        }
        let dialog = self.properties_dialog.as_mut().unwrap();
        dialog.shortcut_tab = shortcut;
        dialog.run_menu = None;
        window.focus(&dialog.focus, cx);
        cx.notify();
    }

    pub(super) fn accept_properties(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &self.properties_dialog else {
            return;
        };
        if dialog.saving {
            return;
        }
        if dialog.shortcut_inputs.is_empty() {
            self.close_file_properties(window, cx);
            return;
        }
        let Some(mut info) = dialog
            .properties
            .as_ref()
            .and_then(|properties| properties.shortcut.clone())
        else {
            return;
        };
        let fields: Vec<_> = dialog
            .shortcut_inputs
            .iter()
            .map(|input| input.read(cx).content().to_string())
            .collect();
        if fields[0].trim().is_empty() || fields.iter().any(|value| value.contains('\0')) {
            self.properties_dialog.as_mut().unwrap().validation_error =
                Some(Message::ShortcutInvalidTarget);
            cx.notify();
            return;
        }
        info.target = fields[0].clone();
        info.arguments = fields[1].clone();
        info.working_dir = fields[2].clone();
        info.description = fields[3].clone();
        info.run_window = dialog.run_window;
        let original = dialog
            .properties
            .as_ref()
            .unwrap()
            .shortcut
            .as_ref()
            .unwrap();
        if info.target == original.target
            && info.arguments == original.arguments
            && info.working_dir == original.working_dir
            && info.description == original.description
            && info.run_window == original.run_window
        {
            self.close_file_properties(window, cx);
            return;
        }
        let path = dialog.path.clone();
        let revision = dialog.revision;
        let dialog = self.properties_dialog.as_mut().unwrap();
        dialog.saving = true;
        dialog.error = None;
        dialog.validation_error = None;
        dialog.run_menu = None;
        window.focus(&dialog.ok_focus, cx);
        cx.notify();
        let window_handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let result = oxyde_lib::commands::io::set_shortcut_info(path, info).await;
            let _ = window_handle.update(cx, |_, window, cx| {
                this.update(cx, |this, cx| {
                    let Some(dialog) = this
                        .properties_dialog
                        .as_mut()
                        .filter(|dialog| dialog.revision == revision)
                    else {
                        return;
                    };
                    dialog.saving = false;
                    match result {
                        Ok(()) => {
                            this.close_file_properties(window, cx);
                            if let Some(owner) = this.owner.upgrade() {
                                owner.update(cx, |owner, cx| owner.refresh_all(cx));
                            }
                        }
                        Err(error) => dialog.error = Some(error.to_string()),
                    }
                    cx.notify();
                })
            });
        })
        .detach();
    }
}
