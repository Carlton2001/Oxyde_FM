//! Context menus of items, panes, tabs and the trash, and their sections.

use super::{dropdown, history_actions, network};
use crate::ui::i18n::{I18n, Message};
use gpui::*;
use item::oxyde_context_menu_items_raw;

mod item;

/// Menu sections, in Explorer order: navigation, tree, network, item state,
/// clipboard, creation, selection, archives, history, renaming/deletion, sort,
/// information and native Shell actions. Empty sections disappear. Items are
/// recognised by their text, which must name one section only in every language.
const GROUPS: &[&[Message]] = &[
    &[Message::OpenFileLocation, Message::Open, Message::OpenWith, Message::OpenNewTab, Message::OpenTerminal],
    &[Message::ExpandAll, Message::CollapseAll],
    &[Message::MapNetworkDrive, Message::DisconnectNetworkDrive, Message::DisconnectThisNetworkDrive],
    &[Message::MountDiskImage, Message::UnmountDiskImage, Message::PinQuickAccess,
        Message::UnpinQuickAccess, Message::UnmountDrive, Message::EjectDrive, Message::TrashRestore, Message::TrashRestoreAll],
    &[Message::Cut, Message::Copy, Message::Paste],
    &[Message::NewFolder, Message::NewFile],
    &[Message::SelectAll],
    &[Message::Compress, Message::OpExtractHere],
    &[], // Undo/Redo, recognised by their icons.
    &[Message::Rename, Message::Delete, Message::TrashDeletePermanent, Message::TrashDeleteAll],
    &[Message::SortBy],
    &[Message::Properties],
    &[Message::MoreOptions],
];
/// The section of Undo and Redo.
const HISTORY: usize = 8;

/// Arranges menu items into those sections.
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
        let group = if matches!(item.icon, Some(IconName::Undo | IconName::Redo)) { HISTORY } else {
            GROUPS.iter().position(|messages| messages.iter()
                .any(|message| item.label == i18n.text(*message).as_ref()))
                .unwrap_or(GROUPS.len())
        };
        if !result.is_empty() && (pending_separator || previous_group != Some(group)) {
            result.push(dropdown::DropdownItem::separator());
        }
        result.push(item);
        previous_group = Some(group);
        pending_separator = false;
    }
    result
}

/// The native Shell submenu behind More options exists only on Windows.
pub(super) fn without_unavailable_shell_menu(items: Vec<dropdown::DropdownItem>, i18n: &I18n) -> Vec<dropdown::DropdownItem> {
    #[cfg(target_os = "windows")]
    {
        let _ = i18n;
        items
    }
    #[cfg(not(target_os = "windows"))]
    {
        let label = i18n.text(Message::MoreOptions);
        items.into_iter().filter(|item| item.label != label.as_ref()).collect()
    }
}

/// Only a real folder can be a terminal's working directory: not the trash, a search,
/// the network neighbourhood, a remote URI nor the inside of an archive.
pub(crate) fn is_terminal_folder(path: &str) -> bool {
    std::path::Path::new(path).is_absolute()
        && oxyde_lib::utils::archive::split_virtual_path(path).is_none()
}

/// Adds Open in Console after the opening actions, where the system offers it.
pub(crate) fn with_terminal_action(
    items: Vec<dropdown::DropdownItem>,
    h: Entity<crate::ui::app::OxydeApp>,
    path: &str,
    i18n: &I18n,
) -> Vec<dropdown::DropdownItem> {
    if !oxyde_lib::platform::platform().offers_terminal() || !is_terminal_folder(path) {
        return items;
    }
    let path = path.to_string();
    let action = dropdown::DropdownItem::new(i18n.text(Message::OpenTerminal).to_string(), move |_, cx| {
        h.update(cx, |this, cx| {
            this.context_menus.clear();
            this.open_terminal(path.clone(), cx);
            cx.notify();
        });
    }).icon(dropdown::IconName::Terminal);
    network::insert_after_open(items, vec![action], i18n)
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
    let mut sort_options = vec![
        (Message::ColumnName, Some(SortColumn::Name), None),
        (Message::SortExtension, Some(SortColumn::Type), None),
        (Message::ColumnSize, Some(SortColumn::Size), None),
        (Message::ColumnModified, Some(SortColumn::Modified), None),
    ];
    if tab.path == "trash://" {
        sort_options.extend([
            (Message::TrashOriginalLocation, Some(SortColumn::OriginalLocation), None),
            (Message::TrashDeletedAt, Some(SortColumn::DeletedAt), None),
        ]);
    }
    sort_options.extend([
        (Message::SortAscending, None, Some(SortDirection::Ascending)),
        (Message::SortDescending, None, Some(SortDirection::Descending)),
    ]);
    for (label, selected_column, selected_direction) in sort_options {
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
                    let id = tab.id.clone();
                    this.prepare_current_tab(id, cx);
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
            native_bounds: None,
                    position,
                    content: ContextMenuContent::Items(choices.clone(), None),
                    scroll_handle: UniformListScrollHandle::new(),
                    width: px(0.0),
                    open_left,
                    hovered_item: None,
                    native_request_id: None, open_up: false,
                });
                cx.notify();
            });
        });
    // Sorting always sits just above the information section.
    let index = items.iter().position(|item| [Message::Properties, Message::MoreOptions].into_iter()
        .any(|message| item.label == i18n.text(message).as_ref())).unwrap_or(items.len());
    items.insert(index, sort);
    context_menu_sections(items, i18n)
}

