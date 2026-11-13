//! The file chooser's navigation bar, a copy of the main window's (`top_bar.rs`)
//! without the file actions: Back and Forward, Up, the separator, the crumbs with
//! their chevron menus, fades and glide, the path being edited in their place, and
//! the name filter where the quick search sits.
use crate::ui::components::top_bar::{breadcrumb_fades, breadcrumb_glide};
use crate::ui::elements::bar_button;
use super::FileChooserWindow;
use crate::ui::components::dropdown::{DropdownItem, GenericDropdown, IconName};
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::logical;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::{DriveInfo, FileEntry};
use std::rc::Rc;

/// The mounted network share holding `path`, whose local root means nothing to users.
fn remote_share<'a>(path: &str, drives: &'a [DriveInfo]) -> Option<&'a DriveInfo> {
    if cfg!(target_os = "windows") {
        return None;
    }
    drives.iter().find(|drive| {
        drive.drive_type == "remote" && drive.remote_path.is_some() && std::path::Path::new(path).starts_with(&drive.path)
    })
}

/// A drive letter's crumb, `C:`, named as the main window names it: its label first.
fn drive_label(part: &str, drives: &[DriveInfo]) -> String {
    let letter = part.to_uppercase();
    drives.iter().find(|drive| drive.path.to_uppercase().starts_with(&letter) && !drive.label.is_empty())
        .map_or_else(|| part.to_string(), |drive| format!("{} ({part})", drive.label))
}

/// The volume holding `path`, as the main window's root icon shows it.
fn root_icon(path: &str, drives: &[DriveInfo]) -> &'static str {
    if oxyde_lib::platform::is_wsl_path(path) {
        return "icons/box.svg";
    }
    if path.starts_with("\\\\") {
        return "icons/network.svg";
    }
    drives.iter().filter(|drive| {
        std::path::Path::new(path).ancestors().any(|ancestor| {
            crate::ui::paths::paths_match(&ancestor.to_string_lossy(), &drive.path)
        })
    }).max_by_key(|drive| drive.path.len())
        .map(crate::ui::components::icons::drive_icon_path)
        .unwrap_or("icons/hard-drive.svg")
}

impl FileChooserWindow {
    /// A chevron's press: lists the folder before it, or closes its open or pending menu.
    fn toggle_crumb_menu(&mut self, path: String, index: usize, cx: &mut Context<Self>) {
        let request = (path, index);
        let open = self.crumbs.menu.as_ref().is_some_and(|(open, current, _)| *open == request.0 && *current == request.1);
        if open || self.crumbs.request.as_ref() == Some(&request) {
            self.crumbs.menu = None;
            self.crumbs.request = None;
            cx.notify();
            return;
        }
        self.crumbs.context = None;
        self.crumbs.request = Some(request.clone());
        let (show_hidden, show_system) = (self.show_hidden, self.show_system);
        cx.spawn(async move |this, cx| {
            let listed = oxyde_lib::commands::io::list_dir(request.0.clone(), Some(show_hidden), Some(show_system)).await;
            let _ = this.update(cx, |this, cx| {
                // A later press replaced or cancelled this request.
                if this.crumbs.request.as_ref() != Some(&request) {
                    return;
                }
                this.crumbs.request = None;
                if let Ok(entries) = listed {
                    let folders: Vec<FileEntry> = entries.into_iter().filter(|entry| entry.is_dir).collect();
                    this.crumbs.menu = Some((request.0, request.1, folders));
                    cx.notify();
                }
            });
        }).detach();
    }

