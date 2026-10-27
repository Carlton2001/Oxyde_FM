//! Permissions tab, outside Windows, as Nautilus's Permissions page: an access level
//! per class, the owner and group and the execution of a file; for a folder, levels
//! for its content. A selection gets levels for its files and for its folders, a
//! class whose items differ showing « Mixed » until a level is chosen. Everything is
//! applied by OK.
mod enclosed;
mod item;
mod save;

use super::{PropertiesDialog, PropertiesWindow};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::OxydeTheme;
use gpui::*;
use oxyde_lib::models::permissions::{
    shared, symbolic, with_executable, Access, Levels, PermissionChange, PermissionClass, UnixPermissions,
};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum PermissionMenu {
    Access { folders: bool, class: PermissionClass },
    Owner,
    Group,
    Enclosed { folders: bool, class: PermissionClass },
}

#[derive(Clone, Copy)]
enum Pick {
    /// None keeps a class as each item has it.
    Access(Option<Access>),
    Owner(u32),
    Group(u32),
}

pub(super) struct Item {
    pub(super) path: String,
    pub(super) loaded: UnixPermissions,
}

pub(super) struct PermissionsEdit {
    items: Vec<Item>,
    levels: Levels,
    /// Execution of a single file, once its checkbox is clicked.
    executable: Option<bool>,
    /// Owner and group of a single item; a selection keeps those of each item.
    owner: u32,
    group: u32,
    /// Levels of the Contents tab, prefilled from the umask as Nautilus does.
    enclosed: Levels,
    /// Its checkbox: OK applies those levels inside the folders.
    enclosed_apply: bool,
    pub(super) menu: Option<PermissionMenu>,
    bounds: HashMap<PermissionMenu, Bounds<Pixels>>,
}

impl PermissionsEdit {
    pub(super) fn new(items: Vec<Item>, enclosed_defaults: Levels) -> Option<Self> {
        let first = items.first()?;
        Some(Self {
            owner: first.loaded.owner.id,
            group: first.loaded.group.id,
            items,
            levels: Levels::default(),
            executable: None,
            enclosed: enclosed_defaults,
            enclosed_apply: false,
            menu: None,
            bounds: HashMap::new(),
        })
    }

    fn single(&self) -> Option<&UnixPermissions> {
        match &self.items[..] {
            [item] => Some(&item.loaded),
            _ => None,
        }
    }

    fn of_kind(&self, folders: bool) -> impl Iterator<Item = &UnixPermissions> {
        self.items.iter().map(|item| &item.loaded).filter(move |loaded| loaded.is_dir == folders)
    }

    fn has_kind(&self, folders: bool) -> bool {
        self.of_kind(folders).next().is_some()
    }

    /// Nautilus lets every item of a kind change, or none of them.
    fn editable(&self, folders: bool) -> bool {
        self.of_kind(folders).all(|loaded| loaded.editable)
    }

    fn all_editable(&self) -> bool {
        self.items.iter().all(|item| item.loaded.editable)
    }

    /// As Nautilus: when every item is a folder its user may change.
    pub(super) fn offers_enclosed(&self) -> bool {
        !self.has_kind(false) && self.all_editable()
    }

    /// The mode an item receives on OK.
    fn mode(&self, loaded: &UnixPermissions) -> u32 {
        let mode = self.levels.mode(loaded.mode, loaded.is_dir);
        match self.executable {
            Some(executable) if !loaded.is_dir => with_executable(mode, executable),
            _ => mode,
        }
    }

    /// The bits of a class the items of a kind share, as they are or as OK leaves them.
    fn shown(&self, folders: bool, class: PermissionClass, original: bool) -> Option<u32> {
        let modes = self.of_kind(folders).map(|loaded| if original { loaded.mode } else { self.mode(loaded) });
        shared(modes.collect::<Vec<_>>(), folders, class)
    }

    fn levels(&mut self, folders: bool) -> &mut [Option<Access>; 3] {
        if folders { &mut self.levels.folders } else { &mut self.levels.files }
    }