fn pane_context_menu_items_raw(
    i18n: &I18n,
    h_app: Entity<crate::ui::app::OxydeApp>,
    pane_id: String,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::IconName;

    without_unavailable_shell_menu(vec![
        crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::NewFolder).to_string(),
            icon: Some(IconName::FolderPlus),
            badge: None,
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
        crate::ui::components::dropdown::DropdownItem::new(i18n.text(Message::NewFile).to_string(), {
            let h = h_app.clone();
            let p_id = pane_id.clone();
            move |_, app| {
                h.update(app, |this, cx| {
                    this.context_menus.clear();
                    this.create_new_file(&p_id, cx);
                });
            }
        })
        .icon(IconName::FilePlus),
        crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::SelectAll).to_string(),
            icon: Some(IconName::SquareDashedMousePointer),
            badge: None,
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
            badge: None,
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
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |window, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        if let Some(pane) = this.state.layout.find_pane(&p_id) {
                            let path = pane.active_tab().path.clone();
                            this.open_file_properties(path, window, cx);
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
            badge: None,
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
                            this.request_native_context_menu(0, vec![path], true, sub_pos, open_left, cx);
                        }
                    });
                }
            })),
        },
    ], i18n)
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
            badge: None,
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
        crate::ui::components::dropdown::DropdownItem::separator(),
        crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::CloseTab).to_string(),
            icon: Some(IconName::CircleX),
            badge: None,
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
        crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::CloseOtherTabs).to_string(),
            icon: Some(IconName::CopyX),
            badge: None,
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
    let mut actions = network::network_actions(i18n, h.clone(), &path, false);
    actions.extend(network::recent_server_action(i18n, h.clone(), &path, view));
    let mut items = network::insert_after_open(
        oxyde_context_menu_items_raw(i18n, h.clone(), is_dir, is_pinned, path.clone(), view),
        actions,
        i18n,
    );
    // A server or a share by URI has no local file to describe until it is opened.
    if oxyde_lib::platform::platform().is_remote_uri(&path) {
        let properties = i18n.text(Message::Properties);
        items.retain(|item| item.label != properties.as_ref());
    }
    // A terminal has one working directory: like Nautilus, no entry for a multiple selection.
    let in_trash = view.state.find_pane(&view.state.active_pane_id).is_some_and(|pane| pane.active_tab().path == "trash://");
    if is_dir && !in_trash && view.context_action_paths(&path).len() == 1 {
        items = with_terminal_action(items, h.clone(), &path, i18n);
    }
    if !path.starts_with("trash:") {
        history_actions::insert_menu_items(&mut items, i18n, h, view);
    }
    context_menu_sections(items, i18n)
}

pub fn pane_context_menu_items(i18n: &I18n, h: Entity<crate::ui::app::OxydeApp>, pane_id: String, view: &crate::ui::app::OxydeApp) -> Vec<dropdown::DropdownItem> {
    let path = view.state.find_pane(&pane_id).map(|pane| pane.active_tab().path.as_str()).unwrap_or_default();
    if path == "trash://" { return context_menu_sections(trash_context_menu_items(i18n, h, None, view), i18n); }
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
            crate::ui::paths::paths_match(&ancestor.to_string_lossy(), &drive.path))) {
        items.extend(network::network_actions(i18n, h.clone(), &drive.path, true));
    }
    items.extend(pane_context_menu_items_raw(i18n, h.clone(), pane_id));
    let mut items = with_terminal_action(items, h.clone(), path, i18n);
    history_actions::insert_menu_items(&mut items, i18n, h, view);
    if path.starts_with("search://") {
        items.retain(|item| ![Message::NewFolder, Message::NewFile, Message::Paste, Message::Properties, Message::MoreOptions]
            .iter().any(|message| item.label == i18n.text(*message).as_ref()));
    } else if oxyde_lib::utils::archive::resolve_virtual_path(std::path::Path::new(path)).is_some() {
        items.retain(|item| item.label != i18n.text(Message::NewFile).as_ref());
    }
    context_menu_sections(items, i18n)
}

fn trash_context_menu_items(i18n: &I18n, handle: Entity<crate::ui::app::OxydeApp>, path: Option<String>, view: &crate::ui::app::OxydeApp) -> Vec<dropdown::DropdownItem> {
    use dropdown::{DropdownItem, IconName};
    let mut items = Vec::new();
    let all = path.is_none();
    if !all {
        let h = handle.clone();
        items.push(DropdownItem::new(i18n.text(Message::Cut).to_string(), move |_, cx| {
            h.update(cx, |this, cx| { this.execute_couper(cx); this.context_menus.clear(); cx.notify(); });
        }).icon(IconName::Scissors));
    }
    for (label, restore, icon) in [
        (if all { Message::TrashRestoreAll } else { Message::TrashRestore }, true, IconName::Restore),
        (if all { Message::TrashDeleteAll } else { Message::TrashDeletePermanent }, false, IconName::TrashPermanent),
    ] {
        let h = handle.clone();
        let mut item = DropdownItem::new(i18n.text(label).to_string(), move |_, cx| {
            h.update(cx, |this, cx| { this.execute_trash_action(restore, all, cx); this.context_menus.clear(); cx.notify(); });
        }).icon(icon);
        item.is_dimmed = view.trash_busy || (all && view.state.find_pane(&view.state.active_pane_id)
            .is_some_and(|pane| pane.active_tab().files.is_empty()));
        items.push(item);
    }
    items.push(DropdownItem::separator());
    items.push(DropdownItem::new(i18n.text(Message::Properties).to_string(), move |window, cx| {
        let path = path.clone().unwrap_or_else(|| "trash://".into());
        handle.update(cx, |this, cx| {
            this.context_menus.clear();
            if path == "trash://" { this.open_trash_settings(window, cx); return; }
            this.open_properties_for_paths(this.context_action_paths(&path), window, cx);
            cx.notify();
        });
    }).icon(IconName::Settings));
    items
}

#[cfg(test)]
#[path = "context_menu/tests/sections.rs"]
mod tests;
