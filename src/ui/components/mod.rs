pub mod conflict_dialog;
pub mod dropdown;
pub mod file_list;
pub mod icons;
pub mod input;
pub mod modal;
pub(crate) mod network;
pub(crate) mod network_dialog;
pub mod operations;
pub mod rename_dialog;
pub mod scrollbar;
pub mod settings;
pub mod sidebar;
pub mod tabs;
pub mod title_bar;
pub mod tooltip;
pub mod top_bar;

use crate::ui::i18n::{I18n, Message};
use gpui::*;

/// Tauri menu sections: navigation, editing/state, destructive actions,
/// information, sorting and native Shell actions. Empty sections disappear.
pub(crate) fn context_menu_sections(items: Vec<dropdown::DropdownItem>, i18n: &I18n) -> Vec<dropdown::DropdownItem> {
    use dropdown::IconName;
    let mut result = Vec::new();
    let mut previous_group = None;
    let mut pending_separator = false;
    for item in items {
        if matches!(item.icon, Some(IconName::Separator)) {
            pending_separator = true;
            continue;
        }
        let group = if [Message::OpenFileLocation, Message::Open, Message::OpenNewTab, Message::ExpandAll, Message::CollapseAll]
            .iter().any(|message| item.label == i18n.text(*message).as_ref()) { 0 }
            else if item.label == i18n.text(Message::Delete).as_ref() { 2 }
            else if item.label == i18n.text(Message::Properties).as_ref() { 3 }
            else if item.label == i18n.text(Message::SortBy).as_ref() { 4 }
            else if item.label == i18n.text(Message::MoreOptions).as_ref() { 5 }
            else { 1 };
        if !result.is_empty() && (pending_separator || previous_group != Some(group)) {
            result.push(dropdown::DropdownItem::separator());
        }
        result.push(item);
        previous_group = Some(group);
        pending_separator = false;
    }
    result
}

/// Attach sorting controls to context menus in grid view.
pub fn with_grid_sort(
    mut items: Vec<dropdown::DropdownItem>,
    app: &crate::ui::app::OxydeApp,
    handle: Entity<crate::ui::app::OxydeApp>,
    pane_id: &str,
    i18n: &I18n,
) -> Vec<dropdown::DropdownItem> {
    use crate::ui::workspace::{SortColumn, SortDirection, ViewMode};
    let Some(pane) = app.state.find_pane(pane_id) else { return items };
    let tab = pane.active_tab();
    if tab.view_mode != ViewMode::Grid { return items; }
    let column = tab.sort_column;
    let direction = tab.sort_direction;
    let mut choices = Vec::new();
    for (label, selected_column, selected_direction) in [
        (Message::ColumnName, Some(SortColumn::Name), None),
        (Message::SortExtension, Some(SortColumn::Type), None),
        (Message::ColumnSize, Some(SortColumn::Size), None),
        (Message::ColumnModified, Some(SortColumn::Modified), None),
        (Message::SortAscending, None, Some(SortDirection::Ascending)),
        (Message::SortDescending, None, Some(SortDirection::Descending)),
    ] {
        if label == Message::SortAscending {
            choices.push(dropdown::DropdownItem::separator());
        }
        let h = handle.clone();
        let pid = pane_id.to_string();
        let mut item = dropdown::DropdownItem::new(i18n.text(label).to_string(), move |_, cx| {
            h.update(cx, |this, cx| {
                if let Some(pane) = this.state.find_pane_mut(&pid) {
                    let tab = pane.active_tab_mut();
                    if let Some(column) = selected_column { tab.sort_column = column; }
                    if let Some(direction) = selected_direction { tab.sort_direction = direction; }
                    tab.sort_files();
                    tab.update_cache();
                }
                this.context_menus.clear();
                cx.notify();
            });
        });
        if selected_column == Some(column) || selected_direction == Some(direction) {
            item.icon = Some(dropdown::IconName::Check);
        }
        choices.push(item);
    }
    let sort = dropdown::DropdownItem::new(i18n.text(Message::SortBy).to_string(), |_, _| {})
        .icon(dropdown::IconName::ListOrdered)
        .stay_open(true)
        .has_submenu(true)
        .on_hover(move |position, open_left, cx| {
            handle.update(cx, |this, cx| {
                use crate::ui::app::{ContextMenuContent, ContextMenuInstance};
                this.context_menus.truncate(1);
                this.context_menus.push(ContextMenuInstance {
                    position,
                    content: ContextMenuContent::Items(choices.clone(), None),
                    scroll_handle: UniformListScrollHandle::new(),
                    width: px(0.0),
                    bottom_padding: 0.0,
                    open_left,
                    hovered_item: None,
                    native_request_id: None,
                });
                cx.notify();
            });
        });
    items.insert(items.len().saturating_sub(1), sort);
    let sort_index = items.len().saturating_sub(2);
    items.insert(sort_index, dropdown::DropdownItem::separator());
    items.insert(items.len().saturating_sub(1), dropdown::DropdownItem::separator());
    context_menu_sections(items, i18n)
}