    fn crumb_menu(&self, handle: Entity<Self>) -> GenericDropdown<Self> {
        let entries = self.crumbs.menu.as_ref().map(|(_, _, entries)| entries.clone()).unwrap_or_default();
        let items = entries.into_iter().map(|entry| {
            let choose = handle.downgrade();
            let path = entry.path.clone();
            DropdownItem {
                label: entry.name.clone(),
                badge: None,
                icon: Some(if self.system_icons { IconName::NativeFolder(entry.path.clone()) } else { IconName::Folder }),
                stay_open: false,
                has_submenu: false,
                is_dimmed: false,
                is_faded: entry.is_hidden || entry.is_system,
                on_click: Rc::new(move |_, cx| {
                    let _ = choose.update(cx, |this, cx| {
                        this.crumbs.menu = None;
                        this.show_folder(path.clone(), cx);
                    });
                }),
                on_hover: None,
            }
        }).collect();
        let close = handle.downgrade();
        GenericDropdown {
            id: "file-chooser-crumb-menu".into(),
            h_app: handle,
            items,
            reserve_icon_space: true,
            is_context_menu: false,
            scroll_handle: self.crumbs.menu_scroll.clone(),
            width: None,
            min_width: None,
            max_height: logical(360.0),
            header: None,
            on_close: Rc::new(move |_, cx| {
                let _ = close.update(cx, |this, cx| {
                    this.crumbs.menu = None;
                    cx.notify();
                });
            }),
            on_outside: None,
            on_drag: Rc::new({
                let scroll = self.crumbs.menu_scroll.0.borrow().base_handle.clone();
                move |this, track, viewport, content, _| this.start_menu_drag(track, viewport, content, scroll.clone())
            }),
            is_dragging: self.dragging_menu(),
            on_item_hover: None,
            on_width_calculated: None,
            on_hover_change: None,
        }
    }

    /// A crumb's right-click menu, as the main bar's: its folder's path to copy.
    fn crumb_context_menu(&self, handle: Entity<Self>, cx: &App) -> GenericDropdown<Self> {
        let path = self.crumbs.context.as_ref().map(|(path, _)| path.clone()).unwrap_or_default();
        let copy = handle.downgrade();
        let items = vec![DropdownItem {
            label: cx.i18n().text(Message::CopyPath).to_string(),
            badge: None,
            icon: Some(IconName::Copy),
            stay_open: false,
            has_submenu: false,
            is_dimmed: false,
            is_faded: false,
            on_click: Rc::new(move |_, cx| {
                #[cfg(target_os = "windows")]
                let _ = oxyde_lib::platform::clipboard::set_clipboard_text(path.clone());
                #[cfg(not(target_os = "windows"))]
                cx.write_to_clipboard(ClipboardItem::new_string(path.clone()));
                let _ = copy.update(cx, |this, cx| {
                    this.crumbs.context = None;
                    cx.notify();
                });
            }),
            on_hover: None,
        }];
        let close = handle.downgrade();
        GenericDropdown {
            id: "file-chooser-crumb-context".into(),
            h_app: handle,
            items,
            reserve_icon_space: true,
            is_context_menu: true,
            scroll_handle: UniformListScrollHandle::new(),
            width: None,
            min_width: None,
            max_height: logical(360.0),
            header: None,
            on_close: Rc::new(move |_, cx| {
                let _ = close.update(cx, |this, cx| {
                    this.crumbs.context = None;
                    cx.notify();
                });
            }),
            on_outside: None,
            on_drag: Rc::new(|_, _, _, _, _| {}),
            is_dragging: false,
            on_item_hover: None,
            on_width_calculated: None,
            on_hover_change: None,
        }
    }

