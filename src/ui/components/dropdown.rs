use crate::ui::scale::{logical, rem_pixels};
use crate::ui::app::OxydeApp;
use crate::ui::i18n::{I18n, I18nContext, Message};
use crate::ui::components::scrollbar::{render_scrollbar_with_pointer, ScrollbarAxis};
use gpui::prelude::*;
use gpui::*;
use std::rc::Rc;


fn context_shortcut(label: &str, i18n: &I18n, shift: bool) -> Option<&'static str> {
    [(Message::Open, "Enter"), (Message::Cut, "Ctrl+X"),
        (Message::Rename, "F2"), (Message::Delete, "Del"),
        (Message::Properties, "Alt+Enter"), (Message::Paste, "Ctrl+V"),
        (Message::NewFolder, "Ctrl+Shift+N"), (Message::Copy, "Ctrl+C"), (Message::CopyName, "Ctrl+Alt+C"),
        (Message::CopyPath, "Ctrl+Shift+C"), (Message::TrashDeletePermanent, if shift { "Shift+Del" } else { "Del" })].into_iter()
        .find_map(|(message, shortcut)| (label == i18n.text(message).as_ref()).then_some(shortcut))
}

#[cfg(test)]
#[path = "dropdown/tests/delete_shortcuts.rs"]
mod delete_shortcuts;

#[derive(Clone)]
pub enum IconName {
    ChartBar,
    Separator,
    ListOrdered,
    SquareDashedMousePointer,
    Disc,
    Eject,
    Unplug,
    Check,
    Square,
    FolderOpen,
    NativeFolder(String),
    HardDrive,
    Copy,
    Shield,
    Printer,
    Settings,
    Play,
    Search,
    Code,
    Terminal,
    GitBranch,
    Share,
    Server,
    ServerOff,
    Cast,
    Pin,
    PinOff,
    ArchiveRestore,
    Refresh,
    BrushCleaning,
    CheckCircle,
    Pencil,
    ExternalLink,
    ShieldCheck,
    Archive,
    Scissors,
    ClipboardPaste,
    FolderPlus,
    Ellipsis,
    ChevronDown,
    ChevronUp,
    CircleX,
    Trash,
    TrashPermanent,
    Restore,
    CopyPlus,
    CopyX,
}

/// Match the semantic Lucide icon used by the former Tauri context menu.
pub fn native_menu_icon(verb: Option<&str>, label: &str) -> Option<IconName> {
    let verb = verb.unwrap_or_default().to_lowercase();
    let label = label.to_lowercase();
    let has = |terms: &[&str]| terms.iter().any(|term| label.contains(term));

    if verb == "runas" || has(&["administrateur", "administrator"]) {
        Some(IconName::Shield)
    } else if verb == "open" || has(&["ouvrir", "open"]) {
        Some(IconName::ExternalLink)
    } else if verb == "print" || has(&["imprimer", "print"]) {
        Some(IconName::Printer)
    } else if verb == "properties" || has(&["propriétés", "properties"]) {
        Some(IconName::Settings)
    } else if verb == "play" || has(&["jouer", "lecture", "play"]) {
        Some(IconName::Play)
    } else if verb == "edit" || has(&["modifier", "edit"]) {
        Some(IconName::Pencil)
    } else if verb == "find" || has(&["rechercher", "find"]) {
        Some(IconName::Search)
    } else if has(&["code", "visual studio"]) {
        Some(IconName::Code)
    } else if has(&["terminal", "powershell", "cmd"]) {
        Some(IconName::Terminal)
    } else if has(&["git ", "github", "commit", "tortoisegit"]) {
        Some(IconName::GitBranch)
    } else if verb == "share" || has(&["partager", "share"]) {
        Some(IconName::Share)
    } else if has(&[
        "scan",
        "defender",
        "antivirus",
        "malware",
        "eset",
        "kaspersky",
        "norton",
        "bitdefender",
    ]) {
        Some(IconName::ShieldCheck)
    } else if verb == "cast" || has(&["diffuser", "cast"]) {
        Some(IconName::Cast)
    } else if verb == "pin" || has(&["épingler", "pin", "accès rapide"]) {
        Some(IconName::Pin)
    } else if verb == "format" || has(&["formater", "format"]) {
        Some(IconName::HardDrive)
    } else if has(&["extraire", "extract", "unzip"]) {
        Some(IconName::ArchiveRestore)
    } else if has(&["compresser", "compress", "zip", "rar", "7z", "ajouter à"]) {
        Some(IconName::Archive)
    } else if has(&["synchroniser", "sync"]) {
        Some(IconName::Refresh)
    } else if has(&["libérer de l'espace", "free up space"]) {
        Some(IconName::BrushCleaning)
    } else if has(&["toujours conserver", "always keep"]) {
        Some(IconName::CheckCircle)
    } else {
        None
    }
}