/// Compare filesystem paths as Windows treats them, ignoring separator, extended
/// path prefix, trailing separator, and case differences.
pub(crate) fn paths_match(left: &str, right: &str) -> bool {
    fn normalize(path: &str) -> String {
        let mut path = path.replace('/', "\\");
        if let Some(rest) = path.strip_prefix("\\\\?\\UNC\\") {
            path = format!("\\\\{rest}");
        } else if let Some(rest) = path.strip_prefix("\\\\?\\") {
            path = rest.to_string();
        }
        while path.len() > 3 && path.ends_with('\\') {
            path.pop();
        }
        path.to_lowercase()
    }

    normalize(left) == normalize(right)
}

fn oxyde_context_menu_items_raw(
    i18n: &I18n,
    h_app: Entity<crate::ui::app::OxydeApp>,
    is_dir: bool,
    is_pinned: bool,
    path: String,
    view: &crate::ui::app::OxydeApp,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::IconName;

    let mut items = vec![];

    if view.state.find_pane(&view.state.active_pane_id).is_some_and(|pane| pane.active_tab().quick_search.is_some()) {
        if let Some(parent) = std::path::Path::new(&path).parent() {
            let parent = parent.to_string_lossy().into_owned();
            let target = path.clone();
            let handle = h_app.clone();
            items.push(crate::ui::components::dropdown::DropdownItem {
                label: i18n.text(Message::OpenFileLocation).to_string(),
                icon: Some(IconName::ExternalLink), stay_open: false, has_submenu: false, is_dimmed: false,
                on_click: std::rc::Rc::new(move |_, app| { handle.update(app, |this, cx| {
                    this.context_menus.clear();
                    let pane = this.state.active_pane_id.clone();
                    this.navigate_to(&pane, parent.clone(), cx);
                    if let Some(pane) = this.state.find_pane_mut(&pane) {
                        pane.active_tab_mut().selected_paths.insert(target.clone());
                        pane.active_tab_mut().selection_anchor = Some(target.clone());
                        pane.active_tab_mut().pending_reveal = Some(target.clone());
                    }
                    cx.notify();
                }); }), on_hover: None,
            });
        }
    }

    items.push(crate::ui::components::dropdown::DropdownItem {
        label: i18n.text(Message::Open).to_string(),
        icon: Some(IconName::ExternalLink),
        stay_open: false,
        has_submenu: false,
        is_dimmed: false,
        on_click: std::rc::Rc::new({
            let h = h_app.clone();
            move |_, app| {
                h.update(app, |this, cx| {
                    this.context_menus.clear();
                    this.execute_ouvrir(cx);
                });
            }
        }),
        on_hover: None,
    });

    if is_dir
        || oxyde_lib::utils::archive::is_navigable_archive(std::path::Path::new(&path))
        || oxyde_lib::utils::disk_image::needs_mount_for_browsing(std::path::Path::new(&path))
    {
        items.push(crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::OpenNewTab).to_string(),
            icon: Some(IconName::ExternalLink),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let p_id = this.state.active_pane_id.clone();
                        this.open_directory_item(&p_id, p.clone(), true, cx);
                    });
                }
            }),
            on_hover: None,
        });
    }
    if !is_dir
        && cfg!(target_os = "windows")
        && oxyde_lib::utils::disk_image::is_disk_image(std::path::Path::new(&path))
    {
        let mounted = view
            .mounted_disk_images
            .iter()
            .any(|image| paths_match(&image.image_path, &path));
        let handle = h_app.clone();
        let image_path = path.clone();
        items.push(
            dropdown::DropdownItem::new(
                i18n.text(if mounted {
                    Message::UnmountDiskImage
                } else {
                    Message::MountDiskImage
                })
                .to_string(),
                move |_, cx| {
                    handle.update(cx, |this, cx| {
                        this.change_disk_image(image_path.clone(), mounted, None, cx)
                    });
                },
            )
            .icon(IconName::ExternalLink),
        );
    }
    if is_dir {
        items.push(crate::ui::components::dropdown::DropdownItem {
            label: i18n
                .text(if is_pinned {
                    Message::UnpinQuickAccess
                } else {
                    Message::PinQuickAccess
                })
                .to_string(),
            icon: Some(if is_pinned {
                IconName::PinOff
            } else {
                IconName::Pin
            }),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        if is_pinned {
                            let _ =
                                oxyde_lib::commands::system::remove_from_quick_access(p.clone());
                        } else {
                            let _ = oxyde_lib::commands::system::add_to_quick_access(p.clone());
                        }
                        this.state.reload_quick_access_and_refresh();
                        cx.notify();
                    });
                }
            }),
            on_hover: None,
        });
    }

    items.push(crate::ui::components::dropdown::DropdownItem {
        label: i18n.text(Message::Cut).to_string(),
        icon: Some(IconName::Scissors),
        stay_open: false,
        has_submenu: false,
        is_dimmed: false,
        on_click: std::rc::Rc::new({
            let h = h_app.clone();
            move |_, app| {
                h.update(app, |this, cx| {
                    this.context_menus.clear();
                    this.execute_couper(cx);
                });
            }
        }),
        on_hover: None,
    });

    items.push(crate::ui::components::dropdown::DropdownItem {
        label: i18n.text(Message::Copy).to_string(),
        icon: Some(IconName::Copy),
        stay_open: false,
        has_submenu: false,
        is_dimmed: false,
        on_click: std::rc::Rc::new({
            let h = h_app.clone();
            move |_, app| {
                h.update(app, |this, cx| {
                    this.context_menus.clear();
                    this.execute_copier(cx);
                });
            }
        }),
        on_hover: None,
    });

    items.push(crate::ui::components::dropdown::DropdownItem {
        label: i18n.text(Message::Paste).to_string(),
        icon: Some(IconName::ClipboardPaste),
        stay_open: false,
        has_submenu: false,
        is_dimmed: false,
        on_click: std::rc::Rc::new({
            let h = h_app.clone();
            move |_, app| {
                h.update(app, |this, cx| {
                    this.context_menus.clear();
                    this.execute_coller(cx);
                });
            }
        }),
        on_hover: None,
    });

    let choices = ["zip", "7z", "tar", "zst"]
        .into_iter()
        .map(|format| {
            let h = h_app.clone();
            let mut args = crate::ui::i18n::FluentArgs::new();
            args.set("format", format);
            crate::ui::components::dropdown::DropdownItem::new(
                i18n.text_with_args(Message::ArchiveFormat, &args)
                    .to_string(),
                move |window, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        this.execute_compress_selected(format, window, cx);
                    });
                },
            )
            .icon(IconName::Archive)
        })
        .collect::<Vec<_>>();
    let h = h_app.clone();
    items.push(
        crate::ui::components::dropdown::DropdownItem::new(
            i18n.text(Message::Compress).to_string(),
            |_, _| {},
        )
        .icon(IconName::Archive)
        .stay_open(true)
        .has_submenu(true)
        .on_hover(move |position, open_left, app| {
            h.update(app, |this, cx| {
                use crate::ui::app::{ContextMenuContent, ContextMenuInstance};
                this.context_menus.truncate(1);
                this.context_menus.push(ContextMenuInstance {
                    position,
                    content: ContextMenuContent::Items(choices.clone(), None),
                    scroll_handle: UniformListScrollHandle::new(),
                    width: px(0.0),
                    bottom_padding: 0.0,
                    open_left,
                    hovered_item: None,
                    native_request_id: None,
                });
                cx.notify();
            });
        }),
    );

    if !is_dir
        && matches!(
            oxyde_lib::utils::archive::ArchiveFormat::from_path(std::path::Path::new(&path)),
            Some(format) if format != oxyde_lib::utils::archive::ArchiveFormat::Rar
        )
    {
        items.push(crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::OpExtractHere).to_string(),
            icon: Some(IconName::ArchiveRestore),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        this.execute_extract_archive(p.clone(), cx);
                    });
                }
            }),
            on_hover: None,
        });
    }

    items.push(crate::ui::components::dropdown::DropdownItem {
        label: i18n.text(Message::Rename).to_string(),
        icon: Some(IconName::Pencil),
        stay_open: false,
        has_submenu: false,
        is_dimmed: false,
        on_click: std::rc::Rc::new({
            let h = h_app.clone();
            move |window, app| {
                h.update(app, |this, cx| {
                    this.context_menus.clear();
                    this.execute_renommer(window, cx);
                });
            }
        }),
        on_hover: None,
    });

    items.push(crate::ui::components::dropdown::DropdownItem {
        label: i18n.text(Message::Delete).to_string(),
        icon: Some(IconName::Trash),
        stay_open: false,
        has_submenu: false,
        is_dimmed: false,
        on_click: std::rc::Rc::new({
            let h = h_app.clone();
            move |_, app| {
                h.update(app, |this, cx| {
                    this.context_menus.clear();
                    this.execute_supprimer(cx);
                });
            }
        }),
        on_hover: None,
    });

    items.push(crate::ui::components::dropdown::DropdownItem {
        label: i18n.text(Message::Properties).to_string(),
        icon: Some(IconName::Settings),
        stay_open: false,
        has_submenu: false,
        is_dimmed: false,
        on_click: std::rc::Rc::new({
            let h = h_app.clone();
            let p = path.clone();
            move |_, app| {
                h.update(app, |this, _cx| {
                    this.context_menus.clear();
                    oxyde_lib::platform::platform().show_properties_dialog(&p);
                });
            }
        }),
        on_hover: None,
    });

    items.push(crate::ui::components::dropdown::DropdownItem {
        label: i18n.text(Message::MoreOptions).to_string(),
        icon: Some(IconName::Ellipsis),
        stay_open: true,
        has_submenu: true,
        is_dimmed: false,
        on_click: std::rc::Rc::new(|_, _| {}),
        on_hover: Some(std::rc::Rc::new({
            let h = h_app.clone();
            let selected_path = path.clone();
            move |sub_pos, open_left, app| {
                h.update(app, |this, cx| {
                    this.request_native_context_menu(
                        0,
                        selected_path.clone(),
                        false,
                        sub_pos,
                        open_left,
                        cx,
                    );
                });
            }
        })),
    });

    items
}

