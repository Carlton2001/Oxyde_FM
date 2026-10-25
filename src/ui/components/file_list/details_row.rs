use crate::ui::components::tooltip::NativeTooltipExt;
use super::presentation::{file_type_display, network_category, selection_checkbox, size_histogram, size_loading};
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, FileDragData, FileDragPreview, FileDropTargetExt, OxydeApp};
use crate::ui::components::{paths_match, with_grid_sort, oxyde_context_menu_items};
use crate::ui::components::icons::render_file_icon;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::components::tooltip::hover::SharedTooltipExt;
use crate::ui::format::{format_date, format_size};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::logical;
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::FileEntry;

pub(super) struct DetailsRowParams<'a> {
    pub(super) file: &'a FileEntry,
    pub(super) path: String,
    pub(super) is_dir: bool,
    pub(super) is_selected: bool,
    pub(super) has_subdirectories: bool,
    pub(super) p_id: String,
    pub(super) h: Entity<OxydeApp>,
    pub(super) editing_view: Option<Entity<crate::ui::components::input::TextInput>>,
    pub(super) rename_error: Option<SharedString>,
    pub(super) rename_shake_id: u64,
    pub(super) selected_paths: &'a std::collections::HashSet<String>,
    pub(super) theme: OxydeTheme,
    pub(super) histogram_total: u64,
    pub(super) total_w: f32,
    pub(super) name_w: f32,
    pub(super) location_w: f32,
    pub(super) type_w: f32,
    pub(super) size_w: f32,
    pub(super) date_w: f32,
    pub(super) active_deleted_width: f32,
    pub(super) is_active_pane: bool,
    pub(super) show_tooltips: bool,
    pub(super) show_media_previews: bool,
    pub(super) show_checkboxes: bool,
    pub(super) use_system_icons: bool,
    pub(super) search_results: bool,
    pub(super) network_panel: bool,
    pub(super) calculating_sizes: bool,
    pub(super) provisional_size: bool,
    pub(super) size_flash: Option<std::time::Instant>,
    pub(super) trash_panel: bool,
    pub(super) language: crate::ui::i18n::Language,
}