#[derive(Clone)]
pub struct DropdownItem {
    pub label: String,
    pub badge: Option<String>,
    pub icon: Option<IconName>,
    pub stay_open: bool,
    pub has_submenu: bool,
    pub is_dimmed: bool,
    pub on_click: Rc<dyn Fn(&mut Window, &mut App)>,
    pub on_hover: Option<Rc<dyn Fn(Point<Pixels>, bool, &mut App)>>,
}

impl DropdownItem {
    pub fn separator() -> Self {
        Self::new("", |_, _| {}).icon(IconName::Separator)
    }
    pub fn new(
        label: impl Into<String>,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            icon: None,
            badge: None,
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            on_click: Rc::new(on_click),
            on_hover: None,
        }
    }

    pub fn on_hover(mut self, f: impl Fn(Point<Pixels>, bool, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(f));
        self
    }

    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn stay_open(mut self, stay_open: bool) -> Self {
        self.stay_open = stay_open;
        self
    }

    pub fn has_submenu(mut self, has_submenu: bool) -> Self {
        self.has_submenu = has_submenu;
        self
    }
}

#[derive(IntoElement)]
pub struct GenericDropdown<T: crate::ui::components::scrollbar::ScrollbarState + 'static = OxydeApp> {
    pub id: ElementId,
    pub items: Vec<DropdownItem>,
    /// Keep labels aligned when some rows have no icon.
    pub reserve_icon_space: bool,
    pub is_context_menu: bool,
    pub bottom_padding: f32,
    pub scroll_handle: UniformListScrollHandle,
    pub h_app: Entity<T>,
    pub on_drag: Rc<dyn Fn(&mut T, String, f32, f32, &mut App)>,
    pub on_close: Rc<dyn Fn(&mut Window, &mut App)>,
    pub on_outside: Option<Rc<dyn Fn(Point<Pixels>, &mut Window, &mut App)>>,
    pub is_dragging: bool,
    pub width: Option<f32>,
    pub max_height: f32,
    pub header: Option<AnyElement>,
    pub on_item_hover: Option<Rc<dyn Fn(usize, Point<Pixels>, &mut App)>>,
    pub on_width_calculated: Option<Rc<dyn Fn(Pixels, &mut App)>>,
    /// Fired with `true` when mouse enters, `false` when mouse leaves.
    pub on_hover_change: Option<Rc<dyn Fn(bool, &mut App)>>,
}

