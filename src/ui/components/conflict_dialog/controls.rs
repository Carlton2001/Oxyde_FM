use super::rules::{decision, filtered_indices};
use crate::ui::app::{OxydeApp, PendingFileOperation};
use crate::ui::components::modal::action_button;
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
        for i in indices {
            p.dialog.record_choice(&p.conflicts, i, action);
        }
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
        action_button(id),
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
    .rounded_sm()
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
    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
        click_callback(&handle, cx);
        restore_focus(&handle, window, cx);
    })
    .on_key_down(move |event, window, cx| {
        if event.keystroke.key == "enter" || event.keystroke.key == "space" {
            cx.stop_propagation();
            callback(&keyboard_handle, cx);
            restore_focus(&keyboard_handle, window, cx);
        }
    })
}
pub(super) fn automatic_badge(cx: &App) -> impl IntoElement {
    div()
        .flex_none()
        .whitespace_nowrap()
        .px(rems(0.25))
        .rounded_sm()
        .border(rems(0.0625))
        .border_color(cx.theme().border)
        .text_size(rems(0.625))
        .text_color(cx.theme().text_muted)
        .child(cx.i18n().text(Message::ConflictAutomatic))
}
