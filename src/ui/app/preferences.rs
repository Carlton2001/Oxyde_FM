use super::OxydeApp;
use crate::ui::i18n::{I18n, I18nContext, Language, Message};
use crate::ui::theme::{OxydeTheme, ThemeId};
use gpui::Context;
use oxyde_lib::models::ConfigManager;

impl OxydeApp {
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
                self.config.extra.insert(
                    crate::ui::workspace::WORKSPACE_CONFIG_KEY.to_string(),
                    snapshot,
                );
                self.save_preferences(cx);
            }
            Err(error) => log::warn!("Cannot serialize workspace state: {error:?}"),
        }
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
        // Chaining guarantees that an older write can never overwrite a newer
        // choice. on_app_quit also awaits the tail of this chain.
        let previous = self.preferences_save_task.take();
        self.preferences_save_task = Some(cx.spawn(async move |this, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            let result = cx
                .background_executor()
                .spawn(async move { ConfigManager::write_to_path(&path, &snapshot) })
                .await;
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
}
