//! Keyboard actions belong to the displayed rows, never to copied business state.
use gpui::{prelude::*, *};
use std::collections::HashMap;
use std::rc::Rc;

pub(crate) type Action = Rc<dyn Fn(&mut Window, &mut App)>;
struct Row {
    action: Action,
    submenu: Option<Action>,
    scroll: Option<(ScrollHandle, usize)>,
}
#[derive(Default)]
struct Navigation {
    selected: Option<usize>,
    rows: Vec<Row>,
}
#[derive(Default)]
struct Navigations(HashMap<WindowId, Navigation>);
impl Global for Navigations {}

pub(super) fn begin(window: &Window, cx: &mut App) {
    cx.default_global::<Navigations>()
        .0
        .entry(window.window_handle().window_id())
        .or_default()
        .rows
        .clear();
}
pub(super) fn remove(id: WindowId, cx: &mut App) {
    cx.default_global::<Navigations>().0.remove(&id);
}

pub(super) fn pointer(window: &mut Window, cx: &mut App) {
    if cx
        .default_global::<Navigations>()
        .0
        .get_mut(&window.window_handle().window_id())
        .is_some_and(|nav| nav.selected.take().is_some())
    {
        window.refresh();
    }
}

pub(super) fn key(key: &str, window: &mut Window, cx: &mut App) -> bool {
    let nav = cx
        .default_global::<Navigations>()
        .0
        .entry(window.window_handle().window_id())
        .or_default();
    if nav.rows.is_empty() {
        return false;
    }
    let count = nav.rows.len();
    match key {
        "up" | "down" | "home" | "end" => {
            nav.selected = selection(nav.selected, count, key);
            if let Some((scroll, index)) = &nav.rows[nav.selected.unwrap()].scroll {
                scroll.scroll_to_item(*index);
            }
            window.refresh();
        }
        "enter" | "space" | "right" => {
            let index = nav.selected.filter(|index| *index < count).unwrap_or(0);
            let row = &nav.rows[index];
            let callback = if key == "right" {
                row.submenu.clone()
            } else {
                Some(row.action.clone())
            };
            let Some(callback) = callback else {
                return false;
            };
            callback(window, cx);
        }
        _ => return false,
    }
    true
}

fn selection(current: Option<usize>, count: usize, key: &str) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match key {
        "home" => 0,
        "end" => count - 1,
        "up" => current
            .filter(|index| *index < count)
            .map_or(count - 1, |index| (index + count - 1) % count),
        _ => current
            .filter(|index| *index < count)
            .map_or(0, |index| (index + 1) % count),
    })
}

#[derive(IntoElement)]
pub(crate) struct KeyboardItem {
    row: Stateful<Div>,
    action: Action,
    submenu: Option<Action>,
    scroll: Option<(ScrollHandle, usize)>,
}
pub(crate) fn item(
    row: Stateful<Div>,
    action: Action,
    submenu: Option<Action>,
    scroll: Option<(ScrollHandle, usize)>,
) -> KeyboardItem {
    KeyboardItem {
        row,
        action,
        submenu,
        scroll,
    }
}
impl RenderOnce for KeyboardItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if !super::is_menu(window, cx) {
            return self.row;
        }
        let nav = cx
            .default_global::<Navigations>()
            .0
            .entry(window.window_handle().window_id())
            .or_default();
        let index = nav.rows.len();
        nav.rows.push(Row {
            action: self.action,
            submenu: self.submenu,
            scroll: self.scroll,
        });
        let selected = nav.selected == Some(index);
        let color = crate::ui::theme::OxydeTheme::global(cx).menu_hover;
        self.row.when(selected, |row| row.bg(color))
    }
}

#[cfg(test)]
#[path = "tests/keyboard.rs"]
mod tests;