impl<T: crate::ui::components::scrollbar::ScrollbarState + 'static> RenderOnce for GenericDropdown<T> {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        use crate::ui::theme::OxydeTheme;
        let theme = *OxydeTheme::global(cx);
        for item in &mut self.items {
            if matches!(item.icon, Some(IconName::Trash)) &&
                [Message::Delete, Message::TrashDeletePermanent].iter().any(|message| item.label == cx.i18n().text(*message).as_ref()) {
                item.label = cx.i18n().text(if window.modifiers().shift { Message::TrashDeletePermanent } else { Message::Delete }).to_string();
            }
        }

        // Menu-specific colors — richer and more visible than the generic theme values
        let menu_bg: Hsla = theme.menu_surface;
        let menu_border: Hsla = theme.menu_border; // ~8% white — subtle but visible
        let item_hover: Hsla = theme.menu_hover; // Distinct but restrained hover state
        let icon_color: Hsla = theme.icon_muted; // ~33% white for icons
        let chevron_color: Hsla = theme.icon_disabled; // ~20% white for chevron arrow

        let mut calculated_width: f32 = logical(180.0);

        let text_system = window.text_system();
        let font = window.text_style().font();

        for item in &self.items {
            let line = text_system.shape_line(
                item.label.clone().into(),
                rem_pixels(rems(0.8125)),
                &[TextRun {
                    len: item.label.len(),
                    font: font.clone(),
                    color: theme.text,
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                }],
                None,
            );
            let icon_width = if item.icon.is_some() || self.reserve_icon_space {
                logical(24.0)
            } else {
                0.0
            };
            let split_copy = self.is_context_menu && item.has_submenu
                && item.label == cx.i18n().text(Message::Copy).as_ref();
            let chevron_width = if item.has_submenu { logical(if split_copy { 23.0 } else { 14.0 }) } else { 0.0 };
            let shortcut_width = if self.is_context_menu {
                context_shortcut(&item.label, cx.i18n(), window.modifiers().shift).map_or(0.0, |shortcut| {
                    text_system.shape_line(shortcut.into(), rem_pixels(rems(0.75)), &[TextRun {
                        len: shortcut.len(), font: font.clone(), color: theme.text_muted,
                        background_color: None, underline: None, strikethrough: None,
                    }], None).width.as_f32() + logical(16.0)
                })
            } else { 0.0 };
            let badge_width = item.badge.as_ref().map_or(0.0, |badge| {
                let line = text_system.shape_line(
                    badge.clone().into(), rem_pixels(rems(0.6875)),
                    &[TextRun {
                        len: badge.len(), font: font.clone(), color: theme.text_muted,
                        background_color: None, underline: None, strikethrough: None,
                    }], None,
                );
                line.width.as_f32().max(logical(12.0)) + logical(16.0)
            });
            // Row padding (16) + optional icon slot + optional submenu arrow.
            calculated_width =
                calculated_width.max(line.width.as_f32() + logical(16.0) + icon_width + chevron_width + badge_width + shortcut_width);
        }

        let requested_width = self.width.unwrap_or(calculated_width);
        let right_padding = if self.is_context_menu { logical(5.0) } else { logical(4.0) };
        let outer_width = (requested_width + logical(4.0) + right_padding)
            .min((window.viewport_size().width.as_f32() - logical(16.0)).max(1.0));
        let width = (outer_width - logical(4.0) - right_padding).max(1.0);

        // Report computed width including the menu's horizontal padding.
        if let Some(on_width) = self.on_width_calculated.take() {
            (on_width)(px(outer_width), cx);
        }

        let h_app_copy = self.h_app.clone();
        let scroll_handle = self.scroll_handle.clone();
        let base_scroll_handle = scroll_handle.0.borrow().base_handle.clone();
        let is_dragging = self.is_dragging;
        let on_drag = self.on_drag.clone();

        let item_h: f32 = logical(26.0);
        // Draw separators between action rows rather than reserving a full row.
        let mut separator_before = Vec::new();
        let mut original_indices = Vec::new();
        let mut pending_separator = false;
        let mut visible_items = Vec::new();
        for (index, item) in self.items.iter().enumerate() {
            if matches!(item.icon, Some(IconName::Separator)) {
                pending_separator = true;
            } else {
                separator_before.push(pending_separator);
                original_indices.push(index);
                visible_items.push(item.clone());
                pending_separator = false;
            }
        }
        self.items = visible_items;
        let item_count = self.items.len();
        let maximum = crate::ui::style::menu_list_max_height(window.viewport_size().height.as_f32(), self.header.is_some(), self.bottom_padding, self.max_height);
        let separator_height = logical(9.0);
        let separator_count = separator_before.iter().filter(|value| **value).count();
        let viewport_h = (item_count as f32 * item_h + separator_count as f32 * separator_height).min(maximum);
        let content_h = item_count as f32 * item_h + separator_count as f32 * separator_height;
        let has_scrollbar = content_h > viewport_h;

        let list_w = if has_scrollbar {
            width - crate::ui::components::scrollbar::zed_track_width()
        } else {
            width
        };

        let self_is_context_menu = self.is_context_menu;
        let on_item_hover = self.on_item_hover.clone();
        let reserve_icon_space = self.reserve_icon_space;

        div()
            .occlude()
            .id(self.id)
            .flex_col()
            .pt(rems(0.25))
            .pb(rems(self.bottom_padding / 16.0))
            .pl(rems(0.25))
            .pr(px(right_padding))
            .w(px(outer_width))
            .bg(menu_bg)
            .border(rems(0.0625))
            .border_color(menu_border)
            .rounded(rems(0.5))
            .shadow(crate::ui::style::rem_shadow(true))
            .when_some(self.header.take(), |s, h| {
                s.child(div().mb(rems(0.125)).child(h))
            })
            .child(
                div().w(px(list_w)).h(px(viewport_h)).child(
                    div().id("dropdown-items").flex_col().overflow_y_scroll().children({
                        let items = self.items.clone();
                        let on_close = self.on_close.clone();
                        let list_w_inner = list_w;
                        let on_hover_list = on_item_hover.clone();
                        (0..item_count)
                                .map(|ix| {
                                    let item = &items[ix];
                                    let original_ix = original_indices[ix];
                                    let has_separator = separator_before[ix];
                                    let on_click = item.on_click.clone();
                                    let on_close_inner = on_close.clone();
                                    let stay_open = item.stay_open;
                                    let is_dimmed = item.is_dimmed;
                                    let is_destructive =
                                        matches!(item.icon.as_ref(), Some(IconName::TrashPermanent))
                                            || (matches!(item.icon.as_ref(), Some(IconName::Trash))
                                                && item.label == cx.i18n().text(Message::TrashDeletePermanent).as_ref());
                                    let on_h = on_hover_list.clone();
                                    let arrow_hover = on_hover_list.clone();
                                    let shortcut = if self_is_context_menu { context_shortcut(&item.label, cx.i18n(), window.modifiers().shift) } else { None };
                                    let split_copy = self_is_context_menu && item.has_submenu
                                        && item.label == cx.i18n().text(Message::Copy).as_ref();

                                    let row = div()
                                        // Keep action hover state stable across menu renders.
                                        .id(("dropdown-item", ix))
                                        .w(px(list_w_inner))
                                        .h(px(item_h))
                                        .px(rems(0.5))
                                        .flex()
                                        .items_center()
                                        .relative()
                                        .rounded(rems(0.3125))
                                        .when(item.is_dimmed, |d| d.opacity(0.42))
                                        .when(!item.is_dimmed, |d| d.hover(|s| s.bg(item_hover)))
                                        .on_mouse_move(move |event, _window, cx| {
                                            if let Some(on_h) = &on_h {
                                                (on_h)(original_ix, event.position, cx);
                                            }
                                        })
                                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                            if !is_dimmed {
                                                (on_click)(window, cx);
                                                if !stay_open {
                                                    (on_close_inner)(window, cx);
                                                }
                                            }
                                        })
                                        // Nested Shell menus omit the empty icon column.
                                        .when(item.icon.is_some() || reserve_icon_space, |row| {
                                            row.child(
                                                div()
                                                    .flex_none()
                                                    .w(rems(1.0))
                                                    .h(rems(1.0))
                                                    .mr(rems(0.5))
                                                    .when_some(item.icon.as_ref(), |d, icon| {
                                                        if let IconName::NativeFolder(path) = icon {
                                                            return d.child(crate::ui::components::icons::render_file_icon(
                                                                path.clone(), crate::ui::components::icons::IconType::Folder,
                                                                rems(1.0), icon_color, true, false,
                                                            )).when(item.is_dimmed, |d| d.opacity(0.5));
                                                        }
                                                        d.child(
                                                            svg()
                                                                .path(match icon {
                                                                    IconName::SquareDashedMousePointer => {
                                                                        "icons/square-dashed-mouse-pointer.svg"
                                                                    }
                                                                    IconName::Disc => "icons/disc.svg",
                                                                    IconName::Eject => "icons/eject.svg",
                                                                    IconName::Unplug => "icons/unplug.svg",
                                                                    IconName::Check => {
                                                                        "icons/check.svg"
                                                                    }
                                                                    IconName::Square => {
                                                                        "icons/square.svg"
                                                                    }
                                                                    IconName::NativeFolder(_) => "icons/folder.svg",
                                                                    IconName::FolderOpen => {
                                                                        "icons/folder-open.svg"
                                                                    }
                                                                    IconName::HardDrive => {
                                                                        "icons/hard-drive.svg"
                                                                    }
                                                                    IconName::ChartBar => "icons/chart-bar.svg",
                                                                    IconName::Copy => {
                                                                        "icons/copy.svg"
                                                                    }
                                                                    IconName::Shield => {
                                                                        "icons/shield.svg"
                                                                    }
                                                                    IconName::Printer => {
                                                                        "icons/printer.svg"
                                                                    }
                                                                    IconName::Settings => {
                                                                        "icons/settings.svg"
                                                                    }
                                                                    IconName::Play => {
                                                                        "icons/play.svg"
                                                                    }
                                                                    IconName::Search => {
                                                                        "icons/search.svg"
                                                                    }
                                                                    IconName::Code => {
                                                                        "icons/code.svg"
                                                                    }
                                                                    IconName::Terminal => {
                                                                        "icons/terminal.svg"
                                                                    }
                                                                    IconName::GitBranch => {
                                                                        "icons/git-branch.svg"
                                                                    }
                                                                    IconName::Share => {
                                                                        "icons/share-2.svg"
                                                                    }
                                                                    IconName::Server => "icons/server.svg",
                                                                    IconName::ServerOff => "icons/server-off.svg",
                                                                    IconName::Cast => {
                                                                        "icons/cast.svg"
                                                                    }
                                                                    IconName::Pin => {
                                                                        "icons/pin.svg"
                                                                    }
                                                                    IconName::PinOff => {
                                                                        "icons/pin-off.svg"
                                                                    }
                                                                    IconName::ArchiveRestore => {
                                                                        "icons/archive-restore.svg"
                                                                    }
                                                                    IconName::Refresh => {
                                                                        "icons/refresh-cw.svg"
                                                                    }
                                                                    IconName::BrushCleaning => {
                                                                        "icons/brush-cleaning.svg"
                                                                    }
                                                                    IconName::CheckCircle => {
                                                                        "icons/circle-check.svg"
                                                                    }
                                                                    IconName::Pencil => {
                                                                        "icons/pencil.svg"
                                                                    }
                                                                    IconName::ExternalLink => {
                                                                        "icons/external-link.svg"
                                                                    }
                                                                    IconName::ShieldCheck => {
                                                                        "icons/shield-check.svg"
                                                                    }
                                                                    IconName::Archive => {
                                                                        "icons/archive.svg"
                                                                    }
                                                                    IconName::Scissors => {
                                                                        "icons/scissors.svg"
                                                                    }
                                                                    IconName::ClipboardPaste => {
                                                                        "icons/clipboard-paste.svg"
                                                                    }
                                                                    IconName::FolderPlus => {
                                                                        "icons/folder-plus.svg"
                                                                    }
                                                                    IconName::Ellipsis => {
                                                                        "icons/ellipsis.svg"
                                                                    }
                                                                    IconName::ChevronDown => {
                                                                        "icons/chevron-down.svg"
                                                                    }
                                                                    IconName::ChevronUp => {
                                                                        "icons/chevron-up.svg"
                                                                    }
                                                                    IconName::CircleX => {
                                                                        "icons/circle-x.svg"
                                                                    }
                                                                    IconName::Restore => { "icons/rotate-ccw.svg" }
                                                                    IconName::TrashPermanent => { "icons/trash-2.svg" }
                                                                    IconName::Trash => {
                                                                        "icons/trash.svg"
                                                                    }
                                                                    IconName::CopyPlus => {
                                                                        "icons/copy-plus.svg"
                                                                    }
                                                                    IconName::Separator => unreachable!("separator rows have no icon"),
                                                                    IconName::ListOrdered => {
                                                                        "icons/list-ordered.svg"
                                                                    }
                                                                    IconName::CopyX => {
                                                                        "icons/copy-x.svg"
                                                                    }
                                                                })
                                                                .size_full()
                                                                .text_color(if item.is_dimmed {
                                                                    theme.icon_disabled
                                                                } else if is_destructive {
                                                                    theme.error
                                                                } else {
                                                                    icon_color
                                                                }),
                                                        )
                                                    }),
                                            )
                                        })
                                        // Label
                                        .child(
                                            div()
                                                .flex_1()
                                                .text_size(rems(0.8125))
                                                .text_color(if item.is_dimmed {
                                                    theme.text_muted
                                                } else if is_destructive {
                                                    theme.error
                                                } else {
                                                    theme.text
                                                })
                                                .child(item.label.clone()),
                                        )
                                        .when_some(item.badge.clone(), |row, badge| {
                                            row.child(div()
                                                .flex_none()
                                                .min_w(rems(1.25))
                                                .px(rems(0.25))
                                                .ml(rems(0.5))
                                                .rounded(rems(0.25))
                                                .bg(theme.badge_surface)
                                                .text_color(theme.text_muted)
                                                .text_size(rems(0.6875))
                                                .text_center()
                                                .child(badge))
                                        })
                                        .when_some(shortcut, |row, shortcut| row.child(
                                            div().flex_none().ml(rems(0.5)).mr(rems(0.25))
                                                .text_size(rems(0.75)).text_color(theme.text_muted).child(shortcut)
                                        ))
                                        .when(split_copy, |row| row.child(
                                            div().flex_none().w(rems(0.0625)).h(rems(1.0))
                                                .mx(rems(0.25)).bg(theme.border)
                                        ))
                                        // Submenu arrow
                                        .when(item.has_submenu, |s| {
                                            s.child(
                                                div().id(("submenu-arrow", ix)).flex_none().h_full()
                                                    .flex().items_center()
                                                    .when(split_copy, |arrow| arrow.on_mouse_down(MouseButton::Left, move |event, _, cx| {
                                                        cx.stop_propagation();
                                                        if let Some(on_hover) = &arrow_hover {
                                                            on_hover(original_ix, event.position, cx);
                                                        }
                                                    }))
                                                    .child(svg().path("icons/chevron-right.svg")
                                                        .size(rems(0.875)).text_color(chevron_color)),
                                            )
                                        });
                                    div().flex_col().flex_none()
                                        .when(has_separator, |group| group.child(
                                            div().h(px(separator_height)).px(rems(0.5)).flex().items_center()
                                                .child(div().w_full().h(rems(0.0625)).bg(menu_border)),
                                        ))
                                        .child(row)
                                })
                                .collect::<Vec<_>>()
                    })
                    .track_scroll(&base_scroll_handle)
                    .size_full(),
                ),
            )
            .when(has_scrollbar, |menu| {
                menu.child({
                    let current_scroll = scroll_handle
                        .0
                        .borrow()
                        .base_handle
                        .offset()
                        .y
                        .as_f32()
                        .abs();
                    render_scrollbar_with_pointer(
                        theme,
                        h_app_copy,
                        ScrollbarAxis::Vertical,
                        viewport_h,
                        content_h,
                        current_scroll,
                        is_dragging,
                        "context-menu-scrollbar".to_string(),
                        None,
                        move |this, tid, vh, ch, _, cx| {
                            (on_drag)(this, tid, vh, ch, cx);
                        },
                    )
                })
            })
            .on_mouse_down_out({
                let on_close = self.on_close.clone();
                let on_outside = self.on_outside.clone();
                move |event, window, cx| {
                    if let Some(on_outside) = &on_outside {
                        (on_outside)(event.position, window, cx);
                    } else {
                        (on_close)(window, cx);
                    }
                }
            })
            .when_some(self.on_hover_change.clone(), |d, cb| {
                d.on_hover(move |is_hovering, _, cx| {
                    (cb)(*is_hovering, cx);
                })
            })
    }
}