pub(super) fn render_details_row(params: DetailsRowParams<'_>, cx: &mut App) -> AnyElement {
    let DetailsRowParams {
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
        selected_paths,
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
        provisional_size,
        size_flash,
        trash_panel,
        language,
    } = params;
    let can_browse = is_dir || file.is_navigable_archive() || file.needs_mount_for_browsing();
    let can_drop = is_dir || file.is_writable_archive();
    {
        let is_dimmed = file.is_hidden || file.is_system;
        div()
            .id(SharedString::from(format!("file-row-{}", path))).relative().when(histogram_total > 0, |d| d.child(size_histogram(file.size, histogram_total, theme.accent))).when(is_selected, |d| d.child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
            .flex()
            .items_center()
            .h(px(crate::ui::style::row_height()))
            .w(px(total_w))
            .bg(if is_selected { theme.selected } else if is_active_pane { theme.active_background } else { theme.background })
            .when(!is_selected, |s| s.hover(|s| s.bg(theme.hover)))
            .when(show_tooltips, |d| d.file_tooltip(h.clone(), p_id.clone(), path.clone(), cx, Tooltip::for_item(file.clone(), search_results, show_media_previews, provisional_size)))
            .on_mouse_down(MouseButton::Left, {
                let p = path.clone();
                let h_inner_inner = h.clone();
                let p_id_cap = p_id.clone();
                move |e, _, cx| {
                    cx.stop_propagation();
                    let multi = e.modifiers.control;
                    let shift = e.modifiers.shift;
                    h_inner_inner.update(cx, |this, cx| {
                        this.activate_pane(&p_id_cap, cx);
                        this.begin_item_press(&p_id_cap, &p, e);
                        // If file is already part of a multi-selection and no modifier
                        // is held, preserve the selection so dragging all files works.
                        // on_click will narrow the selection if no drag happens.
                        let preserve = !multi && !shift && this.state
                            .find_pane(&p_id_cap)
                            .map(|pane| {
                                let sel = &pane.active_tab().selected_paths;
                                sel.len() > 1 && sel.contains(&p)
                            })
                            .unwrap_or(false);
                        if !preserve {
                            this.toggle_selection(&p_id_cap, p.clone(), multi, shift, cx);
                        }
                    });
                    if e.click_count == 2 && !e.modifiers.control && !e.modifiers.shift {
                        h_inner_inner.update(cx, |this, cx| this.open_file_item(&p_id_cap, p.clone(), is_dir, cx));
                    }
                }
            })
            .on_click({
                let h = h.clone();
                let p = path.clone();
                let p_id_cap = p_id.clone();
                move |event, _, cx| {
                    h.update(cx, |this, cx| {
                        this.finish_item_selection_click(&p_id_cap, &p, event, cx);
                    });
                }
            })
            .on_mouse_down(MouseButton::Middle, {
                let p = path.clone();
                let h_inner_inner = h.clone();
                let p_id_cap = p_id.clone();
                move |_, _, cx| {
                    cx.stop_propagation();
                    if can_browse {
                        h_inner_inner.update(cx, |v, cx| v.open_directory_item(&p_id_cap, p.clone(), true, cx));
                    }
                }
            })
            .on_mouse_down(MouseButton::Right, {
                 let h_inner_inner = h.clone();
                 let p_id_cap = p_id.clone();
                 let p_for_right = path.clone();
                 move |e, window, cx| {
                     cx.stop_propagation();
                     let pos = e.position;
                     let p_id_cap_cap = p_id_cap.clone();
                     let p_for_right_cap = p_for_right.clone();
                      let mut editing_input = None;
                      let mut open_text_menu = false;
                      h_inner_inner.update(cx, |this, cx| {
                          if this.inline_rename
                              && this.editing_path.as_deref() == Some(p_for_right_cap.as_str())
                          {
                              editing_input = this.editing_view.clone();
                              open_text_menu = this.context_menus.is_empty();
                              return;
                          }
                          this.activate_pane(&p_id_cap_cap, cx);
                          let is_pinned = this.state.pinned_items.iter().any(|item| {
                              paths_match(&item.path, &p_for_right_cap)
                          });
                          if !this.state.find_pane(&p_id_cap_cap).map(|p| p.active_tab().selected_paths.contains(&p_for_right_cap)).unwrap_or(false) {
                              this.toggle_selection(&p_id_cap_cap, p_for_right_cap.clone(), false, false, cx);
                          }
                          this.context_menus.clear();
                          this.context_menus.push(ContextMenuInstance {
            native_bounds: None,
                              position: pos,
                              content: ContextMenuContent::Items(with_grid_sort(oxyde_context_menu_items(cx.i18n(), h_inner_inner.clone(), is_dir, is_pinned, p_for_right_cap.clone(), this), this, h_inner_inner.clone(), &p_id_cap_cap, cx.i18n()), None),
                              scroll_handle: UniformListScrollHandle::new(),
                              width: px(0.0),
                              bottom_padding: 0.0,
                              open_left: false,
                                            hovered_item: None,
                                            native_request_id: None, open_up: false,
                          });
                          cx.notify();
                      });
                      if open_text_menu {
                          if let Some(input) = editing_input {
                              let has_selection = input.read(cx).has_selection();
                              h_inner_inner.update(cx, |this, cx| {
                                  this.open_text_context_menu(input, has_selection, pos, window, cx);
                              });
                          }
                      }
                 }
             })
            .on_drag({
                let drag_paths = if is_selected && !selected_paths.is_empty() {
                    selected_paths.iter().cloned().collect::<Vec<_>>()
                } else {
                    vec![path.clone()]
                };
                FileDragData { paths: drag_paths.clone(), source_pane_id: p_id.clone() }
            }, {
                let drag_paths = if is_selected && !selected_paths.is_empty() {
                    selected_paths.iter().cloned().collect::<Vec<_>>()
                } else {
                    vec![path.clone()]
                };
                let h_drag = h.clone();
                move |_, cursor_offset, window, cx| {
                    let app = h_drag.clone();
                    let paths = drag_paths.clone();
                    cx.new(|cx| FileDragPreview::new(paths, cursor_offset, is_dir, app, window, cx))
                }
            })
            .when(can_drop, |row| {
                let h_drop = h.clone();
                let target_path = path.clone();
                row
                    .external_file_drop(h.clone(), path.clone())
                    .drag_over::<FileDragData>({
                        let h_over = h.clone();
                        let tgt = path.clone();
                        move |style, data, _, cx| {
                            let default_copy = data.paths.first()
                                .map(|s| crate::ui::app::drives_differ(s, &tgt))
                                .unwrap_or(false);
                            h_over.update(cx, |this, cx| {
                                if this.drag_target_default_copy != default_copy {
                                    this.drag_target_default_copy = default_copy;
                                    this.update_drag_copy_mode(cx);
                                }
                            });
                            style.bg(theme.drag_hover)
                        }
                    })
                    .on_drop(move |data: &FileDragData, _, cx| {
                        let paths = data.paths.clone();
                        let target = target_path.clone();
                        h_drop.update(cx, |this, cx| {
                            this.execute_file_drop(paths, target, cx);
                        });
                    })
            })
            .child({
                let path_str = path.clone();
                let file_name = file.name.clone();

                div()
                    .w(px(name_w))
                    .flex_none()
                    .h_full()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .when(is_dimmed, |d| d.opacity(0.5))
                    .when(show_checkboxes, |d| d.child(
                        div().ml(rems(0.5)).flex_none()
                            .child(selection_checkbox(is_selected, path.clone(), p_id.clone(), h.clone(), theme))
                    ))
                    .child(
                        div()
                            .id(SharedString::from(format!("file-content-{}", path_str)))
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .px(rems(0.5))
                            .flex()
                            .items_center()
                            .child({
                                let icon_type = crate::ui::components::icons::entry_icon(file);
                                div().flex_none().mr_2().child(render_file_icon(path_str.clone(), icon_type, rems(1.0), &theme, use_system_icons, false))
                            })
                            .child(match editing_view {
                                Some(editing_view) => {
                                    let has_error = rename_error.is_some();
                                    let field = div()
                                        .id(SharedString::from(format!("inline-rename-{}", path)))
                                        .flex_1()
                                        .min_w_0()
                                        // The name cell has 8 px horizontal padding on each side,
                                        // then a 16 px icon and its 8 px right margin.
                                        .w(px((name_w - logical(40.0)).max(1.0)))
                                        .h_full()
                                        .flex()
                                        .items_center()
                                        .when_some(rename_error, |field, error| {
                                            field
                                                .native_tooltip(Tooltip::text(error))
                                                .border(rems(0.0625))
                                                .border_color(theme.error)
                                                .rounded_sm()
                                        })
                                        .child(editing_view.into_any_element())
                                        ;
                                    if has_error {
                                        field.with_animation(
                                            SharedString::from(format!("rename-shake-{rename_shake_id}")),
                                            Animation::new(std::time::Duration::from_millis(300)),
                                            |field, delta| {
                                                let amplitude = (1.0 - delta) * logical(8.0);
                                                let offset = (delta * std::f32::consts::PI * 6.0).sin() * amplitude;
                                                field.relative().left(px(offset))
                                            },
                                        ).into_any_element()
                                    } else {
                                        field.into_any_element()
                                    }
                                },
                                None => {
                                    let h_click = h.clone();
                                    let click_path = path.clone();
                                    let name = div()
                                        .id(SharedString::from(format!("rename-name-{path}")))
                                        .flex_shrink(1.0)
                                        .min_w_0()
                                        .truncate()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text)
                                        .child(file_name.clone())
                                        .on_click(move |event, window, cx| {
                                            let path = click_path.clone();
                                            h_click.update(cx, |this, cx| {
                                                this.handle_slow_rename_click(
                                                    path,
                                                    is_dir,
                                                    event,
                                                    window,
                                                    cx,
                                                );
                                            });
                                        });
                                    if has_subdirectories {
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .items_center()
                                            .overflow_hidden()
                                            .child(name)
                                            .child(
                                                div()
                                                    .ml_2()
                                                    .flex_none()
                                                    .child(
                                                        svg()
                                                            .path("icons/chevron-right.svg")
                                                            .size_3()
                                                            .text_color(theme.text),
                                                    ),
                                            )
                                            .into_any_element()
                                    } else {
                                        name.into_any_element()
                                    }
                                }
                            })
                    )
            })
            .when(network_panel, |row| {
                let details = file.network_details.as_ref();
                let category = network_category(file, cx.i18n());
                row.children([(type_w, category), (size_w, details.map(|d| d.workgroup.clone()).unwrap_or_default()), (date_w, details.map(|d| d.location.clone()).unwrap_or_default())].into_iter().map(|(width, value)| div().w(px(width)).flex_none().px(rems(0.5)).text_color(theme.text_muted).truncate().text_size(rems(0.75)).child(value)))
            })
            .when(location_w > 0.0, |row| row.child(div().w(px(location_w)).flex_none().px(rems(0.5)).text_color(theme.text_muted).truncate().text_size(rems(0.75)).child(std::path::Path::new(file.original_path.as_deref().unwrap_or(&file.path)).parent().map(|parent| parent.to_string_lossy().into_owned()).unwrap_or_default())))
            .when(!network_panel, |row| row.child(div().w(px(type_w)).flex_none().px(rems(0.5)).text_color(theme.text_muted).truncate().text_size(rems(0.75)).when(is_dimmed, |d| d.opacity(0.5)).child(file_type_display(file, cx.i18n()))))
            .when(!network_panel, |row| row
                .child(flash_size_cell(div().w(px(size_w)).flex_none().px(rems(0.5)).text_align(TextAlign::Right).text_color(theme.text_muted).truncate().text_size(rems(0.75)).when(is_dimmed, |d| d.opacity(0.5)).child(if calculating_sizes && is_dir && !file.is_calculated {
                    size_loading(&path, theme.accent)
                } else if provisional_size {
                    div().id(SharedString::from(format!("size-provisional-details-{path}")))
                        .text_color(theme.accent)
                        .child(format_size(file.size, is_dir, file.is_calculated, language))
                        .into_any_element()
                } else if is_dir && file.is_calculated && file.size == 0 {
                    div().child(cx.i18n().text(Message::FolderSizeEmpty)).into_any_element()
                } else {
                    div().child(format_size(file.size, is_dir, file.is_calculated, language)).into_any_element()
                }), &path, size_flash, theme.accent))
                .child(div().w(px(date_w)).flex_none().px(rems(0.5)).text_color(theme.text_muted).truncate().text_size(rems(0.75)).when(is_dimmed, |d| d.opacity(0.5)).child(format_date(file.modified, crate::ui::format::date_format(cx)))))
            .when(trash_panel, |row| row.child(div().w(px(logical(active_deleted_width))).flex_none().px(rems(0.5)).text_color(theme.text_muted).truncate().text_size(rems(0.75)).child(format_date(file.deleted_time.and_then(|date| u64::try_from(date).ok()).unwrap_or(0), crate::ui::format::date_format(cx)))))
            .into_any_element()
    }
}

fn flash_size_cell(cell: Div, path: &str, started: Option<std::time::Instant>, color: Hsla) -> AnyElement {
    let Some(started) = started else { return cell.into_any_element(); };
    let duration = crate::ui::workspace::SIZE_FLASH_DURATION;
    if started.elapsed() >= duration {
        // Virtualized rows must not flash later when scrolled into view.
        return cell.into_any_element();
    }
    cell.with_animation(
        SharedString::from(format!("size-flash-{path}-{started:?}")),
        Animation::new(duration),
        move |cell, progress| {
            // GPUI also supplies the final frame immediately for reduced motion.
            let fade = if progress >= 1.0 { 0.0 } else {
                (1.0 - started.elapsed().as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
            };
            cell.bg(color.opacity(0.24 * fade * fade))
        },
    ).into_any_element()
}
