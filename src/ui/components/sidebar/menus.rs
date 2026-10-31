use crate::ui::app::OxydeApp;
use crate::ui::components::dropdown::{DropdownItem, IconName};
use crate::ui::i18n::{I18n, Message};
use gpui::*;
use oxyde_lib::platform::DriveActions;

// ── Per-item right-click menus ───────────────────────────────────────────────

fn pinned_context_menu_raw(i18n: &I18n, h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    vec![
        DropdownItem {
            label: i18n.text(Message::Open).to_string(),
            icon: Some(IconName::ExternalLink),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.navigate_to(&id, p.clone(), cx);
                    });
                }
            }),
            on_hover: None,
        },
        DropdownItem {
            label: i18n.text(Message::OpenNewTab).to_string(),
            icon: Some(IconName::ExternalLink),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.add_tab(&id, p.clone(), false, cx);
                    });
                }
            }),
            on_hover: None,
        },
        DropdownItem {
            label: i18n.text(Message::UnpinQuickAccess).to_string(),
            icon: Some(IconName::PinOff),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        this.change_favorite(p.clone(), true, cx);
                        cx.notify();
                    });
                }
            }),
            on_hover: None,
        },
        DropdownItem {
            label: i18n.text(Message::Properties).to_string(),
            icon: Some(IconName::Settings),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |window, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        if p == "trash://" { this.open_trash_settings(window, cx); return; }
                        this.open_file_properties(p.clone(), window, cx);
                    });
                }
            }),
            on_hover: None,
        },
    ]
}

fn location_context_menu_raw(i18n: &I18n, h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    vec![
        DropdownItem {
            label: i18n.text(Message::Open).to_string(),
            icon: Some(IconName::ExternalLink),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.navigate_to(&id, p.clone(), cx);
                    });
                }
            }),
            on_hover: None,
        },
        DropdownItem {
            label: i18n.text(Message::Properties).to_string(),
            icon: Some(IconName::Settings),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |window, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        if p == "trash://" { this.open_trash_settings(window, cx); return; }
                        this.open_file_properties(p.clone(), window, cx);
                    });
                }
            }),
            on_hover: None,
        },
    ]
}

fn drive_context_menu_raw(i18n: &I18n, h: Entity<OxydeApp>, path: String, actions: DriveActions) -> Vec<DropdownItem> {
    let mut items = vec![
        DropdownItem {
            label: i18n.text(Message::Open).to_string(),
            icon: Some(IconName::ExternalLink),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.navigate_to(&id, p.clone(), cx);
                    });
                }
            }),
            on_hover: None,
        },
        DropdownItem {
            label: i18n.text(Message::OpenNewTab).to_string(),
            icon: Some(IconName::ExternalLink),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.add_tab(&id, p.clone(), false, cx);
                    });
                }
            }),
            on_hover: None,
        },
    ];
    // Unmounting and ejection follow the tree actions, as in Explorer.
    let release = |message, icon, eject| {
        let h = h.clone();
        let path = path.clone();
        DropdownItem {
            label: i18n.text(message).to_string(),
            icon: Some(icon),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new(move |_, app| {
                h.update(app, |this, cx| this.release_drive(path.clone(), eject, cx));
            }),
            on_hover: None,
        }
    };
    let releases: Vec<_> = [
        actions.unmount.then(|| release(Message::UnmountDrive, IconName::Eject, false)),
        actions.eject.then(|| release(Message::EjectDrive, IconName::Unplug, true)),
    ].into_iter().flatten().collect();
    items.extend(sidebar_tree_actions(i18n, h, path, false));
    let index = items.iter().position(|item| item.label == i18n.text(Message::Properties).as_ref())
        .unwrap_or(items.len());
    items.splice(index..index, releases);
    items
}

fn directory_context_menu_raw(i18n: &I18n, h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    let mut items = open_items(i18n, h.clone(), path.clone());
    items.extend(sidebar_tree_actions(i18n, h, path, true));
    items
}

fn open_items(i18n: &I18n, h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    vec![
        DropdownItem {
            label: i18n.text(Message::Open).to_string(),
            icon: Some(IconName::ExternalLink),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.navigate_to(&id, p.clone(), cx);
                    });
                }
            }),
            on_hover: None,
        },
        DropdownItem {
            label: i18n.text(Message::OpenNewTab).to_string(),
            icon: Some(IconName::ExternalLink),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let id = this.state.active_pane_id.clone();
                        this.add_tab(&id, p.clone(), false, cx);
                    });
                }
            }),
            on_hover: None,
        },
    ]
}

