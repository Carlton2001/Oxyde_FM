//! Levels for everything inside a folder, as Nautilus's « Change Permissions for
//! Enclosed Files »: per class, for files and for folders, Unchanged by default.
use super::{choice, form_row, update, PermissionMenu, PermissionsEdit};
use crate::ui::app::properties::{general, PropertiesWindow};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::*;
use oxyde_lib::models::permissions::PermissionClass;

pub(super) fn render(
    mut body: Div,
    edit: &PermissionsEdit,
    handle: &Entity<PropertiesWindow>,
    compact: bool,
    theme: OxydeTheme,
    cx: &mut Context<PropertiesWindow>,
) -> Div {
    let back = handle.clone();
    body = body
        .child(
            div()
                .id("permissions-enclosed-back")
                .flex()
                .items_center()
                .gap(rems(0.375))
                .text_color(theme.accent)
                .child(svg().path("icons/chevron-left.svg").size(rems(0.875)).flex_none().text_color(theme.accent))
                .child(cx.i18n().text(Message::PropertiesPermissions))
                .on_click(move |_, _, cx| update(&back, cx, |edit| {
                    edit.menu = None;
                    edit.enclosed_page = false;
                })),
        )
        .child(div().mt(rems(0.75)).mb(rems(0.5)).text_color(theme.text_muted).child(cx.i18n().text(Message::PermissionsEnclosedHint)));
    for (folders, title) in [(false, Message::PermissionsFiles), (true, Message::PermissionsFolders)] {
        body = body
            .child(general::divider(theme))
            .child(div().mb(rems(0.25)).font_weight(FontWeight::MEDIUM).child(cx.i18n().text(title)));
        for (class, label) in [
            (PermissionClass::Owner, Message::PermissionsOwner),
            (PermissionClass::Group, Message::PermissionsGroup),
            (PermissionClass::Others, Message::PermissionsOthers),
        ] {
            let menu = PermissionMenu::Enclosed { folders, class };
            let current = edit.entries(menu, cx).into_iter().find(|(_, selected, _)| *selected).map(|(label, ..)| label).unwrap_or_default();
            let id = ElementId::Name(format!("permissions-enclosed-{}-{}", folders, class.index()).into());
            body = body.child(form_row(label, choice(edit, menu, id, current, handle, theme), compact, cx));
        }
    }
    body
}