fn pane_context_menu_items_raw(
    i18n: &I18n,
    h_app: Entity<crate::ui::app::OxydeApp>,
    pane_id: String,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::IconName;

    vec![
        crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::NewFolder).to_string(),
            icon: Some(IconName::FolderPlus),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        this.create_new_folder(&p_id, cx);
                    });
                }
            }),
            on_hover: None,
        },
        crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::SelectAll).to_string(),
            icon: Some(IconName::Square),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |_, app| h.update(app, |this, cx| this.select_all(&p_id, cx))
            }),
            on_hover: None,
        },
        crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::Paste).to_string(),
            icon: Some(IconName::ClipboardPaste),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        this.execute_coller(cx);
                    });
                }
            }),
            on_hover: None,
        },
        crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::Properties).to_string(),
            icon: Some(IconName::Settings),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        if let Some(pane) = this.state.layout.find_pane(&p_id) {
                            let path = pane.active_tab().path.clone();
                            oxyde_lib::platform::platform().show_properties_dialog(&path);
                        }
                        cx.notify();
                    });
                }
            }),
            on_hover: None,
        },
        crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::MoreOptions).to_string(),
            icon: Some(IconName::Ellipsis),
            stay_open: true,
            has_submenu: true,
            is_dimmed: false,
            on_click: std::rc::Rc::new(|_, _| {}),
            on_hover: Some(std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |sub_pos, open_left, app| {
                    h.update(app, |this, cx| {
                        if let Some(pane) = this.state.layout.find_pane(&p_id) {
                            let path = pane.active_tab().path.clone();
                            this.request_native_context_menu(0, path, true, sub_pos, open_left, cx);
                        }
                    });
                }
            })),
        },
    ]
}

