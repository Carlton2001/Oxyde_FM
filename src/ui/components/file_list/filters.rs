use super::presentation::file_type_label;
use crate::ui::app::OxydeApp;
use crate::ui::i18n::Message;
use gpui::*;
use oxyde_lib::models::FileEntry;

pub fn get_type_filter_items(
    i18n: &crate::ui::i18n::I18n,
    h: Entity<OxydeApp>,
    pid: String,
    tab_files: std::sync::Arc<Vec<FileEntry>>,
    current_filters: std::collections::HashSet<String>,
    pos: Point<Pixels>,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::{DropdownItem, IconName};

    let mut exts = std::collections::HashMap::<String, u64>::new();
    for f in tab_files.iter() {
        *exts.entry(file_type_label(f)).or_default() += 1;
    }
    let mut sorted_exts: Vec<String> = exts.keys().cloned().collect();
    sorted_exts.sort();

    let mut items = Vec::new();

    for ext in sorted_exts {
        let h_ext = h.clone();
        let pid_ext = pid.clone();
        let e_cap = ext.clone();
        let is_sel = current_filters.contains(&e_cap);

        items.push(DropdownItem {
            label: match ext.as_str() {
                "Dossier" => i18n.text(Message::FileTypeFolder).to_string(),
                "Fichier" => i18n.text(Message::FileTypeFile).to_string(),
                "Symlink" => i18n.text(Message::FileTypeSymlink).to_string(),
                "BrokenLink" => i18n.text(Message::FileTypeBrokenLink).to_string(),
                "Junction" => i18n.text(Message::FileTypeJunction).to_string(),
                _ => ext.clone(),
            },
            icon: Some(IconName::Checkbox(is_sel)),
            badge: Some(crate::ui::format::format_number(exts[&ext], i18n.language())),
            stay_open: true,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new(move |_, app| {
                let pid_cap = pid_ext.clone();
                let e_inner = e_cap.clone();
                h_ext.update(app, |this, cx| {
                    this.toggle_filter_type(&pid_cap, e_inner, cx);
                    this.open_type_filter_menu(&pid_cap, pos, cx);
                });
            }),
            on_hover: None,
        });
    }

    items
}

pub fn get_date_filter_items(
    i18n: &crate::ui::i18n::I18n,
    handle: Entity<OxydeApp>,
    pane_id: String,
    files: std::sync::Arc<Vec<oxyde_lib::models::FileEntry>>,
    current_filters: std::collections::HashSet<crate::ui::date_groups::DateGroup>,
    pos: Point<Pixels>,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::{DropdownItem, IconName};
    use crate::ui::date_groups::DateGroup;
    use std::rc::Rc;

    let categories = DateGroup::ALL;
    let present_categories = crate::ui::date_groups::counts_by_date(
        &files,
        chrono::Local::now().date_naive(),
    );

    let mut items = Vec::new();

    for cat in categories {
        if !present_categories.contains_key(&cat) {
            continue;
        }

        let h = handle.clone();
        let pid = pane_id.clone();
        let active = current_filters.contains(&cat);
        let c_cat = cat;

        items.push(DropdownItem {
            label: i18n.text(cat.message()).to_string(),
            icon: Some(IconName::Checkbox(active)),
            badge: Some(crate::ui::format::format_number(present_categories[&cat], i18n.language())),
            stay_open: true,
            has_submenu: false,
            is_dimmed: false,
            on_click: Rc::new(move |_, cx| {
                let pid_inner = pid.clone();
                h.update(cx, |this, cx| {
                    this.update_filter_date(&pid_inner, c_cat, !active, cx);
                    this.open_date_filter_menu(&pid_inner, pos, cx);
                });
            }),
            on_hover: None,
        });
    }

    items
}

pub fn get_size_filter_items(
    i18n: &crate::ui::i18n::I18n,
    handle: Entity<OxydeApp>,
    pane_id: String,
    files: std::sync::Arc<Vec<oxyde_lib::models::FileEntry>>,
    current_filters: std::collections::HashSet<crate::ui::workspace::SizeCategory>,
    pos: Point<Pixels>,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::{DropdownItem, IconName};
    use crate::ui::workspace::SizeCategory;
    use std::rc::Rc;

    let categories = [
        SizeCategory::Minuscule,
        SizeCategory::Petit,
        SizeCategory::Moyen,
        SizeCategory::Grand,
        SizeCategory::Huge,
    ];

    let mut present_categories = std::collections::HashMap::<_, u64>::new();
    for f in files.iter() {
        for cat in &categories {
            if cat.matches(f.size) {
                *present_categories.entry(*cat).or_default() += 1;
                break;
            }
        }
    }

    let mut items = Vec::new();

    for cat in categories {
        if !present_categories.contains_key(&cat) {
            continue;
        }

        let h = handle.clone();
        let pid = pane_id.clone();
        let active = current_filters.contains(&cat);
        let c_cat = cat;

        items.push(DropdownItem {
            label: i18n.text(cat.message()).to_string(),
            icon: Some(IconName::Checkbox(active)),
            badge: Some(crate::ui::format::format_number(present_categories[&cat], i18n.language())),
            stay_open: true,
            has_submenu: false,
            is_dimmed: false,
            on_click: Rc::new(move |_, cx| {
                let pid_inner = pid.clone();
                h.update(cx, |this, cx| {
                    this.update_filter_size(&pid_inner, c_cat, !active, cx);
                    this.open_size_filter_menu(&pid_inner, pos, cx);
                });
            }),
            on_hover: None,
        });
    }

    items
}
