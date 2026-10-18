use super::dropdown::{DropdownItem, IconName};
use crate::ui::app::OxydeApp;
use crate::ui::i18n::{FluentArgs, I18n, Message};
use gpui::Entity;
use oxyde_lib::models::TransactionType;

pub(super) fn operation_label(i18n: &I18n, message: Message, operation: TransactionType) -> String {
    let operation = match operation {
        TransactionType::Copy => Message::OpCopy,
        TransactionType::Move => Message::OpMove,
        TransactionType::Rename => Message::OpRename,
        TransactionType::Delete => Message::OpDelete,
        TransactionType::NewFolder => Message::OpNewFolder,
    };
    let mut args = FluentArgs::new();
    args.set("operation", i18n.text(operation).to_string());
    i18n.text_with_args(message, &args).to_string()
}

pub(super) fn insert_menu_items(items: &mut Vec<DropdownItem>, i18n: &I18n, handle: Entity<OxydeApp>, view: &OxydeApp) {
    let position = items.iter().position(|item| {
        [Message::Rename, Message::Delete, Message::Properties, Message::MoreOptions].into_iter()
            .any(|message| item.label == i18n.text(message).as_ref())
    }).unwrap_or(items.len());
    items.splice(position..position, menu_items(i18n, handle, view));
}

fn menu_items(i18n: &I18n, handle: Entity<OxydeApp>, view: &OxydeApp) -> Vec<DropdownItem> {
    let summary = view.history.summary();
    [
        (true, summary.undo, Message::UndoOperation, IconName::Undo),
        (false, summary.redo, Message::RedoOperation, IconName::Redo),
    ]
    .into_iter()
    .filter_map(|(undo, operation, message, icon)| {
        if view.history_operation_in_progress || (undo && summary.undo_blocked_by_overwrite) {
            return None;
        }
        let label = operation_label(i18n, message, operation?);
        let handle = handle.clone();
        let item = DropdownItem::new(label, move |_, cx| {
            handle.update(cx, |view, cx| {
                view.context_menus.clear();
                if undo { view.execute_undo(cx); } else { view.execute_redo(cx); }
                cx.notify();
            });
        }).icon(icon);
        Some(item)
    })
    .collect()
}
