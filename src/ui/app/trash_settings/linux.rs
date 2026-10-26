//! Linux trash Properties: content summary, the desktop's own automatic cleanup (GNOME
//! housekeeping or KDE trash limits) and the delete confirmation preference.
use super::*;
use crate::ui::components::input::{InputAppearance, TextInputOptions};
use oxyde_lib::platform::TrashRetention;

mod render;
pub(crate) use render::render;

pub(crate) struct TrashSettings {
    retention: TrashRetention,
    initial: TrashRetention,
    /// KDE trash directory whose limits are shown.
    selected: usize,
    days: Entity<TextInput>,
    percent: Entity<TextInput>,
    days_value: String,
    percent_value: String,
    confirm: bool,
    usage: (u64, u64),
    loading: bool,
    pub(in crate::ui::app) busy: bool,
    error: Option<String>,
    pub(in crate::ui::app) focus: FocusHandle,
    save_focus: FocusHandle,
}

impl TrashSettings {
    pub(in crate::ui::app) fn request_id(&self) -> EntityId { self.days.entity_id() }
}

#[derive(Clone, Copy)]
enum Field {
    Days,
    Percent,
}

fn parse_days(value: &str) -> Option<u32> {
    value.trim().parse().ok().filter(|days| (1..=36_500).contains(days))
}

/// KDE stores a share of the partition; accept a decimal comma as typed in French.
fn parse_percent(value: &str) -> Option<f64> {
    value.trim().replace(',', ".").parse().ok().filter(|percent| (0.1..=100.0).contains(percent))
}

fn format_percent(percent: f64) -> String {
    let text = format!("{percent:.1}");
    text.strip_suffix(".0").map(str::to_string).unwrap_or(text)
}

impl OxydeApp {
    fn trash_number_input(&self, field: Field, window: &mut Window, cx: &mut Context<Self>) -> Entity<TextInput> {
        let h = self.handle.as_ref().unwrap().clone();
        let (change, enter, escape, context) = (h.clone(), h.clone(), h.clone(), h);
        cx.new(|cx| TextInput::new(
            cx,
            "",
            String::new(),
            move |value, _, cx| change.update(cx, |this, cx| {
                if let Some(dialog) = this.trash_settings.as_mut().filter(|dialog| !dialog.busy && !dialog.loading) {
                    match field {
                        Field::Days => dialog.days_value = value.into(),
                        Field::Percent => dialog.percent_value = value.into(),
                    }
                }
                cx.notify();
            }),
            move |_, cx| enter.update(cx, |this, cx| this.save_trash_settings(cx)),
            move |window, cx| escape.update(cx, |this, cx| this.close_trash_settings(window, cx)),
            move |input, selection, position, window, cx| context.update(cx, |this, cx| {
                this.open_text_context_menu(input, selection, position, window, cx)
            }),
            TextInputOptions { appearance: InputAppearance::Bare, ..Default::default() },
            window,
        ))
    }

