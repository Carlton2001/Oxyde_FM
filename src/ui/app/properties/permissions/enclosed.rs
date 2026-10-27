//! Contents tab: levels for everything inside the folders, as Nautilus's « Change
//! Permissions for Enclosed Files »: per class, for files and for folders, prefilled
//! from the umask. Nothing inside changes unless its checkbox is ticked before OK.
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
    let toggle = handle.clone();
    let apply = edit.enclosed_apply;
    body = body
        .child(
            div()
                .id("permissions-enclosed-apply")
                .flex()
                .items_start()
                .gap(rems(0.5))
                .min_w_0()
                .child(div().flex_none().mt(rems(0.0625)).child(crate::ui::elements::checkbox::checkbox(theme, apply, false)))
                .child(div().flex_1().min_w_0().child(cx.i18n().text(Message::PermissionsEnclosed)))
                .on_click(move |_, _, cx| update(&toggle, cx, move |edit| {
                    edit.menu = None;
                    edit.enclosed_apply = !apply;
                })),
        )
        .child(div().mt(rems(0.5)).text_color(theme.text_muted).child(cx.i18n().text(Message::PermissionsEnclosedHint)));
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
            // The levels are shown as text until the checkbox makes them apply.
            body = body.child(form_row(label, choice(menu, id, current, apply, handle, theme), compact, cx));
        }
    }
    body
}
