use super::{PropertiesDialog, PropertiesWindow};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::*;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
pub(super) struct SelectionProperties {
    pub files: u64,
    pub folders: u64,
    pub size: u64,
    pub location: Option<String>,
}

/// The selection's summary, the folders to measure, and the permissions of its
/// items when every one has them, with the levels proposed for enclosed items.
fn inspect_selection(
    paths: &[String],
    cancellation: &AtomicBool,
) -> Result<(SelectionProperties, Vec<String>, Option<super::permissions::PermissionsEdit>), String> {
    let mut properties = Vec::with_capacity(paths.len());
    for path in paths {
        if cancellation.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        properties.push(
            oxyde_lib::commands::io::get_file_properties(path.clone())
                .map_err(|error| error.to_string())?,
        );
    }
    let location = properties
        .first()
        .map(|p| super::property_presentation(p).location.to_owned())
        .filter(|location| {
            properties.iter().all(|p| {
                let other = super::property_presentation(p).location;
                if cfg!(target_os = "windows") {
                    crate::ui::paths::paths_match(location, other)
                } else {
                    location == other
                }
            })
        });
    let mut summary = SelectionProperties {
        files: 0,
        folders: 0,
        size: 0,
        location,
    };
    let mut directories = Vec::new();
    for p in &properties {
        // A directory scan already includes selected descendants from search results.
        if properties.iter().any(|parent| {
            parent.is_dir
                && parent.path != p.path
                && std::path::Path::new(&p.path).starts_with(&parent.path)
        }) {
            continue;
        }
        if p.is_dir {
            summary.folders += 1;
            directories.push(p.path.clone());
        } else {
            summary.files += 1;
            summary.size = summary.size.saturating_add(p.size);
        }
    }
    let permissions = properties
        .iter()
        .map(|p| Some(super::permissions::Item { path: p.path.clone(), loaded: p.permissions.clone()? }))
        .collect::<Option<_>>()
        .and_then(|items| super::permissions::PermissionsEdit::new(items, oxyde_lib::commands::io::enclosed_defaults()));
    Ok((summary, directories, permissions))
}

impl PropertiesWindow {
    pub(super) fn load_selection(&mut self, paths: Vec<String>, cx: &mut Context<Self>) {
        let cancellation = self.cancellation.clone();
        self.properties_dialog.as_mut().unwrap().calculating = true;
        self._calculation = Some(cx.spawn(async move |this, cx| {
            let flag = cancellation.clone();
            let result =
                tokio::task::spawn_blocking(move || inspect_selection(&paths, &flag)).await;
            let directories = this
                .update(cx, |this, cx| {
                    let dialog = this.properties_dialog.as_mut()?;
                    let directories = match result {
                        Ok(Ok((summary, directories, permissions))) => {
                            dialog.selection = Some(summary);
                            dialog.permissions = permissions;
                            dialog.calculating = !directories.is_empty();
                            Some(directories)
                        }
                        other => {
                            dialog.calculating = false;
                            dialog.error = Some(match other {
                                Ok(Err(error)) => error,
                                Err(error) => error.to_string(),
                                Ok(Ok(_)) => unreachable!(),
                            });
                            None
                        }
                    };
                    cx.notify();
                    directories
                })
                .ok()
                .flatten();
            let Some(directories) = directories else {
                return;
            };
            for path in directories {
                if cancellation.load(Ordering::Relaxed) {
                    return;
                }
                let result = oxyde_lib::commands::io::calculate_folder_size_cancellable(
                    path,
                    cancellation.clone(),
                )
                .await;
                let keep_scanning = this
                    .update(cx, |this, cx| {
                        let Some(dialog) = &mut this.properties_dialog else {
                            return false;
                        };
                        match result {
                            Ok(size) => {
                                if let Some(summary) = &mut dialog.selection {
                                    summary.size = summary.size.saturating_add(size.size);
                                    summary.files = summary.files.saturating_add(size.files_count);
                                    summary.folders =
                                        summary.folders.saturating_add(size.folders_count);
                                }
                            }
                            Err(error) => {
                                dialog.calculating = false;
                                dialog.error = Some(error.to_string());
                                cx.notify();
                                return false;
                            }
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !keep_scanning {
                    return;
                }
            }
            let _ = this.update(cx, |this, cx| {
                if let Some(dialog) = &mut this.properties_dialog {
                    dialog.calculating = false;
                }
                cx.notify();
            });
        }));
    }
}

pub(super) fn render(
    dialog: &PropertiesDialog,
    label_width: f32,
    compact: bool,
    theme: OxydeTheme,
    cx: &mut Context<PropertiesWindow>,
) -> Div {
    let mut args = FluentArgs::new();
    args.set("count", dialog.paths.len() as i64);
    args.set(
        "number",
        crate::ui::format::format_number(dialog.paths.len() as u64, cx.i18n().language()),
    );
    let name = cx.i18n().text_with_args(Message::PropertiesSelectionItems, &args);
    let row = |label, value| super::section::row(label, value, px(label_width), compact, theme);
    let total = if dialog.calculating {
        cx.i18n().text(Message::PropertiesCalculatingSize).to_string()
    } else if dialog.error.is_some() {
        cx.i18n().text(Message::OperationFailed).to_string()
    } else if let Some(summary) = &dialog.selection {
        crate::ui::format::format_size(summary.size, false, true, cx.i18n().language())
    } else {
        cx.i18n().text(Message::PropertiesLoading).to_string()
    };
    let graphic = svg().path("icons/folder.svg").size(rems(3.0)).text_color(theme.text);
    let mut body = div()
        .flex()
        .flex_col()
        .gap(rems(1.25))
        .child(super::section::header(graphic, name, total, theme));
    if let Some(summary) = &dialog.selection {
        let mut args = FluentArgs::new();
        args.set("files", crate::ui::format::format_number(summary.files, cx.i18n().language()));
        args.set("folders", crate::ui::format::format_number(summary.folders, cx.i18n().language()));
        let details = super::section::card(theme)
            .child(row(
                cx.i18n().text(Message::ColumnLocation),
                summary
                    .location
                    .as_deref()
                    .map(|location| super::location_text(location, cx.i18n()))
                    .unwrap_or_else(|| cx.i18n().text(Message::PropertiesMultipleLocations).to_string()),
            ))
            .child(row(
                cx.i18n().text(Message::PropertiesContains),
                cx.i18n().text_with_args(Message::PropertiesCounts, &args).to_string(),
            ));
        body = body.child(super::section::section(
            cx.i18n().text(Message::PropertiesSectionDetails),
            "icons/info.svg",
            details,
            theme,
        ));
    }
    body
}

#[cfg(test)]
#[path = "selection/tests/summary.rs"]
mod tests;
