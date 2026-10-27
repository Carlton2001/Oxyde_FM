//! The permissions of the item, or of the selection: owner, group and others, each
//! with its access, for files and for folders when the selection has both.
use super::{choice, form_row, level_label, plain, update, PermissionMenu, PermissionsEdit};
use crate::ui::app::properties::{general, PropertiesDialog, PropertiesWindow};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use oxyde_lib::models::permissions::{executable, Account, PermissionClass, UnixPermissions};

/// The account chosen among those offered, or the item's own.
fn account_name(offered: &[Account], own: &Account, id: u32) -> String {
    offered
        .iter()
        .chain([own])
        .find(|account| account.id == id)
        .map_or_else(|| id.to_string(), |account| account.name.clone())
}

/// As Nautilus for a selection: the account the items share, or « Multiple ».
fn shared_name(edit: &PermissionsEdit, account: fn(&UnixPermissions) -> &Account, cx: &App) -> String {
    let mut accounts = edit.items.iter().map(|item| account(&item.loaded));
    let first = accounts.next().expect("an item");
    if accounts.all(|other| other.id == first.id) {
        first.name.clone()
    } else {
        cx.i18n().text(Message::PermissionsMultiple).to_string()
    }
}

pub(super) fn render(
    mut body: Div,
    dialog: &PropertiesDialog,
    edit: &PermissionsEdit,
    handle: &Entity<PropertiesWindow>,
    compact: bool,
    theme: OxydeTheme,
    cx: &mut Context<PropertiesWindow>,
) -> Div {
    let both = edit.has_kind(false) && edit.has_kind(true);
    // One access row per class, or one for folders and one for files, as Nautilus.
    let access = |mut body: Div, class: PermissionClass, label: Message, cx: &App| {
        for folders in [true, false].into_iter().filter(|folders| edit.has_kind(*folders)) {
            let menu = PermissionMenu::Access { folders, class };
            let id = ElementId::Name(format!("permissions-access-{folders}-{}", class.index()).into());
            let current = level_label(edit.shown(folders, class, false), folders, class, cx);
            let value = choice(menu, id, current, edit.editable(folders), handle, theme);
            let label = match (both, folders) {
                (false, _) => label,
                (true, true) => Message::PermissionsFolderAccess,
                (true, false) => Message::PermissionsFileAccess,
            };
            body = body.child(form_row(label, value, compact, cx));
        }
        body
    };
    if !edit.all_editable() {
        body = body.child(div().mb(rems(0.75)).text_color(theme.text_muted).child(cx.i18n().text(Message::PermissionsNotOwner)));
    }
    let owner = match edit.single() {
        Some(loaded) if loaded.editable && !loaded.owners.is_empty() => {
            let name = account_name(&loaded.owners, &loaded.owner, edit.owner);
            choice(PermissionMenu::Owner, "permissions-owner-choice".into(), name, true, handle, theme)
        }
        Some(loaded) => plain(loaded.owner.name.clone()),
        None => plain(shared_name(edit, |loaded| &loaded.owner, cx)),
    };
    body = body.child(form_row(Message::PermissionsOwner, owner, compact, cx));
    body = access(body, PermissionClass::Owner, Message::PermissionsAccess, cx).child(general::divider(theme));
    let group = match edit.single() {
        Some(loaded) if loaded.editable && loaded.groups.len() > 1 => {
            let name = account_name(&loaded.groups, &loaded.group, edit.group);
            choice(PermissionMenu::Group, "permissions-group".into(), name, true, handle, theme)
        }
        Some(loaded) => plain(loaded.group.name.clone()),
        None => plain(shared_name(edit, |loaded| &loaded.group, cx)),
    };
    body = body.child(form_row(Message::PermissionsGroup, group, compact, cx));
    body = access(body, PermissionClass::Group, Message::PermissionsAccess, cx).child(general::divider(theme));
    if both {
        body = body.child(div().mb(rems(0.25)).font_weight(FontWeight::MEDIUM).child(cx.i18n().text(Message::PermissionsOthers)));
    }
    body = access(body, PermissionClass::Others, Message::PermissionsOthers, cx);
    // As Nautilus, through GIO: execution is offered for a program or text alone.
    let executable_type = dialog
        .properties
        .as_ref()
        .and_then(|properties| properties.mime_type.as_ref())
        .is_some_and(|mime| mime.can_be_executable);
    if let Some(loaded) = edit.single().filter(|loaded| !loaded.is_dir && executable_type) {
        let toggle = handle.clone();
        let editable = loaded.editable;
        let checked = executable(edit.mode(loaded));
        body = body.child(general::divider(theme)).child(form_row(
            Message::PermissionsExecution,
            div()
                .id("permissions-executable")
                .flex()
                .items_start()
                .gap(rems(0.5))
                .min_w_0()
                .when(!editable, |field| field.opacity(0.5))
                .child(div().flex_none().mt(rems(0.0625)).child(crate::ui::elements::checkbox::checkbox(theme, checked, false)))
                .child(div().flex_1().min_w_0().child(cx.i18n().text(Message::PermissionsExecutable)))
                .when(editable, |field| {
                    field.on_click(move |_, _, cx| update(&toggle, cx, move |edit| edit.executable = Some(!checked)))
                }),
            compact,
            cx,
        ));
    }
    // As Nautilus: offered when every item is a folder its user may change.
    if !edit.has_kind(false) && edit.all_editable() {
        let open = handle.clone();
        body = body.child(general::divider(theme)).child(
            div()
                .flex()
                .flex_col()
                .items_start()
                .gap(rems(0.5))
                .child(
                    crate::ui::components::tool_window::action_button(
                        "permissions-enclosed",
                        cx.i18n().text(Message::PermissionsEnclosed),
                        false,
                        cx,
                    )
                    .on_click(move |_, _, cx| update(&open, cx, |edit| {
                        edit.menu = None;
                        edit.enclosed_page = true;
                    })),
                )
                .when(edit.enclosed.is_some(), |section| {
                    section.child(div().text_color(theme.text_muted).child(cx.i18n().text(Message::PermissionsEnclosedPending)))
                }),
        );
    }
    body
}
