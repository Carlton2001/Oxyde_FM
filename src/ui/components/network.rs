use crate::ui::app::OxydeApp;
use crate::ui::components::dropdown::{DropdownItem, IconName};
use crate::ui::i18n::{I18n, Message};
use gpui::Entity;

pub(crate) fn insert_after_open(
    mut items: Vec<DropdownItem>,
    actions: Vec<DropdownItem>,
    i18n: &I18n,
) -> Vec<DropdownItem> {
    let index = items
        .iter()
        .rposition(|item| {
            item.label == i18n.text(Message::Open).as_ref()
                || item.label == i18n.text(Message::OpenNewTab).as_ref()
        })
        .map_or(0, |index| index + 1);
    items.splice(index..index, actions);
    items
}

pub(crate) fn network_actions(
    i18n: &I18n,
    handle: Entity<OxydeApp>,
    path: &str,
    remote_drive: bool,
) -> Vec<DropdownItem> {
    if !cfg!(target_os = "windows") {
        return Vec::new();
    }
    let mut items = Vec::new();
    let neighborhood = path == "__network_vincinity__";
    if neighborhood || crate::ui::app::network::mappable_network_path(path) {
        let h = handle.clone();
        let path = if neighborhood {
            None
        } else {
            Some(path.to_string())
        };
        items.push(
            DropdownItem::new(
                i18n.text(Message::MapNetworkDrive).to_string(),
                move |_, cx| {
                    h.update(cx, |this, cx| {
                        this.network_drive_dialog(false, path.clone(), cx)
                    });
                },
            )
            .icon(IconName::Server),
        );
    }
    if neighborhood || remote_drive {
        let path = if neighborhood {
            None
        } else {
            Some(path.to_string())
        };
        items.push(
            DropdownItem::new(
                i18n.text(Message::DisconnectNetworkDrive).to_string(),
                move |_, cx| {
                    handle.update(cx, |this, cx| {
                        this.network_drive_dialog(true, path.clone(), cx)
                    });
                },
            )
            .icon(IconName::ServerOff),
        );
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_location_menu_places_drive_actions_between_separators_after_open() {
        for language in [
            crate::ui::i18n::Language::French,
            crate::ui::i18n::Language::English,
        ] {
            let i18n = I18n::for_language(language);
            let action = |message| DropdownItem::new(i18n.text(message).to_string(), |_, _| {});
            let items = insert_after_open(
                vec![action(Message::Open), action(Message::Properties)],
                vec![
                    action(Message::MapNetworkDrive).icon(IconName::Server),
                    action(Message::DisconnectNetworkDrive).icon(IconName::ServerOff),
                ],
                &i18n,
            );
            let items = crate::ui::components::context_menu_sections(items, &i18n);
            assert_eq!(items.len(), 6);
            assert_eq!(items[0].label, i18n.text(Message::Open).as_ref());
            assert!(matches!(items[1].icon, Some(IconName::Separator)));
            assert_eq!(items[2].label, i18n.text(Message::MapNetworkDrive).as_ref());
            assert!(matches!(items[2].icon, Some(IconName::Server)));
            assert_eq!(
                items[3].label,
                i18n.text(Message::DisconnectNetworkDrive).as_ref()
            );
            assert!(matches!(items[3].icon, Some(IconName::ServerOff)));
            assert!(matches!(items[4].icon, Some(IconName::Separator)));
            assert_eq!(items[5].label, i18n.text(Message::Properties).as_ref());
        }
    }
}
