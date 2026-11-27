use super::{context_shortcut, DropdownItem, IconName};
use crate::ui::components::history_actions::operation_label;
use crate::ui::i18n::{I18n, Message};
use oxyde_lib::models::{preferences::Language, TransactionType};

#[test]
fn permanent_delete_keeps_del_without_shift() {
    for language in Language::ALL {
        let i18n = I18n::for_language(language);
        let label = i18n.text(Message::TrashDeletePermanent);
        let item = DropdownItem::new(label.to_string(), |_, _| {});
        assert_eq!(context_shortcut(&item, &i18n, false), Some("Del"));
        assert_eq!(context_shortcut(&item, &i18n, true), Some("Shift+Del"));
    }
}

#[test]
fn history_shortcuts_follow_the_operation() {
    for language in Language::ALL {
        let i18n = I18n::for_language(language);
        for (message, icon, shortcut) in [
            (Message::UndoOperation, IconName::Undo, "Ctrl+Z"),
            (Message::RedoOperation, IconName::Redo, "Ctrl+Y"),
        ] {
            let label = operation_label(&i18n, message, TransactionType::Rename);
            let item = DropdownItem::new(label, |_, _| {}).icon(icon);
            assert_eq!(context_shortcut(&item, &i18n, false), Some(shortcut));
        }

        let reopen = DropdownItem::new(i18n.text(Message::ReopenClosedTab).to_string(), |_, _| {})
            .icon(IconName::ReopenClosedTab);
        assert_eq!(context_shortcut(&reopen, &i18n, false), None);
    }
}
