//! Permissions tab, outside Windows, as Nautilus's Permissions page: an access level
//! per class, the group and the execution of a file; for a folder, levels for its
//! content. Everything is applied by OK.
mod enclosed;
mod item;

use super::{PropertiesDialog, PropertiesWindow};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::*;
use oxyde_lib::models::permissions::{
    symbolic, Access, EnclosedChange, PermissionChange, PermissionClass, PermissionError, UnixPermissions,
};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum PermissionMenu {
    Access(PermissionClass),
    Group,
    Enclosed { folders: bool, class: PermissionClass },
}

#[derive(Clone, Copy)]
enum Pick {
    /// None keeps an enclosed class unchanged.
    Access(Option<Access>),
    Group(u32),
}

pub(super) struct PermissionsEdit {
    loaded: UnixPermissions,
    mode: u32,
    group: u32,
    enclosed: EnclosedChange,
    /// The page for enclosed items is shown instead of the item's own.
    enclosed_page: bool,
    pub(super) menu: Option<PermissionMenu>,
    bounds: HashMap<PermissionMenu, Bounds<Pixels>>,
}

impl PermissionsEdit {
    pub(super) fn new(loaded: UnixPermissions) -> Self {
        Self {
            mode: loaded.mode,
            group: loaded.group.id,
            loaded,
            enclosed: EnclosedChange::default(),
            enclosed_page: false,
            menu: None,
            bounds: HashMap::new(),
        }
    }

    /// The change to apply on OK, if anything was changed.
    pub(super) fn change(&self) -> Option<PermissionChange> {
        let changed = (self.mode, self.group) != (self.loaded.mode, self.loaded.group.id) || !self.enclosed.is_empty();
        changed.then(|| PermissionChange {
            expected: self.loaded.clone(),
            mode: self.mode,
            group: self.group,
            enclosed: self.enclosed.clone(),
        })
    }

    fn pick(&mut self, menu: PermissionMenu, pick: Pick) {
        match (menu, pick) {
            (PermissionMenu::Access(class), Pick::Access(Some(access))) => {
                self.mode = access.apply(self.mode, self.loaded.is_dir, class);
            }
            (PermissionMenu::Enclosed { folders, class }, Pick::Access(level)) => {
                let levels = if folders { &mut self.enclosed.folders } else { &mut self.enclosed.files };
                levels[class.index()] = level;
            }
            (PermissionMenu::Group, Pick::Group(group)) => self.group = group,
            _ => {}
        }
        self.menu = None;
    }

    /// Choices of a menu: label, whether it is the current one, and its value.
    fn entries(&self, menu: PermissionMenu, cx: &App) -> Vec<(String, bool, Pick)> {
        let level = |access: Option<Access>| match access {
            Some(access) => cx.i18n().text(access_message(access)).to_string(),
            None => cx.i18n().text(Message::PermissionsUnchanged).to_string(),
        };
        match menu {
            PermissionMenu::Access(class) => {
                let current = Access::of(self.mode, self.loaded.is_dir, class);
                Access::choices(self.loaded.is_dir, class)
                    .iter()
                    .map(|access| (level(Some(*access)), current == Some(*access), Pick::Access(Some(*access))))
                    .collect()
            }
            PermissionMenu::Enclosed { folders, class } => {
                let levels = if folders { &self.enclosed.folders } else { &self.enclosed.files };
                let current = levels[class.index()];
                std::iter::once(None)
                    .chain(Access::choices(folders, class).iter().copied().map(Some))
                    .map(|access| (level(access), current == access, Pick::Access(access)))
                    .collect()
            }
            PermissionMenu::Group => self
                .loaded
                .groups
                .iter()
                .map(|group| (group.name.clone(), group.id == self.group, Pick::Group(group.id)))
                .collect(),
        }
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

/// The current level of a class of the item, or its bits outside the levels.
fn access_label(edit: &PermissionsEdit, class: PermissionClass, cx: &App) -> String {
    match Access::of(edit.mode, edit.loaded.is_dir, class) {
        Some(access) => cx.i18n().text(access_message(access)).to_string(),
        None => symbolic(edit.mode, class),
    }
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

fn form_row(label: Message, value: impl IntoElement, compact: bool, cx: &App) -> Div {
    crate::ui::components::modal::responsive_form_row(cx.i18n().text(label), value, rems(6.0), compact, cx)
}

/// A value with a menu of choices when the item is editable, plain text otherwise.
fn choice(
    edit: &PermissionsEdit,
    menu: PermissionMenu,
    id: ElementId,
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
    body: Div,
    dialog: &PropertiesDialog,
    edit: &PermissionsEdit,
    handle: &Entity<PropertiesWindow>,
    compact: bool,
    theme: OxydeTheme,
    cx: &mut Context<PropertiesWindow>,
) -> Div {
    let body = if edit.enclosed_page {
        enclosed::render(body, edit, handle, compact, theme, cx)
    } else {
        item::render(body, dialog, edit, handle, compact, theme, cx)
    };
    match edit.menu.filter(|_| !dialog.saving) {
        Some(menu) => body.child(menu_view(edit, menu, handle, theme, cx)),
        None => body,
    }
}

fn menu_view(
    edit: &PermissionsEdit,
    menu: PermissionMenu,
    handle: &Entity<PropertiesWindow>,
    theme: OxydeTheme,
    cx: &App,
) -> AnyElement {
    let Some(bounds) = edit.bounds.get(&menu).copied() else {
        return div().into_any_element();
    };
    let mut list = crate::ui::components::select_menu::surface(theme)
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .w(bounds.size.width);
    for (index, (label, selected, pick)) in edit.entries(menu, cx).into_iter().enumerate() {
        let choose = handle.clone();
        let key_choose = handle.clone();
        let key_action = std::rc::Rc::new(move |_: &mut Window, cx: &mut App| {
            update(&key_choose, cx, move |edit| edit.pick(menu, pick))
        });
        list = list.child(crate::ui::components::popup::keyboard::item(
            crate::ui::components::select_menu::row(div().id(("permissions-choice", index)), theme, selected)
                .child(label)
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    update(&choose, cx, move |edit| edit.pick(menu, pick));
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
        let cancel = self.cancellation.clone();
        cx.spawn(async move |this, cx| {
            let result = oxyde_lib::commands::io::set_unix_permissions(path, change, cancel).await;
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
                        Err(PermissionError::Incomplete { failed, first }) => {
                            let mut args = crate::ui::i18n::FluentArgs::new();
                            args.set("count", crate::ui::format::format_number(failed, cx.i18n().language()));
                            args.set("detail", first);
                            dialog.error = Some(cx.i18n().text_with_args(Message::PermissionsIncomplete, &args).to_string());
                            // The folder changed: what is shown is now what it has.
                            if let Some(edit) = &mut dialog.permissions {
                                edit.loaded.mode = edit.mode;
                                edit.loaded.group.id = edit.group;
                                edit.enclosed = EnclosedChange::default();
                            }
                            if let Some(owner) = this.owner.upgrade() {
                                owner.update(cx, |owner, cx| owner.refresh_all(cx));
                            }
                        }
                    }
                    cx.notify();
                })
            });
        })
        .detach();
    }
}
