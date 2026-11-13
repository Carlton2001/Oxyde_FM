use super::controls::{button, button_with_label, choose, edit, restore_focus};
use super::rules::decision;
use crate::ui::app::{OxydeApp, PendingFileOperation};
use crate::ui::components::modal::Modal;
use crate::ui::format::format_number;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::ConflictAction;

pub(super) fn render_frame(
    pending: &PendingFileOperation,
    body: Div,
    simple_width: Option<Pixels>,
    handle: Entity<OxydeApp>,
    window: &mut Window,
    cx: &App,
) -> impl IntoElement {
    let advanced = pending.dialog.advanced;
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
    window.set_window_title(&title);
    let header = crate::ui::components::tool_window::header(
        "icons/copy-x.svg", title.clone().into(), window, advanced, cx,
        move |window, cx| {
            cancel.update(cx, |app, cx| app.cancel_pending_file_operation(cx));
            restore_focus(&cancel, window, cx);
        },
    );
    let mut modal = Modal::new("conflict-dialog", title, body)
        .fill_window(advanced)
        .content_width(simple_width)
        .header(header)
        .body_padding(
            rems(if advanced { 0.0 } else { 1.25 }),
            rems(if advanced { 0.0 } else { 1.25 }),
        );
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
                let Some((start, initial_width)) = moved
                    .read(cx)
                    .pending_file_operations
                    .front()
                    .and_then(|p| p.dialog.sidebar_resize)
                else {
                    return;
                };
                let delta = event.position - start;
                let dx = delta.x / window.rem_size();
                edit(&moved, cx, |p| {
                    if !event.dragging() {
                        p.dialog.sidebar_resize = None;
                        return;
                    }
                    p.dialog.sidebar_width =
                        (initial_width + dx).clamp(super::MIN_SIDEBAR_WIDTH, super::max_sidebar_width(p.dialog.width));
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
                        .is_some_and(|p| p.dialog.sidebar_resize.is_some())
                {
                    edit(&released, cx, |p| p.dialog.sidebar_resize = None);
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
