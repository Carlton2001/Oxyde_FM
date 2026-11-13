pub(crate) mod folder_row;
mod linux_row;
pub(crate) mod measure;
mod menus;
mod pinned;
mod rail;
mod reorder;
mod rows;
mod toolbar;
mod tree;
pub(crate) mod volume_row;

use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18n, Message};
use crate::ui::scale::rem_pixels;
use crate::ui::workspace::SidebarItem;
use gpui::*;

pub(crate) use pinned::render_pinned_menu;

/// Width of the collapsed sidebar, the rail: its entries are square cells.
pub(crate) const RAIL_WIDTH: Rems = rems(2.25);

/// Folders of the tree, at the height of the file list's rows.
const SIDEBAR_ROW_H: Rems = rems(1.75);
fn sidebar_row_h() -> f32 { rem_pixels(SIDEBAR_ROW_H).as_f32() }
/// Places, devices and WSL, at the height of the bars and menu entries.
fn sidebar_entry_h() -> f32 { crate::ui::style::menu_item_height() }
/// Space above the first row and below the last one.
pub(crate) const LIST_PADDING: Rems = rems(0.5);

pub(crate) fn sidebar_item_height(item: &SidebarItem) -> f32 {
    match item {
        SidebarItem::Directory { .. } => sidebar_row_h(),
        // A section is a separator between groups, as in menus.
        SidebarItem::Header(_) => crate::ui::style::menu_separator_height(),
        SidebarItem::Pinned { .. } | SidebarItem::OneDrive { .. } | SidebarItem::Linux { .. }
        | SidebarItem::Drive { .. } | SidebarItem::Volume { .. } => sidebar_entry_h(),
    }
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
        // The home folder among the places.
        _ if item.name.is_empty() => i18n.text(Message::SidebarHome).to_string(),
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
