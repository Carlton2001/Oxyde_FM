//! Shared external-file targets for lists, folders, sidebar and tabs.
use super::super::OxydeApp;
use crate::ui::theme::ThemeContext;
use gpui::{prelude::*, *};
use oxyde_lib::platform::FileDropEffect;

pub(crate) struct ExternalDropState {
    pub effect: FileDropEffect,
    pub can_copy: bool,
    pub can_move: bool,
    pub submitted: bool,
    directory_paths: Vec<String>,
    directories: Vec<String>,
    directory_lease: std::rc::Rc<()>,
}
impl Default for ExternalDropState {
    fn default() -> Self {
        Self {
            effect: FileDropEffect::None,
            can_copy: true,
            can_move: true,
            submitted: false,
            directory_paths: Vec::new(),
            directories: Vec::new(),
            directory_lease: std::rc::Rc::new(()),
        }
    }
}

fn paths(data: &ExternalPaths) -> Vec<String> {
    data.paths()
        .iter()
        .map(|path| {
            let path_string = path.to_str()?;
            (path.is_absolute() && !path_string.contains('\0')).then(|| path_string.to_owned())
        })
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}

fn transfer_paths(data: &ExternalPaths, target: &str) -> Vec<String> {
    if !std::path::Path::new(target).is_absolute() || target.contains("://") {
        return Vec::new();
    }
    super::actionable_drop_paths(paths(data), target)
        .into_iter()
        .filter(|source| {
            let source = source.replace('\\', "/");
            let target = target.replace('\\', "/");
            #[cfg(target_os = "windows")]
            let (source, target) = (source.to_lowercase(), target.to_lowercase());
            !target.starts_with(&format!("{}/", source.trim_end_matches('/')))
        })
        .collect()
}

fn choose_effect(
    paths: &[String],
    target: &str,
    modifiers: Modifiers,
    state: &ExternalDropState,
) -> FileDropEffect {
    if paths.is_empty() {
        return FileDropEffect::None;
    }
    let copy = super::drag_copy_override(modifiers).unwrap_or_else(|| {
        paths
            .iter()
            .any(|source| super::drives_differ(source, target))
    });
    match (
        copy,
        state.can_copy,
        state.can_move,
        super::drag_copy_override(modifiers).is_some(),
    ) {
        (true, true, _, _) | (false, true, false, false) => FileDropEffect::Copy,
        (false, _, true, _) | (true, false, true, false) => FileDropEffect::Move,
        _ => FileDropEffect::None,
    }
}

pub(crate) trait FileDropTargetExt: InteractiveElement + Sized {
    fn external_file_drop(self, app: Entity<OxydeApp>, target: String) -> Self {
        self.on_drag_move::<ExternalPaths>({
            let app = app.clone();
            let target = target.clone();
            move |event, _, cx| {
                if !event.bounds.contains(&event.event.position) {
                    return;
                }
                let paths = transfer_paths(event.drag(cx), &target);
                app.update(cx, |app, cx| {
                    let effect =
                        choose_effect(&paths, &target, event.event.modifiers, &app.external_drop);
                    app.external_drop.effect = effect;
                    if effect != FileDropEffect::None {
                        app.drag_target_default_copy = effect == FileDropEffect::Copy;
                        app.sync_pointer_modifiers(event.event.modifiers, cx);
                        app.update_drag_copy_mode(cx);
                    }
                });
            }
        })
        .drag_over::<ExternalPaths>({
            let app = app.clone();
            let target = target.clone();
            move |style, data, window, cx| {
                if choose_effect(
                    &transfer_paths(data, &target),
                    &target,
                    window.modifiers(),
                    &app.read(cx).external_drop,
                ) == FileDropEffect::None
                {
                    style
                } else {
                    style.bg(cx.theme().drag_hover)
                }
            }
        })
        .on_drop(move |data: &ExternalPaths, window, cx| {
            let paths = transfer_paths(data, &target);
            app.update(cx, |app, cx| {
                let effect = choose_effect(&paths, &target, window.modifiers(), &app.external_drop);
                app.external_drop.effect = effect;
                if effect == FileDropEffect::None || app.external_drop.submitted {
                    return;
                }
                app.external_drop.submitted = true;
                if effect == FileDropEffect::Copy {
                    app.execute_file_copy(paths, target.clone(), cx);
                } else {
                    app.execute_file_move(paths, target.clone(), cx);
                }
            });
        })
    }

    fn external_tab_drop(self, app: Entity<OxydeApp>, pane: String) -> Self {
        self.on_drag_move::<ExternalPaths>({
            let app = app.clone();
            move |event, _, cx| {
                if event.bounds.contains(&event.event.position) {
                    let paths = paths(event.drag(cx));
                    app.update(cx, |app, cx| {
                        if app.external_drop.directory_paths != paths {
                            app.external_drop.directory_paths = paths.clone();
                            app.external_drop.directories.clear();
                            app.external_drop.directory_lease = std::rc::Rc::new(());
                            let lease = app.external_drop.directory_lease.clone();
                            cx.spawn(async move |app, cx| {
                                let directories = tokio::task::spawn_blocking(move || {
                                    paths
                                        .into_iter()
                                        .filter(|path| std::path::Path::new(path).is_dir())
                                        .collect::<Vec<_>>()
                                })
                                .await
                                .unwrap_or_default();
                                let _ = app.update(cx, |app, cx| {
                                    if std::rc::Rc::ptr_eq(
                                        &lease,
                                        &app.external_drop.directory_lease,
                                    ) {
                                        app.external_drop.directories = directories;
                                        cx.notify();
                                    }
                                });
                            })
                            .detach();
                        }
                        app.external_drop.effect = if app.external_drop.can_copy
                            && !app.external_drop.directories.is_empty()
                        {
                            FileDropEffect::Copy
                        } else {
                            FileDropEffect::None
                        };
                    });
                }
            }
        })
        .on_drop(move |data: &ExternalPaths, _, cx| {
            let paths = paths(data);
            app.update(cx, |app, cx| {
                if app.external_drop.submitted
                    || !app.external_drop.can_copy
                    || app.external_drop.directory_paths != paths
                    || app.external_drop.directories.is_empty()
                {
                    app.external_drop.effect = FileDropEffect::None;
                    return;
                }
                app.external_drop.submitted = true;
                app.external_drop.effect = FileDropEffect::Copy;
                let directories = app.external_drop.directories.clone();
                if app.state.find_pane(&pane).is_none() {
                    return;
                }
                for path in directories {
                    app.add_tab(&pane, path, false, cx);
                }
            });
        })
    }
}
impl<T: InteractiveElement> FileDropTargetExt for T {}

#[cfg(test)]
#[path = "tests/external.rs"]
mod tests;
