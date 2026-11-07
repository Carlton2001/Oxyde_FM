//! Saving. One file: the name to save under, typed in the footer as in other file
//! choosers, its stem selected first; a file pressed in the list gives its name, and
//! what the name leads to decides: a folder opens, a file is replaced once confirmed, a
//! free name in an existing folder is chosen. Several files: the folder chosen holds
//! them under the application's names, those already there replaced once confirmed.
use super::FileChooserWindow;
use crate::ui::components::input::{TextInput, TextInputOptions};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::logical;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::platform::FileChooserPurpose;
use oxyde_lib::utils::path_text;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(super) struct Name {
    pub(super) input: Option<Entity<TextInput>>,
    invalid: bool,
    shake: u64,
    task: Option<Task<()>>,
}

/// What a path to save to holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Folder,
    /// A file, or a link, which saving would replace.
    File,
    /// Nothing, in a folder that exists.
    Free,
    /// Nothing, and no folder to hold it, or it cannot be read.
    Unreachable,
}

/// Reads what `path` holds, on the disk: run it off the UI thread.
pub(super) fn target(path: &Path) -> Target {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Target::Folder,
        Ok(_) => Target::File,
        // A dangling link: saving would write where it points.
        Err(_) if std::fs::symlink_metadata(path).is_ok() => Target::File,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && path.parent().is_some_and(Path::is_dir) => Target::Free,
        Err(_) => Target::Unreachable,
    }
}

/// Where a name's stem ends, which the field selects first: before its last
/// extension, at its end when it has none or only starts with a dot.
pub(super) fn stem_end(name: &str) -> usize {
    match name.rfind('.') {
        Some(0) | None => name.len(),
        Some(dot) => dot,
    }
}

impl FileChooserWindow {
    pub(super) fn saving(&self) -> bool {
        !matches!(self.purpose, FileChooserPurpose::Open)
    }

    /// The name field, holding `name` with its stem selected, and the keyboard.
    pub(super) fn open_name(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        let (changed, entered, escaped) = (this.clone(), this.clone(), this);
        let stem = stem_end(&name);
        let input = cx.new(|cx| TextInput::new(
            cx,
            cx.i18n().text(Message::FileChooserNamePlaceholder),
            name,
            move |_, _, cx| {
                let _ = changed.update(cx, |this, cx| {
                    this.name.invalid = false;
                    this.name.task = None;
                    cx.notify();
                });
            },
            move |window, cx| { let _ = entered.update(cx, |this, cx| this.save(window, cx)); },
            move |window, cx| { let _ = escaped.update(cx, |this, cx| this.finish(None, window, cx)); },
            |_, _, _, _, _| {},
            TextInputOptions { initial_selection: Some(0..stem), ..Default::default() },
            window,
        ));
        let focus = input.read(cx).focus_handle.clone();
        self.name.input = Some(input);
        window.focus(&focus, cx);
        cx.on_next_frame(window, move |_, window, cx| window.focus(&focus, cx));
    }

    /// The name typed, without the spaces around it.
    pub(super) fn typed_name(&self, cx: &App) -> String {
        self.name.input.as_ref().map(|input| input.read(cx).content().trim().to_owned()).unwrap_or_default()
    }

    fn set_name(&mut self, name: String, cx: &mut Context<Self>) {
        if let Some(input) = &self.name.input {
            input.update(cx, |input, cx| input.set_content(name, cx));
        }
        self.name.invalid = false;
        self.name.task = None;
    }

    /// A file reached in the list while saving one file gives its name, as in GTK.
    pub(super) fn take_name_of(&mut self, position: usize, cx: &mut Context<Self>) {
        if !matches!(self.purpose, FileChooserPurpose::Save { .. }) { return; }
        let Some(entry) = self.shown_entry(position).filter(|entry| !entry.is_dir) else { return };
        let name = path_text::decode(&entry.path).file_name().map(|name| name.to_string_lossy().into_owned());
        if let Some(name) = name {
            self.set_name(name, cx);
        }
    }

