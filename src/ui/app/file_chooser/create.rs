//! A new folder in the folder open, as the main window's New folder makes one
//! (`rename.rs`, `create_new_item`): created off the UI thread under the first free
//! "New folder" name, recorded in the main window's history so its Undo removes it,
//! selected, and its name edited in place until Enter, a click elsewhere or Escape.
use super::selection::Kept;
use super::FileChooserWindow;
use crate::ui::components::input::{InputAppearance, TextInput, TextInputOptions};
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::i18n::{FluentArgs, I18n, I18nContext, Message};
use crate::ui::scale::logical;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

/// The name being edited in place.
pub(super) struct Rename {
    pub(super) path: String,
    input: Entity<TextInput>,
    text: String,
    error: Option<Message>,
    shake: u64,
    pending: bool,
}

impl FileChooserWindow {
    /// Ctrl+Shift+N or the navigation bar's button.
    pub(super) fn create_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let folder = self.folder.clone();
        let language = cx.i18n().language();
        cx.spawn_in(window, async move |this, cx| {
            let parent = folder.clone();
            let created = cx.background_spawn(async move {
                let i18n = I18n::for_language(language);
                for counter in 0.. {
                    let name = if counter == 0 {
                        i18n.text(Message::NewFolderName).to_string()
                    } else {
                        let mut args = FluentArgs::new();
                        args.set("number", counter);
                        i18n.text_with_args(Message::NewFolderNumbered, &args).replace(['\u{2068}', '\u{2069}'], "")
                    };
                    let path = oxyde_lib::utils::path_text::decode(&parent).join(name);
                    match oxyde_lib::commands::io::create_local_folder(&path) {
                        Ok(identity) => return Ok((path, identity)),
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                        Err(error) => return Err(error),
                    }
                }
                unreachable!()
            }).await;
            let _ = this.update_in(cx, |this, window, cx| match created {
                Ok((path, identity)) => {
                    let _ = this.owner.update(cx, |owner, _| {
                        owner.history.push(super::super::rename::new_folder_transaction(&path, identity, folder.clone()));
                    });
                    let path = oxyde_lib::utils::path_text::encode(&path);
                    if this.folder != folder {
                        return;
                    }
                    // The new folder shows whatever the name filter holds.
                    this.filter.clear();
                    this.list_folder(Kept::path(path.clone()), cx);
                    this.begin_rename(path, window, cx);
                }
                Err(error) => {
                    log::warn!("The file chooser cannot create a folder in {folder}: {error}");
                    let _ = this.owner.update(cx, |owner, cx| owner.toast(super::super::ToastKind::Error, Message::ToastFailed, error.to_string(), 0, cx));
                }
            });
        }).detach();
    }

    /// The folder's name in place of its label, selected and given the keyboard.
    fn begin_rename(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        let name = std::path::Path::new(&path).file_name().and_then(|name| name.to_str())
            .map(oxyde_lib::utils::path_text::display).unwrap_or_default();
        let this = cx.entity().downgrade();
        let (changed, entered, escaped) = (this.clone(), this.clone(), this);
        let length = name.len();
        let input = cx.new(|cx| TextInput::new(
            cx,
            cx.i18n().text(Message::RenameNamePlaceholder),
            name.clone(),
            move |text, _, cx| {
                let _ = changed.update(cx, |this, cx| {
                    if let Some(rename) = &mut this.rename {
                        rename.text = text.to_string();
                        rename.error = None;
                    }
                    cx.notify();
                });
            },
            // After the field's own update, which closing it would collide with.
            move |window, cx| {
                let entered = entered.clone();
                window.defer(cx, move |window, cx| { let _ = entered.update(cx, |this, cx| this.submit_rename(window, cx)); });
            },
            move |window, cx| {
                let escaped = escaped.clone();
                window.defer(cx, move |window, cx| { let _ = escaped.update(cx, |this, cx| this.end_rename(window, cx)); });
            },
            |_, _, _, _, _| {},
            TextInputOptions {
                appearance: InputAppearance::Inline,
                submit_on_outside: true,
                initial_selection: Some(0..length),
                ..Default::default()
            },
            window,
        ));
        let focus = input.read(cx).focus_handle.clone();
        self.rename = Some(Rename { path, input, text: name, error: None, shake: 0, pending: false });
        window.focus(&focus, cx);
        cx.notify();
    }

    fn end_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.rename.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    /// Renames the folder off the UI thread, as the main window's `submit_rename`.
    fn submit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rename) = &mut self.rename else { return };
        if rename.pending {
            return;
        }
        let current = std::path::Path::new(&rename.path).file_name().and_then(|name| name.to_str())
            .map(oxyde_lib::utils::path_text::display).unwrap_or_default();
        if rename.text == current {
            self.end_rename(window, cx);
            return;
        }
        if rename.text.trim().is_empty() {
            rename.error = Some(Message::RenameEmptyName);
            rename.shake = rename.shake.wrapping_add(1);
            cx.notify();
            return;
        }
        rename.pending = true;
        let old_path = rename.path.clone();
        let new_path = std::path::Path::new(&old_path).parent().unwrap_or(std::path::Path::new(""))
            .join(&rename.text).to_string_lossy().into_owned();
        let owner = self.owner.upgrade();
        let quality = owner.as_ref().map(|owner| owner.read(cx).archive_quality_for_path(std::path::Path::new(&old_path))).unwrap_or_default();
        let history = owner.map(|owner| owner.read(cx).history.clone());
        cx.spawn_in(window, async move |this, cx| {
            let (old, new) = (old_path.clone(), new_path.clone());
            let runtime = tokio::runtime::Handle::current();
            let result = tokio::task::spawn_blocking(move || {
                let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let result = runtime.block_on(oxyde_lib::commands::io::rename_item_controlled_with_identity(
                    old.clone(), new.clone(), quality, cancel));
                if let (Ok(Some(identity)), Some(history)) = (&result, &history) {
                    oxyde_lib::commands::io::record_captured_rename(history, &old, &new, *identity);
                }
                result
            }).await.unwrap_or_else(|error| Err(oxyde_lib::models::error::CommandError::IoError(error.to_string())));
            let _ = this.update_in(cx, |this, window, cx| {
                // Escape or another folder meanwhile.
                if this.rename.as_ref().is_none_or(|rename| rename.path != old_path) {
                    if result.is_ok() { this.refresh(cx); }
                    return;
                }
                match result {
                    Ok(_) => {
                        this.end_rename(window, cx);
                        this.list_folder(Kept::path(new_path), cx);
                    }
                    Err(error) => {
                        log::warn!("The file chooser cannot rename {old_path}: {error:?}");
                        if let Some(rename) = &mut this.rename {
                            rename.pending = false;
                            rename.error = Some(crate::ui::errors::rename_message(&error));
                            rename.shake = rename.shake.wrapping_add(1);
                        }
                        cx.notify();
                    }
                }
            });
        }).detach();
    }

    /// The field in place of `path`'s name, framed in red and shaken while refused,
    /// as the main window's rows show it; `width` is the room it takes in a row.
    pub(super) fn render_rename(&self, path: &str, width: Option<f32>, theme: OxydeTheme, cx: &App) -> Option<AnyElement> {
        let rename = self.rename.as_ref().filter(|rename| rename.path == path)?;
        let field = div()
            .id(SharedString::from(format!("file-chooser-rename-{path}")))
            .flex_1()
            .min_w_0()
            .when_some(width, |field, width| field.w(px(width.max(1.0))))
            .h_full()
            .flex()
            .items_center()
            .when_some(rename.error, |field, error| field
                .fixed_tooltip(Tooltip::text(cx.i18n().text(error)))
                .border(rems(0.0625))
                .border_color(theme.error)
                .rounded_sm())
            .child(rename.input.clone());
        if rename.error.is_none() {
            return Some(field.into_any_element());
        }
        Some(field.with_animation(
            SharedString::from(format!("file-chooser-rename-shake-{}", rename.shake)),
            Animation::new(std::time::Duration::from_millis(300)),
            |field, delta| {
                let amplitude = (1.0 - delta) * logical(8.0);
                let offset = (delta * std::f32::consts::PI * 6.0).sin() * amplitude;
                field.relative().left(px(offset))
            },
        ).into_any_element())
    }

    /// Whether `path`'s name is refused, which the grid's card frames in red.
    pub(super) fn rename_refused(&self, path: &str) -> bool {
        self.rename.as_ref().is_some_and(|rename| rename.path == path && rename.error.is_some())
    }
}
