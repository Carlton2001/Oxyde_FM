//! Permissions tab, outside Windows: an access level per class, the group and the
//! execution of a file, as on Nautilus's Permissions page. Applied by OK.
use super::{general, PropertiesDialog, PropertiesWindow};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::FluentBuilder;
use gpui::*;
use oxyde_lib::models::permissions::{
    executable, symbolic, with_executable, Access, PermissionChange, PermissionClass, PermissionError,
    UnixPermissions,
};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum PermissionMenu {
    Access(PermissionClass),
    Group,
}

pub(super) struct PermissionsEdit {
    loaded: UnixPermissions,
    mode: u32,
    group: u32,
    pub(super) menu: Option<PermissionMenu>,
    bounds: HashMap<PermissionMenu, Bounds<Pixels>>,
}

impl PermissionsEdit {
    pub(super) fn new(loaded: UnixPermissions) -> Self {
        Self { mode: loaded.mode, group: loaded.group.id, loaded, menu: None, bounds: HashMap::new() }
    }

    /// The change to apply on OK, if anything was changed.
    pub(super) fn change(&self) -> Option<PermissionChange> {
        ((self.mode, self.group) != (self.loaded.mode, self.loaded.group.id)).then(|| PermissionChange {
            expected: self.loaded.clone(),
            mode: self.mode,
            group: self.group,
        })
    }
}

fn access_message(access: Access) -> Message {
    match access {
        Access::None => Message::PermissionsNone,
        Access::ReadOnly => Message::PermissionsReadOnly,
        Access::ReadWrite => Message::PermissionsReadWrite,
        Access::ListFiles => Message::PermissionsListFiles,
        Access::AccessFiles => Message::PermissionsAccessFiles,
        Access::CreateDelete => Message::PermissionsCreateDelete,
    }
}

fn access_label(edit: &PermissionsEdit, class: PermissionClass, cx: &App) -> String {
    match Access::of(edit.mode, edit.loaded.is_dir, class) {
        Some(access) => cx.i18n().text(access_message(access)).to_string(),
        None => symbolic(edit.mode, class),
    }
}

fn group_name(edit: &PermissionsEdit) -> String {
    edit.loaded
        .groups
        .iter()
        .chain([&edit.loaded.group])
        .find(|group| group.id == edit.group)
        .map_or_else(|| edit.group.to_string(), |group| group.name.clone())
}

fn update(handle: &Entity<PropertiesWindow>, cx: &mut App, change: impl FnOnce(&mut PermissionsEdit) + 'static) {
    handle.update(cx, |this, cx| {
        if let Some(edit) = this
            .properties_dialog
            .as_mut()
            .filter(|dialog| !dialog.saving)
            .and_then(|dialog| dialog.permissions.as_mut())
        {
            change(edit);
            cx.notify();
        }
    });
}

/// A value with a menu of choices when the item is editable, plain text otherwise.
fn choice(
    edit: &PermissionsEdit,
    menu: PermissionMenu,
    id: &'static str,
    label: String,
    handle: &Entity<PropertiesWindow>,
    theme: OxydeTheme,
) -> AnyElement {
    if !edit.loaded.editable {
        return div().child(label).into_any_element();
    }
    let measure = handle.clone();
    let open = handle.clone();
    div()
        .id(id)
        .relative()
        .flex_1()
        .min_w_0()
        .h(rems(2.0))
        .px(rems(0.625))
        .flex()
        .items_center()
        .justify_between()
        .border(rems(0.0625))
        .border_color(theme.border)
        .bg(theme.surface)
        .rounded_sm()
        .child(
            canvas(
                |bounds, _, _| bounds,
                move |bounds, _, _, cx| {
                    measure.update(cx, |this, _| {
                        if let Some(edit) = this.properties_dialog.as_mut().and_then(|dialog| dialog.permissions.as_mut()) {
                            edit.bounds.insert(menu, bounds);
                        }
                    })
                },
            )
            .absolute()
            .inset_0()
            .size_full(),
        )
        .child(div().flex_1().min_w_0().truncate().child(label))
        .child(svg().path("icons/chevron-down.svg").size(rems(0.75)).flex_none().text_color(theme.text))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            update(&open, cx, move |edit| {
                edit.menu = (edit.menu != Some(menu)).then_some(menu);
            });
        })
        .into_any_element()
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
    let row = |label: Message, value: AnyElement, cx: &App| {
        crate::ui::components::modal::responsive_form_row(cx.i18n().text(label), value, rems(6.0), compact, cx)
    };
    let access = |class, id, cx: &App| {
        let label = access_label(edit, class, cx);
        row(Message::PermissionsAccess, choice(edit, PermissionMenu::Access(class), id, label, handle, theme), cx)
    };
    if !edit.loaded.editable {
        body = body.child(div().mb(rems(0.75)).text_color(theme.text_muted).child(cx.i18n().text(Message::PermissionsNotOwner)));
    }
    body = body
        .child(row(Message::PermissionsOwner, div().child(edit.loaded.owner.name.clone()).into_any_element(), cx))
        .child(access(PermissionClass::Owner, "permissions-owner", cx))
        .child(general::divider(theme));
    let group = if edit.loaded.groups.len() > 1 {
        choice(edit, PermissionMenu::Group, "permissions-group", group_name(edit), handle, theme)
    } else {
        div().child(group_name(edit)).into_any_element()
    };
    body = body
        .child(row(Message::PermissionsGroup, group, cx))
        .child(access(PermissionClass::Group, "permissions-group-access", cx))
        .child(general::divider(theme))
        .child(row(
            Message::PermissionsOthers,
            choice(
                edit,
                PermissionMenu::Access(PermissionClass::Others),
                "permissions-others-access",
                access_label(edit, PermissionClass::Others, cx),
                handle,
                theme,
            ),
            cx,
        ));
    if !edit.loaded.is_dir {
        let toggle = handle.clone();
        let checked = executable(edit.mode);
        let editable = edit.loaded.editable;
        body = body.child(general::divider(theme)).child(row(
            Message::PermissionsExecution,
            div()
                .id("permissions-executable")
                .flex()
                .items_center()
                .gap(rems(0.5))
                .when(!editable, |field| field.opacity(0.5))
                .child(crate::ui::elements::checkbox::checkbox(theme, checked, false))
                .child(cx.i18n().text(Message::PermissionsExecutable))
                .when(editable, |field| {
                    field.on_click(move |_, _, cx| {
                        update(&toggle, cx, |edit| edit.mode = with_executable(edit.mode, !executable(edit.mode)));
                    })
                })
                .into_any_element(),
            cx,
        ));
    }
    if let Some(menu) = edit.menu.filter(|_| !dialog.saving) {
        body = body.child(menu_view(edit, menu, handle, theme, cx));
    }
    body
}