    /// The folder selected alone, which the button opens instead of saving.
    pub(super) fn selected_folder(&self) -> Option<String> {
        self.selection.single().and_then(|position| self.shown_entry(position)).filter(|entry| entry.is_dir).map(|entry| entry.path.clone())
    }

    /// The button, or Enter in the name: saves under the name typed, relative to the
    /// folder open; without a name, opens the folder selected.
    pub(super) fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.typed_name(cx);
        if text.is_empty() {
            if let Some(folder) = self.selected_folder() {
                self.show_folder(folder, cx);
            }
            return;
        }
        let home = dirs::home_dir();
        let path = super::location::typed_path(&text, &path_text::decode(&self.folder), home.as_deref());
        self.name.task = Some(cx.spawn_in(window, async move |this, cx| {
            let checked = path.clone();
            let found = cx.background_spawn(async move { target(&checked) }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.name.task = None;
                if !this.save_at(path, found, window, cx) {
                    this.name.invalid = true;
                    this.name.shake = this.name.shake.wrapping_add(1);
                }
                cx.notify();
            });
        }));
    }

    /// Saves one file at `path`, which holds `found`: false when nothing can be saved there.
    pub(super) fn save_at(&mut self, path: PathBuf, found: Target, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match found {
            Target::Folder => {
                // The name led to a folder: it opens, and the name is typed anew there.
                self.set_name(String::new(), cx);
                self.show_folder(path_text::encode(&path), cx);
            }
            Target::File => self.confirm_overwrite(vec![path.clone()], vec![path], window, cx),
            Target::Free => self.finish(Some(vec![path]), window, cx),
            Target::Unreachable => return false,
        }
        true
    }

    /// Saves the application's files in `folder`, asking first for those already there.
    pub(super) fn save_files(&mut self, folder: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let FileChooserPurpose::SaveFiles { names } = &self.purpose else { return };
        let targets: Vec<PathBuf> = names.iter().map(|name| folder.join(name)).collect();
        self.name.task = Some(cx.spawn_in(window, async move |this, cx| {
            let checked = targets.clone();
            let existing = cx.background_spawn(async move {
                checked.into_iter().filter(|target| std::fs::symlink_metadata(target).is_ok()).collect::<Vec<_>>()
            }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.name.task = None;
                if existing.is_empty() {
                    this.finish(Some(targets), window, cx);
                } else {
                    this.confirm_overwrite(existing, targets, window, cx);
                }
            });
        }));
    }

    /// Back in the name once a replacement is declined.
    pub(super) fn refocus_name(&self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.name.input {
            Some(input) => window.focus(&input.read(cx).focus_handle.clone(), cx),
            None => window.focus(&self.focus, cx),
        }
    }

    /// The name's label and field, red and shaken when it leads nowhere, as the path
    /// edited in the navigation bar.
    pub(super) fn render_name_field(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let input = self.name.input.clone()?;
        let invalid = self.name.invalid;
        let field = div()
            .flex_1()
            .min_w(rems(10.0))
            .max_w(rems(28.0))
            .when(invalid, |field| field.border(rems(0.0625)).border_color(theme.error).rounded_md())
            .child(input);
        let row = div()
            .flex_1()
            .min_w(rems(14.0))
            .flex()
            .items_center()
            .gap(rems(0.5))
            .child(div().flex_none().child(cx.i18n().text(Message::FileChooserName)))
            .child(field);
        if !invalid {
            return Some(row.into_any_element());
        }
        Some(row.with_animation(
            SharedString::from(format!("file-chooser-name-shake-{}", self.name.shake)),
            Animation::new(std::time::Duration::from_millis(300)),
            |row, delta| {
                let amplitude = (1.0 - delta) * logical(8.0);
                let offset = (delta * std::f32::consts::PI * 6.0).sin() * amplitude;
                row.relative().left(px(offset))
            },
        ).into_any_element())
    }
}

#[cfg(test)]
#[path = "tests/save.rs"]
mod tests;
