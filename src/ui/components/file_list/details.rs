use crate::ui::components::tooltip::NativeTooltipExt;
use super::details_row::{render_details_row, DetailsRowParams};
use super::headers::render_headers;
use super::presentation::{date_group_button, date_group_header, view_mode_button};
use super::status_bar_height;
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, DragTarget, FileDragData, OxydeApp};
use crate::ui::components::{with_grid_sort, pane_context_menu_items};
use crate::ui::components::scrollbar::{render_scrollbar, ScrollbarAxis};
use crate::ui::components::tooltip::Tooltip;
use crate::ui::date_groups::{DisplayRow};
use crate::ui::format::{format_number, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Language, Message};
use crate::ui::scale::{logical, rem_pixels};
use crate::ui::style::DetailsColumnWidths;
use crate::ui::theme::OxydeTheme;
use crate::ui::workspace::{PaneState, TabLoadStatus, ViewMode};
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::FileEntry;

pub(super) fn render_details_file_list(
    view: &OxydeApp,
    pane_id: String,
    handle: Entity<OxydeApp>,
    p_id_cap: String,
    theme: OxydeTheme,
    language: Language,
    pane: &PaneState,
    cx: &mut Context<OxydeApp>,
) -> AnyElement {
    let active_tab = pane.active_tab();
    let search_results = active_tab.quick_search.is_some();
    let calculating_sizes = active_tab.size_calculation.is_some();
    let presentation = active_tab.presentation(false, 1);
    let files = presentation.files.clone();
    let histogram_total = if active_tab.show_size_histogram { presentation.histogram_total } else { 0 };
    let rows = presentation.rows.clone();
    let count = (rows.len() + 1) as f32;
    let empty_message = if !files.is_empty() {
        None
    } else if !active_tab.files.is_empty() {
        Some(cx.i18n().text(Message::EmptyFiltered).to_string())
    } else {
        match &active_tab.load_status {
            TabLoadStatus::Loading => Some(cx.i18n().text(Message::FolderLoading).to_string()),
            TabLoadStatus::Ready => Some(cx.i18n().text(if active_tab.quick_search.is_some() { Message::QuickSearchEmpty } else { Message::FolderEmpty }).to_string()),
            TabLoadStatus::Error(message) => Some(cx.i18n().text(*message).to_string()),
            TabLoadStatus::ArchiveLimit(failure) => Some(crate::ui::format::error_detail(&failure.to_string(), cx.i18n())),
            TabLoadStatus::ArchiveListing(failure) => Some(crate::ui::format::error_detail(&failure.to_string(), cx.i18n())),
        }
    };

    let pane_size = view.pane_sizes.get(&pane_id).cloned().unwrap_or(Size {
        width: view.list_width,
        height: view.list_height,
    });
    let pane_viewport_w = pane_size.width.max(1.0);

    let v_v_h =
        (pane_size.height - rem_pixels(crate::ui::style::TAB_BAR_HEIGHT).as_f32() - rem_pixels(crate::ui::style::NAV_BAR_HEIGHT).as_f32() - if active_tab.quick_search.is_some() { logical(32.0) } else { 0.0 } - crate::ui::style::column_header_height() - status_bar_height() - crate::ui::components::scrollbar::zed_track_width()).max(1.0);
    let content_h = (count * crate::ui::style::row_height()).max(1.0);

    let trash_panel = active_tab.path == "trash://";
    let active_deleted_width = active_tab.deleted_col_width;
    let columns = DetailsColumnWidths::for_viewport_with_search(
        pane_viewport_w - if trash_panel { logical(active_tab.deleted_col_width) } else { 0.0 },
        active_tab.type_col_width,
        active_tab.size_col_width,
        active_tab.date_col_width,
        search_results || trash_panel,
    );
    let network_panel = active_tab.path == "__network_vincinity__";
    let name_w = if network_panel { columns.total() * 0.34 } else { columns.name };
    let type_w = if network_panel { columns.total() * 0.22 } else { columns.file_type };
    let size_w = if network_panel { columns.total() * 0.22 } else { columns.size };
    let date_w = if network_panel { columns.total() * 0.22 } else { columns.date };
    let location_w = columns.location;
    let total_w = columns.total() + if trash_panel { logical(active_tab.deleted_col_width) } else { 0.0 };

    let is_active_pane = view.state.active_pane_id == pane_id;
    let is_dragging_v = matches!(view.drag_target, Some(DragTarget::FileListV { ref pane_id, .. }) if pane_id == &p_id_cap);
    let show_tooltips = view.context_menus.is_empty();
    let use_system_icons = cfg!(target_os = "windows") && view.use_system_icons();
    let show_checkboxes = view.show_checkboxes();
    let show_media_previews = view.show_media_previews;

    let selected_paths = active_tab.selected_paths.clone();
    let displayed_folders = presentation.folders;
    let displayed_files = files.len().saturating_sub(displayed_folders);
    let selected_entries: Vec<&FileEntry> = if selected_paths.is_empty() { Vec::new() } else { files
        .iter()
        .filter(|file| selected_paths.contains(&file.path))
        .collect() };
    let selected_folders = selected_entries.iter().filter(|file| file.is_dir).count();
    let selected_files = selected_entries.len().saturating_sub(selected_folders);
    let selected_bytes: u64 = selected_entries
        .iter()
        .filter(|file| !file.is_dir || file.is_calculated)
        .map(|file| file.size)
        .sum();
    let selection_summary = if selected_entries.is_empty() {
        None
    } else {
        let mut args = FluentArgs::new();
        args.set("count", selected_entries.len());
        args.set(
            "number",
            format_number(selected_entries.len() as u64, language),
        );
        args.set("folder_count", selected_folders);
        args.set("file_count", selected_files);
        args.set("folders", format_number(selected_folders as u64, language));
        args.set("files", format_number(selected_files as u64, language));
        args.set("size", format_size(selected_bytes, false, false, language));
        let message = match (selected_folders, selected_files) {
            (0, _) => Message::SelectedFiles,
            (_, 0) => Message::SelectedFolders,
            _ => Message::SelectedMixed,
        };
        Some(cx.i18n().text_with_args(message, &args))
    };
    let inline_rename = view.inline_rename;
    let inline_rename_path = view.editing_path.clone();
    let inline_rename_view = view.editing_view.clone();
    let inline_rename_error = view.rename_error.map(|message| cx.i18n().text(message));
    let rename_shake_id = view.rename_shake_id;
    let folder_subdirectory_flags = view.folder_has_subdirs.clone();

    let mut container = div()
        .on_scroll_wheel(|_, window, cx| crate::ui::components::tooltip::hover::dismiss(window, cx))
        .child(crate::ui::components::tooltip::hover::container(handle.clone(), pane_id.clone(), px(crate::ui::style::column_header_height()), px(status_bar_height()), cx))
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .bg(if is_active_pane { theme.active_background } else { theme.background })
        .overflow_hidden()
        .relative()
        .child(
            canvas(
                |bounds, _, _| bounds,
                {
                    let h = handle.clone();
                    let p_id = pane_id.clone();
                    move |_, bounds: Bounds<Pixels>, _, cx| {
                        let b = Bounds {
                            origin: Point { x: bounds.origin.x.as_f32(), y: bounds.origin.y.as_f32() },
                            size: Size { width: bounds.size.width.as_f32(), height: (bounds.size.height.as_f32() - status_bar_height() - crate::ui::components::scrollbar::zed_track_width()).max(1.0) },
                        };
                        h.update(cx, |this, _| {
                            this.list_bounds.insert(p_id.clone(), b);
                        });
                    }
                }
            ).absolute().size_full()
        )
        // 1. Background fixe du header (en dessous)
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(crate::ui::style::COLUMN_HEADER_HEIGHT)
                .bg(theme.header)
        )
        // 2. Contenu défilant (Headers + List)
        .child(
            div()
                .size_full()
                .child(
                    div()
                        .absolute()
                        .left(active_tab.list_scroll_h.offset().x + rem_pixels(rems(0.5)))
                        .w(px(total_w))
                        .top_0()
                        .bottom(px(status_bar_height() + crate::ui::components::scrollbar::zed_track_width()))
                        .flex()
                        .flex_col()
                        .child(render_headers(view, pane_id.clone(), handle.clone(), name_w, location_w, type_w, size_w, date_w, if trash_panel { logical(active_tab.deleted_col_width) } else { 0.0 }, &theme, cx.i18n()))
                        .child(
                            uniform_list(
                                SharedString::from(format!("file-list-{}", pane_id)),
                                rows.len() + 1,
                                {
                                    let files = files.clone();
                                    let rows = rows.clone();
                                    let collapsed = active_tab.collapsed_date_groups.clone();
                                    let h_inner = handle.clone();
                                    let p_id_for_list = pane_id.clone();
                                    let selected_paths = selected_paths.clone();
                                    let folder_subdirectory_flags = folder_subdirectory_flags.clone();
                                    move |range, _, cx| {
                                        let folder_subdirectory_flags = folder_subdirectory_flags.clone();
                                        range.map(|i| {
                                            if i == rows.len() {
                                                return div().w_full().h(px(crate::ui::style::row_height())).into_any_element();
                                            }
                                            let theme = *OxydeTheme::global(cx);
                                            let index = match &rows[i] {
                                                DisplayRow::Header(group, count) => return date_group_header(*group, *count, collapsed.contains(group), p_id_for_list.clone(), h_inner.clone(), crate::ui::style::row_height(), cx),
                                                DisplayRow::Files(indices) => indices[0],
                                            };
                                            let file = &files[index];
                                            let is_dir = file.is_dir;
                                            let path = file.path.clone();
                                            let is_selected = selected_paths.contains(&path);
                                            let has_subdirectories = is_dir
                                                && folder_subdirectory_flags.get(&path).copied().unwrap_or(false);
                                            let p_id = p_id_for_list.clone();
                                            let h = h_inner.clone();
                                            let inline_editing = inline_rename
                                                && inline_rename_path.as_deref() == Some(path.as_str());
                                            let editing_view = inline_editing
                                                .then(|| inline_rename_view.clone())
                                                .flatten();
                                            let rename_error = inline_editing
                                                .then(|| inline_rename_error.clone())
                                                .flatten();

                                            render_details_row(DetailsRowParams {
                                                file,
                                                path,
                                                is_dir,
                                                is_selected,
                                                has_subdirectories,
                                                p_id,
                                                h,
                                                editing_view,
                                                rename_error,
                                                rename_shake_id,
                                                selected_paths: &selected_paths,
                                                theme,
                                                histogram_total,
                                                total_w,
                                                name_w,
                                                location_w,
                                                type_w,
                                                size_w,
                                                date_w,
                                                active_deleted_width,
                                                is_active_pane,
                                                show_tooltips,
                                                show_media_previews,
                                                show_checkboxes,
                                                use_system_icons,
                                                search_results,
                                                network_panel,
                                                calculating_sizes,
                                                trash_panel,
                                                language,
                                            }, cx)
                                        }).collect::<Vec<_>>()
                                    }
                                }
                            )
                            .on_mouse_down(MouseButton::Left, {
                                let h = handle.clone();
                                let p_id_cap = pane_id.clone();
                                move |e, _, cx| {
                                    let pos = e.position;
                                    let modifiers = e.modifiers;
                                    h.update(cx, |this, cx| {
                                        this.activate_pane(&p_id_cap, cx);
                                        if !modifiers.control && !modifiers.shift {
                                            if let Some(pane) = this.state.find_pane_mut(&p_id_cap) {
                                                let tab = pane.active_tab_mut();
                                                tab.selected_paths.clear();
                                                tab.selection_anchor = None;
                                            }
                                        }
                                        this.drag_target = Some(DragTarget::SelectionRectangle {
                                            pane_id: p_id_cap.clone(),
                                            start_pos: pos,
                                            current_pos: pos,
                                            modifiers,
                                            initial_selection: this.state.find_pane(&p_id_cap).map(|p| p.active_tab().selected_paths.clone()).unwrap_or_default(),
                                        });
                                        cx.notify();
                                    });
                                }
                            })
                            .on_mouse_down(MouseButton::Right, {
                                 let h = handle.clone();
                                 let p_id_cap = pane_id.clone();
                                 move |e, _, cx| {
                                     let pos = e.position;
                                     let p_id_cap_cap = p_id_cap.clone();
                                      h.update(cx, |this, cx| {
                                          this.activate_pane(&p_id_cap_cap, cx);
                                          this.context_menus.clear();
                                          this.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                                              position: pos,
                                              content: ContextMenuContent::Items(with_grid_sort(pane_context_menu_items(cx.i18n(), h.clone(), p_id_cap_cap.clone(), this), this, h.clone(), &p_id_cap_cap, cx.i18n()), None),
                                              scroll_handle: UniformListScrollHandle::new(),
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
                            .drag_over::<FileDragData>({
                                let p_id_cap = pane_id.clone();
                                let h_over = handle.clone();
                                let current_path = active_tab.path.clone();
                                move |style, data, _, cx| {
                                    if data.source_pane_id != p_id_cap {
                                        let default_copy = data.paths.first()
                                            .map(|s| crate::ui::app::drives_differ(s, &current_path))
                                            .unwrap_or(false);
                                        h_over.update(cx, |this, cx| {
                                            if this.drag_target_default_copy != default_copy {
                                                this.drag_target_default_copy = default_copy;
                                                this.update_drag_copy_mode(cx);
                                            }
                                        });
                                        style.bg(theme.selected)
                                    } else {
                                        style
                                    }
                                }
                            })
                            .on_drop({
                                let h = handle.clone();
                                let p_id_cap = pane_id.clone();
                                move |data: &FileDragData, _, cx| {
                                    let paths = data.paths.clone();
                                    h.update(cx, |this, cx| {
                                        let target = this.state.find_pane(&p_id_cap)
                                            .map(|p| p.active_tab().path.clone())
                                            .unwrap_or_default();
                                        if !target.is_empty() {
                                            this.execute_file_drop(paths, target, cx);
                                        }
                                    });
                                }
                            })
                            .track_scroll(&active_tab.list_scroll)
                            .w_full()
                            .flex_1()
                            .min_h_0()
                        )
                )
        )
        .child(super::selection_button::render(view, &pane_id, handle.clone(), theme, cx))
        // 3. Gutters interactifs
        .child(
            div()
                .absolute()
                .top(px(crate::ui::style::column_header_height()))
                .left_0()
                .bottom_0()
                .w(rems(0.5))
                .on_mouse_down(MouseButton::Left, {
                    let h = handle.clone();
                    let p_id_cap = pane_id.clone();
                    move |e, _, cx| {
                        cx.stop_propagation();
                        let pos = e.position;
                        let modifiers = e.modifiers;
                        h.update(cx, |this, cx| {
                            this.activate_pane(&p_id_cap, cx);
                            if !modifiers.control && !modifiers.shift {
                                if let Some(pane) = this.state.find_pane_mut(&p_id_cap) {
                                    let tab = pane.active_tab_mut();
                                    tab.selected_paths.clear();
                                    tab.selection_anchor = None;
                                }
                            }
                            this.drag_target = Some(DragTarget::SelectionRectangle {
                                pane_id: p_id_cap.clone(),
                                start_pos: pos,
                                current_pos: pos,
                                modifiers,
                                initial_selection: this.state.find_pane(&p_id_cap).map(|p| p.active_tab().selected_paths.clone()).unwrap_or_default(),
                            });
                            cx.notify();
                            });
                    }
                })
        )
        .child(
            div()
                .absolute()
                .top(px(crate::ui::style::column_header_height()))
                .right(px(crate::ui::components::scrollbar::zed_track_width()))
                .bottom_0()
                .w(rems(0.5))
                .on_mouse_down(MouseButton::Left, {
                    let h = handle.clone();
                    let p_id_cap = pane_id.clone();
                    move |e, _, cx| {
                        cx.stop_propagation();
                        let pos = e.position;
                        let modifiers = e.modifiers;
                        h.update(cx, |this, cx| {
                            this.activate_pane(&p_id_cap, cx);
                            if !modifiers.control && !modifiers.shift {
                                if let Some(pane) = this.state.find_pane_mut(&p_id_cap) {
                                    let tab = pane.active_tab_mut();
                                    tab.selected_paths.clear();
                                    tab.selection_anchor = None;
                                }
                            }
                            this.drag_target = Some(DragTarget::SelectionRectangle {
                                pane_id: p_id_cap.clone(),
                                start_pos: pos,
                                current_pos: pos,
                                modifiers,
                                initial_selection: this.state.find_pane(&p_id_cap).map(|p| p.active_tab().selected_paths.clone()).unwrap_or_default(),
                            });
                            cx.notify();
                            });
                    }
                })
        )
    ;

    // Selection Rectangle
    if let Some(DragTarget::SelectionRectangle {
        pane_id: ref drag_pane_id,
        start_pos,
        current_pos,
        ..
    }) = view.drag_target
    {
        if drag_pane_id == &pane_id {
            let list_origin = view
                .list_bounds
                .get(&pane_id)
                .map(|b| b.origin)
                .unwrap_or(Point { x: 0.0, y: 0.0 });

            let rect_x = start_pos.x.as_f32().min(current_pos.x.as_f32()) - list_origin.x;
            let rect_y = start_pos.y.as_f32().min(current_pos.y.as_f32()) - list_origin.y;
            let rect_w = (start_pos.x.as_f32() - current_pos.x.as_f32()).abs();
            let rect_h = (start_pos.y.as_f32() - current_pos.y.as_f32()).abs();

            container = container.child(
                div()
                    .absolute()
                    .left(px(rect_x))
                    .top(px(rect_y))
                    .w(px(rect_w))
                    .h(px(rect_h))
                    .bg(theme.selected)
                    .border(rems(0.0625))
                    .border_color(theme.accent),
            );
        }
    }

    let pane_suffix = format!("pane-{}", pane_id);
    let v_offset = -active_tab
        .list_scroll
        .0
        .borrow()
        .base_handle
        .offset()
        .y
        .as_f32();

    // Vertical scrollbar and its scroll-to-top affordance.
    let mut vertical_scroll_zone = div()
        .absolute()
        .top(px(crate::ui::style::column_header_height()))
        .right_0()
        .bottom(px(status_bar_height()))
        .w(rems(3.625))
        .group("file-list-scroll-zone")
        .child(render_scrollbar(
            theme,
            handle.clone(),
            ScrollbarAxis::Vertical,
            v_v_h,
            content_h,
            v_offset,
            is_dragging_v,
            format!("v-{}", pane_suffix),
            None,
            {
                let p_id = p_id_cap.clone();
                move |this, track_id, v_h, c_h, _| {
                    this.drag_target = Some(DragTarget::FileListV {
                        pane_id: p_id.clone(),
                        viewport_h: v_h,
                        content_h: c_h,
                        track_id,
                    });
                }
            },
        ));

    vertical_scroll_zone = vertical_scroll_zone.child(super::scroll_top::button(theme, handle.clone(), pane_id.clone(), v_offset, v_v_h, content_h));
    container = container.child(vertical_scroll_zone);

    // Horizontal scrollbar overlay
    container = container.child(
        div().absolute().left_0().bottom(px(status_bar_height())).right(px(crate::ui::components::scrollbar::zed_track_width())).h(px(crate::ui::components::scrollbar::zed_track_width()))
            .child(render_scrollbar(theme, handle.clone(), ScrollbarAxis::Horizontal, pane_viewport_w, columns.content_width() + if trash_panel { logical(active_deleted_width) } else { 0.0 }, -active_tab.list_scroll_h.offset().x.as_f32(), matches!(view.drag_target, Some(DragTarget::FileListH { ref pane_id, .. }) if pane_id == &p_id_cap), format!("h-{}", pane_suffix), None, {
                let p_id = p_id_cap.clone();
                move |this, track_id, v_w, c_w, _| {
                    this.drag_target = Some(DragTarget::FileListH { pane_id: p_id.clone(), viewport_w: v_w, content_w: c_w, track_id });
                }
            }))
    );

    if let Some(message) = empty_message {
        container = container.child(
            div()
                .absolute()
                .top(px(crate::ui::style::column_header_height() + logical(18.0)))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .text_color(theme.text_muted)
                .text_size(rems(0.8125))
                .child(message),
        );
    }

    let count_label = |count: usize, message| {
        let mut args = FluentArgs::new();
        args.set("count", count);
        args.set("number", format_number(count as u64, language));
        cx.i18n().text_with_args(message, &args)
    };
    let folders_label = count_label(displayed_folders, Message::StatusFolders);
    let files_label = count_label(displayed_files, Message::StatusFiles);
    container = container.child(
        div()
            .occlude()
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .h(px(status_bar_height()))
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .pr(rems(0.5))
            .py(rems(0.25))
            .bg(theme.header)
            .border_t(rems(0.0625))
            .border_color(theme.border)
            .text_size(rems(0.75))
            .text_color(theme.text)
            .font_weight(FontWeight::MEDIUM)
            .child(folders_label)
            .child(files_label)
            .when_some(selection_summary, |bar, summary| {
                bar.child("•")
                    .child(
                        div()
                            .id(SharedString::from(format!("file-selection-summary-{pane_id}")))
                            .min_w_0()
                            .text_ellipsis()
                            .overflow_hidden()
                            .text_color(theme.accent)
                            .native_tooltip(Tooltip::text(summary.clone()))
                            .child(summary),
                    )
            })
            .child(div().flex_1())
            .child(date_group_button(&pane_id, active_tab.group_by_date, handle.clone(), theme, cx))
            .child(div().flex_none().w(rems(0.0625)).h(rems(1.0)).bg(theme.border))
            .child(view_mode_button(
                &pane_id,
                ViewMode::Details,
                ViewMode::Details,
                "icons/list.svg",
                Message::ViewDetails,
                handle.clone(),
                theme,
                cx,
            ))
            .child(view_mode_button(
                &pane_id,
                ViewMode::Grid,
                ViewMode::Details,
                "icons/layout-grid.svg",
                Message::ViewGrid,
                handle.clone(),
                theme,
                cx,
            )),
    );

    container.into_any_element()
}