pub fn tab_context_menu_items(
    i18n: &I18n,
    h_app: Entity<crate::ui::app::OxydeApp>,
    pane_id: String,
    tab_idx: usize,
    can_close_tab: bool,
    can_close_other_tabs: bool,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::IconName;

    vec![
        crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::DuplicateTab).to_string(),
            icon: Some(IconName::CopyPlus),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        this.duplicate_tab(&p_id, tab_idx, cx);
                    });
                }
            }),
            on_hover: None,
        },
        crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::CloseOtherTabs).to_string(),
            icon: Some(IconName::CopyX),
            stay_open: false,
            has_submenu: false,
            is_dimmed: !can_close_other_tabs,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        this.close_other_tabs(&p_id, tab_idx, cx);
                    });
                }
            }),
            on_hover: None,
        },
        crate::ui::components::dropdown::DropdownItem::separator(),
        crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::CloseTab).to_string(),
            icon: Some(IconName::CircleX),
            stay_open: false,
            has_submenu: false,
            is_dimmed: !can_close_tab,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        this.close_tab(&p_id, tab_idx, cx);
                    });
                }
            }),
            on_hover: None,
        },
    ]
}

pub fn oxyde_context_menu_items(
    i18n: &I18n,
    h: Entity<crate::ui::app::OxydeApp>,
    is_dir: bool,
    is_pinned: bool,
    path: String,
    view: &crate::ui::app::OxydeApp,
) -> Vec<dropdown::DropdownItem> {
    let mut items = network::network_actions(i18n, h.clone(), &path, false);
    items.extend(oxyde_context_menu_items_raw(
        i18n, h, is_dir, is_pinned, path, view,
    ));
    context_menu_sections(items, i18n)
}

