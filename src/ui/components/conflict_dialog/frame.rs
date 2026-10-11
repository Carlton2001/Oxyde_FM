use super::controls::{button, button_with_label, choose, edit, restore_focus};
use super::rules::decision;
use crate::ui::app::{OxydeApp, PendingFileOperation};
use crate::ui::components::modal::Modal;
use crate::ui::format::format_number;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::ConflictAction;

pub(super) fn render_frame(
    pending: &PendingFileOperation,
    body: Div,
    width: f32,
    handle: Entity<OxydeApp>,
    cx: &App,
) -> impl IntoElement {
    let t = *cx.theme();
    let advanced = pending.dialog.advanced;
    let corner_drag = handle.clone();
    let title = if advanced {
        cx.i18n().text(Message::ConflictAdvancedTitle).to_string()
    } else {
        let mut args = FluentArgs::new();
        args.set(
            "current",
            format_number((pending.dialog.selected + 1) as u64, cx.i18n().language()),
        );
        args.set(
            "total",
            format_number(pending.conflicts.len() as u64, cx.i18n().language()),
        );
        cx.i18n()
            .text_with_args(Message::ConflictStepTitle, &args)
            .to_string()
    };
    let cancel = handle.clone();
    let header = div()
        .flex()
        .items_center()
        .gap(rems(0.5))
        .w_full()
        .child(
            svg()
                .path("icons/copy-x.svg")
                .size(rems(1.0))
                .text_color(t.text_muted),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .whitespace_nowrap()
                .text_size(rems(0.875))
                .font_weight(FontWeight::MEDIUM)
                .child(title.clone()),
        )
        .when(!advanced, |d| {
            d.child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .mr(rems(1.0))
                    .text_size(rems(0.65))
                    .font_weight(FontWeight::BOLD)
                    .text_color(t.text_muted.opacity(0.7))
                    .child(format!(
                        "{} : {} {}",
                        cx.i18n()
                            .text(if pending.is_move {
                                Message::OpMove
                            } else {
                                Message::Copy
                            })
                            .to_uppercase(),
                        format_number(pending.paths.len() as u64, cx.i18n().language()),
                        cx.i18n().text(Message::ConflictItems).to_uppercase()
                    )),
            )
        })
        .child(
            div()
                .id("conflict-close")
                .p(rems(0.25))
                .hover(|d| d.bg(t.control_hover))
                .child(
                    svg()
                        .path("icons/x.svg")
                        .size(rems(1.0))
                        .text_color(t.text_muted),
                )
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    cancel.update(cx, |a, cx| a.cancel_pending_file_operation(cx));
                    restore_focus(&cancel, window, cx);
                }),
        );
    let mut modal = Modal::new("conflict-dialog", title, body)
        .width(rems(if advanced { width } else { 42.0 }))
        .header(header)
        .header_background(t.surface)
        .body_background(t.background)
        .body_padding(
            rems(if advanced { 0.0 } else { 1.25 }),
            rems(if advanced { 0.0 } else { 1.25 }),
        )
        .wrap_actions();
    if advanced {
        let all_resolved = pending
            .conflicts
            .iter()
            .all(|c| decision(&pending.dialog, &c.source.path).is_some());
        let (replace, skip) = pending.dialog.counts(&pending.conflicts);
        let mut args = FluentArgs::new();
        args.set("replace_count", replace);
        args.set("skip_count", skip);
        args.set(
            "replace_number",
            format_number(replace as u64, cx.i18n().language()),
        );
        args.set(
            "skip_number",
            format_number(skip as u64, cx.i18n().language()),
        );
        let apply_label = cx
            .i18n()
            .text_with_args(Message::ConflictApplySummary, &args);
        modal = modal
            .action(button(
                "conflict-back",
                Message::ConflictBack,
                false,
                handle.clone(),
                |h, cx| edit(h, cx, |p| p.dialog.advanced = false),
                cx,
            ))
            .action(div().flex_1())
            .action(
                button_with_label(
                    "conflict-apply",
                    Message::ConflictApply,
                    Some(apply_label),
                    all_resolved,
                    handle.clone(),
                    move |h, cx| {
                        if all_resolved {
                            h.update(cx, |a, cx| a.apply_conflict_decisions(cx));
                        }
                    },
                    cx,
                )
                .when(!all_resolved, |d| d.opacity(0.5)),
            );
    } else {
        modal = modal
            .action(button(
                "conflict-cancel",
                Message::Cancel,
                false,
                handle.clone(),
                |h, cx| h.update(cx, |a, cx| a.cancel_pending_file_operation(cx)),
                cx,
            ))
            .action(div().flex_1())
            .action(button(
                "conflict-advanced",
                Message::ConflictAdvanced,
                false,
                handle.clone(),
                |h, cx| edit(h, cx, |p| p.dialog.advanced = true),
                cx,
            ))
            .action(div().flex_1())
            .action(button(
                "conflict-skip-all",
                Message::ConflictSkipAll,
                false,
                handle.clone(),
                |h, cx| choose(h, ConflictAction::Skip, true, cx),
                cx,
            ))
            .action(button(
                "conflict-replace-all",
                Message::ConflictReplaceAll,
                true,
                handle.clone(),
                |h, cx| choose(h, ConflictAction::Replace, true, cx),
                cx,
            ));
    }
    if advanced {
        modal = modal.overlay(
            div()
                .absolute()
                .bottom(rems(0.125))
                .right(rems(0.125))
                .id("conflict-resize")
                .size(rems(0.875))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .path("icons/grip.svg")
                        .size(rems(0.75))
                        .flex_none()
                        .text_color(t.text_muted.opacity(0.5)),
                )
                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                    cx.stop_propagation();
                    edit(&corner_drag, cx, |p| {
                        p.dialog.resize =
                            Some((event.position, p.dialog.width, p.dialog.height, false));
                    });
                }),
        );
    }
    let resize_handle = handle.clone();
    let release_handle = handle.clone();
    let resize_events = canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            let moved = resize_handle.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                let Some((start, initial_width, initial_height, sidebar)) = moved
                    .read(cx)
                    .pending_file_operations
                    .front()
                    .and_then(|p| p.dialog.resize)
                else {
                    return;
                };
                let delta = event.position - start;
                let dx = delta.x / window.rem_size();
                let dy = delta.y / window.rem_size();
                let max_w = (window.viewport_size().width / window.rem_size() - 2.0).max(20.0);
                let max_h = (window.viewport_size().height / window.rem_size() - 13.0).max(10.0);
                edit(&moved, cx, |p| {
                    if !event.dragging() {
                        p.dialog.resize = None;
                        return;
                    }
                    if sidebar {
                        p.dialog.sidebar_width =
                            (initial_width + dx).clamp(10.0, (p.dialog.width - 20.0).max(10.0));
                    } else {
                        p.dialog.width =
                            (initial_width + 2.0 * dx).clamp(47.5_f32.min(max_w), max_w);
                        p.dialog.height =
                            (initial_height + 2.0 * dy).clamp(18.0_f32.min(max_h), max_h);
                    }
                });
            });
            let released = release_handle.clone();
            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Capture
                    && event.button == MouseButton::Left
                    && released
                        .read(cx)
                        .pending_file_operations
                        .front()
                        .is_some_and(|p| p.dialog.resize.is_some())
                {
                    edit(&released, cx, |p| p.dialog.resize = None);
                }
            });
        },
    )
    .absolute()
    .size_full();
    div()
        .absolute()
        .inset_0()
        .track_focus(pending.dialog.focus.as_ref().unwrap())
        .on_key_down(move |event, window, cx| {
            if event.keystroke.key != "tab" {
                cx.stop_propagation();
            }
            if event.keystroke.key == "escape" {
                handle.update(cx, |a, cx| a.cancel_pending_file_operation(cx));
                restore_focus(&handle, window, cx);
            }
        })
        .on_action(|_: &crate::ui::app::Ouvrir, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Copier, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::CopierNom, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::CopierChemin, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Couper, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Coller, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Renommer, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Supprimer, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::ToutSelectionner, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Annuler, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Refaire, _, cx| cx.stop_propagation())
        .on_action(|_: &crate::ui::app::Actualiser, _, cx| cx.stop_propagation())
        .child(resize_events)
        .child(modal)
}
