use crate::ui::components::tooltip::NativeTooltipExt;
use super::grid_card::{render_grid_card, GridCardParams};
use super::presentation::{date_group_button, date_group_header, view_mode_button};
use super::status_bar_height;
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, DragTarget, OxydeApp};
use crate::ui::components::{with_grid_sort, pane_context_menu_items};
use crate::ui::components::scrollbar::{render_scrollbar, ScrollbarAxis};
use crate::ui::components::tooltip::Tooltip;
use crate::ui::date_groups::{display_rows, DisplayRow};
use crate::ui::format::{format_number, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::scale::logical;
use crate::ui::theme::{OxydeTheme, ThemeContext};
use crate::ui::workspace::{TabLoadStatus, ViewMode};
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::FileEntry;

pub(super) fn render_grid_file_list(
    view: &OxydeApp,
    pane_id: String,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let pane = view.state.layout.find_pane(&pane_id).expect("pane exists");
    let tab = pane.active_tab();
    let search_results = tab.quick_search.is_some();
    let calculating_sizes = tab.size_calculation.is_some();
    let files = tab.filtered_files().to_vec();
    let histogram_total = if tab.show_size_histogram && files.iter().all(|file| !file.is_dir || file.is_calculated) {
        files.iter().fold(0u64, |total, file| total.saturating_add(file.size))
    } else { 0 };
    let selected = tab.selected_paths.clone();
    let inline_rename = view.inline_rename;
    let editing_path = view.editing_path.clone();
    let editing_view = view.editing_view.clone();
    let rename_error = view.rename_error.map(|message| cx.i18n().text(message));
    let show_tooltips = view.context_menus.is_empty();
    let use_system_icons = cfg!(target_os = "windows") && view.use_system_icons();
    let show_checkboxes = view.show_checkboxes();
    let show_media_previews = view.show_media_previews;
    let show_grid_thumbnails = view.show_grid_thumbnails;
    let folder_subdirectory_flags = view.folder_has_subdirs.clone();
    let active = view.state.active_pane_id == pane_id;
    let theme = *cx.theme();
    let lang = cx.i18n().language();
    let pane_width = view
        .pane_sizes
        .get(&pane_id)
        .map(|s| s.width)
        .unwrap_or(view.list_width)
        .max(1.0);
    // Keep the grid inside the panel's padded viewport and leave a clear lane
    // for the overlay scrollbar, especially in the rightmost split pane.
    let grid = crate::ui::style::GridGeometry::for_viewport(pane_width, crate::ui::components::scrollbar::zed_track_width());
    let columns = grid.columns;
    let card_width = grid.cell_width - logical(4.0);
    let rows = display_rows(&files, tab.group_by_date, &tab.collapsed_date_groups, columns);
    let empty_message = if !files.is_empty() {
        None
    } else if !tab.files.is_empty() {
        Some(cx.i18n().text(Message::EmptyFiltered).to_string())
    } else {
        match &tab.load_status {
            TabLoadStatus::Loading => Some(cx.i18n().text(Message::FolderLoading).to_string()),
            TabLoadStatus::Ready => Some(cx.i18n().text(if tab.quick_search.is_some() { Message::QuickSearchEmpty } else { Message::FolderEmpty }).to_string()),
            TabLoadStatus::Error(message) => Some(cx.i18n().text(*message).to_string()),
            TabLoadStatus::ArchiveLimit(failure) => Some(crate::ui::format::error_detail(&failure.to_string(), cx.i18n())),
            TabLoadStatus::ArchiveListing(failure) => Some(crate::ui::format::error_detail(&failure.to_string(), cx.i18n())),
        }
    };
    let folders = files.iter().filter(|file| file.is_dir).count();
    let file_count = files.len().saturating_sub(folders);
    let mut folder_args = FluentArgs::new();
    folder_args.set("count", folders);
    folder_args.set("number", format_number(folders as u64, lang));
    let mut file_args = FluentArgs::new();
    file_args.set("count", file_count);
    file_args.set("number", format_number(file_count as u64, lang));
    let contents = if let Some(message) = empty_message {
        div()
            .id(SharedString::from(format!("file-grid-empty-{pane_id}")))
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .text_color(theme.text_muted)
            .child(message)
            .on_mouse_down(MouseButton::Right, {
                let h = handle.clone();
                let pid = pane_id.clone();
                move |event, _, cx| {
                    h.update(cx, |this, cx| {
                        this.activate_pane(&pid, cx);
                        let items = with_grid_sort(
                            pane_context_menu_items(cx.i18n(), h.clone(), pid.clone(), this),
                            this,
                            h.clone(),
                            &pid,
                            cx.i18n(),
                        );
                        this.context_menus.clear();
                        this.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                            position: event.position,
                            content: ContextMenuContent::Items(items, None),
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
            .into_any_element()
    } else {
        let render_rows = {
                let files = files.clone();
                let rows = rows.clone();
                let collapsed = tab.collapsed_date_groups.clone();
                let selected = selected.clone();
                let pane_id = pane_id.clone();
                let handle = handle.clone();
                let editing_path = editing_path.clone();
                let editing_view = editing_view.clone();
                let rename_error = rename_error.clone();
                let folder_subdirectory_flags = folder_subdirectory_flags.clone();
                move |range: std::ops::Range<usize>, cx: &mut App| {
                    range
                        .map(|row_index| {
                            let theme = *OxydeTheme::global(cx);
                            let indices = match &rows[row_index] {
                                DisplayRow::Header(group, count) => return div().flex_none().child(date_group_header(*group, *count, collapsed.contains(group), pane_id.clone(), handle.clone(), crate::ui::style::row_height(), cx)),
                                DisplayRow::Files(indices) => indices,
                            };
                            let cells = indices
                                .iter()
                                .map(|index| {
                                    let file = &files[*index];
                                    let path = file.path.clone();
                                    let name = file.name.clone();
                                    let is_dir = file.is_dir;
                                    let has_subdirectories = is_dir
                                        && folder_subdirectory_flags
                                            .get(&path)
                                            .copied()
                                            .unwrap_or(false);
                                    let is_selected = selected.contains(&path);
                                    let h = handle.clone();
                                    let pid = pane_id.clone();
                                    let paths = if is_selected && selected.len() > 1 {
                                        selected.iter().cloned().collect::<Vec<_>>()
                                    } else {
                                        vec![path.clone()]
                                    };
                                    let icon = crate::ui::components::icons::entry_icon(file);
                                    let inline_editing = inline_rename
                                        && editing_path.as_deref() == Some(path.as_str());
                                    let editing_view =
                                        inline_editing.then(|| editing_view.clone()).flatten();
                                    let rename_error =
                                        inline_editing.then(|| rename_error.clone()).flatten();
                                    let has_rename_error = rename_error.is_some();
                                    render_grid_card(GridCardParams {
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
                                    }, cx)
                                })
                                .collect::<Vec<_>>();
                            div()
                                .w_full()
                                .flex_none()
                                .h(px(crate::ui::style::grid_card_row_height()))
                                .flex()
                                .items_start()
                                .children(cells)
                        })
                        .collect::<Vec<_>>()
                }
            };
        let list = {
            let scroll = tab.list_scroll.0.borrow().base_handle.clone();
            let viewport = view.pane_sizes.get(&pane_id).map(|size| size.height).unwrap_or(view.list_height).max(logical(200.0));
            let actual_viewport = scroll.bounds().size.height.as_f32();
            let max_scroll = (crate::ui::date_groups::content_height(&rows, true) + crate::ui::style::row_height() - if actual_viewport > 0.0 { actual_viewport } else { viewport }).max(0.0);
            let offset = (-scroll.offset().y.as_f32()).clamp(0.0, max_scroll);
            if scroll.offset().y.as_f32() != -offset {
                scroll.set_offset(point(px(0.0), px(-offset)));
            }
            let visible = super::grid_scroll::visible_rows(&rows, tab.group_by_date, offset, if actual_viewport > 0.0 { actual_viewport } else { viewport });
            let scroll_handle = handle.clone();
            div().id(SharedString::from(if tab.group_by_date { format!("file-grid-grouped-{pane_id}") } else { format!("file-grid-{pane_id}") }))
                .h_full().flex().flex_col().overflow_y_scroll().track_scroll(&scroll)
                .on_scroll_wheel(move |_, _, cx| scroll_handle.update(cx, |_, cx| cx.notify()))
                .child(div().h(px(visible.top)).flex_none())
                .children(render_rows(visible.range, cx))
                .child(div().h(px((crate::ui::date_groups::content_height(&rows, true) - visible.bottom).max(0.0))).flex_none())
                .child(div().w_full().flex_none().h(px(crate::ui::style::row_height())))
                .into_any_element()
        };
        let viewport_height = tab
            .list_scroll
            .0
            .borrow()
            .base_handle
            .bounds()
            .size
            .height
            .as_f32();
        let content_height = crate::ui::date_groups::content_height(&rows, true) + crate::ui::style::row_height();
        let scroll_offset = -tab
            .list_scroll
            .0
            .borrow()
            .base_handle
            .offset()
            .y
            .as_f32();
        let is_dragging = matches!(view.drag_target, Some(DragTarget::FileListV { pane_id: ref drag_pane_id, .. }) if drag_pane_id == &pane_id);
        div()
            .id(SharedString::from(format!("file-grid-scroll-{pane_id}")))
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .px_2()
            .pr(px(crate::ui::components::scrollbar::zed_track_width()))
            .pt_1()
            .child(
                canvas(|bounds, _, _| bounds, {
                    let h = handle.clone();
                    let pid = pane_id.clone();
                    move |_, bounds: Bounds<Pixels>, _, cx| {
                        h.update(cx, |this, _| {
                            this.list_bounds.insert(
                                pid.clone(),
                                Bounds {
                                    origin: Point {
                                        x: bounds.origin.x.as_f32(),
                                        y: bounds.origin.y.as_f32(),
                                    },
                                    size: Size {
                                        width: bounds.size.width.as_f32(),
                                        height: bounds.size.height.as_f32(),
                                    },
                                },
                            );
                        });
                    }
                })
                .absolute()
                .size_full(),
            )
            .child(list)
            .child(render_scrollbar(
                theme,
                handle.clone(),
                ScrollbarAxis::Vertical,
                viewport_height,
                content_height,
                scroll_offset,
                is_dragging,
                format!("grid-{pane_id}"),
                None,
                {
                    let pid = pane_id.clone();
                    move |this, track_id, viewport_h, content_h, _| {
                        this.drag_target = Some(DragTarget::FileListV {
                            pane_id: pid.clone(),
                            viewport_h,
                            content_h,
                            track_id,
                        });
                    }
                },
            ))
            .on_mouse_down(MouseButton::Left, {
                let h = handle.clone();
                let pid = pane_id.clone();
                move |e, _, cx| {
                    let position = e.position;
                    let modifiers = e.modifiers;
                    h.update(cx, |this, cx| {
                        this.activate_pane(&pid, cx);
                        if !modifiers.control && !modifiers.shift {
                            if let Some(pane) = this.state.find_pane_mut(&pid) {
                                pane.active_tab_mut().selected_paths.clear();
                                pane.active_tab_mut().selection_anchor = None;
                            }
                        }
                        let initial_selection = this
                            .state
                            .find_pane(&pid)
                            .map(|pane| pane.active_tab().selected_paths.clone())
                            .unwrap_or_default();
                        this.drag_target = Some(DragTarget::SelectionRectangle {
                            pane_id: pid.clone(),
                            start_pos: position,
                            current_pos: position,
                            modifiers,
                            initial_selection,
                        });
                        cx.notify();
                    });
                }
            })
            .on_mouse_down(MouseButton::Right, {
                let h = handle.clone();
                let pid = pane_id.clone();
                move |e, _, cx| {
                    let position = e.position;
                    h.update(cx, |this, cx| {
                        this.activate_pane(&pid, cx);
                        this.context_menus.clear();
                        this.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                            position,
                            content: ContextMenuContent::Items(
                                with_grid_sort(pane_context_menu_items(cx.i18n(), h.clone(), pid.clone(), this), this, h.clone(), &pid, cx.i18n()),
                                None,
                            ),
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
            .into_any_element()
    };

    div()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .flex()
        .flex_col()
        .relative()
        .pb(px(status_bar_height()))
        .on_scroll_wheel(|_, window, cx| crate::ui::components::tooltip::hover::dismiss(window, cx))
        .child(crate::ui::components::tooltip::hover::container(handle.clone(), pane_id.clone(), px(0.0), px(status_bar_height()), cx))
        .overflow_hidden()
        .bg(if active {
            theme.active_background
        } else {
            theme.background
        })
        .child(contents)
        .when_some(
            match &view.drag_target {
                Some(DragTarget::SelectionRectangle {
                    pane_id: drag_pane_id,
                    start_pos,
                    current_pos,
                    ..
                }) if drag_pane_id == &pane_id => view.list_bounds.get(&pane_id).map(|bounds| {
                    let x = start_pos.x.as_f32().min(current_pos.x.as_f32()) - bounds.origin.x;
                    let y = start_pos.y.as_f32().min(current_pos.y.as_f32()) - bounds.origin.y;
                    let width = (start_pos.x.as_f32() - current_pos.x.as_f32()).abs();
                    let height = (start_pos.y.as_f32() - current_pos.y.as_f32()).abs();
                    div()
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .w(px(width))
                        .h(px(height))
                        .bg(theme.selected)
                        .border(rems(0.0625))
                        .border_color(theme.accent)
                }),
                _ => None,
            },
            |root, overlay| root.child(overlay),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(px(status_bar_height()))
                .occlude()
                .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .min_w_0()
                .flex_none()
                .overflow_hidden()
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
                .child(
                    cx.i18n()
                        .text_with_args(Message::StatusFolders, &folder_args),
                )
                .child(cx.i18n().text_with_args(Message::StatusFiles, &file_args))
                .when(!selected.is_empty(), |bar| {
                    let entries: Vec<&FileEntry> = files
                        .iter()
                        .filter(|file| selected.contains(&file.path))
                        .collect();
                    let folder_count = entries.iter().filter(|file| file.is_dir).count();
                    let file_count = entries.len().saturating_sub(folder_count);
                    let size: u64 = entries
                        .iter()
                        .filter(|file| !file.is_dir || file.is_calculated)
                        .map(|file| file.size)
                        .sum();
                    let mut args = FluentArgs::new();
                    args.set("count", entries.len());
                    args.set("number", format_number(entries.len() as u64, lang));
                    args.set("folder_count", folder_count);
                    args.set("file_count", file_count);
                    args.set("folders", format_number(folder_count as u64, lang));
                    args.set("files", format_number(file_count as u64, lang));
                    args.set("size", format_size(size, false, false, lang));
                    let message = match (folder_count, file_count) {
                        (0, _) => Message::SelectedFiles,
                        (_, 0) => Message::SelectedFolders,
                        _ => Message::SelectedMixed,
                    };
                    let summary = cx.i18n().text_with_args(message, &args);
                    bar.child("•").child(
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
                .child(date_group_button(&pane_id, tab.group_by_date, handle.clone(), theme, cx))
                .child(div().flex_none().w(rems(0.0625)).h(rems(1.0)).bg(theme.border))
                .child(view_mode_button(
                    &pane_id,
                    ViewMode::Details,
                    ViewMode::Grid,
                    "icons/list.svg",
                    Message::ViewDetails,
                    handle.clone(),
                    theme,
                    cx,
                ))
                .child(view_mode_button(
                    &pane_id,
                    ViewMode::Grid,
                    ViewMode::Grid,
                    "icons/layout-grid.svg",
                    Message::ViewGrid,
                    handle.clone(),
                    theme,
                    cx,
                )),
        )
}
