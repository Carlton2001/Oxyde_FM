mod menus;
mod pinned;
mod rail;
mod rows;
mod toolbar;
mod tree;

use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18n, Message};
use crate::ui::scale::rem_pixels;
use crate::ui::workspace::SidebarItem;
use gpui::*;

pub(crate) use pinned::render_pinned_menu;

pub(crate) fn sidebar_paths_equal(left: &str, right: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        left.replace('/', "\\").trim_end_matches('\\').to_lowercase()
            == right.replace('/', "\\").trim_end_matches('\\').to_lowercase()
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::path::Path::new(left) == std::path::Path::new(right)
    }
}

const SIDEBAR_ROW_H: Rems = rems(1.75);
fn sidebar_row_h() -> f32 { rem_pixels(SIDEBAR_ROW_H).as_f32() }

pub(crate) fn sidebar_item_height(item: &SidebarItem) -> f32 {
    if matches!(item, SidebarItem::Drive { .. }) { rem_pixels(rems(2.25)).as_f32() }
    else { sidebar_row_h() }
}

pub(crate) fn sidebar_item_visible(item: &SidebarItem, show_hidden: bool, show_system: bool, show_network: bool) -> bool {
    match item {
        SidebarItem::Directory { node, .. } =>
            (show_hidden || !node.is_hidden) && (show_system || !node.is_system),
        SidebarItem::Pinned { item } => (show_network || item.path != "__network_vincinity__")
            && (show_hidden || !item.name.starts_with('.')),
        SidebarItem::Header(Message::SidebarQuickAccess) => false,
        _ => true,
    }
}

fn pinned_display_name(item: &oxyde_lib::models::QuickAccessItem, i18n: &I18n) -> String {
    match item.path.as_str() {
        "__network_vincinity__" => i18n.text(Message::SidebarNetwork).to_string(),
        "trash://" => i18n.text(Message::SidebarTrash).to_string(),
        _ => item.name.clone(),
    }
}
// Selection / hover colours

pub fn render_sidebar(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    if view.sidebar_collapsed {
        rail::render_sidebar_rail(view, handle, cx)
    } else {
        tree::render_sidebar_tree(view, handle, window, cx)
    }
}