fn menu_view(
    edit: &PermissionsEdit,
    menu: PermissionMenu,
    handle: &Entity<PropertiesWindow>,
    theme: OxydeTheme,
    cx: &App,
) -> impl IntoElement {
    let Some(bounds) = edit.bounds.get(&menu).copied() else {
        return div().into_any_element();
    };
    let mut list = crate::ui::components::select_menu::surface(theme)
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .w(bounds.size.width);
    let entries: Vec<(String, bool, u32)> = match menu {
        PermissionMenu::Access(class) => Access::choices(edit.loaded.is_dir, class)
            .iter()
            .enumerate()
            .map(|(index, access)| {
                let selected = Access::of(edit.mode, edit.loaded.is_dir, class) == Some(*access);
                (cx.i18n().text(access_message(*access)).to_string(), selected, index as u32)
            })
            .collect(),
        PermissionMenu::Group => edit
            .loaded
            .groups
            .iter()
            .map(|group| (group.name.clone(), group.id == edit.group, group.id))
            .collect(),
    };
    for (index, (label, selected, value)) in entries.into_iter().enumerate() {
        let choose = handle.clone();
        let key_choose = handle.clone();
        let pick = move |edit: &mut PermissionsEdit| {
            match menu {
                PermissionMenu::Access(class) => {
                    let access = Access::choices(edit.loaded.is_dir, class)[value as usize];
                    edit.mode = access.apply(edit.mode, edit.loaded.is_dir, class);
                }
                PermissionMenu::Group => edit.group = value,
            }
            edit.menu = None;
        };
        let key_action = std::rc::Rc::new(move |_: &mut Window, cx: &mut App| update(&key_choose, cx, pick));
        list = list.child(crate::ui::components::popup::keyboard::item(
            crate::ui::components::select_menu::row(div().id(("permissions-choice", index)), theme, selected)
                .child(label)
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    update(&choose, cx, pick);
                }),
            key_action,
            None,
            None,
        ));
    }
    let close = handle.clone();
    deferred(
        anchored()
            .position(point(bounds.left(), bounds.bottom()))
            .snap_to_window()
            .child(list.on_mouse_down_out(move |event, _, cx| {
                if bounds.contains(&event.position) {
                    return;
                }
                update(&close, cx, |edit| edit.menu = None);
            })),
    )
    .into_any_element()
}

impl PropertiesWindow {
    pub(super) fn save_permissions(&mut self, change: PermissionChange, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.properties_dialog.as_mut() else {
            return;
        };
        let path = dialog.path.clone();
        let revision = dialog.revision;
        dialog.saving = true;
        dialog.error = None;
        dialog.validation_error = None;
        if let Some(edit) = &mut dialog.permissions {
            edit.menu = None;
        }
        window.focus(&dialog.ok_focus, cx);
        cx.notify();
        let window_handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let result = oxyde_lib::commands::io::set_unix_permissions(path, change).await;
            let _ = window_handle.update(cx, |_, window, cx| {
                this.update(cx, |this, cx| {
                    let Some(dialog) = this.properties_dialog.as_mut().filter(|dialog| dialog.revision == revision) else {
                        return;
                    };
                    dialog.saving = false;
                    match result {
                        Ok(()) => {
                            this.close_file_properties(window, cx);
                            if let Some(owner) = this.owner.upgrade() {
                                owner.update(cx, |owner, cx| owner.refresh_all(cx));
                            }
                        }
                        Err(PermissionError::Changed) => dialog.validation_error = Some(Message::PermissionsChanged),
                        Err(PermissionError::NotKept) => dialog.validation_error = Some(Message::PermissionsNotKept),
                        Err(PermissionError::Failed(detail)) => dialog.error = Some(detail),
                    }
                    cx.notify();
                })
            });
        })
        .detach();
    }
}
