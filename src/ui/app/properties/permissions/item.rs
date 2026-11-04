//! The permissions of the item, or of the selection: owner, group and others, each
//! with its access as segments, for files or for folders when the selection has both.
use super::{choice, form_row, kind_switch, levels, plain, update, PermissionMenu, PermissionsEdit};
use crate::ui::app::properties::section::{card, rule, section, toggle_row};
use crate::ui::app::properties::{PropertiesDialog, PropertiesWindow};
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
    app: Option<&Entity<crate::ui::app::OxydeApp>>,
    handle: &Entity<PropertiesWindow>,
    compact: bool,
    theme: OxydeTheme,
    cx: &mut Context<PropertiesWindow>,
) -> Div {
    let both = edit.has_kind(false) && edit.has_kind(true);
    // A selection holding both shows the levels of one kind at a time.
    let folders = if both { edit.folders_shown } else { edit.has_kind(true) };
    if !edit.all_editable() {
        body = body.child(
            div()
                .flex()
                .items_start()
                .gap(rems(0.5))
                .px(rems(0.25))
                .text_color(theme.text_muted)
                .child(svg().path("icons/lock.svg").size(rems(0.875)).mt(rems(0.1875)).flex_none().text_color(theme.text_muted))
                .child(div().flex_1().min_w_0().child(cx.i18n().text(Message::PermissionsNotOwner))),
        );
    }
    let owner = match edit.single() {
        Some(loaded) if loaded.editable && !loaded.owners.is_empty() => {
            let name = account_name(&loaded.owners, &loaded.owner, edit.owner);
            choice(PermissionMenu::Owner, "permissions-owner-choice".into(), name, true, handle, theme)
        }
        Some(loaded) => plain(loaded.owner.name.clone()),
        None => plain(shared_name(edit, |loaded| &loaded.owner, cx)),
    };
    let group = match edit.single() {
        Some(loaded) if loaded.editable && loaded.groups.len() > 1 => {
            let name = account_name(&loaded.groups, &loaded.group, edit.group);
            choice(PermissionMenu::Group, "permissions-group".into(), name, true, handle, theme)
        }
        Some(loaded) => plain(loaded.group.name.clone()),
        None => plain(shared_name(edit, |loaded| &loaded.group, cx)),
    };
    body = body.child(section(
        cx.i18n().text(Message::PermissionsSectionAccounts),
        "icons/users.svg",
        card(theme)
            .child(form_row(Message::PermissionsOwner, owner, compact, theme, cx))
            .child(form_row(Message::PermissionsGroup, group, compact, theme, cx)),
        theme,
    ));
    let mut access = card(theme);
    if both {
        access = access
            .child(kind_switch("permissions-kind", folders, handle, theme, cx, |edit, folders| edit.folders_shown = folders))
            .child(rule(theme));
    }
    for (class, label) in [
        (PermissionClass::Owner, Message::PermissionsOwner),
        (PermissionClass::Group, Message::PermissionsGroup),
        (PermissionClass::Others, Message::PermissionsOthers),
    ] {
        let menu = PermissionMenu::Access { folders, class };
        let id = format!("permissions-access-{folders}-{}", class.index());
        access = access.child(form_row(label, levels(menu, id, edit, edit.editable(folders), app, handle, theme, cx), compact, theme, cx));
    }
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
        access = access.child(rule(theme)).child(
            toggle_row("permissions-executable", cx.i18n().text(Message::PermissionsExecutable), checked, editable, theme)
                .when(editable, |row| {
                    row.on_click(move |_, _, cx| update(&toggle, cx, move |edit| edit.executable = Some(!checked)))
                }),
        );
    }
    body.child(section(cx.i18n().text(Message::PermissionsAccess), "icons/key.svg", access, theme))
}
