//! Editing the folder's path in place of its crumbs, as `begin_breadcrumb_edit` does in
//! the main window: the whole path selected, suggestions, submitted by Enter or a click
//! outside, cancelled by Escape, and shaken with an error border when it leads nowhere.
//! Here a typed file is chosen, unless folders are, or saved to, and `~` and relative
//! paths are understood.
use super::save::{target, Target};
use super::FileChooserWindow;
use crate::ui::components::input::{InputAppearance, TextInput, TextInputOptions};
use crate::ui::i18n::{I18nContext, Message};
use gpui::*;
use oxyde_lib::platform::FileChooserPurpose;
use std::path::{Component, Path, PathBuf};

#[derive(Default)]
pub(super) struct Location {
    pub(super) input: Option<Entity<TextInput>>,
    pub(super) invalid: bool,
    pub(super) shake: u64,
    task: Option<Task<()>>,
}

impl Location {
    pub(super) fn close(&mut self) {
        self.input = None;
        self.invalid = false;
        self.task = None;
    }
}

/// The path `text` names, typed in `folder`: `~` is home, `.` and `..` are resolved
/// without following links, as the list would reach them.
pub(super) fn typed_path(text: &str, folder: &Path, home: Option<&Path>) -> PathBuf {
    let text = text.trim();
    let path = match (text.strip_prefix('~'), home) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with(std::path::is_separator) => home.join(rest.trim_start_matches(std::path::is_separator)),
        _ => folder.join(text),
    };
    // A Windows path starts at its drive or its share, which `..` does not leave.
    let mut resolved = if cfg!(target_os = "windows") { PathBuf::new() } else { PathBuf::from("/") };
    for component in path.components() {
        match component {
            Component::Normal(name) => resolved.push(name),
            Component::ParentDir => { resolved.pop(); }
            Component::Prefix(_) | Component::RootDir if cfg!(target_os = "windows") => resolved.push(component),
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    resolved
}

impl FileChooserWindow {
    /// Replaces the crumbs with the folder's path, selected, or with `text` typed in
    /// the list.
    pub(super) fn begin_location(&mut self, text: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.location.input {
            let focus = input.read(cx).focus_handle.clone();
            window.focus(&focus, cx);
            return;
        }
        let selected = text.is_none();
        let path = text.unwrap_or_else(|| crate::ui::components::input::native_path_text(&oxyde_lib::utils::path_text::display(&self.folder)));
        let this = cx.entity().downgrade();
        let (changed, entered, escaped) = (this.clone(), this.clone(), this);
        let (show_hidden, show_system) = (self.show_hidden, self.show_system);
        let input = cx.new(|cx| TextInput::new(
            cx,
            cx.i18n().text(Message::PathPlaceholder).to_string(),
            path.clone(),
            move |_, _, cx| {
                let _ = changed.update(cx, |this, cx| {
                    this.location.task = None;
                    this.location.invalid = false;
                    cx.notify();
                });
            },
            move |window, cx| { let _ = entered.update(cx, |this, cx| this.submit_location(window, cx)); },
            move |window, cx| { let _ = escaped.update(cx, |this, cx| this.cancel_location(window, cx)); },
            |_, _, _, _, _| {},
            TextInputOptions {
                appearance: InputAppearance::Inline,
                submit_on_outside: true,
                initial_selection: Some(if selected { 0..path.len() } else { path.len()..path.len() }),
                ..Default::default()
            },
            window,
        ).with_path_submit(show_hidden, show_system));
        let focus = input.read(cx).focus_handle.clone();
        self.location.close();
        self.location.input = Some(input);
        window.focus(&focus, cx);
        cx.on_next_frame(window, move |this, window, cx| if this.location.input.is_some() {
            window.focus(&focus, cx);
        });
        cx.notify();
    }

    fn submit_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = &self.location.input else { return };
        let text = crate::ui::components::input::navigation_path_text(input.read(cx).content());
        if text.is_empty() {
            self.cancel_location(window, cx);
            return;
        }
        let home = dirs::home_dir();
        let path = typed_path(&text, &oxyde_lib::utils::path_text::decode(&self.folder), home.as_deref());
        self.location.task = Some(cx.spawn_in(window, async move |this, cx| {
            let checked = path.clone();
            let found = cx.background_spawn(async move { target(&checked) }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.location.task = None;
                let saving = matches!(this.purpose, FileChooserPurpose::Save { .. });
                match found {
                    Target::Folder => {
                        this.location.close();
                        window.focus(&this.focus, cx);
                        this.show_folder(path.to_string_lossy().into_owned(), cx);
                    }
                    // Saving one file, a file is replaced once confirmed, a free name chosen.
                    Target::File | Target::Free if saving => {
                        this.location.close();
                        window.focus(&this.focus, cx);
                        this.save_at(path, found, window, cx);
                    }
                    // A file is chosen, unless the application asks for folders.
                    Target::File if !saving && !this.directory => this.finish(Some(vec![path]), window, cx),
                    _ => {
                        this.location.shake = this.location.shake.wrapping_add(1);
                        this.location.invalid = true;
                    }
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn cancel_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.location.close();
        window.focus(&self.focus, cx);
        cx.notify();
    }
}

#[cfg(test)]
#[path = "tests/location.rs"]
mod tests;
