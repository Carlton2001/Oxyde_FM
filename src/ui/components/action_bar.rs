use gpui::*;
use gpui::prelude::FluentBuilder;
use crate::ui::app::OxydeApp;
use crate::ui::components::tooltip::Tooltip;
use oxyde_lib::models::TransactionType;

fn op_label(op: &TransactionType) -> &'static str {
    match op {
        TransactionType::Copy      => "Copie",
        TransactionType::Move      => "Déplacement",
        TransactionType::Rename    => "Renommage",
        TransactionType::Delete    => "Suppression",
        TransactionType::NewFolder => "Nouveau dossier",
        TransactionType::Restore   => "Restauration",
    }
}

pub fn render_action_bar(
    view: &OxydeApp,
    handle: Entity<OxydeApp>,
    _cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let history_state = view.history.get_state();
    let can_undo = !history_state.undo_stack.is_empty();
    let can_redo = !history_state.redo_stack.is_empty();

    let undo_tooltip = history_state.undo_stack.last()
        .map(|t| format!("Annuler : {}", op_label(&t.op_type)))
        .unwrap_or_else(|| "Rien à annuler".to_string());

    let redo_tooltip = history_state.redo_stack.last()
        .map(|t| format!("Rétablir : {}", op_label(&t.op_type)))
        .unwrap_or_else(|| "Rien à rétablir".to_string());

    div()
        .id("action-bar")
        .h(px(28.0))
        .w_full()
        .flex()
        .items_center()
        .px_2()
        .gap_1()
        .bg(rgb(0x0c0c0c))
        .border_b_1()
        .border_color(rgb(0x1f1f1f))
        // Bouton Annuler
        .child({
            let color = if can_undo { rgb(0x888888) } else { rgb(0x333333) };
            let h = handle.clone();
            let mut btn = div()
                .id("undo-btn")
                .w_6()
                .h_6()
                .flex()
                .items_center()
                .justify_center()
                .rounded_md();

            if can_undo {
                btn = btn
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0x1a1a1a)))
                    .tooltip(Tooltip::with_meta("Annuler", undo_tooltip))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let _ = h.update(cx, |this, cx| this.execute_undo(cx));
                    });
            } else {
                btn = btn.tooltip(Tooltip::text("Rien à annuler"));
            }

            btn.child(
                svg()
                    .path("icons/undo.svg")
                    .size_4()
                    .text_color(color)
            )
        })
        // Bouton Rétablir
        .child({
            let color = if can_redo { rgb(0x888888) } else { rgb(0x333333) };
            let h = handle.clone();
            let mut btn = div()
                .id("redo-btn")
                .w_6()
                .h_6()
                .flex()
                .items_center()
                .justify_center()
                .rounded_md();

            if can_redo {
                btn = btn
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0x1a1a1a)))
                    .tooltip(Tooltip::with_meta("Rétablir", redo_tooltip))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        let _ = h.update(cx, |this, cx| this.execute_redo(cx));
                    });
            } else {
                btn = btn.tooltip(Tooltip::text("Rien à rétablir"));
            }

            btn.child(
                svg()
                    .path("icons/redo.svg")
                    .size_4()
                    .text_color(color)
            )
        })
        // Séparateur vertical
        .child(
            div()
                .w(px(1.0))
                .h(px(16.0))
                .bg(rgb(0x2a2a2a))
                .mx_1()
        )
        // Label de la dernière opération (discret, en muted)
        .child({
            let label = if can_undo {
                history_state.undo_stack.last()
                    .map(|t| format!("Dernière action : {}", op_label(&t.op_type)))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            div()
                .text_color(rgb(0x444444))
                .text_size(px(11.0))
                .when(!label.is_empty(), |s| s.child(label))
        })
}
