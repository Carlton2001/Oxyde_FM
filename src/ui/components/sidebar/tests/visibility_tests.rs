use super::visible_sidebar_items;
use crate::ui::workspace::SidebarItem;
use oxyde_lib::models::SidebarNode;

fn folder(name: &str, depth: usize, is_hidden: bool) -> SidebarItem {
    SidebarItem::Directory {
        node: SidebarNode { name: name.into(), path: name.into(), is_hidden, is_system: false, has_subdirs: false },
        depth,
        is_expanded: true,
    }
}

fn names(items: &[SidebarItem], show_hidden: bool) -> Vec<String> {
    visible_sidebar_items(items, show_hidden, true, true)
        .map(|item| match item {
            SidebarItem::Directory { node, .. } => node.name.clone(),
            _ => String::new(),
        })
        .collect()
}

#[test]
fn folders_under_a_hidden_folder_are_hidden_with_it() {
    let items = [
        folder("home", 0, false),
        folder("AppData", 1, true),
        folder("Local", 2, false),
        folder("Temp", 3, false),
        folder("Documents", 1, false),
    ];
    assert_eq!(names(&items, false), ["home", "Documents"]);
    assert_eq!(names(&items, true), ["home", "AppData", "Local", "Temp", "Documents"]);
}