fn sidebar_tree_actions(
    i18n: &I18n,
    h: Entity<OxydeApp>,
    path: String,
    pin: bool,
) -> Vec<DropdownItem> {
    let mut items = Vec::new();
    for (message, icon, expand) in [
        (Message::ExpandAll, IconName::ChevronDown, true),
        (Message::CollapseAll, IconName::ChevronUp, false),
    ] {
        let h_action = h.clone();
        let path_action = path.clone();
        items.push(DropdownItem {
            label: i18n.text(message).to_string(),
            icon: Some(icon),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new(move |_, app| {
                h_action.update(app, |this, cx| {
                    this.context_menus.clear();
                    if expand {
                        this.expand_all_sidebar_path(path_action.clone(), cx);
                    } else {
                        this.state._collapse_all_sidebar_path(path_action.clone());
                        this.reconcile_watchers(cx);
                        cx.notify();
                    }
                })
            }),
            on_hover: None,
        });
    }
    if pin {
        let path_pin = path.clone();
        let h_pin = h.clone();
        items.push(DropdownItem {
            label: i18n.text(Message::PinQuickAccess).to_string(),
            icon: Some(IconName::Pin),
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new(move |_, app| {
                h_pin.update(app, |this, cx| {
                    this.context_menus.clear();
                    this.change_favorite(path_pin.clone(), false, cx);
                    cx.notify();
                })
            }),
            on_hover: None,
        });
    }
    let p = path.clone();
    let h_prop = h.clone();
    items.push(DropdownItem {
        label: i18n.text(Message::Properties).to_string(),
        icon: Some(IconName::Settings),
        badge: None,
        stay_open: false,
        has_submenu: false,
        is_dimmed: false,
        on_click: std::rc::Rc::new(move |window, app| {
            h_prop.update(app, |this, cx| {
                this.context_menus.clear();
                if p == "trash://" { this.open_trash_settings(window, cx); return; }
                this.open_file_properties(p.clone(), window, cx);
            })
        }),
        on_hover: None,
    });
    let p = path;
    items.push(DropdownItem {
        label: i18n.text(Message::MoreOptions).to_string(),
        icon: Some(IconName::Ellipsis),
        badge: None,
        stay_open: true,
        has_submenu: true,
        is_dimmed: false,
        on_click: std::rc::Rc::new(|_, _| {}),
        on_hover: Some(std::rc::Rc::new(move |pos, left, app| {
            h.update(app, |this, cx| {
                this.request_native_context_menu(0, vec![p.clone()], false, pos, left, cx);
            })
        })),
    });
    super::super::without_unavailable_shell_menu(items, i18n)
}

pub(super) fn pinned_context_menu(i18n: &I18n, h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    let actions = super::super::network::network_actions(i18n, h.clone(), &path, false);
    let items = super::super::network::insert_after_open(pinned_context_menu_raw(i18n, h.clone(), path.clone()), actions, i18n);
    let items = super::super::with_terminal_action(items, h, &path, i18n);
    super::super::context_menu_sections(items, i18n)
}

pub(super) fn location_context_menu(i18n: &I18n, h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    let actions = super::super::network::network_actions(i18n, h.clone(), &path, false);
    let items = super::super::network::insert_after_open(location_context_menu_raw(i18n, h.clone(), path.clone()), actions, i18n);
    let items = super::super::with_terminal_action(items, h, &path, i18n);
    super::super::context_menu_sections(items, i18n)
}

pub(super) fn drive_context_menu(i18n: &I18n, h: Entity<OxydeApp>, path: String, releases: DriveActions, is_remote: bool) -> Vec<DropdownItem> {
    let actions = super::super::network::network_actions(i18n, h.clone(), &path, is_remote);
    let items = super::super::network::insert_after_open(drive_context_menu_raw(i18n, h.clone(), path.clone(), releases), actions, i18n);
    let items = super::super::with_terminal_action(items, h, &path, i18n);
    super::super::context_menu_sections(items, i18n)
}

/// The parent of the distributions is no folder: it has no properties or shell menu.
pub(super) fn linux_context_menu(i18n: &I18n, h: Entity<OxydeApp>) -> Vec<DropdownItem> {
    super::super::context_menu_sections(open_items(i18n, h, oxyde_lib::platform::WSL_ROOT.to_string()), i18n)
}

pub(super) fn directory_context_menu(i18n: &I18n, h: Entity<OxydeApp>, path: String) -> Vec<DropdownItem> {
    let actions = super::super::network::network_actions(i18n, h.clone(), &path, false);
    let items = super::super::network::insert_after_open(directory_context_menu_raw(i18n, h.clone(), path.clone()), actions, i18n);
    let items = super::super::with_terminal_action(items, h, &path, i18n);
    super::super::context_menu_sections(items, i18n)
}
