use super::OxydeApp;
use crate::ui::i18n::{I18n, I18nContext, Language, Message};
use crate::ui::theme::{OxydeTheme, ThemeId};
use gpui::{Context, Window};
use oxyde_lib::models::{AppConfig, ConfigManager};
use std::sync::Mutex;

/// Generation of the last snapshot on disk; an older one is never written over it.
static WRITTEN_GENERATION: Mutex<u64> = Mutex::new(0);

fn write_snapshot(path: &std::path::Path, snapshot: &AppConfig, generation: u64) -> Result<(), oxyde_lib::models::CommandError> {
    let mut written = WRITTEN_GENERATION.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if *written >= generation {
        return Ok(());
    }
    ConfigManager::write_to_path(path, snapshot)?;
    *written = generation;
    Ok(())
}

impl OxydeApp {
    pub(crate) fn default_turbo_mode(&self) -> bool { self.config.default_turbo_mode }

    pub(crate) fn toggle_default_turbo(&mut self, cx: &mut Context<Self>) {
        self.config.default_turbo_mode = !self.config.default_turbo_mode;
        oxyde_lib::commands::io::set_folder_size_turbo(self.config.default_turbo_mode);
        self.save_preferences(cx);
    }

    pub(crate) fn search_limit(&self) -> u32 {
        oxyde_lib::models::preferences::normalized_search_limit(self.config.search_limit)
    }

    pub(crate) fn reset_search_limit(&mut self, cx: &mut Context<Self>) {
        self.config.search_limit = oxyde_lib::models::AppConfig::default().search_limit;
        self.save_preferences(cx);
        cx.notify();
    }

    pub(crate) fn step_search_limit(&mut self, increase: bool, cx: &mut Context<Self>) {
        self.config.search_limit = oxyde_lib::models::preferences::stepped_search_limit(
            self.config.search_limit, increase,
        );
        self.save_preferences(cx);
    }

    pub(crate) fn open_settings_confirmation(
        &mut self,
        action: super::SettingsDataAction,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if action == super::SettingsDataAction::ClearCache && self.clearing_cache {
            return;
        }
        self.finish_pointer_drag(cx);
        self.settings_open = false;
        self.settings_picker = None;
        self.settings_submenu_bounds = None;
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.settings_confirmation = Some(super::SettingsDataConfirmation {
            action,
            focus,
            confirm_focus: cx.focus_handle(),
        });
        cx.notify();
    }

    pub(crate) fn resolve_settings_confirmation(
        &mut self,
        confirm: bool,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let Some(confirmation) = self.settings_confirmation.take() else { return; };
        self.settings_open = true;
        window.focus(&self.focus_handle, cx);
        if confirm {
            match confirmation.action {
                super::SettingsDataAction::ClearCache => self.clear_cache(cx),
                super::SettingsDataAction::ResetPreferences => self.reset_preferences(window, cx),
            }
        }
        cx.notify();
    }

