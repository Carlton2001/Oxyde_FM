//! The folder's path in the file chooser: its crumbs, and the path the user types in
//! their place. A typed folder opens, a typed file is chosen.
use super::FileChooserWindow;
use crate::ui::components::input::{InputAppearance, TextInput, TextInputOptions};
use crate::ui::i18n::{I18nContext, Message};
use gpui::*;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CrumbKind {
    Root,
    Home,
    Folder,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Crumb {
    pub(super) kind: CrumbKind,
    pub(super) path: String,
    /// A folder's own name, as the list shows it.
    pub(super) name: String,
}

/// The folders from the root, or from home within it, down to `folder`.
pub(super) fn crumbs(folder: &str, home: Option<&str>) -> Vec<Crumb> {
    let home = home.map(|home| home.trim_end_matches('/')).filter(|home| !home.is_empty());
    let (mut path, kind, rest) = match home {
        Some(home) if folder == home => (home.to_string(), CrumbKind::Home, ""),
        Some(home) if folder.strip_prefix(home).is_some_and(|rest| rest.starts_with('/')) =>
            (home.to_string(), CrumbKind::Home, &folder[home.len()..]),
        _ => ("/".to_string(), CrumbKind::Root, folder),
    };
    let mut crumbs = vec![Crumb { kind, path: path.clone(), name: String::new() }];
    for part in rest.split('/').filter(|part| !part.is_empty()) {
        if !path.ends_with('/') {
            path.push('/');
        }
        path.push_str(part);
        crumbs.push(Crumb {
            kind: CrumbKind::Folder,
            path: path.clone(),
            name: oxyde_lib::utils::path_text::display(part),
        });
    }
    crumbs
}

/// The path `text` names, typed in `folder`: `~` is home, `.` and `..` are resolved
/// without following links, as the list would reach them.
pub(super) fn typed_path(text: &str, folder: &Path, home: Option<&Path>) -> PathBuf {
    let text = text.trim();
    let path = match (text.strip_prefix('~'), home) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => home.join(rest.trim_start_matches('/')),
        _ => folder.join(text),
    };
    let mut resolved = PathBuf::from("/");
    for component in path.components() {
        match component {
            Component::Normal(name) => resolved.push(name),
            Component::ParentDir => { resolved.pop(); }
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    resolved
}

impl FileChooserWindow {
    /// Replaces the crumbs with the folder's path to edit, or with `text` typed in the
    /// list.
    pub(super) fn begin_location(&mut self, text: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.location {
            let focus = input.read(cx).focus_handle.clone();
            window.focus(&focus, cx);
            return;
        }
        let selection = text.is_none();
        let initial = text.unwrap_or_else(|| oxyde_lib::utils::path_text::display(&self.folder));
        let this = cx.entity().downgrade();
        let (changed, entered, escaped) = (this.clone(), this.clone(), this);
        let (show_hidden, show_system) = (self.show_hidden, self.show_system);
        let input = cx.new(|cx| TextInput::new(
            cx,
            cx.i18n().text(Message::PathPlaceholder),
            initial.clone(),
            move |_, _, cx| {
                let _ = changed.update(cx, |this, cx| if this.location_missing {
                    this.location_missing = false;
                    cx.notify();
                });
            },
            move |window, cx| { let _ = entered.update(cx, |this, cx| this.submit_location(window, cx)); },
            move |window, cx| { let _ = escaped.update(cx, |this, cx| this.cancel_location(window, cx)); },
            |_, _, _, _, _| {},
            TextInputOptions {
                appearance: InputAppearance::Inline,
                initial_selection: selection.then(|| 0..initial.len()),
                ..Default::default()
            },
            window,
        ).with_path_submit(show_hidden, show_system));
        let focus = input.read(cx).focus_handle.clone();
        self.location = Some(input);
        self.location_missing = false;
        window.focus(&focus, cx);
        // The field exists once drawn: it keeps the keyboard then.
        cx.on_next_frame(window, move |this, window, cx| if this.location.is_some() {
            window.focus(&focus, cx);
        });
        cx.notify();
    }

    fn submit_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = &self.location else { return };
        let text = input.read(cx).content().to_string();
        if text.trim().is_empty() {
            return;
        }
        let home = dirs::home_dir();
        let path = typed_path(&text, &oxyde_lib::utils::path_text::decode(&self.folder), home.as_deref());
        self.location_task = Some(cx.spawn_in(window, async move |this, cx| {
            let checked = path.clone();
            let folder = cx.background_spawn(async move { std::fs::metadata(&checked).map(|metadata| metadata.is_dir()) }).await;
            let _ = this.update_in(cx, |this, window, cx| match folder {
                Ok(true) => this.show_folder(path.to_string_lossy().into_owned(), cx),
                Ok(false) => this.finish(Some(vec![path]), window, cx),
                Err(_) => {
                    this.location_missing = true;
                    cx.notify();
                }
            });
        }));
    }

    pub(super) fn cancel_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.location = None;
        self.location_missing = false;
        self.location_task = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }
}

#[cfg(test)]
#[path = "tests/location.rs"]
mod tests;
