pub mod dropdown;
pub mod file_list;
pub mod icons;
pub mod input;
pub mod scrollbar;
pub mod settings;
pub mod sidebar;
pub mod tabs;
pub mod title_bar;
pub mod tooltip;
pub mod top_bar;

use gpui::*;

pub fn oxyde_context_menu_items(
    h_app: Entity<crate::ui::app::OxydeApp>,
    is_dir: bool,
    path: String,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::IconName;

    let mut items = vec![];

    items.push(crate::ui::components::dropdown::DropdownItem {
        label: "Ouvrir".to_string(),
        icon: Some(IconName::FolderOpen),
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

    if is_dir {
        items.push(crate::ui::components::dropdown::DropdownItem {
            label: "Ouvrir dans un nouvel onglet".to_string(),
            icon: Some(IconName::Copy),
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
                        this.add_tab(&p_id, p.clone(), false, cx);
                    });
                }
            }),
            on_hover: None,
        });

        items.push(crate::ui::components::dropdown::DropdownItem {
            label: "Epingler à l'accès rapide".to_string(),
            icon: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p = path.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        let _ = oxyde_lib::commands::system::add_to_quick_access(p.clone());
                        this.state.reload_quick_access_and_refresh();
                        cx.notify();
                    });
                }
            }),
            on_hover: None,
        });
    }

    items.push(crate::ui::components::dropdown::DropdownItem {
        label: "Couper".to_string(),
        icon: Some(IconName::Minimize),
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
        label: "Copier".to_string(),
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
        label: "Coller".to_string(),
        icon: Some(IconName::Plus),
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

    items.push(crate::ui::components::dropdown::DropdownItem {
        label: "Renommer".to_string(),
        icon: Some(IconName::ALargeSmall),
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
        label: "Supprimer".to_string(),
        icon: Some(IconName::Delete),
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
        label: "Propriétés".to_string(),
        icon: None,
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
        label: "Plus d'options".to_string(),
        icon: None,
        stay_open: true,
        has_submenu: true, is_dimmed: false,
        on_click: std::rc::Rc::new(|_, _| {}),
        on_hover: Some(std::rc::Rc::new({
            let h = h_app.clone();
            move |sub_pos, open_left, app| {
                h.update(app, |this, cx| {
                    let pane_id = this.state.active_pane_id.clone();
                    if let Some(pane) = this.state.layout.find_pane(&pane_id) {
                        let tab = pane.active_tab();
                        if let Some(path_sel) = tab.selected_paths.iter().next() {
                            use crate::ui::app::ContextMenuContent;
                            let already_open = this.context_menus.get(1)
                                .map(|m| matches!(&m.content, ContextMenuContent::NativeItems { path, is_background: false, .. } if path == path_sel))
                                .unwrap_or(false);
                            if already_open { return; }
                            if let Ok(native_items) = oxyde_lib::commands::system::get_native_context_menu_items(path_sel.clone(), false) {
                                use crate::ui::app::ContextMenuInstance;
                                this.context_menus.truncate(1);
                                this.context_menus.push(ContextMenuInstance {
                                    position: sub_pos,
                                    content: ContextMenuContent::NativeItems {
                                        path: path_sel.clone(),
                                        is_background: false,
                                        items: native_items
                                    },
                                    scroll_handle: gpui::UniformListScrollHandle::new(),
                                    width: px(0.0),
                                    open_left,
                                });
                                cx.notify();
                            }
                        }
                    }
                });
            }
        })),
    });

    items
}

pub fn pane_context_menu_items(
    h_app: Entity<crate::ui::app::OxydeApp>,
    pane_id: String,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::IconName;

    vec![
        crate::ui::components::dropdown::DropdownItem {
            label: "Nouveau dossier".to_string(),
            icon: Some(IconName::Plus),
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
            label: "Coller".to_string(),
            icon: Some(IconName::Plus),
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
            label: "Propriétés".to_string(),
            icon: None,
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
            label: "Plus d'options".to_string(),
            icon: None,
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
                            use crate::ui::app::ContextMenuContent;
                            let already_open = this.context_menus.get(1)
                                .map(|m| matches!(&m.content, ContextMenuContent::NativeItems { path: p, is_background: true, .. } if *p == path))
                                .unwrap_or(false);
                            if already_open { return; }
                            if let Ok(items) = oxyde_lib::commands::system::get_native_context_menu_items(path.clone(), true) {
                                use crate::ui::app::ContextMenuInstance;
                                this.context_menus.truncate(1);
                                this.context_menus.push(ContextMenuInstance {
                                    position: sub_pos,
                                    content: ContextMenuContent::NativeItems {
                                        path,
                                        is_background: true,
                                        items
                                    },
                                    scroll_handle: UniformListScrollHandle::new(),
                                    width: px(0.0),
                                    open_left,
                                });
                                cx.notify();
                            }
                        }
                    });
                }
            })),
        },
    ]
}

pub fn tab_context_menu_items(
    h_app: Entity<crate::ui::app::OxydeApp>,
    pane_id: String,
    tab_idx: usize,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::IconName;

    vec![
        crate::ui::components::dropdown::DropdownItem {
            label: "Dupliquer l'onglet".to_string(),
            icon: Some(IconName::Plus),
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
            label: "Fermer les autres onglets".to_string(),
            icon: Some(IconName::X),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        this.state.close_other_tabs(&p_id, tab_idx);
                        cx.notify();
                    });
                }
            }),
            on_hover: None,
        },
        crate::ui::components::dropdown::DropdownItem {
            label: "Fermer l'onglet".to_string(),
            icon: Some(IconName::Delete),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |_, app| {
                    h.update(app, |this, cx| {
                        this.context_menus.clear();
                        this.state.close_tab_id(&p_id, tab_idx);
                        cx.notify();
                    });
                }
            }),
            on_hover: None,
        },
    ]
}
