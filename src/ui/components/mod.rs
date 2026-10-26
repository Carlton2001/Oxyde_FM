pub(crate) mod client_decorations;
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
pub(crate) mod scroll_area;
pub(crate) mod select_menu;
pub mod settings;
pub mod sidebar;
pub mod tabs;
pub mod title_bar;
pub mod tooltip;
pub(crate) mod popup;
pub(crate) mod tool_window;
pub mod top_bar;
mod history_actions;

use crate::ui::i18n::{I18n, Message};
use gpui::*;

/// Menu sections, in Explorer order: navigation, tree, network, item state,
/// clipboard, creation, selection, archives, history, renaming/deletion, sort,
/// information and native Shell actions. Empty sections disappear.
pub(crate) fn context_menu_sections(items: Vec<dropdown::DropdownItem>, i18n: &I18n) -> Vec<dropdown::DropdownItem> {
    use dropdown::IconName;
    const GROUPS: &[&[Message]] = &[
        &[Message::OpenFileLocation, Message::Open, Message::OpenNewTab],
        &[Message::ExpandAll, Message::CollapseAll],
        &[Message::MapNetworkDrive, Message::DisconnectNetworkDrive, Message::DisconnectThisNetworkDrive],
        &[Message::MountDiskImage, Message::UnmountDiskImage, Message::PinQuickAccess,
            Message::UnpinQuickAccess, Message::EjectDrive, Message::TrashRestore, Message::TrashRestoreAll],
        &[Message::Cut, Message::Copy, Message::Paste],
        &[Message::NewFolder],
        &[Message::SelectAll],
        &[Message::Compress, Message::OpExtractHere],
        &[], // Undo/Redo, recognised by their icons.
        &[Message::Rename, Message::Delete, Message::TrashDeletePermanent, Message::TrashDeleteAll],
        &[Message::SortBy],
        &[Message::Properties],
        &[Message::MoreOptions],
    ];
    const HISTORY: usize = 8;
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
fn without_unavailable_shell_menu(items: Vec<dropdown::DropdownItem>, i18n: &I18n) -> Vec<dropdown::DropdownItem> {
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
                    bottom_padding: 0.0,
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

    let entry = view.file_for_action(&path, is_dir);

    if view.state.find_pane(&view.state.active_pane_id).is_some_and(|pane| pane.active_tab().path == "trash://") {
        return trash_context_menu_items(i18n, h_app, Some(path), view);
    }
    let mut items = vec![];

    if view.state.find_pane(&view.state.active_pane_id).is_some_and(|pane| pane.active_tab().quick_search.is_some()) {
        if let Some(parent) = std::path::Path::new(&path).parent() {
            let parent = parent.to_string_lossy().into_owned();
            let target = path.clone();
            let handle = h_app.clone();
            items.push(crate::ui::components::dropdown::DropdownItem {
                label: i18n.text(Message::OpenFileLocation).to_string(),
                icon: Some(IconName::ExternalLink), badge: None, stay_open: false, has_submenu: false, is_dimmed: false,
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
        badge: None,
        stay_open: false,
        has_submenu: false,
        is_dimmed: false,
        on_click: std::rc::Rc::new({
            let h = h_app.clone();
            let p = path.clone();
            move |_, app| {
                h.update(app, |this, cx| {
                    this.context_menus.clear();
                    if is_dir
                        || this.file_for_action(&p, is_dir).is_navigable_archive()
                        || this.file_for_action(&p, is_dir).needs_mount_for_browsing()
                    {
                        let pane_id = this.state.active_pane_id.clone();
                        this.open_file_item(&pane_id, p.clone(), is_dir, cx);
                    } else {
                        this.execute_ouvrir(cx);
                    }
                });
            }
        }),
        on_hover: None,
    });

    if is_dir
        || entry.is_navigable_archive()
        || entry.needs_mount_for_browsing()
    {
        items.push(crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::OpenNewTab).to_string(),
            icon: Some(IconName::ExternalLink),
            badge: None,
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
                        let paths = this.context_action_paths(&p);
                        let directories: Vec<_> = this.state.find_pane(&p_id)
                            .into_iter().flat_map(|pane| pane.active_tab().files.iter())
                            .filter(|file| paths.contains(&file.path)
                                && (file.is_dir
                                    || file.is_navigable_archive()
                                    || file.needs_mount_for_browsing()))
                            .map(|file| file.path.clone()).collect();
                        for path in directories {
                            this.open_directory_item(&p_id, path, true, cx);
                        }
                    });
                }
            }),
            on_hover: None,
        });
    }
    if !is_dir
        && cfg!(target_os = "windows")
        && entry.is_disk_image()
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
            .icon(if mounted { IconName::Eject } else { IconName::Disc }),
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
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let paths = this.context_action_paths(&p);
                        let directories: Vec<_> = this.state.find_pane(&this.state.active_pane_id)
                            .into_iter().flat_map(|pane| pane.active_tab().files.iter())
                            .filter(|file| file.is_dir && paths.contains(&file.path))
                            .map(|file| file.path.clone()).collect();
                        for path in directories {
                            this.change_favorite(path, is_pinned, cx);
                        }
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
        badge: None,
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
        badge: None,
        stay_open: false,
        has_submenu: true,
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
        on_hover: Some(std::rc::Rc::new({
            let h = h_app.clone();
            let labels = [(i18n.text(Message::CopyName).to_string(), true),
                (i18n.text(Message::CopyPath).to_string(), false)];
            move |position, open_left, app| {
                h.update(app, |this, cx| {
                    use crate::ui::app::{ContextMenuContent, ContextMenuInstance};
                    let choices = labels.iter().map(|(label, names_only)| {
                        let h = h.clone();
                        let names_only = *names_only;
                        crate::ui::components::dropdown::DropdownItem::new(label.clone(), move |_, app| {
                            h.update(app, |this, cx| {
                                this.execute_copy_text(names_only, cx);
                                this.context_menus.clear();
                                cx.notify();
                            });
                        }).icon(IconName::Copy)
                    }).collect();
                    this.context_menus.truncate(1);
                    this.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                        position, content: ContextMenuContent::Items(choices, None),
                        scroll_handle: UniformListScrollHandle::new(), width: px(0.0),
                        bottom_padding: 0.0, open_left, hovered_item: None, native_request_id: None, open_up: false,
                    });
                    cx.notify();
                });
            }
        })),
    });

    items.push(crate::ui::components::dropdown::DropdownItem {
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
    });

    let h = h_app.clone();
    items.push(crate::ui::components::dropdown::DropdownItem::new(
        i18n.text(Message::Compress).to_string(), move |window, app| {
            h.update(app, |this, cx| {
                this.context_menus.clear();
                this.execute_compress_selected(window, cx);
            });
        }).icon(IconName::Archive));

    if !is_dir
        && matches!(
            oxyde_lib::utils::archive::ArchiveFormat::from_path(std::path::Path::new(&path)),
            Some(format) if format != oxyde_lib::utils::archive::ArchiveFormat::Rar
        )
    {
        items.push(crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::OpExtractHere).to_string(),
            icon: Some(IconName::ArchiveRestore),
            badge: None,
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
        badge: None,
        stay_open: false,
        has_submenu: false,
        is_dimmed: false,
        on_click: std::rc::Rc::new({
            let h = h_app.clone();
            let p = path.clone();
            move |window, app| {
                h.update(app, |this, cx| {
                    this.context_menus.clear();
                    this.rename_context_item(p.clone(), is_dir, window, cx);
                });
            }
        }),
        on_hover: None,
    });

    let permanent = view.pane_delete_is_permanent(&view.state.active_pane_id);
    items.push(crate::ui::components::dropdown::DropdownItem {
        label: i18n.text(if permanent { Message::TrashDeletePermanent } else { Message::Delete }).to_string(),
        icon: Some(if permanent { IconName::TrashPermanent } else { IconName::Trash }),
        badge: None,
        stay_open: false,
        has_submenu: false,
        is_dimmed: false,
        on_click: std::rc::Rc::new({
            let h = h_app.clone();
            move |event, app| {
                h.update(app, |this, cx| {
                    this.shift_delete = event.modifiers().shift;
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
        badge: None,
        stay_open: false,
        has_submenu: false,
        is_dimmed: false,
        on_click: std::rc::Rc::new({
            let h = h_app.clone();
            let p = path.clone();
            move |window, app| {
                h.update(app, |this, cx| {
                    this.open_properties_for_paths(this.context_action_paths(&p), window, cx);
                });
            }
        }),
        on_hover: None,
    });

    items.push(crate::ui::components::dropdown::DropdownItem {
        label: i18n.text(Message::MoreOptions).to_string(),
        icon: Some(IconName::Ellipsis),
        badge: None,
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
                        this.context_action_paths(&selected_path),
                        false,
                        sub_pos,
                        open_left,
                        cx,
                    );
                });
            }
        })),
    });

    without_unavailable_shell_menu(items, i18n)
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
    let actions = network::network_actions(i18n, h.clone(), &path, false);
    let mut items = network::insert_after_open(
        oxyde_context_menu_items_raw(i18n, h.clone(), is_dir, is_pinned, path.clone(), view),
        actions,
        i18n,
    );
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
            sidebar::sidebar_paths_equal(&ancestor.to_string_lossy(), &drive.path))) {
        items.extend(network::network_actions(i18n, h.clone(), &drive.path, true));
    }
    items.extend(pane_context_menu_items_raw(i18n, h.clone(), pane_id));
    history_actions::insert_menu_items(&mut items, i18n, h, view);
    if path.starts_with("search://") {
        items.retain(|item| ![Message::NewFolder, Message::Paste, Message::Properties, Message::MoreOptions]
            .iter().any(|message| item.label == i18n.text(*message).as_ref()));
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
            assert_eq!(boundaries, vec![2, 4, 6, 9, 11, 13]);
            assert!(!matches!(items.last().unwrap().icon, Some(dropdown::IconName::Separator)));
        }
    }

    fn labels_by_section(items: &[dropdown::DropdownItem]) -> Vec<Vec<String>> {
        items.split(|item| matches!(item.icon, Some(dropdown::IconName::Separator)))
            .map(|section| section.iter().map(|item| item.label.clone()).collect())
            .collect()
    }

    #[test]
    fn file_menu_splits_actions_by_family() {
        let i18n = I18n::for_language(crate::ui::i18n::Language::English);
        let action = |message| dropdown::DropdownItem::new(i18n.text(message).to_string(), |_, _| {});
        let text = |message| i18n.text(message).to_string();
        let items = context_menu_sections(vec![
            action(Message::Open), action(Message::MapNetworkDrive), action(Message::PinQuickAccess),
            action(Message::Cut), action(Message::Copy), action(Message::Paste),
            action(Message::Compress), action(Message::OpExtractHere),
            dropdown::DropdownItem::new("Undo".to_string(), |_, _| {}).icon(dropdown::IconName::Undo),
            dropdown::DropdownItem::new("Redo".to_string(), |_, _| {}).icon(dropdown::IconName::Redo),
            action(Message::Rename), action(Message::Delete), action(Message::Properties),
        ], &i18n);
        assert_eq!(labels_by_section(&items), vec![
            vec![text(Message::Open)], vec![text(Message::MapNetworkDrive)],
            vec![text(Message::PinQuickAccess)],
            vec![text(Message::Cut), text(Message::Copy), text(Message::Paste)],
            vec![text(Message::Compress), text(Message::OpExtractHere)],
            vec!["Undo".to_string(), "Redo".to_string()],
            vec![text(Message::Rename), text(Message::Delete)], vec![text(Message::Properties)],
        ]);
    }

    #[test]
    fn background_and_trash_menus_isolate_their_families() {
        let i18n = I18n::for_language(crate::ui::i18n::Language::English);
        let action = |message| dropdown::DropdownItem::new(i18n.text(message).to_string(), |_, _| {});
        let text = |message| i18n.text(message).to_string();
        let background = context_menu_sections(vec![
            action(Message::NewFolder), action(Message::SelectAll), action(Message::Paste),
            action(Message::Properties),
        ], &i18n);
        assert_eq!(labels_by_section(&background).len(), 4);
        let trash = context_menu_sections(vec![
            action(Message::Cut), action(Message::TrashRestore), action(Message::TrashDeletePermanent),
            dropdown::DropdownItem::separator(), action(Message::Properties),
        ], &i18n);
        assert_eq!(labels_by_section(&trash), vec![
            vec![text(Message::Cut)], vec![text(Message::TrashRestore)],
            vec![text(Message::TrashDeletePermanent)], vec![text(Message::Properties)],
        ]);
        let empty = context_menu_sections(vec![
            action(Message::TrashRestoreAll), action(Message::TrashDeleteAll),
        ], &i18n);
        assert_eq!(labels_by_section(&empty).len(), 2);
    }

    #[test]
    fn tree_actions_and_ejection_get_their_own_sections() {
        let i18n = I18n::for_language(crate::ui::i18n::Language::English);
        let action = |message| dropdown::DropdownItem::new(i18n.text(message).to_string(), |_, _| {});
        let items = context_menu_sections(vec![
            action(Message::Open), action(Message::OpenNewTab), action(Message::ExpandAll),
            action(Message::CollapseAll), action(Message::EjectDrive), action(Message::Properties),
        ], &i18n);
        assert_eq!(labels_by_section(&items).iter().map(Vec::len).collect::<Vec<_>>(), vec![2, 2, 1, 1]);
        // Removing a whole section afterwards must not leave two separators.
        let filtered = context_menu_sections(items.into_iter()
            .filter(|item| item.label != i18n.text(Message::ExpandAll).as_ref()
                && item.label != i18n.text(Message::CollapseAll).as_ref())
            .collect(), &i18n);
        assert_eq!(labels_by_section(&filtered).iter().map(Vec::len).collect::<Vec<_>>(), vec![2, 1, 1]);
    }
}