    pub(crate) fn clear_cache(&mut self, cx: &mut Context<Self>) {
        if self.clearing_cache { return; }
        self.clearing_cache = true;
        crate::ui::audio_covers::clear();
        self.settings_data_status = Some(Message::SettingsCacheClearing);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async {
                oxyde_lib::commands::system::clear_app_cache()
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.clearing_cache = false;
                this.settings_data_status = None;
                this.toast(if result.is_ok() { super::ToastKind::Success } else { super::ToastKind::Error }, if result.is_ok() { Message::SettingsCacheCleared } else { Message::SettingsCacheError }, String::new(), 0, cx);
                if let Err(error) = result { log::warn!("Cache cleanup failed: {error}"); }
                crate::ui::components::file_list::clear_grid_thumbnails(cx);
                this.state.layout.visit_panes(&mut |pane| {
                    for tab in &pane.tabs {
                        for file in tab.files.iter() {
                            crate::ui::components::icons::invalidate_file_icon(&file.path, cx);
                            crate::ui::components::file_list::invalidate_thumbnail(&file.path, cx);
                        }
                    }
                });
                cx.refresh_windows();
                cx.notify();
            });
        }).detach();
    }

    pub(crate) fn reset_preferences(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        if self.preferences_load_failed {
            self.settings_error = Some(Message::SettingsLoadError);
            cx.notify();
            return;
        }
        self.config.reset_preferences();
        oxyde_lib::commands::io::set_folder_size_turbo(self.config.default_turbo_mode);
        self.refresh_quick_access(cx);
        self.show_hidden = self.config.show_hidden;
        self.show_system = self.config.show_system;
        self.show_media_previews = self.config.show_previews;
        self.show_grid_thumbnails = self.config.show_grid_thumbnails;
        self.sidebar_follow_path = None;
        self.sidebar_follow_task = None;
        self.finish_pointer_drag(cx);
        self.state.layout.visit_panes_mut(&mut |pane| {
            for tab in &mut pane.tabs {
                tab.group_mode = crate::ui::date_groups::GroupMode::None;
                tab.collapsed_groups.clear();
            }
        });
        self.set_theme(ThemeId::default(), cx);
        self.set_language(Language::default(), cx);
        cx.set_global(crate::ui::format::DateFormatPreference(crate::ui::format::DateFormat::default()));
        crate::ui::scale::set_reference(self.config.ui_rem_size);
        window.set_rem_size(gpui::px(self.config.ui_rem_size as f32));
        self.context_menus.clear();
        self.breadcrumb_menu = None;
        self.refresh_all_tabs(cx);
        self.save_workspace(cx);
        self.settings_data_status = None;
        self.toast(super::ToastKind::Success, Message::SettingsPreferencesReset, String::new(), 0, cx);
        // Resolve the regional default away from the UI thread.
        cx.spawn(async move |this, cx| {
            let format = cx.background_executor().spawn(async {
                crate::ui::format::DateFormat::for_locale(
                    &oxyde_lib::platform::platform().regional_locale().unwrap_or_default())
            }).await;
            let _ = this.update(cx, |this, cx| {
                if this.config.date_format.is_empty() {
                    this.set_date_format(format, cx);
                }
            });
        }).detach();
        cx.refresh_windows();
        cx.notify();
    }

    pub(crate) fn follows_active_path(&self) -> bool { self.config.sync_sidebar_with_path }
    pub(crate) fn toggle_path_tracking(&mut self, cx: &mut Context<Self>) {
        self.config.sync_sidebar_with_path = !self.config.sync_sidebar_with_path;
        self.sidebar_follow_path = None;
        self.sidebar_follow_task = None;
        self.save_preferences(cx);
    }
    pub(crate) fn show_checkboxes(&self) -> bool { self.config.show_checkboxes }
    pub(crate) fn toggle_checkboxes(&mut self, cx: &mut Context<Self>) {
        self.config.show_checkboxes = !self.config.show_checkboxes;
        self.save_preferences(cx);
    }
    pub(crate) fn use_system_icons(&self) -> bool { self.config.use_system_icons }
    pub(crate) fn toggle_system_icons(&mut self, cx: &mut Context<Self>) {
        self.config.use_system_icons = !self.config.use_system_icons;
        self.save_preferences(cx);
    }

    pub(crate) fn ui_rem_size(&self) -> u32 {
        crate::ui::scale::normalized_reference(self.config.ui_rem_size)
    }
    pub(crate) fn set_ui_rem_size(
        &mut self,
        value: u32,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let value = crate::ui::scale::normalized_reference(value);
        if self.config.ui_rem_size == value {
            return;
        }
        self.config.ui_rem_size = value;
        crate::ui::scale::set_reference(value);
        window.set_rem_size(gpui::px(value as f32));
        // Pointer drag snapshots belong to the previous layout geometry.
        self.finish_pointer_drag(cx);
        self.context_menus.clear();
        self.breadcrumb_menu = None;
        self.save_preferences(cx);
        cx.refresh_windows();
    }
    pub(crate) fn save_workspace(&mut self, cx: &mut Context<Self>) {
        if self.preferences_load_failed {
            return;
        }
        let mut snapshot = self.state.saved_snapshot(self.sidebar_width);
        snapshot.sidebar_collapsed = self.sidebar_collapsed;
        match serde_json::to_value(snapshot) {
            Ok(snapshot) => {
                self.config.workspace_state = Some(snapshot);
                self.save_preferences(cx);
            }
            Err(error) => log::warn!("Cannot serialize workspace state: {error:?}"),
        }
    }

    pub(crate) fn compression_quality(&self, format: &str) -> &str {
        match format {
            "7z" => &self.config.seven_zip_quality,
            "zst" => &self.config.zstd_quality,
            _ => &self.config.zip_quality,
        }
    }
    pub(crate) fn set_compression_quality(
        &mut self,
        format: &str,
        quality: &str,
        cx: &mut Context<Self>,
    ) {
        let field = match format {
            "7z" => &mut self.config.seven_zip_quality,
            "zst" => &mut self.config.zstd_quality,
            _ => &mut self.config.zip_quality,
        };
        *field = quality.into();
        self.save_preferences(cx);
        cx.notify();
    }

    pub(crate) fn set_theme(&mut self, theme: ThemeId, cx: &mut Context<Self>) {
        if self.theme_id == theme {
            return;
        }
        self.theme_id = theme;
        cx.set_global(OxydeTheme::for_id(theme));
        cx.refresh_windows();
        self.save_preferences(cx);
    }

    pub(crate) fn set_date_format(&mut self, format: crate::ui::format::DateFormat, cx: &mut Context<Self>) {
        self.config.date_format = format.as_str().into();
        cx.set_global(crate::ui::format::DateFormatPreference(format));
        self.save_preferences(cx);
        cx.refresh_windows();
    }
    pub(crate) fn toggle_network(&mut self, cx: &mut Context<Self>) {
        self.config.show_network = !self.config.show_network;
        self.save_preferences(cx);
    }
    pub(crate) fn toggle_onedrive(&mut self, cx: &mut Context<Self>) {
        self.config.show_onedrive = !self.config.show_onedrive;
        self.save_preferences(cx);
        if !self.config.show_onedrive {
            self.state.onedrive_items.clear();
            self.state.refresh_sidebar();
        }
        self.refresh_quick_access(cx);
    }
    pub(crate) fn show_onedrive(&self) -> bool { self.config.show_onedrive }

    pub(crate) fn show_network(&self) -> bool { self.config.show_network }

    pub(crate) fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        if self.language == language {
            return;
        }
        self.language = language;
        cx.set_global(I18n::for_language(language));
        if self.editing_path.is_some() || self.breadcrumb_editing {
            if let Some(input) = self.editing_view.clone() {
                let placeholder = cx.i18n().text(if self.breadcrumb_editing {
                    Message::PathPlaceholder
                } else {
                    Message::RenameNamePlaceholder
                });
                input.update(cx, |input, cx| input.set_placeholder(placeholder, cx));
            }
        }
        // Menus contain snapshots of labels; recreate them on the next opening.
        let placeholder = cx.i18n().text(Message::QuickSearch);
        self.state.layout.visit_panes(&mut |pane| {
            for tab in &pane.tabs {
                if let Some(input) = &tab.search_input {
                    input.update(cx, |input, cx| input.set_placeholder(placeholder.clone(), cx));
                }
            }
        });
        self.context_menus.clear();
        cx.refresh_windows();
        self.save_preferences(cx);
    }

    pub(crate) fn save_preferences(&mut self, cx: &mut Context<Self>) {
        if self.preferences_load_failed {
            self.settings_error = Some(Message::SettingsLoadError);
            cx.notify();
            return;
        }
        let Some(path) = self.config_path.clone() else {
            return;
        };
        self.config.theme = self.theme_id.as_str().into();
        self.config.language = self.language.as_str().into();
        self.config.show_hidden = self.show_hidden;
        self.config.show_system = self.show_system;
        self.config.show_previews = self.show_media_previews;
        self.config.show_grid_thumbnails = self.show_grid_thumbnails;
        let snapshot = self.config.clone();
        self.settings_error = None;
        self.preferences_save_generation += 1;
        let generation = self.preferences_save_generation;
        log::info!("preferences save queued; generation={generation}");
        // Chaining guarantees that an older write can never overwrite a newer
        // choice. on_app_quit also awaits the tail of this chain.
        let previous = self.preferences_save_task.take();
        self.preferences_save_task = Some(cx.spawn(async move |this, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            log::info!("preferences write started; generation={generation}");
            let result = cx
                .background_executor()
                .spawn(async move { write_snapshot(&path, &snapshot, generation) })
                .await;
            log::info!("preferences write finished; generation={generation}; success={}", result.is_ok());
            if let Err(error) = &result {
                log::warn!("Cannot save preferences: {error:?}");
            }
            let _ = this.update(cx, |this, cx| {
                if this.preferences_save_generation == generation {
                    this.settings_error = result.err().map(|_| Message::SettingsSaveError);
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }

    /// Takes over the queued save chain at quit; see `shutdown::begin_preferences_write`.
    pub(crate) fn final_preferences_write(&mut self) -> Option<super::shutdown::Cleanup> {
        // Dropping the chain cancels it; an older write still running is ordered by generation.
        drop(self.preferences_save_task.take()?);
        let path = self.config_path.clone()?;
        let snapshot = self.config.clone();
        let generation = self.preferences_save_generation;
        Some(super::shutdown::begin_preferences_write(move || {
            let result = write_snapshot(&path, &snapshot, generation);
            log::info!("final preferences write finished; generation={generation}; success={}", result.is_ok());
            if let Err(error) = result {
                log::warn!("Cannot save preferences: {error:?}");
            }
        }))
    }

    pub(crate) fn flush_window_geometry(&mut self, cx: &mut Context<Self>) {
        if self.window_geometry_save_task.take().is_some() {
            self.save_preferences(cx);
        }
    }

    pub(crate) fn observe_main_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_main_window(window.window_handle());
        #[cfg(target_os = "windows")]
        {
            let activation = cx.observe_window_activation(window, |app, window, cx| app.promote_file_drag(window, cx));
            self._subscriptions.push(activation);
        }
        let subscription = cx.observe_window_bounds(window, |this, window, cx| {
            if let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) {
                if let Err(error) = oxyde_lib::platform::platform().follow_fixed_dialog_owner(handle.as_raw()) {
                    log::debug!("Cannot recenter fixed dialogs: {error}");
                }
            }
            let Some(placement) = crate::ui::window_geometry::capture(window) else { return; };
            if this.config.window_placement == Some(placement) { return; }
            this.config.window_placement = Some(placement);
            this.window_geometry_save_task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(std::time::Duration::from_millis(350)).await;
                let _ = this.update(cx, |this, cx| this.save_preferences(cx));
            }));
        });
        self._subscriptions.push(subscription);
    }
}

#[cfg(test)]
#[path = "preferences/tests/writes.rs"]
mod tests;
