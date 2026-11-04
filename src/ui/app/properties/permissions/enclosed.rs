//! Levels for everything inside the folders, as Nautilus's « Change Permissions for
//! Enclosed Files »: per class, for files and for folders, prefilled from the umask.
//! Nothing inside changes unless its switch is on before OK.
use super::{form_row, kind_switch, levels, update, PermissionMenu, PermissionsEdit};
use crate::ui::app::properties::section::{card, rule, section, toggle_row};
use crate::ui::app::properties::PropertiesWindow;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::*;
use oxyde_lib::models::permissions::PermissionClass;

pub(super) fn render(
    body: Div,
    edit: &PermissionsEdit,
    owner: Option<&Entity<crate::ui::app::OxydeApp>>,
    handle: &Entity<PropertiesWindow>,
    compact: bool,
    theme: OxydeTheme,
    cx: &mut Context<PropertiesWindow>,
) -> Div {
    let toggle = handle.clone();
    let apply = edit.enclosed_apply;
    let mut content = card(theme).child(
        toggle_row("permissions-enclosed-apply", cx.i18n().text(Message::PermissionsEnclosed), apply, true, theme)
            .on_click(move |_, _, cx| update(&toggle, cx, move |edit| {
                edit.menu = None;
                edit.enclosed_apply = !apply;
            })),
    );
    // The levels appear once the switch makes them apply.
    if apply {
        let folders = edit.enclosed_folders_shown;
        content = content
            .child(div().text_size(rems(0.75)).text_color(theme.text_muted).child(cx.i18n().text(Message::PermissionsEnclosedHint)))
            .child(rule(theme))
            .child(kind_switch("permissions-enclosed-kind", folders, handle, theme, cx, |edit, folders| {
                edit.enclosed_folders_shown = folders
            }));
        for (class, label) in [
            (PermissionClass::Owner, Message::PermissionsOwner),
            (PermissionClass::Group, Message::PermissionsGroup),
            (PermissionClass::Others, Message::PermissionsOthers),
        ] {
            let menu = PermissionMenu::Enclosed { folders, class };
            let id = format!("permissions-enclosed-{folders}-{}", class.index());
            content = content.child(form_row(label, levels(menu, id, edit, true, owner, handle, theme, cx), compact, theme, cx));
        }
    }
    body.child(section(cx.i18n().text(Message::PropertiesContents), "icons/folder-tree.svg", content, theme))
}
