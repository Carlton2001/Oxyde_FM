use super::presentation::{file_type_display, grid_display_name, network_category, network_entry, selection_checkbox, size_loading};
use super::thumbnails::render_grid_thumbnail;
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, FileDragData, FileDragPreview, OxydeApp};
use crate::ui::components::{paths_match, with_grid_sort, oxyde_context_menu_items};
use crate::ui::components::icons::render_file_icon;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::components::tooltip::hover::SharedTooltipExt;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::logical;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::FileEntry;

pub(super) struct GridCardParams<'a> {
    pub(super) file: &'a FileEntry,
    pub(super) path: String,
    pub(super) name: String,
    pub(super) is_dir: bool,
    pub(super) has_subdirectories: bool,
    pub(super) is_selected: bool,
    pub(super) h: Entity<OxydeApp>,
    pub(super) pid: String,
    pub(super) paths: Vec<String>,
    pub(super) icon: crate::ui::components::icons::IconType,
    pub(super) editing_view: Option<Entity<crate::ui::components::input::TextInput>>,
    pub(super) has_rename_error: bool,
    pub(super) theme: OxydeTheme,
    pub(super) calculating_sizes: bool,
    pub(super) histogram_total: u64,
    pub(super) card_width: f32,
    pub(super) active: bool,
    pub(super) show_tooltips: bool,
    pub(super) show_media_previews: bool,
    pub(super) search_results: bool,
    pub(super) show_grid_thumbnails: bool,
    pub(super) use_system_icons: bool,
    pub(super) show_checkboxes: bool,
}

