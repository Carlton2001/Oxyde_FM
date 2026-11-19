//! Keyboard actions belong to the displayed rows, never to copied business state.
use gpui::{prelude::*, *};
use std::collections::HashMap;
use std::rc::Rc;

pub(crate) type Action = Rc<dyn Fn(&mut Window, &mut App)>;
/// Left (`false`) and Right (`true`) on a row holding a value, such as a stepper.
pub(crate) type Adjust = Rc<dyn Fn(bool, &mut Window, &mut App)>;
struct Row {
    action: Action,
    submenu: Option<Action>,
    adjust: Option<Adjust>,
    scroll: Option<(ScrollHandle, usize)>,
    /// Laid out side by side with the menu's other rows: Left and Right move between them.
    horizontal: bool,
}
#[derive(Default)]
struct Navigation {
    selected: Option<usize>,
    /// Whether the pointer has moved over this menu: Enter then means no row by default.
    pointed: bool,
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
        .is_some_and(|nav| {
            nav.pointed = true;
            nav.selected.take().is_some()
        })
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
        "left" | "right" if nav.rows.iter().all(|row| row.horizontal) => {
            nav.selected = selection(nav.selected, count, if key == "left" { "up" } else { "down" });
            window.refresh();
        }
        "left" | "right" => {
            // Left is the menu's own key elsewhere: the caller closes a submenu with it.
            let adjust = nav.selected.and_then(|index| nav.rows.get(index)).and_then(|row| row.adjust.clone());
            if let Some(adjust) = adjust {
                adjust(key == "right", window, cx);
            } else if key == "left" {
                return false;
            } else {
                let Some(index) = keyboard_row(nav, count) else {
                    return false;
                };
                let Some(submenu) = nav.rows[index].submenu.clone() else {
                    return false;
                };
                submenu(window, cx);
            }
        }
        "enter" | "space" => {
            if let Some(index) = keyboard_row(nav, count) {
                let action = nav.rows[index].action.clone();
                action(window, cx);
            }
        }
        _ => return false,
    }
    true
}

/// The row Enter or Right acts on: the one chosen with the arrows, else the first while
/// only the keyboard has been used. Once the pointer has moved over the menu, a row it
/// rests on must not make Enter run the first one.
fn keyboard_row(nav: &Navigation, count: usize) -> Option<usize> {
    match nav.selected.filter(|index| *index < count) {
        Some(index) => Some(index),
        None if !nav.pointed && count > 0 => Some(0),
        None => None,
    }
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
    adjust: Option<Adjust>,
    horizontal: bool,
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
        adjust: None,
        horizontal: false,
    }
}
impl KeyboardItem {
    /// Left and Right change the row's value instead of leaving or opening a submenu.
    pub(crate) fn adjust(mut self, adjust: Adjust) -> Self {
        self.adjust = Some(adjust);
        self
    }
    /// One of a line of buttons, which Left and Right walk through as Up and Down do.
    pub(crate) fn horizontal(mut self) -> Self {
        self.horizontal = true;
        self
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
            adjust: self.adjust,
            horizontal: self.horizontal,
        });
        let selected = nav.selected == Some(index);
        let color = crate::ui::theme::OxydeTheme::global(cx).menu_hover;
        self.row.when(selected, |row| row.bg(color))
    }
}

#[cfg(test)]
#[path = "tests/keyboard.rs"]
mod tests;
