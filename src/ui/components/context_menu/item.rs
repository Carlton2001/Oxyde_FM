//! The menu of a file or folder.

use super::{dropdown, trash_context_menu_items, without_unavailable_shell_menu};
use crate::ui::i18n::{I18n, Message};
use crate::ui::paths::paths_match;
use gpui::*;

pub(super) fn oxyde_context_menu_items_raw(
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

    if view.state.find_pane(&view.state.active_pane_id).is_some_and(|pane| pane.active_tab().quick_search.is_some())
        && std::path::Path::new(&path).parent().is_some()
    {
        let target = path.clone();
        let handle = h_app.clone();
        items.push(crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::OpenFileLocation).to_string(),
            icon: Some(IconName::ExternalLink), badge: None, stay_open: false, has_submenu: false, is_dimmed: false, is_faded: false,
            on_click: std::rc::Rc::new(move |_, app| { handle.update(app, |this, cx| {
                this.context_menus.clear();
                let pane = this.state.active_pane_id.clone();
                this.reveal_item(&pane, target.clone(), cx);
            }); }), on_hover: None,
        });
    }

    items.push(crate::ui::components::dropdown::DropdownItem {
        label: i18n.text(Message::Open).to_string(),
        icon: Some(IconName::ExternalLink),
        badge: None,
        stay_open: false,
        has_submenu: false,
        is_dimmed: false,
        is_faded: false,
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

    if crate::ui::app::open_with::offers(&path, is_dir) && !entry.is_navigable_archive() {
        let (menu, prompt) = (h_app.clone(), h_app.clone());
        let (hovered, clicked) = (path.clone(), path.clone());
        items.push(crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::OpenWith).to_string(),
            icon: Some(IconName::Image(None)),
            badge: None,
            stay_open: true,
            has_submenu: true,
            is_dimmed: false,
            is_faded: false,
            on_click: std::rc::Rc::new(move |_, app| {
                prompt.update(app, |this, cx| {
                    this.context_menus.clear();
                    let paths = this.open_with_paths(&clicked);
                    this.show_open_with_prompt(paths, cx);
                });
            }),
            on_hover: Some(std::rc::Rc::new(move |position, open_left, app| {
                menu.update(app, |this, cx| {
                    let paths = this.open_with_paths(&hovered);
                    this.show_open_with_menu(paths, position, open_left, cx);
                });
            })),
        });
    }

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
            is_faded: false,
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
    if !is_dir && entry.can_mount_disk_image() {
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
            is_faded: false,
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
        is_faded: false,
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
        is_faded: false,
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
                        open_left, hovered_item: None, native_request_id: None, open_up: false,
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
        is_faded: false,
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
        && oxyde_lib::utils::archive::ArchiveFormat::from_path(std::path::Path::new(&path)).is_some()
    {
        items.push(crate::ui::components::dropdown::DropdownItem {
            label: i18n.text(Message::OpExtractHere).to_string(),
            icon: Some(IconName::ArchiveRestore),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            is_faded: false,
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
        is_faded: false,
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
        is_faded: false,
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
        is_faded: false,
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
        is_faded: false,
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