    /// What OK applies, item by item; empty when nothing was changed.
    pub(super) fn changes(&self) -> Vec<(String, PermissionChange)> {
        let single = self.single().is_some();
        self.items
            .iter()
            .map(|item| {
                let mut change = PermissionChange::of(item.loaded.clone());
                change.mode = self.mode(&item.loaded);
                if single {
                    (change.owner, change.group) = (self.owner, self.group);
                }
                if self.enclosed_apply && item.loaded.is_dir {
                    change.enclosed = self.enclosed.clone();
                }
                (item.path.clone(), change)
            })
            .filter(|(_, change)| !change.is_empty())
            .collect()
    }

    fn pick(&mut self, menu: PermissionMenu, pick: Pick) {
        match (menu, pick) {
            (PermissionMenu::Access { folders, class }, Pick::Access(level)) => self.levels(folders)[class.index()] = level,
            (PermissionMenu::Enclosed { folders, class }, Pick::Access(level)) => {
                let levels = if folders { &mut self.enclosed.folders } else { &mut self.enclosed.files };
                levels[class.index()] = level;
            }
            (PermissionMenu::Owner, Pick::Owner(owner)) => self.owner = owner,
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
            PermissionMenu::Access { folders, class } => {
                let chosen = if folders { self.levels.folders } else { self.levels.files }[class.index()];
                let original = self.shown(folders, class, true);
                let current = chosen.or_else(|| original.and_then(|mode| Access::of(mode, folders, class)));
                let mut entries: Vec<_> = Access::choices(folders, class)
                    .iter()
                    .map(|access| (level(Some(*access)), current == Some(*access), Pick::Access(Some(*access))))
                    .collect();
                // As Nautilus: what the items have, when no level says it, stays a choice.
                if original.and_then(|mode| Access::of(mode, folders, class)).is_none() {
                    entries.push((level_label(original, folders, class, cx), chosen.is_none(), Pick::Access(None)));
                }
                entries
            }
            PermissionMenu::Enclosed { folders, class } => {
                let levels = if folders { &self.enclosed.folders } else { &self.enclosed.files };
                let current = levels[class.index()];
                // A umask matching no level leaves the class unchanged, as Nautilus does.
                current
                    .is_none()
                    .then_some(None)
                    .into_iter()
                    .chain(Access::choices(folders, class).iter().copied().map(Some))
                    .map(|access| (level(access), current == access, Pick::Access(access)))
                    .collect()
            }
            PermissionMenu::Owner => self
                .single()
                .map(|loaded| &loaded.owners[..])
                .unwrap_or_default()
                .iter()
                .map(|owner| (owner.name.clone(), owner.id == self.owner, Pick::Owner(owner.id)))
                .collect(),
            PermissionMenu::Group => self
                .single()
                .map(|loaded| &loaded.groups[..])
                .unwrap_or_default()
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

/// A level, the bits outside the levels, or « Mixed » when the items differ.
fn level_label(shown: Option<u32>, folders: bool, class: PermissionClass, cx: &App) -> String {
    match shown {
        Some(mode) => match Access::of(mode, folders, class) {
            Some(access) => cx.i18n().text(access_message(access)).to_string(),
            None => symbolic(mode, class),
        },
        None => cx.i18n().text(Message::PermissionsMixed).to_string(),
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

/// A value that cannot be changed, its text where a menu's would be.
fn plain(label: String) -> AnyElement {
    div()
        .flex_1()
        .min_w_0()
        .h(rems(2.0))
        .px(rems(0.625))
        .flex()
        .items_center()
        .border(rems(0.0625))
        .border_color(transparent_black())
        .child(div().min_w_0().truncate().child(label))
        .into_any_element()
}

/// A value with a menu of choices when the item is editable, plain text otherwise.
fn choice(
    menu: PermissionMenu,
    id: ElementId,
    label: String,
    editable: bool,
    handle: &Entity<PropertiesWindow>,
    theme: OxydeTheme,
) -> AnyElement {
    if !editable {
        return plain(label);
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

/// The Permissions tab, or the Contents tab when `enclosed`.
pub(super) fn render(
    body: Div,
    dialog: &PropertiesDialog,
    edit: &PermissionsEdit,
    enclosed: bool,
    handle: &Entity<PropertiesWindow>,
    compact: bool,
    theme: OxydeTheme,
    cx: &mut Context<PropertiesWindow>,
) -> Div {
    let body = if enclosed {
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

#[cfg(test)]
#[path = "permissions/tests/edit.rs"]
mod tests;
