pub mod sidebar;
pub mod top_bar;
pub mod file_list;
pub mod scrollbar;
pub mod tabs;
pub mod dropdown;
pub mod input;

use gpui::*;

pub fn oxyde_context_menu_items(h_app: Entity<crate::ui::app::OxydeApp>) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::app::{Ouvrir, Copier, Couper, Coller, Renommer, Supprimer};
    use crate::ui::components::dropdown::IconName;
    
    vec![
        crate::ui::components::dropdown::DropdownItem {
            label: "Ouvrir".to_string(),
            icon: Some(IconName::FolderOpen),
            stay_open: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                move |_, app| { let _ = h.update(app, |this, cx| {
                    this.context_menu = None;
                    cx.dispatch_action(&Ouvrir);
                }); }
            }),
        },
        crate::ui::components::dropdown::DropdownItem {
            label: "Copier".to_string(),
            icon: Some(IconName::Copy),
            stay_open: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                move |_, app| { let _ = h.update(app, |this, cx| {
                    this.context_menu = None;
                    cx.dispatch_action(&Copier);
                }); }
            }),
        },
        crate::ui::components::dropdown::DropdownItem {
            label: "Couper".to_string(),
            icon: Some(IconName::Minimize),
            stay_open: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                move |_, app| { let _ = h.update(app, |this, cx| {
                    this.context_menu = None;
                    cx.dispatch_action(&Couper);
                }); }
            }),
        },
        crate::ui::components::dropdown::DropdownItem {
            label: "Coller".to_string(),
            icon: Some(IconName::Plus),
            stay_open: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                move |_, app| { let _ = h.update(app, |this, cx| {
                    this.context_menu = None;
                    cx.dispatch_action(&Coller);
                }); }
            }),
        },
        crate::ui::components::dropdown::DropdownItem {
            label: "Renommer".to_string(),
            icon: Some(IconName::ALargeSmall),
            stay_open: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                move |_, app| { let _ = h.update(app, |this, cx| {
                    this.context_menu = None;
                    cx.dispatch_action(&Renommer);
                }); }
            }),
        },
        crate::ui::components::dropdown::DropdownItem {
            label: "Supprimer".to_string(),
            icon: Some(IconName::Delete),
            stay_open: false,
            on_click: std::rc::Rc::new({
                let h = h_app.clone();
                move |_, app| { let _ = h.update(app, |this, cx| {
                    this.context_menu = None;
                    cx.dispatch_action(&Supprimer);
                }); }
            }),
        },
    ]
}
