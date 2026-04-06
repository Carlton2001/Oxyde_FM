pub mod sidebar;
pub mod top_bar;
pub mod action_bar;
pub mod file_list;
pub mod scrollbar;
pub mod tabs;
pub mod dropdown;
pub mod input;
pub mod tooltip;
pub mod icons;

use gpui::*;

pub fn oxyde_context_menu_items(h_app: Entity<crate::ui::app::OxydeApp>) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::app::{Ouvrir, Copier, Couper, Coller, Renommer, Supprimer};
    use crate::ui::components::dropdown::IconName;
    
    vec![
        crate::ui::components::dropdown::DropdownItem {
            label: "Ouvrir".to_string(),
            icon: Some(IconName::FolderOpen),
            stay_open: false,
            has_submenu: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                move |_, app| { let _ = h.update(app, |this, cx| {
                    this.context_menus.clear();
                    cx.dispatch_action(&Ouvrir);
                }); }
            }),
            on_hover: None,
        },
        crate::ui::components::dropdown::DropdownItem {
            label: "Copier".to_string(),
            icon: Some(IconName::Copy),
            stay_open: false,
            has_submenu: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                move |_, app| { let _ = h.update(app, |this, cx| {
                    this.context_menus.clear();
                    cx.dispatch_action(&Copier);
                }); }
            }),
            on_hover: None,
        },
        crate::ui::components::dropdown::DropdownItem {
            label: "Couper".to_string(),
            icon: Some(IconName::Minimize),
            stay_open: false,
            has_submenu: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                move |_, app| { let _ = h.update(app, |this, cx| {
                    this.context_menus.clear();
                    cx.dispatch_action(&Couper);
                }); }
            }),
            on_hover: None,
        },
        crate::ui::components::dropdown::DropdownItem {
            label: "Coller".to_string(),
            icon: Some(IconName::Plus),
            stay_open: false,
            has_submenu: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                move |_, app| { let _ = h.update(app, |this, cx| {
                    this.context_menus.clear();
                    cx.dispatch_action(&Coller);
                }); }
            }),
            on_hover: None,
        },
        crate::ui::components::dropdown::DropdownItem {
            label: "Renommer".to_string(),
            icon: Some(IconName::ALargeSmall),
            stay_open: false,
            has_submenu: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                move |_, app| { let _ = h.update(app, |this, cx| {
                    this.context_menus.clear();
                    cx.dispatch_action(&Renommer);
                }); }
            }),
            on_hover: None,
        },
        crate::ui::components::dropdown::DropdownItem {
            label: "Supprimer".to_string(),
            icon: Some(IconName::Delete),
            stay_open: false,
            has_submenu: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                move |_, app| { let _ = h.update(app, |this, cx| {
                    this.context_menus.clear();
                    cx.dispatch_action(&Supprimer);
                }); }
            }),
            on_hover: None,
        },
        crate::ui::components::dropdown::DropdownItem {
            label: "Plus d'options".to_string(),
            icon: None,
            stay_open: true,
            has_submenu: true,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                move |_, app| { 
                    let _ = h.update(app, |this, cx| {
                        let pane_id = this.state.active_pane_id.clone();
                        if let Some(pane) = this.state.layout.find_pane(&pane_id) {
                            let tab = pane.active_tab();
                            if let Some(path) = tab.selected_paths.iter().next() {
                                if let Ok(items) = oxyde_lib::commands::system::get_native_context_menu_items(path.clone(), false) {
                                    use crate::ui::app::{ContextMenuContent, ContextMenuInstance};
                                    // Push to stack (keep parent)
                                    let mut new_pos = this.mouse_pos;
                                    new_pos.x -= px(200.0);
                                    this.context_menus.push(ContextMenuInstance { 
                                        position: new_pos,
                                        content: ContextMenuContent::NativeItems { 
                                            path: path.clone(), 
                                            is_background: false, 
                                            items 
                                        },
                                        scroll_handle: UniformListScrollHandle::new(),
                                        width: px(0.0),
                                    open_left: false,
                                    });
                                }
                            }
                        }
                        cx.notify();
                    });
                }
            }),
            on_hover: None,
        },
    ]
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
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |_, app| { 
                    let _ = h.update(app, |this, cx| {
                        this.context_menus.clear();
                        this.create_new_folder(&p_id, cx);
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
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |_, app| { 
                    let _ = h.update(app, |this, cx| {
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
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |_, app| { 
                    let _ = h.update(app, |this, cx| {
                        if let Some(pane) = this.state.layout.find_pane(&p_id) {
                            let path = pane.active_tab().path.clone();
                            // Fetch native items (background = true because we are in the empty area of the folder)
                            if let Ok(items) = oxyde_lib::commands::system::get_native_context_menu_items(path.clone(), true) {
                                use crate::ui::app::{ContextMenuContent, ContextMenuInstance};
                                // Push to stack
                                let mut sub_pos = this.mouse_pos;
                                sub_pos.x -= px(220.0);
                                this.context_menus.push(ContextMenuInstance {
                                    position: sub_pos,
                                    content: ContextMenuContent::NativeItems { 
                                        path, 
                                        is_background: true, 
                                        items 
                                    },
                                    scroll_handle: UniformListScrollHandle::new(),
                                    width: px(0.0),
                                    open_left: false,
                                });
                            }
                        }
                        cx.notify();
                    });
                }
            }),
            on_hover: None,
        },
        crate::ui::components::dropdown::DropdownItem {
            label: "Debug: Sous-menus".to_string(),
            icon: None,
            stay_open: true,
            has_submenu: true,
            on_click: std::rc::Rc::new(|_, _| {}),
            on_hover: Some(std::rc::Rc::new({
                let h_app = h_app.clone();
                move |pos, open_left, cx| {
                    let h_deferred = h_app.clone();
                    cx.defer(move |cx| {
                        let _ = h_deferred.update(cx, |this: &mut crate::ui::app::OxydeApp, cx| {
                            use crate::ui::app::{ContextMenuContent, ContextMenuInstance};
                            if this.context_menus.len() > 1 { return; }

                            let h_lvl2_base = h_deferred.clone();
                            this.context_menus.push(ContextMenuInstance {
                                position: pos,
                                open_left,
                                content: ContextMenuContent::Items(vec![
                                    crate::ui::components::dropdown::DropdownItem::new("Niveau 2 - Action", |_, _| {}),
                                    crate::ui::components::dropdown::DropdownItem::new("Niveau 2 - Sub", |_, _| {})
                                        .has_submenu(true)
                                        .on_hover({
                                            let h_lvl2 = h_lvl2_base.clone();
                                            move |pos2, open_left2, cx| {
                                                let h_lvl2_deferred = h_lvl2.clone();
                                                cx.defer(move |cx| {
                                                    let _ = h_lvl2_deferred.update(cx, |this: &mut crate::ui::app::OxydeApp, cx| {
                                                        if this.context_menus.len() > 2 { return; }
                                                        this.context_menus.push(ContextMenuInstance {
                                                            position: pos2,
                                                            open_left: open_left2,
                                                            content: ContextMenuContent::Items(vec![
                                                                crate::ui::components::dropdown::DropdownItem::new("Niveau 3 - Final", |_, _| {})
                                                            ], None),
                                                            scroll_handle: UniformListScrollHandle::new(),
                                                            width: px(0.0),
                                                        });
                                                        cx.notify();
                                                    });
                                                });
                                            }
                                        }),
                                ], None),
                                scroll_handle: UniformListScrollHandle::new(),
                                width: px(0.0),
                            });
                            cx.notify();
                        });
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
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |_, app| { 
                    let _ = h.update(app, |this, cx| {
                        this.context_menus.clear();
                        this.state.duplicate_tab(&p_id, tab_idx);
                        cx.notify();
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
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |_, app| { 
                    let _ = h.update(app, |this, cx| {
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
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                let p_id = pane_id.clone();
                move |_, app| { 
                    let _ = h.update(app, |this, cx| {
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
