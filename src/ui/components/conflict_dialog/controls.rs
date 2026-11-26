use super::rules::{decision, filtered_indices};
use crate::ui::app::{OxydeApp, PendingFileOperation};
use crate::ui::elements::button::action;
use crate::ui::elements::button::{button_style, ButtonVariant};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::ConflictAction;
use std::collections::HashSet;

pub(super) fn visible(p: &PendingFileOperation) -> Vec<usize> {
    filtered_indices(&p.conflicts, &p.dialog)
}
pub(super) fn restore_focus(handle: &Entity<OxydeApp>, window: &mut Window, cx: &mut App) {
    if handle.read(cx).pending_file_operations.is_empty() {
        window.remove_window();
    }
}
pub(super) fn edit(
    handle: &Entity<OxydeApp>,
    cx: &mut App,
    update: impl FnOnce(&mut PendingFileOperation),
) {
    handle.update(cx, |app, cx| {
        if let Some(p) = app.pending_file_operations.front_mut() {
            update(p);
        }
        cx.notify();
    });
}
pub(super) fn choose(handle: &Entity<OxydeApp>, action: ConflictAction, all: bool, cx: &mut App) {
    handle.update(cx, |app, cx| {
        let Some(p) = app.pending_file_operations.front_mut() else {
            return;
        };
        let indices = if all {
            if p.dialog.advanced {
                visible(p)
            } else {
                (0..p.conflicts.len()).collect()
            }
        } else {
            vec![p.dialog.selected]
        };
        p.dialog.record_choices(&p.conflicts, &indices, action);
        let visible_indices: HashSet<_> = visible(p).into_iter().collect();
        let next = (p.dialog.selected + 1..p.conflicts.len())
            .chain(0..p.dialog.selected)
            .find(|&i| {
                (decision(&p.dialog, &p.conflicts[i].source.path).is_none()
                    || (p.dialog.advanced
                        && p.dialog.automatic.contains(&p.conflicts[i].source.path)))
                    && (!p.dialog.advanced || visible_indices.contains(&i))
            });
        if let Some(i) = next {
            p.dialog.selected = i;
        }
        if !p.dialog.advanced && (all || next.is_none()) {
            app.apply_conflict_decisions(cx);
        }
        cx.notify();
    });
}
pub(super) fn button(
    id: &'static str,
    message: Message,
    primary: bool,
    handle: Entity<OxydeApp>,
    callback: impl Fn(&Entity<OxydeApp>, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    button_with_label(id, message, None, primary, handle, callback, cx)
}
pub(super) fn button_with_label(
    id: &'static str,
    message: Message,
    label: Option<SharedString>,
    primary: bool,
    handle: Entity<OxydeApp>,
    callback: impl Fn(&Entity<OxydeApp>, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    let t = *cx.theme();
    let icon = match message {
        Message::ConflictSkip => Some("icons/ban.svg"),
        Message::ConflictReplace => Some("icons/check.svg"),
        Message::ConflictAdvanced => Some("icons/settings-2.svg"),
        _ => None,
    };
    let callback = std::rc::Rc::new(callback);
    let click_callback = callback.clone();
    let keyboard_handle = handle.clone();
    button_style(
        action(div().id(id)),
        t,
        if primary {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        },
        true,
    )
    .tab_index(0)
    .flex_none()
    .whitespace_nowrap()
    .rounded(crate::ui::elements::button::ACTION_BUTTON_RADIUS)
    .border(rems(0.0625))
    .when(primary, |d| d.border_color(gpui::transparent_black()))
    .focus(|d| d.border_color(t.accent))
    .flex()
    .items_center()
    .gap(rems(0.375))
    .when_some(icon, |d, path| {
        d.child(
            svg()
                .path(path)
                .size(rems(0.875))
                .flex_none()
                .text_color(if primary { t.text_on_accent } else { t.text }),
        )
    })
    .child(label.unwrap_or_else(|| cx.i18n().text(message)))
    // Keys are handled below, on press.
    .on_click(move |event, window, cx| {
        if accepts_click(event.is_keyboard(), event.click_count()) {
            click_callback(&handle, cx);
            restore_focus(&handle, window, cx);
        }
    })
    .on_key_down(move |event, window, cx| {
        if is_activation_key(&event.keystroke.key) {
            cx.stop_propagation();
            if !event.is_held {
                callback(&keyboard_handle, cx);
                restore_focus(&keyboard_handle, window, cx);
            }
        }
    })
}
/// A choice moves on to the next conflict, under the pointer: the second click of a
/// double-click would answer a conflict the user has not seen.
fn accepts_click(keyboard: bool, click_count: usize) -> bool {
    !keyboard && click_count <= 1
}
fn is_activation_key(key: &str) -> bool {
    matches!(key, "enter" | "space")
}
/// Conflict dialog tag, in the tinted style of the operation badges.
pub(super) fn tag(message: Message, color: Hsla, cx: &App) -> Div {
    div()
        .flex_none()
        .whitespace_nowrap()
        .px(rems(0.375))
        .py(rems(0.0625))
        .rounded(rems(0.375))
        .text_size(rems(0.6875))
        .font_weight(FontWeight::MEDIUM)
        .bg(color.opacity(0.10))
        .text_color(color)
        .child(cx.i18n().text(message))
}
pub(super) fn automatic_badge(cx: &App) -> impl IntoElement {
    tag(Message::ConflictAutomatic, cx.theme().text_muted, cx)
}

/// The row Up, Down, Home or End selects among the shown `rows`, with its position.
pub(super) fn next_row(rows: &[usize], selected: usize, key: &str) -> Option<(usize, usize)> {
    let last = rows.len().checked_sub(1)?;
    let current = rows.iter().position(|&row| row == selected);
    let position = match key {
        "up" => current.map_or(0, |position| position.saturating_sub(1)),
        "down" => current.map_or(0, |position| (position + 1).min(last)),
        "home" => 0,
        "end" => last,
        _ => return None,
    };
    Some((rows[position], position))
}

/// Up, Down, Home and End move through the advanced list; Left and Right fold
/// and unfold the selected folder. `false` when the key is not one of them.
pub(super) fn list_key(p: &mut PendingFileOperation, key: &str) -> bool {
    if !p.dialog.advanced {
        return false;
    }
    if matches!(key, "left" | "right") {
        let Some(conflict) = p.conflicts.get(p.dialog.selected).filter(|conflict| conflict.source.is_dir) else {
            return false;
        };
        let path = conflict.source.path.clone();
        if key == "left" { p.dialog.collapsed.insert(path); } else { p.dialog.collapsed.remove(&path); }
        return true;
    }
    let rows = visible(p);
    let Some((row, position)) = next_row(&rows, p.dialog.selected, key) else {
        return false;
    };
    p.dialog.selected = row;
    p.dialog.list_scroll.scroll_to_item(position, ScrollStrategy::Nearest);
    true
}

#[cfg(test)]
#[path = "controls/tests/activation.rs"]
mod tests;

#[cfg(test)]
#[path = "controls/tests/keyboard.rs"]
mod keyboard_tests;