pub fn pane_context_menu_items(i18n: &I18n, h: Entity<crate::ui::app::OxydeApp>, pane_id: String, view: &crate::ui::app::OxydeApp) -> Vec<dropdown::DropdownItem> {
    let path = view.state.find_pane(&pane_id).map(|pane| pane.active_tab().path.as_str()).unwrap_or_default();
    let mut items = network::network_actions(i18n, h.clone(), path, false);
    if path == "__network_vincinity__" {
        let h = h.clone();
        items.push(dropdown::DropdownItem::new(i18n.text(Message::Properties).to_string(), move |_, cx| {
            h.update(cx, |this, cx| {
                this.context_menus.clear();
                oxyde_lib::platform::platform().show_properties_dialog("__network_vincinity__");
                cx.notify();
            });
        }).icon(dropdown::IconName::Settings));
        return context_menu_sections(items, i18n);
    }
    if let Some(drive) = view.state.drives.iter().find(|drive| drive.drive_type == "remote"
        && std::path::Path::new(path).ancestors().any(|ancestor|
            sidebar::sidebar_paths_equal(&ancestor.to_string_lossy(), &drive.path))) {
        items.extend(network::network_actions(i18n, h.clone(), &drive.path, true));
    }
    items.extend(pane_context_menu_items_raw(i18n, h, pane_id));
    if path.starts_with("search://") {
        items.retain(|item| ![Message::NewFolder, Message::Paste, Message::Properties, Message::MoreOptions]
            .iter().any(|message| item.label == i18n.text(*message).as_ref()));
    }
    context_menu_sections(items, i18n)
}

#[cfg(test)]
mod context_menu_tests {
    use super::{context_menu_sections, dropdown};
    use crate::ui::i18n::{I18n, Message};

    #[test]
    fn sections_match_tauri_and_remove_empty_boundaries() {
        for language in [crate::ui::i18n::Language::French, crate::ui::i18n::Language::English] {
            let i18n = I18n::for_language(language);
            let action = |message| dropdown::DropdownItem::new(i18n.text(message).to_string(), |_, _| {});
            let items = context_menu_sections(vec![
                dropdown::DropdownItem::separator(),
                action(Message::Open), action(Message::OpenNewTab),
                action(Message::PinQuickAccess), action(Message::Cut), action(Message::Rename),
                action(Message::Delete), action(Message::Properties),
                dropdown::DropdownItem::separator(), dropdown::DropdownItem::separator(),
                action(Message::SortBy), action(Message::MoreOptions),
                dropdown::DropdownItem::separator(),
            ], &i18n);
            let boundaries: Vec<_> = items.iter().enumerate()
                .filter_map(|(index, item)| matches!(item.icon, Some(dropdown::IconName::Separator)).then_some(index))
                .collect();
            assert_eq!(boundaries, vec![2, 6, 8, 10, 12]);
            assert!(!matches!(items.last().unwrap().icon, Some(dropdown::IconName::Separator)));
        }
    }
}