pub(super) fn render_grid_card(params: GridCardParams<'_>, cx: &mut App) -> Stateful<Div> {
    let GridCardParams {
        file,
        path,
        name,
        is_dir,
        has_subdirectories,
        is_selected,
        h,
        pid,
        paths,
        icon,
        editing_view,
        has_rename_error,
        theme,
        calculating_sizes,
        histogram_total,
        card_width,
        active,
        show_tooltips,
        show_media_previews,
        search_results,
        show_grid_thumbnails,
        use_system_icons,
        show_checkboxes,
    } = params;
    let can_browse = is_dir || file.is_navigable_archive() || file.needs_mount_for_browsing();
    let can_drop = is_dir || file.is_writable_archive();
    let loading_size = calculating_sizes && is_dir && !file.is_calculated;
    let mut card = div()
        .id(SharedString::from(format!("file-card-{path}"))).relative().when(histogram_total > 0, |d| d.child(
            crate::ui::elements::rounded_fill::rounded_fill(
                (file.size as f64 / histogram_total as f64) as f32,
                rems(0.3125),
                theme.accent.opacity(0.12),
            ).absolute().left(rems(0.0625)).right(rems(0.0625)).top(rems(0.0625)).bottom(rems(0.0625))
        ))
        .w(px(card_width - logical(4.0)))
        .h(px(crate::ui::style::grid_card_row_height() - logical(4.0)))
        .m_0p5()
        .p_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_0p5()
        .rounded_md()
        .border(rems(0.0625))
        .border_color(if has_rename_error {
            theme.error
        } else if is_selected {
            theme.accent
        } else {
            theme.border.opacity(0.0)
        })
        .bg(if is_selected {
            theme.selected
        } else if active {
            theme.active_background
        } else {
            theme.background
        })
        .group("file-grid-card")
        .relative().when(!is_selected, |d| d.hover(|d| d.bg(theme.hover)))
        .when(show_tooltips, |d| d.file_tooltip(h.clone(), pid.clone(), path.clone(), cx, Tooltip::for_item(file.clone(), search_results, show_media_previews)))
        .on_mouse_down(MouseButton::Left, {
            let path = path.clone();
            let h = h.clone();
            let pid = pid.clone();
            move |e, _, cx| {
                cx.stop_propagation();
                let path = path.clone();
                let is_dir = is_dir;
                h.update(cx, |this, cx| {
                    this.activate_pane(&pid, cx);
                    this.begin_item_press(&pid, &path, e);
                    let preserve_multi_selection = !e.modifiers.control
                        && !e.modifiers.shift
                        && this.state.find_pane(&pid).is_some_and(|pane| {
                            let selection = &pane.active_tab().selected_paths;
                            selection.len() > 1 && selection.contains(&path)
                        });
                    if !preserve_multi_selection {
                        this.toggle_selection(
                            &pid,
                            path.clone(),
                            e.modifiers.control,
                            e.modifiers.shift,
                            cx,
                        );
                    }
                    if e.click_count == 2 && !e.modifiers.control && !e.modifiers.shift {
                        this.open_file_item(&pid, path, is_dir, cx);
                    }
                });
            }
        })
        .on_click({
            let h = h.clone();
            let path = path.clone();
            let pid = pid.clone();
            move |event, _, cx| {
                h.update(cx, |this, cx| {
                    this.finish_item_selection_click(&pid, &path, event, cx);
                });
            }
        })
        .on_mouse_down(MouseButton::Middle, {
            let h = h.clone();
            let pid = pid.clone();
            let path = path.clone();
            move |_, _, cx| {
                cx.stop_propagation();
                if can_browse {
                    h.update(cx, |this, cx| {
                        this.open_directory_item(&pid, path.clone(), true, cx)
                    });
                }
            }
        })
        .on_mouse_down(MouseButton::Right, {
            let h = h.clone();
            let pid = pid.clone();
            let path = path.clone();
            move |e, _, cx| {
                cx.stop_propagation();
                let pos = e.position;
                h.update(cx, |this, cx| {
                    this.activate_pane(&pid, cx);
                    let pinned = this
                        .state
                        .pinned_items
                        .iter()
                        .any(|item| {
                            paths_match(&item.path, &path)
                        });
                    if !this.state.find_pane(&pid).is_some_and(
                        |p| {
                            p.active_tab()
                                .selected_paths
                                .contains(&path)
                        },
                    ) {
                        this.toggle_selection(
                            &pid,
                            path.clone(),
                            false,
                            false,
                            cx,
                        );
                    }
                    this.context_menus.clear();
                    this.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                        position: pos,
                        content: ContextMenuContent::Items(
                            with_grid_sort(oxyde_context_menu_items(
                                cx.i18n(),
                                h.clone(),
                                is_dir,
                                pinned,
                                path.clone(),
                                this,
                            ), this, h.clone(), &pid, cx.i18n()),
                            None,
                        ),
                        scroll_handle: UniformListScrollHandle::new(
                        ),
                        width: px(0.0),
                        bottom_padding: 0.0,
                        open_left: false,
                        hovered_item: None,
                        native_request_id: None,
                    });
                    cx.notify();
                });
            }
        })
        .on_drag(
            FileDragData {
                paths: paths.clone(),
                source_pane_id: pid.clone(),
            },
            {
                let h = h.clone();
                move |_, cursor_offset, window, cx| {
                    let app = h.clone();
                    let paths = paths.clone();
                    cx.new(|cx| {
                        FileDragPreview::new(
                            paths,
                            cursor_offset,
                            is_dir,
                            app,
                            window,
                            cx,
                        )
                    })
                }
            },
        )
        .child(
            div()
                .relative()
                .w_full()
                .h(rems(4.25))
                .when(file.is_hidden || file.is_system, |d| d.opacity(0.5))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(loading_size, |icon| icon.opacity(0.3))
                        .child(if show_grid_thumbnails
                            && !is_dir
                            && super::thumbnails::supports_thumbnail(file)
                        {
                            render_grid_thumbnail(file, &pid, icon, theme, use_system_icons).into_any_element()
                        } else {
                            render_file_icon(path.clone(), icon, rems(4.0), &theme, use_system_icons, true)
                        }),
                )
                .when(loading_size, |area| {
                    area.child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(size_loading(&path, theme.accent)),
                    )
                })
                .when(has_subdirectories, |row| {
                    row.child(
                        div()
                            .absolute()
                            .right_0()
                            .top_0()
                            .h_full()
                            .flex()
                            .items_center()
                            .child(svg().path("icons/chevron-right.svg").size_3().text_color(theme.text)),
                    )
                }),
        )
        .child(
            div()
                .w_full()
                .px_1()
                .text_center()
                .text_size(rems(0.75))
                .text_color(theme.text)
                .h(rems(1.125))
                .text_ellipsis()
                .overflow_hidden()
                .when(file.is_hidden || file.is_system, |d| d.opacity(0.5))
                .child(match editing_view {
                    Some(editing_view) => {
                        editing_view.into_any_element()
                    }
                    None => {
                        let h_click = h.clone();
                        let click_path = path.clone();
                        div()
                            .id(SharedString::from(format!("rename-name-{path}")))
                            .font_weight(FontWeight::MEDIUM)
                            .child(if file.is_dir {
                                name.clone()
                            } else {
                                grid_display_name(&name)
                            })
                            .on_click(move |event, window, cx| {
                                h_click.update(cx, |this, cx| {
                                    this.handle_slow_rename_click(
                                        click_path.clone(),
                                        is_dir,
                                        event,
                                        window,
                                        cx,
                                    );
                                });
                            })
                            .into_any_element()
                    }
                }),
        )
        .child(
            div()
                .text_size(rems(0.625))
                .text_color(theme.text_muted)
                .text_ellipsis()
                .when(file.is_hidden || file.is_system, |d| d.opacity(0.5))
                .child(if network_entry(file) {
                    network_category(file, cx.i18n())
                } else if file.is_dir {
                    cx.i18n().text(Message::FileTypeFolder).to_uppercase()
                } else {
                    file_type_display(file, cx.i18n())
                }),
        );
    if can_drop {
        card = card
            .drag_over::<FileDragData>({
                let h = h.clone();
                let target = path.clone();
                move |style, data, _, cx| {
                    let copy = data
                        .paths
                        .first()
                        .map(|p| {
                            crate::ui::app::drives_differ(
                                p, &target,
                            )
                        })
                        .unwrap_or(false);
                    h.update(cx, |this, cx| {
                        if this.drag_target_default_copy != copy {
                            this.drag_target_default_copy = copy;
                            this.update_drag_copy_mode(cx);
                        }
                    });
                    style.bg(theme.drag_hover)
                }
            })
            .on_drop({
                let h = h.clone();
                let target = path.clone();
                move |data: &FileDragData, _, cx| {
                    let paths = data.paths.clone();
                    let target = target.clone();
                    h.update(cx, |this, cx| {
                        this.execute_file_drop(paths, target, cx);
                    });
                }
            });
    }
    card.when(show_checkboxes, |d| d.child(
        div().absolute().top(rems(0.375)).left(rems(0.375))
            .when(!is_selected, |d| d.opacity(0.0).group_hover("file-grid-card", |s| s.opacity(1.0)))
            .child(selection_checkbox(is_selected, path.clone(), pid.clone(), h.clone(), theme))
    ))
}
