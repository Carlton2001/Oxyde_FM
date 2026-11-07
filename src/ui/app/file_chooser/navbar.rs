//! The file chooser's navigation bar, a copy of the main window's (`top_bar.rs`)
//! without the file actions: Back and Forward, Up, the separator, the crumbs with
//! their chevron menus, fades and glide, the path being edited in their place, and
//! the name filter where the quick search sits.
use super::crumbs;
use super::FileChooserWindow;
use crate::ui::components::dropdown::{DropdownItem, GenericDropdown, IconName};
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::elements::button::{button_style, ButtonVariant};
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

/// The volume holding `path`, as the main window's root icon shows it.
fn root_icon(path: &str, drives: &[DriveInfo]) -> &'static str {
    drives.iter().filter(|drive| {
        std::path::Path::new(path).ancestors().any(|ancestor| {
            crate::ui::components::sidebar::sidebar_paths_equal(&ancestor.to_string_lossy(), &drive.path)
        })
    }).max_by_key(|drive| drive.path.len())
        .map(crate::ui::components::icons::drive_icon_path)
        .unwrap_or("icons/hard-drive.svg")
}

impl FileChooserWindow {
    /// A chevron's press: lists the folder before it, or closes its open menu.
    fn toggle_crumb_menu(&mut self, path: String, index: usize, cx: &mut Context<Self>) {
        if self.crumbs.menu.as_ref().is_some_and(|(open, current, _)| *open == path && *current == index) {
            self.crumbs.menu = None;
            cx.notify();
            return;
        }
        let (show_hidden, show_system) = (self.show_hidden, self.show_system);
        cx.spawn(async move |this, cx| {
            if let Ok(entries) = oxyde_lib::commands::io::list_dir(path.clone(), Some(show_hidden), Some(show_system)).await {
                let _ = this.update(cx, |this, cx| {
                    let folders: Vec<FileEntry> = entries.into_iter().filter(|entry| entry.is_dir).collect();
                    this.crumbs.menu = Some((path, index, folders));
                    cx.notify();
                });
            }
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
                icon: Some(if self.system_icons { IconName::NativeFolder(entry.path.clone()) } else { IconName::FolderOpen }),
                stay_open: false,
                has_submenu: false,
                is_dimmed: entry.is_hidden || entry.is_system,
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
            max_height: logical(360.0),
            header: None,
            on_close: Rc::new(move |_, cx| {
                let _ = close.update(cx, |this, cx| {
                    this.crumbs.menu = None;
                    cx.notify();
                });
            }),
            on_outside: None,
            // The menu scrolls with the wheel; its thumb is not dragged here.
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
                    .h(rems(1.5))
                    .w(rems(1.125))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .hover(move |style| style.bg(theme.control_hover))
                    .child(svg().path("icons/chevron-right.svg").size_3().text_color(theme.text))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| this.toggle_crumb_menu(parent.clone(), index, cx))),
            )
            // Where owned menu windows are not available, the menu draws in this window.
            .when(open && !crate::ui::components::popup::supported_source(&handle, cx), |chevron| {
                chevron.child(deferred(anchored().anchor(Anchor::BottomLeft).child(self.crumb_menu(handle.clone()))))
            })
            .into_any_element()
    }

    fn render_edit_field(&self, input: Entity<crate::ui::components::input::TextInput>, theme: OxydeTheme) -> AnyElement {
        let invalid = self.location.invalid;
        let field = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .when(invalid, |field| field.border(rems(0.0625)).border_color(theme.error).rounded_sm())
            .child(input);
        let shell = div()
            .flex_1()
            .min_w_0()
            .h(px(crate::ui::style::row_height()))
            .when(invalid, |shell| shell.px(rems(0.5)))
            .flex()
            .items_center()
            .child(field);
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
            crumbs::start(&self.crumbs.glide, &previous, &current_path, &self.crumbs.scroll);
            if total_parts == 0 {
                self.crumbs.scroll.set_offset(point(px(0.0), px(0.0)));
            }
            // The scroll handle's dimensions are updated during prepaint.
            cx.notify();
        }
        let separator = std::path::MAIN_SEPARATOR;
        let mut accumulated = if current_path.starts_with('/') { separator.to_string() } else { String::new() };
        let mut elements: Vec<AnyElement> = Vec::new();
        for (index, part) in path_parts.iter().enumerate() {
            let is_last = index + 1 == total_parts;
            if index == 0 && share.is_some() {
                accumulated = part.clone();
            } else {
                if !accumulated.is_empty() && !accumulated.ends_with(separator) {
                    accumulated.push(separator);
                }
                accumulated.push_str(part);
            }
            let level = accumulated.clone();
            if index > 0 {
                let parent = level[..level.len() - part.len()].to_string();
                elements.push(self.render_chevron(parent, index, theme, cx));
            }
            let label = match share {
                Some(share) if index == 0 => share.label.clone(),
                _ => oxyde_lib::utils::path_text::display(part),
            };
            elements.push(
                div()
                    .id(("file-chooser-crumb", index))
                    .flex_none()
                    .h(rems(1.5))
                    .px(rems(0.375))
                    .flex()
                    .items_center()
                    .rounded_md()
                    .text_color(if is_last { theme.text } else { theme.text_muted })
                    .when(!is_last, |crumb| crumb.hover(move |style| style.text_color(theme.text_muted).bg(theme.control_hover)))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        if event.click_count >= 2 {
                            cx.stop_propagation();
                            this.begin_location(None, window, cx);
                        } else if !is_last {
                            this.show_folder(level.clone(), cx);
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
                    if !crumbs::advance(&glide, &resize_scroll, window) && path_changed {
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
                                crumbs::scroll_by(&wheel_glide, &wheel_scroll, amount * 1.5);
                                cx.notify(entity_id);
                            }
                            // GPUI's default wheel listener otherwise applies an
                            // unclamped offset, even when the whole path fits.
                            cx.stop_propagation();
                        })
                        .children(elements),
                )
                .child(crumbs::fades(self.crumbs.scroll.clone(), total_parts.checked_mul(2).and_then(|count| count.checked_sub(2)), theme.surface)))
    }

    pub(super) fn render_navbar(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let (can_go_back, can_go_forward, parent) = (!self.back.is_empty(), !self.forward.is_empty(), self.parent().is_some());
        let editing = self.location.input.clone();
        let i18n = cx.i18n();
        let (back_label, forward_label, up_label) = (i18n.text(Message::Back), i18n.text(Message::Forward), i18n.text(Message::Up));
        let history_button = |id: &'static str, icon: &'static str, enabled: bool| {
            button_style(div().id(id).w_6().h_6().flex().items_center().justify_center().rounded_md(), theme, ButtonVariant::Ghost, enabled)
                .child(svg().path(icon).text_color(if enabled { theme.text } else { theme.icon_disabled }).size_4())
        };
        div()
            .h(crate::ui::style::NAV_BAR_HEIGHT)
            .flex_none()
            .text_size(rems(0.8125))
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .w_full()
            .flex()
            .items_center()
            .pl(rems(0.5))
            .pr(rems(0.25))
            .when(editing.is_some(), |bar| bar.pr(rems(0.5)))
            .bg(theme.surface)
            .border_b(rems(0.0625))
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(history_button("file-chooser-back", "icons/arrow-left.svg", can_go_back)
                        .when(can_go_back, |button| button
                            .fixed_tooltip(Tooltip::with_shortcut(back_label, Some("Alt+←".into())))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.go_back(cx)))))
                    .child(history_button("file-chooser-forward", "icons/arrow-right.svg", can_go_forward)
                        .when(can_go_forward, |button| button
                            .fixed_tooltip(Tooltip::with_shortcut(forward_label, Some("Alt+→".into())))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.go_forward(cx))))),
            )
            .child(
                div().flex().items_center().gap_1().ml_1().flex_shrink_0()
                    .when(parent, |actions| actions.child(
                        div().id("file-chooser-up").min_w(rems(1.5)).h_6().px(rems(0.125)).flex().items_center().justify_center().rounded_md()
                            .fixed_tooltip(Tooltip::with_shortcut(up_label, Some("Alt+↑".into())))
                            .hover(move |style| style.bg(theme.control_hover))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.go_up(cx)))
                            .child(svg().path("icons/arrow-up.svg").size_4().text_color(theme.text)),
                    )),
            )
            // The only separator divides the actions from the crumbs.
            .child(div().w(rems(0.0625)).h(rems(1.0)).mx_3().flex_shrink_0().bg(theme.border))
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .map(|bar| match editing {
                        Some(input) => bar.child(self.render_edit_field(input, theme)),
                        None => bar.child(self.render_crumbs(theme, cx).flex().items_center().flex_1().min_w_0().h_full()),
                    }),
            )
            .child(self.render_filter_control(theme, cx))
    }
}
