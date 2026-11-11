use super::{context_menu_sections, dropdown, GROUPS, HISTORY};
use crate::ui::i18n::{I18n, Message};

#[test]
fn sections_match_tauri_and_remove_empty_boundaries() {
    for language in [crate::ui::i18n::Language::French, crate::ui::i18n::Language::English] {
        let i18n = I18n::for_language(language);
        let action = |message| dropdown::DropdownItem::new(i18n.text(message).to_string(), |_, _| {});
        let items = context_menu_sections(vec![
            dropdown::DropdownItem::separator(),
            action(Message::Open), action(Message::OpenNewTab),
            action(Message::PinQuickAccess), action(Message::Cut), action(Message::Rename),
            action(Message::Delete), action(Message::Properties),
            dropdown::DropdownItem::separator(), dropdown::DropdownItem::separator(),
            action(Message::SortBy), action(Message::MoreOptions),
            dropdown::DropdownItem::separator(),
        ], &i18n);
        let boundaries: Vec<_> = items.iter().enumerate()
            .filter_map(|(index, item)| matches!(item.icon, Some(dropdown::IconName::Separator)).then_some(index))
            .collect();
        assert_eq!(boundaries, vec![2, 4, 6, 9, 11, 13]);
        assert!(!matches!(items.last().unwrap().icon, Some(dropdown::IconName::Separator)));
    }
}

fn labels_by_section(items: &[dropdown::DropdownItem]) -> Vec<Vec<String>> {
    items.split(|item| matches!(item.icon, Some(dropdown::IconName::Separator)))
        .map(|section| section.iter().map(|item| item.label.clone()).collect())
        .collect()
}

#[test]
fn file_menu_splits_actions_by_family() {
    let i18n = I18n::for_language(crate::ui::i18n::Language::English);
    let action = |message| dropdown::DropdownItem::new(i18n.text(message).to_string(), |_, _| {});
    let text = |message| i18n.text(message).to_string();
    let items = context_menu_sections(vec![
        action(Message::Open), action(Message::MapNetworkDrive), action(Message::PinQuickAccess),
        action(Message::Cut), action(Message::Copy), action(Message::Paste),
        action(Message::Compress), action(Message::OpExtractHere),
        dropdown::DropdownItem::new("Undo".to_string(), |_, _| {}).icon(dropdown::IconName::Undo),
        dropdown::DropdownItem::new("Redo".to_string(), |_, _| {}).icon(dropdown::IconName::Redo),
        action(Message::Rename), action(Message::Delete), action(Message::Properties),
    ], &i18n);
    assert_eq!(labels_by_section(&items), vec![
        vec![text(Message::Open)], vec![text(Message::MapNetworkDrive)],
        vec![text(Message::PinQuickAccess)],
        vec![text(Message::Cut), text(Message::Copy), text(Message::Paste)],
        vec![text(Message::Compress), text(Message::OpExtractHere)],
        vec!["Undo".to_string(), "Redo".to_string()],
        vec![text(Message::Rename), text(Message::Delete)], vec![text(Message::Properties)],
    ]);
}

#[test]
fn background_and_trash_menus_isolate_their_families() {
    let i18n = I18n::for_language(crate::ui::i18n::Language::English);
    let action = |message| dropdown::DropdownItem::new(i18n.text(message).to_string(), |_, _| {});
    let text = |message| i18n.text(message).to_string();
    let background = context_menu_sections(vec![
        action(Message::NewFolder), action(Message::SelectAll), action(Message::Paste),
        action(Message::Properties),
    ], &i18n);
    assert_eq!(labels_by_section(&background).len(), 4);
    let trash = context_menu_sections(vec![
        action(Message::Cut), action(Message::TrashRestore), action(Message::TrashDeletePermanent),
        dropdown::DropdownItem::separator(), action(Message::Properties),
    ], &i18n);
    assert_eq!(labels_by_section(&trash), vec![
        vec![text(Message::Cut)], vec![text(Message::TrashRestore)],
        vec![text(Message::TrashDeletePermanent)], vec![text(Message::Properties)],
    ]);
    let empty = context_menu_sections(vec![
        action(Message::TrashRestoreAll), action(Message::TrashDeleteAll),
    ], &i18n);
    assert_eq!(labels_by_section(&empty).len(), 2);
}

#[test]
fn tree_actions_and_ejection_get_their_own_sections() {
    let i18n = I18n::for_language(crate::ui::i18n::Language::English);
    let action = |message| dropdown::DropdownItem::new(i18n.text(message).to_string(), |_, _| {});
    let items = context_menu_sections(vec![
        action(Message::Open), action(Message::OpenNewTab), action(Message::ExpandAll),
        action(Message::CollapseAll), action(Message::EjectDrive), action(Message::Properties),
    ], &i18n);
    assert_eq!(labels_by_section(&items).iter().map(Vec::len).collect::<Vec<_>>(), vec![2, 2, 1, 1]);
    // Removing a whole section afterwards must not leave two separators.
    let filtered = context_menu_sections(items.into_iter()
        .filter(|item| item.label != i18n.text(Message::ExpandAll).as_ref()
            && item.label != i18n.text(Message::CollapseAll).as_ref())
        .collect(), &i18n);
    assert_eq!(labels_by_section(&filtered).iter().map(Vec::len).collect::<Vec<_>>(), vec![2, 1, 1]);
}

#[test]
fn every_section_label_names_one_section_in_every_language() {
    assert!(GROUPS[HISTORY].is_empty());
    for language in crate::ui::i18n::Language::ALL {
        let i18n = I18n::for_language(language);
        let mut sections = std::collections::HashMap::new();
        for (index, messages) in GROUPS.iter().enumerate() {
            for message in *messages {
                let label = i18n.text(*message).to_string();
                let previous = *sections.entry(label.clone()).or_insert(index);
                assert_eq!(previous, index, "{language:?}: \"{label}\" names two sections");
            }
        }
    }
}