    pub(crate) fn open_trash_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.trash_settings.is_some() {
            cx.defer(super::super::dialog_windows::activate_trash_window);
            return;
        }
        self.context_menus.clear();
        let days = self.trash_number_input(Field::Days, window, cx);
        let percent = self.trash_number_input(Field::Percent, window, cx);
        self.trash_settings = Some(TrashSettings {
            retention: TrashRetention::Unavailable,
            initial: TrashRetention::Unavailable,
            selected: 0,
            days,
            percent,
            days_value: String::new(),
            percent_value: String::new(),
            confirm: self.config.confirm_delete,
            usage: (0, 0),
            loading: true,
            busy: false,
            error: None,
            focus: cx.focus_handle(),
            save_focus: cx.focus_handle(),
        });
        cx.notify();
        let request = self.trash_settings.as_ref().unwrap().request_id();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(|| {
                let platform = oxyde_lib::platform::platform();
                let usage = platform.recycle_bin_usage();
                // Without desktop settings the summary and confirmation remain usable.
                let retention = platform.trash_retention().unwrap_or_else(|error| {
                    log::warn!("Cannot read the desktop trash settings: {error}");
                    TrashRetention::Unavailable
                });
                (usage, retention)
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                let Some(dialog) = this.trash_settings.as_mut().filter(|dialog| dialog.request_id() == request) else {
                    return;
                };
                dialog.loading = false;
                match result {
                    Ok((usage, retention)) => {
                        match usage {
                            Ok(usage) => dialog.usage = usage,
                            Err(error) => dialog.error = Some(error),
                        }
                        dialog.initial = retention.clone();
                        dialog.retention = retention;
                    }
                    Err(error) => dialog.error = Some(error.to_string()),
                }
                this.show_trash_retention_values(cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn close_trash_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.trash_settings.as_ref().is_some_and(|dialog| dialog.busy) {
            return;
        }
        self.trash_settings = None;
        window.remove_window();
        cx.notify();
    }

    /// Fill the inputs from the GNOME setting or the selected KDE trash directory.
    fn show_trash_retention_values(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = &mut self.trash_settings else { return };
        let (days, percent) = match &dialog.retention {
            TrashRetention::Gnome { days, .. } => (days.to_string(), String::new()),
            TrashRetention::Kde(limits) => limits.get(dialog.selected)
                .map(|limits| (limits.days.to_string(), format_percent(limits.percent)))
                .unwrap_or_default(),
            TrashRetention::Unavailable => (String::new(), String::new()),
        };
        dialog.days_value = days.clone();
        dialog.percent_value = percent.clone();
        dialog.days.update(cx, |input, cx| input.set_content(days, cx));
        dialog.percent.update(cx, |input, cx| input.set_content(percent, cx));
    }

    /// Validate the typed values into the retention being edited.
    fn commit_trash_retention_values(&mut self) -> bool {
        let Some(dialog) = &mut self.trash_settings else { return false };
        let valid = match &mut dialog.retention {
            TrashRetention::Gnome { days, .. } => parse_days(&dialog.days_value).map(|value| *days = value).is_some(),
            TrashRetention::Kde(limits) => match limits.get_mut(dialog.selected) {
                Some(limits) => match (parse_days(&dialog.days_value), parse_percent(&dialog.percent_value)) {
                    (Some(days), Some(percent)) => {
                        limits.days = days;
                        limits.percent = percent;
                        true
                    }
                    _ => false,
                },
                None => true,
            },
            TrashRetention::Unavailable => true,
        };
        dialog.error = (!valid).then(String::new);
        valid
    }

    pub(crate) fn select_trash_location(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.trash_settings.as_ref().is_some_and(|dialog| dialog.busy || dialog.loading || dialog.selected == index)
            || !self.commit_trash_retention_values()
        {
            cx.notify();
            return;
        }
        if let Some(dialog) = &mut self.trash_settings {
            dialog.selected = index;
        }
        self.show_trash_retention_values(cx);
        cx.notify();
    }

    fn edit_trash_retention(&mut self, edit: impl FnOnce(&mut TrashRetention, usize), cx: &mut Context<Self>) {
        if let Some(dialog) = self.trash_settings.as_mut().filter(|dialog| !dialog.busy && !dialog.loading) {
            edit(&mut dialog.retention, dialog.selected);
        }
        cx.notify();
    }

    pub(crate) fn save_trash_settings(&mut self, cx: &mut Context<Self>) {
        if self.preferences_load_failed {
            if let Some(dialog) = &mut self.trash_settings {
                dialog.error = Some(String::new());
            }
            cx.notify();
            return;
        }
        if self.trash_settings.as_ref().is_none_or(|dialog| dialog.loading || dialog.busy)
            || !self.commit_trash_retention_values()
        {
            cx.notify();
            return;
        }
        let dialog = self.trash_settings.as_mut().unwrap();
        let changed = (dialog.retention != dialog.initial).then(|| dialog.retention.clone());
        let confirm = dialog.confirm;
        dialog.busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(move || match changed {
                Some(retention) => oxyde_lib::platform::platform().set_trash_retention(&retention),
                None => Ok(()),
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(())) => {
                        this.config.confirm_delete = confirm;
                        this.save_preferences(cx);
                        this.trash_settings = None;
                    }
                    result => {
                        if let Some(dialog) = &mut this.trash_settings {
                            dialog.busy = false;
                            dialog.error = Some(match result {
                                Ok(Err(error)) => error,
                                Err(error) => error.to_string(),
                                Ok(Ok(())) => unreachable!(),
                            });
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn empty_trash_from_settings(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = &mut self.trash_settings else { return };
        if dialog.busy || dialog.loading || dialog.usage.1 == 0 {
            return;
        }
        dialog.busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = tokio::task::spawn_blocking(|| {
                tokio::runtime::Handle::current().block_on(async {
                    oxyde_lib::commands::ops::empty_trash().await?;
                    oxyde_lib::commands::ops::get_total_recycle_bin_usage().await
                })
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(dialog) = &mut this.trash_settings {
                    dialog.busy = false;
                    match result {
                        Ok(Ok(usage)) => dialog.usage = usage,
                        Ok(Err(error)) => dialog.error = Some(error.to_string()),
                        Err(error) => dialog.error = Some(error.to_string()),
                    }
                }
                this.refresh_all_tabs(cx);
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
#[path = "linux/tests/values.rs"]
mod tests;