    fn render_chevron(&self, parent: String, index: usize, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let handle = cx.entity();
        let (alive_path, factory_path) = (parent.clone(), parent.clone());
        let open = self.crumbs.menu.as_ref().is_some_and(|(path, current, _)| *path == parent && *current == index);
        div()
            .relative()
            .flex_none()
            .child(crate::ui::components::popup::anchored_surface(format!("file-chooser-crumb-menu-{index}"), handle.clone(),
                move |this| this.crumbs.menu.as_ref().is_some_and(|(path, current, _)| *path == alive_path && *current == index),
                move |this, _, cx| {
                    let open = this.crumbs.menu.as_ref().is_some_and(|(path, current, _)| *path == factory_path && *current == index);
                    if open { this.crumb_menu(cx.entity()).into_any_element() } else { div().into_any_element() }
                },
                |this, _, cx| { this.crumbs.menu = None; cx.notify(); }))
            .child(
                div()
                    .id(("file-chooser-chevron", index))
                    .h(crate::ui::style::CRUMB_HEIGHT)
                    .w(rems(1.125))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(crate::ui::style::CRUMB_RADIUS)
                    .text_color(theme.text_muted)
                    // The fill stays while its menu is open.
                    .when(open, |chevron| chevron.bg(theme.control_hover))
                    .hover(move |style| style.bg(theme.control_hover))
                    .child(std::path::MAIN_SEPARATOR_STR)
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| this.toggle_crumb_menu(parent.clone(), index, cx))),
            )
            // Where owned menu windows are not available, the menu draws in this window.
            .when(open && !crate::ui::components::popup::supported_source(&handle, cx), |chevron| {
                chevron.child(deferred(anchored().anchor(Anchor::BottomLeft).child(self.crumb_menu(handle.clone()))))
            })
            .into_any_element()
    }

    fn render_edit_field(&self, input: Entity<crate::ui::components::input::TextInput>) -> AnyElement {
        let invalid = self.location.invalid;
        // The input draws its own frame, red while the path is refused.
        let shell = div()
            .flex_1()
            .min_w_0()
            .h(crate::ui::components::input::BAR_FIELD_HEIGHT)
            .flex()
            .items_center()
            .child(input);
        if !invalid {
            return shell.into_any_element();
        }
        shell.with_animation(
            SharedString::from(format!("file-chooser-path-shake-{}", self.location.shake)),
            Animation::new(std::time::Duration::from_millis(300)),
            |shell, delta| {
                let amplitude = (1.0 - delta) * logical(8.0);
                let offset = (delta * std::f32::consts::PI * 6.0).sin() * amplitude;
                shell.relative().left(px(offset))
            },
        ).into_any_element()
    }

    fn render_crumbs(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> Div {
        let current_path = self.folder.clone();
        let share = remote_share(&current_path, &self.places.drives);
        let path_parts: Vec<String> = match share {
            Some(share) => std::iter::once(share.path.clone())
                .chain(current_path.get(share.path.len()..).unwrap_or_default().split(std::path::is_separator)
                    .filter(|part| !part.is_empty()).map(str::to_owned))
                .collect(),
            None => current_path.split(std::path::is_separator).filter(|part| !part.is_empty()).map(str::to_owned).collect(),
        };
        let total_parts = path_parts.len();
        let path_changed = *self.crumbs.path.borrow() != current_path;
        if path_changed {
            let previous = self.crumbs.path.replace(current_path.clone());
            breadcrumb_glide::start(&self.crumbs.glide, &previous, &current_path, &self.crumbs.scroll);
            if total_parts == 0 {
                self.crumbs.scroll.set_offset(point(px(0.0), px(0.0)));
            }
            // The scroll handle's dimensions are updated during prepaint.
            cx.notify();
        }
        let separator = std::path::MAIN_SEPARATOR;
        // A share starts with two separators, a drive letter with its root, as in the main bar.
        let mut accumulated = if current_path.starts_with("\\\\") || current_path.starts_with("//") {
            format!("{separator}{separator}")
        } else if current_path.starts_with('/') {
            separator.to_string()
        } else {
            String::new()
        };
        let mut elements: Vec<AnyElement> = Vec::new();
        for (index, part) in path_parts.iter().enumerate() {
            let is_last = index + 1 == total_parts;
            if index == 0 && share.is_some() {
                accumulated = part.clone();
            } else if index == 0 && part.ends_with(':') {
                accumulated = format!("{part}{separator}");
            } else {
                if !accumulated.is_empty() && !accumulated.ends_with(separator) {
                    accumulated.push(separator);
                }
                accumulated.push_str(part);
            }
            let level = accumulated.clone();
            if index > 0 {
                let parent = level.strip_suffix(part.as_str()).unwrap_or(&level).to_string();
                elements.push(self.render_chevron(parent, index, theme, cx));
            }
            let label = match share {
                Some(share) if index == 0 => share.label.clone(),
                None if index == 0 && part.ends_with(':') => drive_label(part, &self.places.drives),
                _ => oxyde_lib::utils::path_text::display(part),
            };
            let context_level = level.clone();
            let context_open = self.crumbs.context.as_ref().is_some_and(|(_, current)| *current == index);
            let handle = cx.entity();
            elements.push(
                div()
                    .id(("file-chooser-crumb", index))
                    .relative()
                    .child(crate::ui::components::popup::anchored_surface(format!("file-chooser-crumb-context-{index}"), handle.clone(),
                        move |this| this.crumbs.context.as_ref().is_some_and(|(_, current)| *current == index),
                        |this, _, cx| this.crumb_context_menu(cx.entity(), cx).into_any_element(),
                        |this, _, cx| { this.crumbs.context = None; cx.notify(); }))
                    .when(context_open && !crate::ui::components::popup::supported_source(&handle, cx), |crumb| {
                        crumb.child(deferred(anchored().anchor(Anchor::TopLeft).child(self.crumb_context_menu(handle.clone(), cx))))
                    })
                    .flex_none()
                    .h(crate::ui::style::CRUMB_HEIGHT)
                    .px(rems(0.375))
                    .flex()
                    .items_center()
                    .rounded(crate::ui::style::CRUMB_RADIUS)
                    .text_color(if is_last { theme.text } else { theme.text_muted })
                    .when(!is_last, |crumb| crumb.hover(move |style| style.text_color(theme.text).bg(theme.control_hover)))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        if event.click_count >= 2 {
                            cx.stop_propagation();
                            this.begin_location(None, window, cx);
                        } else if !is_last {
                            this.show_folder(level.clone(), cx);
                        }
                    }))
                    .on_mouse_down(MouseButton::Right, cx.listener({
                        let level = context_level;
                        move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.crumbs.menu = None;
                            this.crumbs.context = if this.crumbs.context.as_ref().is_some_and(|(_, current)| *current == index) { None } else { Some((level.clone(), index)) };
                            cx.notify();
                        }
                    }))
                    .child(label)
                    .into_any_element(),
            );
        }
        elements.push(
            div()
                .id("file-chooser-path-blank")
                .flex_1()
                .min_w(rems(1.5))
                .h_full()
                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.begin_location(None, window, cx);
                }))
                .into_any_element(),
        );
        let entity_id = cx.entity().entity_id();
        let (glide, resize_scroll, viewport_width) = (self.crumbs.glide.clone(), self.crumbs.scroll.clone(), self.crumbs.viewport.clone());
        let (wheel_glide, wheel_scroll) = (self.crumbs.glide.clone(), self.crumbs.scroll.clone());
        div()
            .child(
                div().w_4().h_4().mr_2().flex().items_center().justify_center().flex_none()
                    .child(svg().path(root_icon(&current_path, &self.places.drives)).size_4().text_color(theme.text)),
            )
            .child(div()
                .relative()
                .flex()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(canvas(|bounds, _, _| bounds, move |_, bounds: Bounds<Pixels>, window, cx| {
                    let available = bounds.size.width.as_f32().max(0.0);
                    if !breadcrumb_glide::advance(&glide, &resize_scroll, window) && path_changed {
                        resize_scroll.set_offset(point(-resize_scroll.max_offset().x, px(0.0)));
                    }
                    if (viewport_width.get() - available).abs() > 1.0 {
                        viewport_width.set(available);
                        if total_parts > 0 {
                            resize_scroll.scroll_to_item(total_parts * 2 - 2);
                        }
                        cx.notify(entity_id);
                    }
                }).absolute().size_full())
                .child(
                    div()
                        .id("file-chooser-crumbs")
                        .flex()
                        .flex_1()
                        .min_w_0()
                        .items_center()
                        .h_full()
                        .overflow_x_scroll()
                        .track_scroll(&self.crumbs.scroll)
                        .on_scroll_wheel(move |event, window, cx| {
                            let delta = event.delta.pixel_delta(window.line_height());
                            let amount = if delta.y.as_f32().abs() > delta.x.as_f32().abs() { delta.y } else { delta.x };
                            if wheel_scroll.max_offset().x > px(0.0) && amount != px(0.0) {
                                breadcrumb_glide::scroll_by(&wheel_glide, &wheel_scroll, amount * 1.5);
                                cx.notify(entity_id);
                            }
                            // GPUI's default wheel listener otherwise applies an
                            // unclamped offset, even when the whole path fits.
                            cx.stop_propagation();
                        })
                        .children(elements),
                )
                .child(breadcrumb_fades::render(self.crumbs.scroll.clone(), total_parts.checked_mul(2).and_then(|count| count.checked_sub(2)), theme.surface)))
    }

    pub(super) fn render_navbar(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let (can_go_back, can_go_forward, parent) = (!self.back.is_empty(), !self.forward.is_empty(), self.parent().is_some());
        let editing = self.location.input.clone();
        let i18n = cx.i18n();
        let (back_label, forward_label, up_label) = (i18n.text(Message::Back), i18n.text(Message::Forward), i18n.text(Message::Up));
        let new_folder_label = i18n.text(Message::NewFolder);
        let history_button = |id: &'static str, icon: &'static str, enabled: bool| {
            bar_button::fill(bar_button::cell(id, crate::ui::style::NAV_BAR_HEIGHT), enabled, false, theme)
                .child(bar_button::icon(icon, bar_button::icon_color(enabled, false, theme)))
        };
        div()
            .h(crate::ui::style::NAV_BAR_HEIGHT)
            .flex_none()
            .text_size(rems(0.8125))
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .w_full()
            .flex()
            .items_center()
            // The filter's cell closes the bar at its right edge.
            .bg(theme.surface)
            .border_b(rems(0.0625))
            .border_color(theme.border)
            .child(
                div()
                    .h_full()
                    .flex()
                    .items_center()
                    .child(history_button("file-chooser-back", "icons/arrow-left.svg", can_go_back)
                        .when(can_go_back, |button| button
                            .fixed_tooltip(Tooltip::with_shortcut(back_label, Some("Alt+←".into())))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.go_back(cx)))))
                    .child(history_button("file-chooser-forward", "icons/arrow-right.svg", can_go_forward)
                        .when(can_go_forward, |button| button
                            .fixed_tooltip(Tooltip::with_shortcut(forward_label, Some("Alt+→".into())))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.go_forward(cx))))),
            )
            // Up stays in place at the root, disabled, then New folder, as in the main window.
            .child(
                div().h_full().flex().items_center().flex_shrink_0().child(
                    bar_button::fill(bar_button::cell("file-chooser-up", crate::ui::style::NAV_BAR_HEIGHT), parent, false, theme)
                        .fixed_tooltip(Tooltip::with_shortcut(up_label, Some("Alt+↑".into())))
                        .when(parent, |button| button.on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.go_up(cx))))
                        .child(bar_button::icon("icons/arrow-up.svg", bar_button::icon_color(parent, false, theme))),
                ).child(
                    bar_button::fill(bar_button::cell("file-chooser-new-folder", crate::ui::style::NAV_BAR_HEIGHT), true, false, theme)
                        .fixed_tooltip(Tooltip::with_shortcut(new_folder_label, Some("Ctrl+Shift+N".into())))
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.create_folder(window, cx)))
                        .child(bar_button::icon("icons/folder-plus.svg", theme.text)),
                ),
            )
            // The only separator divides the actions from the crumbs.
            .child(crate::ui::elements::separator::vertical(theme.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .map(|bar| match editing {
                        Some(input) => bar.child(self.render_edit_field(input)),
                        None => bar.child(self.render_crumbs(theme, cx).flex().items_center().flex_1().min_w_0().h_full()),
                    }),
            )
            .child(self.render_filter_control(theme, cx))
    }
}
