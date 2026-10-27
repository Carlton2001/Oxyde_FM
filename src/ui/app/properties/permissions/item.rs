//! The item's own permissions.
use super::{access_label, choice, form_row, update, PermissionMenu, PermissionsEdit};
use crate::ui::app::properties::{general, PropertiesDialog, PropertiesWindow};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use oxyde_lib::models::permissions::{executable, with_executable, PermissionClass};

fn group_name(edit: &PermissionsEdit) -> String {
    edit.loaded
        .groups
        .iter()
        .chain([&edit.loaded.group])
        .find(|group| group.id == edit.group)
        .map_or_else(|| edit.group.to_string(), |group| group.name.clone())
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
    let access = |class, id: &'static str, label: Message, cx: &App| {
        let value = choice(edit, PermissionMenu::Access(class), id.into(), access_label(edit, class, cx), handle, theme);
        form_row(label, value, compact, cx)
    };
    if !edit.loaded.editable {
        body = body.child(div().mb(rems(0.75)).text_color(theme.text_muted).child(cx.i18n().text(Message::PermissionsNotOwner)));
    }
    body = body
        .child(form_row(Message::PermissionsOwner, div().child(edit.loaded.owner.name.clone()), compact, cx))
        .child(access(PermissionClass::Owner, "permissions-owner", Message::PermissionsAccess, cx))
        .child(general::divider(theme));
    let group = if edit.loaded.groups.len() > 1 {
        choice(edit, PermissionMenu::Group, "permissions-group".into(), group_name(edit), handle, theme)
    } else {
        div().child(group_name(edit)).into_any_element()
    };
    body = body
        .child(form_row(Message::PermissionsGroup, group, compact, cx))
        .child(access(PermissionClass::Group, "permissions-group-access", Message::PermissionsAccess, cx))
        .child(general::divider(theme))
        .child(access(PermissionClass::Others, "permissions-others-access", Message::PermissionsOthers, cx));
    // As Nautilus, through GIO: execution is offered for programs and text only.
    let executable_type = dialog
        .properties
        .as_ref()
        .and_then(|properties| properties.mime_type.as_ref())
        .is_some_and(|mime| mime.can_be_executable);
    if !edit.loaded.is_dir && executable_type {
        let toggle = handle.clone();
        let editable = edit.loaded.editable;
        body = body.child(general::divider(theme)).child(form_row(
            Message::PermissionsExecution,
            div()
                .id("permissions-executable")
                .flex()
                .items_start()
                .gap(rems(0.5))
                .min_w_0()
                .when(!editable, |field| field.opacity(0.5))
                .child(div().flex_none().mt(rems(0.0625)).child(crate::ui::elements::checkbox::checkbox(theme, executable(edit.mode), false)))
                .child(div().flex_1().min_w_0().child(cx.i18n().text(Message::PermissionsExecutable)))
                .when(editable, |field| {
                    field.on_click(move |_, _, cx| {
                        update(&toggle, cx, |edit| edit.mode = with_executable(edit.mode, !executable(edit.mode)));
                    })
                }),
            compact,
            cx,
        ));
    }
    if edit.loaded.is_dir && edit.loaded.editable {
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
                .when(!edit.enclosed.is_empty(), |section| {
                    section.child(div().text_color(theme.text_muted).child(cx.i18n().text(Message::PermissionsEnclosedPending)))
                }),
        );
    }
    body
}
