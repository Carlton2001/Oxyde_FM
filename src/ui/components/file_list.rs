use crate::ui::scale::{logical, rem_pixels};
use crate::ui::app::{
    ContextMenuContent, ContextMenuInstance, DragTarget, FileDragData, FileDragPreview, OxydeApp};
use crate::ui::components::scrollbar::{render_scrollbar, ScrollbarAxis};
use crate::ui::components::tooltip::Tooltip;
use crate::ui::format::{format_date, format_number, format_size};
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::style::{DetailsColumnWidths};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use crate::ui::workspace::{SortColumn, SortDirection, TabLoadStatus, ViewMode};
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::models::FileEntry;

use crate::ui::components::icons::render_file_icon;
use crate::ui::date_groups::{display_rows, DateGroup, DisplayRow};

fn date_group_header(group: DateGroup, count: usize, collapsed: bool, pane_id: String, handle: Entity<OxydeApp>, height: f32, cx: &mut App) -> AnyElement {
    let theme = *OxydeTheme::global(cx);
    let mut args = FluentArgs::new();
    args.set("count", count);
    args.set("number", format_number(count as u64, cx.i18n().language()));
    div().id(SharedString::from(format!("date-group-{pane_id}-{group:?}"))).flex_none()
        .h(px(height)).w_full().flex().items_center().gap(rems(0.5))
        .px(rems(0.5)).text_size(rems(0.8125)).text_color(theme.text)
        .border_b(rems(0.0625)).border_color(theme.border)
        .hover(|style| style.bg(theme.control_hover))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            handle.update(cx, |view, cx| {
                view.finish_pointer_drag(cx);
                view.activate_pane(&pane_id, cx);
                if let Some(pane) = view.state.find_pane_mut(&pane_id) {
                    let groups = &mut pane.active_tab_mut().collapsed_date_groups;
                    if !groups.remove(&group) { groups.insert(group); }
                }
                cx.notify();
            });
        })
        .child(svg().path(if collapsed { "icons/chevron-right.svg" } else { "icons/chevron-down.svg" }).size(rems(1.0)).text_color(theme.accent))
        .child(cx.i18n().text(group.message()))
        .child(div().rounded_sm().border(rems(0.0625)).border_color(theme.border).px(rems(0.375)).text_size(rems(0.75)).text_color(theme.text_muted).child(cx.i18n().text_with_args(Message::DateGroupCount, &args)))
        .into_any_element()
}

fn date_group_button(pane_id: &str, enabled: bool, handle: Entity<OxydeApp>, theme: OxydeTheme, cx: &mut Context<OxydeApp>) -> impl IntoElement {
    let pane_id = pane_id.to_string();
    div().id(SharedString::from(format!("group-by-date-{pane_id}")))
        .size(rems(1.5)).flex_none().flex().items_center().justify_center().rounded_md()
        .bg(if enabled { theme.selected } else { theme.header })
        .hover(|style| style.bg(theme.control_hover))
        .tooltip(Tooltip::text(cx.i18n().text(Message::SettingsGroupByDate)))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            handle.update(cx, |view, cx| view.toggle_date_grouping(&pane_id, cx));
        })
        .child(svg().path("icons/calendar-days.svg").size(rems(1.0)).text_color(if enabled { theme.accent } else { theme.text_muted }))
}

struct GridThumbnailAsset;
pub(crate) fn invalidate_thumbnail(path: &str, cx: &mut App) {
    cx.remove_asset::<GridThumbnailAsset>(&SharedString::from(path.to_string()));
}

impl Asset for GridThumbnailAsset {
    type Source = SharedString;
    type Output = Result<std::sync::Arc<RenderImage>, ImageCacheError>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl std::future::Future<Output = Self::Output> + Send + 'static {
        let task = cx.background_executor().spawn(async move {
            oxyde_lib::platform::platform()
                .thumbnail(source.to_string())
                .map_err(|error| ImageCacheError::Asset(error.to_string().into()))
        });
        async move {
            let rgba = task.await?;
            Ok(std::sync::Arc::new(RenderImage::new(vec![
                image::Frame::new(rgba),
            ])))
        }
    }
}

const STATUS_BAR_HEIGHT: Rems = rems(2.0);
fn status_bar_height() -> f32 { rem_pixels(STATUS_BAR_HEIGHT).as_f32() }

/// Returns the display type label for a file entry.
/// Symlinks and junctions are identified before extension/dir detection.
pub(crate) fn file_type_label(f: &FileEntry) -> String {
    if network_entry(f) { return "Réseau".to_string(); }
    if f.is_symlink {
        return "Symlink".to_string();
    }
    if f.is_junction {
        return "Junction".to_string();
    }
    if f.is_dir {
        return "Dossier".to_string();
    }
    std::path::Path::new(&f.name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_uppercase())
        .unwrap_or_else(|| "Fichier".to_string())
}

fn file_type_display(f: &FileEntry, i18n: &crate::ui::i18n::I18n) -> String {
    let message = if network_entry(f) {
        Some(Message::SidebarNetwork)
    } else if f.is_symlink {
        Some(Message::FileTypeSymlink)
    } else if f.is_junction {
        Some(Message::FileTypeJunction)
    } else if f.is_dir {
        Some(Message::FileTypeFolder)
    } else if std::path::Path::new(&f.name).extension().is_none() {
        Some(Message::FileTypeFile)
    } else {
        None
    };
    message
        .map(|message| i18n.text(message).to_string())
        .unwrap_or_else(|| file_type_label(f))
}

/// Shell devices and UNC server roots belong to the network neighborhood.
fn network_entry(file: &FileEntry) -> bool {
    file.path.starts_with("::{") || file.path.starts_with('?')
        || file.path.strip_prefix("\\\\").is_some_and(|tail| {
            let server = tail.trim_end_matches('\\');
            !server.is_empty() && !server.contains('\\')
        })
}

fn network_category(file: &FileEntry, i18n: &crate::ui::i18n::I18n) -> String {
    if let Some(category) = file.network_details.as_ref().map(|d| &d.category).filter(|category| !category.is_empty()) {
        return category.clone();
    }
    i18n.text(if oxyde_lib::commands::io::is_network_server(&file.path) { Message::NetworkComputer } else { Message::NetworkOtherDevices }).to_string()
}

#[cfg(test)]
mod network_type_tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn network_devices_and_servers_use_localized_network_type_without_relabeling_shares() {
        for language in [crate::ui::i18n::Language::French, crate::ui::i18n::Language::English] {
            let i18n = crate::ui::i18n::I18n::for_language(language);
            for (path, is_dir) in [("::{device}", false), ("?device", false), ("\\\\server", true)] {
                let file = FileEntry { name: "Bbox".into(), path: path.into(), is_dir, ..Default::default() };
                assert_eq!(file_type_display(&file, &i18n), i18n.text(Message::SidebarNetwork).as_ref());
                assert_eq!(file_type_label(&file), "Réseau");
            }
            for path in ["\\\\server\\share", "\\\\server\\share\\folder", "C:\\folder"] {
                let file = FileEntry { name: "folder".into(), path: path.into(), is_dir: true, ..Default::default() };
                assert_eq!(file_type_display(&file, &i18n), i18n.text(Message::FileTypeFolder).as_ref());
            }
        }
    }
}

/// Uppercases only the extension for presentation in grid cards.
fn grid_display_name(name: &str) -> String {
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return name.to_string();
    };
    if stem.is_empty() || extension.is_empty() {
        return name.to_string();
    }
    format!("{stem}.{}", extension.to_uppercase())
}

pub fn render_file_list(
    view: &OxydeApp,
    pane_id: String,
    handle: Entity<OxydeApp>,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let p_id_cap = pane_id.clone();
    let theme = *cx.theme();
    let language = cx.i18n().language();
    let pane = view.state.layout.find_pane(&pane_id).expect("pane exists");
    if pane.active_tab().view_mode == ViewMode::Grid {
        return render_grid_file_list(view, pane_id, handle, cx).into_any_element();
    }
    let active_tab = pane.active_tab();
    let search_results = active_tab.quick_search.is_some();
    let calculating_sizes = active_tab.size_calculation.is_some();
    let files = active_tab.filtered_files().to_vec();
    let histogram_total = if active_tab.show_size_histogram && files.iter().all(|file| !file.is_dir || file.is_calculated) {
        files.iter().fold(0u64, |total, file| total.saturating_add(file.size))
    } else { 0 };
    let rows = display_rows(&files, active_tab.group_by_date, &active_tab.collapsed_date_groups, 1);
    let count = rows.len() as f32;
    let empty_message = if !files.is_empty() {
        None
    } else if !active_tab.files.is_empty() {
        Some(cx.i18n().text(Message::EmptyFiltered).to_string())
    } else {
        match &active_tab.load_status {
            TabLoadStatus::Loading => Some(cx.i18n().text(Message::FolderLoading).to_string()),
            TabLoadStatus::Ready => Some(cx.i18n().text(if active_tab.quick_search.is_some() { Message::QuickSearchEmpty } else { Message::FolderEmpty }).to_string()),
            TabLoadStatus::Error(message) => Some(cx.i18n().text(*message).to_string()),
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
    let displayed_folders = files.iter().filter(|file| file.is_dir).count();
    let displayed_files = files.len().saturating_sub(displayed_folders);
    let selected_entries: Vec<&FileEntry> = files
        .iter()
        .filter(|file| selected_paths.contains(&file.path))
        .collect();
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
        .flex_1()
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
                            size: Size { width: bounds.size.width.as_f32(), height: bounds.size.height.as_f32() },
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
                        .h_full()
                        .flex()
                        .flex_col()
                        .child(render_headers(view, pane_id.clone(), handle.clone(), name_w, location_w, type_w, size_w, date_w, if trash_panel { logical(active_tab.deleted_col_width) } else { 0.0 }, &theme, cx.i18n()))
                        .child(
                            uniform_list(
                                SharedString::from(format!("file-list-{}", pane_id)),
                                rows.len(),
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

                                            {
                                            let file_name_tt = file.name.clone();
                                            let path_tt = path.clone();
                                            let ext_tt = file_type_label(file);
                                            let size_tt = (file.size, is_dir, file.is_calculated);
                                            let date_tt = file.modified;
                                            let is_dimmed = file.is_hidden || file.is_system;
                                            div()
                                                .id(SharedString::from(format!("file-row-{}", path))).relative().when(histogram_total > 0, |d| d.child(size_histogram(file.size, histogram_total, theme.accent))).when(is_selected, |d| d.child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
                                                .flex()
                                                .items_center()
                                                .h(px(crate::ui::style::row_height()))
                                                .w(px(total_w))
                                                .bg(if is_selected { theme.selected } else if is_active_pane { theme.active_background } else { theme.background })
                                                .when(!is_selected, |s| s.hover(|s| s.bg(theme.hover)))
                                                .when(show_tooltips && file.original_path.is_some(), |d| d.tooltip(Tooltip::for_trash(file.clone(), ext_tt.clone(), show_media_previews)))
                                                .when(show_tooltips && network_entry(file), |d| d.tooltip(Tooltip::for_network(file.clone())))
                                                .when(show_tooltips && search_results, |d| d.tooltip(Tooltip::for_search_result(file.clone(), ext_tt.clone(), show_media_previews)))
                                                .when(show_tooltips && file.original_path.is_none() && !search_results && !network_entry(file) && show_media_previews, |d| d.tooltip(Tooltip::for_file_full(file_name_tt.clone(), path_tt, ext_tt.clone(), size_tt.clone(), date_tt.clone())))
                                                .when(show_tooltips && file.original_path.is_none() && !search_results && !network_entry(file) && !show_media_previews, |d| d.tooltip(Tooltip::for_file_no_preview(file_name_tt, ext_tt, size_tt, date_tt)))
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
                                                        if e.click_count == 2 {
                                                            h_inner_inner.update(cx, |this, cx| this.open_file_item(&p_id_cap, p.clone(), is_dir, cx));
                                                        }
                                                    }
                                                })
                                                .on_click({
                                                    let h = h.clone();
                                                    let p = path.clone();
                                                    let p_id_cap = p_id.clone();
                                                    move |event, _, cx| {
                                                        // Only narrow when no modifier: click without drag on a
                                                        // multi-selected file should deselect the others.
                                                        if event.modifiers().control || event.modifiers().shift {
                                                            return;
                                                        }
                                                        h.update(cx, |this, cx| {
                                                            let is_multi = this.state.find_pane(&p_id_cap)
                                                                .map(|pane| pane.active_tab().selected_paths.len() > 1)
                                                                .unwrap_or(false);
                                                            if is_multi {
                                                                if let Some(pane) = this.state.find_pane_mut(&p_id_cap) {
                                                                    let tab = pane.active_tab_mut();
                                                                    tab.selected_paths.clear();
                                                                    tab.selected_paths.insert(p.clone());
                                                                    tab.selection_anchor = Some(p.clone());
                                                                }
                                                                cx.notify();
                                                            }
                                                        });
                                                    }
                                                })
                                                .on_mouse_down(MouseButton::Middle, {
                                                    let p = path.clone();
                                                    let h_inner_inner = h.clone();
                                                    let p_id_cap = p_id.clone();
                                                    move |_, _, cx| {
                                                        cx.stop_propagation();
                                                        if is_dir || oxyde_lib::utils::archive::is_navigable_archive(std::path::Path::new(&p)) || oxyde_lib::utils::disk_image::needs_mount_for_browsing(std::path::Path::new(&p)) {
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
                                                                  super::paths_match(&item.path, &p_for_right_cap)
                                                              });
                                                              if !this.state.find_pane(&p_id_cap_cap).map(|p| p.active_tab().selected_paths.contains(&p_for_right_cap)).unwrap_or(false) {
                                                                  this.toggle_selection(&p_id_cap_cap, p_for_right_cap.clone(), false, false, cx);
                                                              }
                                                              this.context_menus.clear();
                                                              this.context_menus.push(ContextMenuInstance {
                                                                  position: pos,
                                                                  content: ContextMenuContent::Items(super::with_grid_sort(super::oxyde_context_menu_items(cx.i18n(), h_inner_inner.clone(), is_dir, is_pinned, p_for_right_cap.clone(), this), this, h_inner_inner.clone(), &p_id_cap_cap, cx.i18n()), None),
                                                                  scroll_handle: UniformListScrollHandle::new(),
                                                                  width: px(0.0),
                                                                  bottom_padding: 0.0,
                                                                  open_left: false,
                                    hovered_item: None,
                                    native_request_id: None,
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
                                                    move |_, cursor_offset, _window, cx| {
                                                        let app = h_drag.clone();
                                                        let paths = drag_paths.clone();
                                                        cx.new(|cx| FileDragPreview::new(paths, cursor_offset, app, cx))
                                                    }
                                                })
                                                .when(is_dir || oxyde_lib::utils::archive::is_writable_archive(std::path::Path::new(&path)), |row| {
                                                    let h_drop = h.clone();
                                                    let target_path = path.clone();
                                                    row
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
                                                                if this.drag_copy_mode {
                                                                    this.execute_file_copy(paths, target, cx);
                                                                } else {
                                                                    this.execute_file_move(paths, target, cx);
                                                                }
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
                                                                    div().flex_none().mr_2().child(render_file_icon(path_str.clone(), icon_type, rems(1.0), if is_dir { theme.accent } else { theme.text_muted }, use_system_icons, false))
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
                                                                                    .tooltip(Tooltip::text(error))
                                                                                    .border(rems(0.0625))
                                                                                    .border_color(theme.error)
                                                                                    .rounded_sm()
                                                                            })
                                                                            .child(editing_view.into_any_element())
                                                                            ;
                                                                        if has_error {
                                                                            field.with_animation(
                                                                                SharedString::from(format!("rename-shake-{rename_shake_id}")),
                                                                                Animation::new(std::time::Duration::from_millis(480)),
                                                                                |field, delta| {
                                                                                    let amplitude = (1.0 - delta) * logical(8.0);
                                                                                    let offset = (delta * std::f32::consts::PI * 8.0).sin() * amplitude;
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
                                                                            .flex_shrink(1.0)
                                                                            .min_w_0()
                                                                            .truncate()
                                                                            .font_weight(FontWeight::MEDIUM)
                                                                            .text_color(theme.text)
                                                                            .child(file_name.clone())
                                                                            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                                                                                let path = click_path.clone();
                                                                                h_click.update(cx, |this, cx| {
                                                                                    this.handle_slow_rename_click(
                                                                                        path,
                                                                                        is_dir,
                                                                                        event.click_count,
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
                                                                                                .text_color(theme.text_muted.opacity(0.65)),
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
                                                    .child(div().w(px(size_w)).flex_none().px(rems(0.5)).text_align(TextAlign::Right).text_color(theme.text_muted).truncate().text_size(rems(0.75)).when(is_dimmed, |d| d.opacity(0.5)).child(if calculating_sizes && is_dir && !file.is_calculated { size_loading(&path, theme.accent) } else if is_dir && file.is_calculated && file.size == 0 { div().child(cx.i18n().text(Message::FolderSizeEmpty)).into_any_element() } else { div().child(format_size(file.size, is_dir, file.is_calculated, language)).into_any_element() }))
                                                    .child(div().w(px(date_w)).flex_none().px(rems(0.5)).text_color(theme.text_muted).truncate().text_size(rems(0.75)).when(is_dimmed, |d| d.opacity(0.5)).child(format_date(file.modified, crate::ui::format::date_format(cx)))))
                                                .when(trash_panel, |row| row.child(div().w(px(logical(active_deleted_width))).flex_none().px(rems(0.5)).text_color(theme.text_muted).truncate().text_size(rems(0.75)).child(format_date(file.deleted_time.and_then(|date| u64::try_from(date).ok()).unwrap_or(0), crate::ui::format::date_format(cx)))))
                                                .into_any_element()
                                            }}).collect::<Vec<_>>()
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
                                              position: pos,
                                              content: ContextMenuContent::Items(super::with_grid_sort(super::pane_context_menu_items(cx.i18n(), h.clone(), p_id_cap_cap.clone(), this), this, h.clone(), &p_id_cap_cap, cx.i18n()), None),
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
                                            if this.drag_copy_mode {
                                                this.execute_file_copy(paths, target, cx);
                                            } else {
                                                this.execute_file_move(paths, target, cx);
                                            }
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
    let v_ratio = (v_v_h / content_h.max(1.0)).min(1.0);
    let v_thumb_size = (v_v_h * v_ratio).max(logical(40.0)).min(v_v_h);
    let v_max_offset = (content_h - v_v_h).max(1.0);
    let v_thumb_pos = (v_offset / v_max_offset).clamp(0.0, 1.0) * (v_v_h - v_thumb_size);
    let scroll_button_top =
        (v_thumb_pos + v_thumb_size / 2.0 - logical(16.0)).clamp(logical(4.0), (v_v_h - logical(36.0)).max(logical(4.0)));

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

    if v_offset > crate::ui::style::row_height() * 2.0 && content_h > v_v_h {
        let h_scroll_top = handle.clone();
        let pane_id_scroll_top = pane_id.clone();
        vertical_scroll_zone = vertical_scroll_zone.child(
            div()
                .id(format!("scroll-to-top-{pane_id_scroll_top}"))
                .absolute()
                .top(px(scroll_button_top))
                .right(rems(1.0))
                .w(rems(2.0))
                .h(rems(2.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(rems(0.5))
                .bg(theme.accent)
                .shadow(crate::ui::style::rem_shadow(false))
                .opacity(0.0)
                .group_hover("file-list-scroll-zone", |style| style.opacity(1.0))
                .hover(|style| style.opacity(0.9))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    h_scroll_top.update(cx, |this, cx| {
                        if let Some(pane) = this.state.find_pane_mut(&pane_id_scroll_top) {
                            pane.active_tab_mut()
                                .list_scroll
                                .0
                                .borrow()
                                .base_handle
                                .set_offset(point(px(0.0), px(0.0)));
                        }
                        cx.notify();
                    });
                })
                .child(
                    svg()
                        .path("icons/arrow-up.svg")
                        .w(rems(1.0))
                        .h(rems(1.0))
                        .text_color(theme.text),
                ),
        );
    }
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
                            .tooltip(Tooltip::text(summary.clone()))
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

fn render_grid_file_list(
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
    let row_count = rows.len();
    let empty_message = if !files.is_empty() {
        None
    } else if !tab.files.is_empty() {
        Some(cx.i18n().text(Message::EmptyFiltered).to_string())
    } else {
        match &tab.load_status {
            TabLoadStatus::Loading => Some(cx.i18n().text(Message::FolderLoading).to_string()),
            TabLoadStatus::Ready => Some(cx.i18n().text(if tab.quick_search.is_some() { Message::QuickSearchEmpty } else { Message::FolderEmpty }).to_string()),
            TabLoadStatus::Error(message) => Some(cx.i18n().text(*message).to_string()),
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
                        let items = super::with_grid_sort(
                            super::pane_context_menu_items(cx.i18n(), h.clone(), pid.clone(), this),
                            this,
                            h.clone(),
                            &pid,
                            cx.i18n(),
                        );
                        this.context_menus.clear();
                        this.context_menus.push(ContextMenuInstance {
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
                let show_grid_thumbnails = show_grid_thumbnails;
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
                                    let file_name_tt = name.clone();
                                    let path_tt = path.clone();
                                    let ext_tt = file_type_label(file);
                                    let size_tt = (file.size, is_dir, file.is_calculated);
                                    let date_tt = file.modified;
                                    let mut card = div()
                                        .id(SharedString::from(format!("file-card-{path}"))).when(calculating_sizes && is_dir && !file.is_calculated, |d| d.child(size_loading(&path, theme.accent))).relative().when(histogram_total > 0, |d| d.child(size_histogram(file.size, histogram_total, theme.accent)))
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
                                        .when(show_tooltips && file.original_path.is_some(), |d| d.tooltip(Tooltip::for_trash(file.clone(), ext_tt.clone(), show_media_previews)))
                                                .when(show_tooltips && network_entry(file), |d| d.tooltip(Tooltip::for_network(file.clone())))
                                        .when(show_tooltips && search_results, |d| d.tooltip(Tooltip::for_search_result(file.clone(), ext_tt.clone(), show_media_previews)))
                                                .when(show_tooltips && file.original_path.is_none() && !search_results && !network_entry(file) && show_media_previews, |d| {
                                            d.tooltip(Tooltip::for_file_full(
                                                file_name_tt.clone(),
                                                path_tt.clone(),
                                                ext_tt.clone(),
                                                size_tt,
                                                date_tt,
                                            ))
                                        })
                                        .when(show_tooltips && file.original_path.is_none() && !search_results && !network_entry(file) && !show_media_previews, |d| {
                                            d.tooltip(Tooltip::for_file_no_preview(
                                                file_name_tt,
                                                ext_tt,
                                                size_tt,
                                                date_tt,
                                            ))
                                        })
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
                                                    if e.click_count == 2 {
                                                        this.open_file_item(&pid, path, is_dir, cx);
                                                    }
                                                });
                                            }
                                        })
                                        .on_mouse_down(MouseButton::Middle, {
                                            let h = h.clone();
                                            let pid = pid.clone();
                                            let path = path.clone();
                                            move |_, _, cx| {
                                                cx.stop_propagation();
                                                if is_dir || oxyde_lib::utils::archive::is_navigable_archive(std::path::Path::new(&path)) || oxyde_lib::utils::disk_image::needs_mount_for_browsing(std::path::Path::new(&path)) {
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
                                                            super::paths_match(&item.path, &path)
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
                                                        position: pos,
                                                        content: ContextMenuContent::Items(
                                                            super::with_grid_sort(super::oxyde_context_menu_items(
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
                                                move |_, cursor_offset, _, cx| {
                                                    let app = h.clone();
                                                    let paths = paths.clone();
                                                    cx.new(|cx| {
                                                        FileDragPreview::new(
                                                            paths,
                                                            cursor_offset,
                                                            app,
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
                                                        .child(if show_grid_thumbnails
                                                            && !is_dir
                                                            && icon == crate::ui::components::icons::IconType::FileImage
                                                        {
                                                            render_grid_thumbnail(path.clone(), icon, theme, use_system_icons).into_any_element()
                                                        } else {
                                                            render_file_icon(path.clone(), icon, rems(4.0), if is_dir { theme.accent } else { theme.text_muted }, use_system_icons, true)
                                                        }),
                                                )
                                                .when(has_subdirectories, |row| {
                                                    row.child(
                                                        div()
                                                            .absolute()
                                                            .right_0()
                                                            .top_0()
                                                            .h_full()
                                                            .flex()
                                                            .items_center()
                                                            .child(svg().path("icons/chevron-right.svg").size_3().text_color(theme.text_muted.opacity(0.75))),
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
                                                        div()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .child(if file.is_dir {
                                                                name.clone()
                                                            } else {
                                                                grid_display_name(&name)
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
                                    if is_dir || oxyde_lib::utils::archive::is_writable_archive(std::path::Path::new(&path)) {
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
                                                        if this.drag_copy_mode {
                                                            this.execute_file_copy(
                                                                paths, target, cx,
                                                            )
                                                        } else {
                                                            this.execute_file_move(
                                                                paths, target, cx,
                                                            )
                                                        }
                                                    });
                                                }
                                            });
                                    }
                                    card.when(show_checkboxes, |d| d.child(
                                        div().absolute().top(rems(0.375)).left(rems(0.375))
                                            .when(!is_selected, |d| d.opacity(0.0).group_hover("file-grid-card", |s| s.opacity(1.0)))
                                            .child(selection_checkbox(is_selected, path.clone(), pid.clone(), h.clone(), theme))
                                    ))
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
        let list = if tab.group_by_date {
            let scroll = tab.list_scroll.0.borrow().base_handle.clone();
            let viewport = view.pane_sizes.get(&pane_id).map(|size| size.height).unwrap_or(view.list_height).max(logical(200.0));
            let actual_viewport = scroll.bounds().size.height.as_f32();
            let max_scroll = (crate::ui::date_groups::content_height(&rows, true) - if actual_viewport > 0.0 { actual_viewport } else { viewport }).max(0.0);
            let offset = (-scroll.offset().y.as_f32()).clamp(0.0, max_scroll);
            if scroll.offset().y.as_f32() != -offset {
                scroll.set_offset(point(px(0.0), px(-offset)));
            }
            let mut top = 0.0;
            let mut start = 0;
            while start < rows.len() && top + crate::ui::date_groups::row_height(&rows[start], true) < offset {
                top += crate::ui::date_groups::row_height(&rows[start], true);
                start += 1;
            }
            let mut end = start;
            let mut bottom = top;
            while end < rows.len() && bottom < offset + viewport + crate::ui::style::grid_card_row_height() {
                bottom += crate::ui::date_groups::row_height(&rows[end], true);
                end += 1;
            }
            let scroll_handle = handle.clone();
            div().id(SharedString::from(format!("file-grid-grouped-{pane_id}")))
                .h_full().flex().flex_col().overflow_y_scroll().track_scroll(&scroll)
                .on_scroll_wheel(move |_, _, cx| scroll_handle.update(cx, |_, cx| cx.notify()))
                .child(div().h(px(top)).flex_none())
                .children(render_rows(start..end, cx))
                .child(div().h(px((crate::ui::date_groups::content_height(&rows, true) - bottom).max(0.0))).flex_none())
                .into_any_element()
        } else {
            uniform_list(SharedString::from(format!("file-grid-{pane_id}")), row_count, move |range, _, cx| render_rows(range, cx))
                .h_full().track_scroll(&tab.list_scroll).into_any_element()
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
        let content_height = crate::ui::date_groups::content_height(&rows, true);
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
            .overflow_y_scroll()
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
                            position,
                            content: ContextMenuContent::Items(
                                super::with_grid_sort(super::pane_context_menu_items(cx.i18n(), h.clone(), pid.clone(), this), this, h.clone(), &pid, cx.i18n()),
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
        .min_w_0()
        .flex()
        .flex_col()
        .relative()
        .pb(px(status_bar_height()))
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
                            .tooltip(Tooltip::text(summary.clone()))
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

fn selection_checkbox(
    selected: bool,
    path: String,
    pane_id: String,
    handle: Entity<OxydeApp>,
    theme: OxydeTheme,
) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("selection-checkbox-{path}")))
        .size(rems(1.5))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            handle.update(cx, |view, cx| {
                view.state.active_pane_id = pane_id.clone();
                view.focus_handle.focus(window, cx);
                view.toggle_selection(&pane_id, path.clone(), true, false, cx);
            });
        })
        .child(
            div()
                .size(rems(1.0))
                .rounded(rems(0.25))
                .border(rems(0.0625))
                .border_color(if selected { theme.accent } else { theme.border })
                .bg(if selected { theme.accent } else { theme.surface })
                .flex()
                .items_center()
                .justify_center()
                .when(selected, |d| d.child(svg().path("icons/check.svg").size(rems(0.75)).text_color(theme.text_on_accent))),
        )
}

fn render_grid_thumbnail(
    path: String,
    fallback_icon: crate::ui::components::icons::IconType,
    theme: OxydeTheme,
    system: bool,
) -> impl IntoElement {
    let fallback_path = path.clone();
    let source: SharedString = path.clone().into();
    img(move |window: &mut Window, cx: &mut App| {
        window.use_asset::<GridThumbnailAsset>(&source, cx)
    })
    .id(SharedString::from(format!("grid-thumbnail-{path}")))
    .size(rems(5.0))
    .object_fit(ObjectFit::Contain)
    .with_fallback(move || {
        render_file_icon(fallback_path.clone(), fallback_icon, rems(4.0), theme.text_muted, system, true)
    })
}

fn view_mode_button(
    pane_id: &str,
    mode: ViewMode,
    active_mode: ViewMode,
    icon: &'static str,
    tooltip: Message,
    handle: Entity<OxydeApp>,
    theme: OxydeTheme,
    cx: &mut Context<OxydeApp>,
) -> impl IntoElement {
    let selected = mode == active_mode;
    let pane_id = pane_id.to_string();
    div()
        .id(SharedString::from(format!(
            "view-mode-{}-{}",
            pane_id,
            if mode == ViewMode::Grid {
                "grid"
            } else {
                "details"
            }
        )))
        .w(rems(1.5))
        .h(rems(1.5))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .bg(if selected {
            theme.selected
        } else {
            theme.header
        })
        .hover(|d| d.bg(theme.control_hover))
        .tooltip(Tooltip::text(cx.i18n().text(tooltip)))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            handle.update(cx, |this, cx| {
                if let Some(pane) = this.state.find_pane_mut(&pane_id) {
                    pane.active_tab_mut().view_mode = mode;
                }
                this.save_workspace(cx);
                cx.notify();
            });
        })
        .child(svg().path(icon).size_4().text_color(if selected {
            theme.accent
        } else {
            theme.text_muted
        }))
}

fn render_headers(
    view: &OxydeApp,
    pane_id: String,
    handle: Entity<OxydeApp>,
    name_w: f32,
    location_w: f32,
    type_w: f32,
    size_w: f32,
    date_w: f32,
    deleted_w: f32,
    theme: &OxydeTheme,
    i18n: &crate::ui::i18n::I18n,
) -> impl IntoElement {
    let pane = view.state.layout.find_pane(&pane_id).expect("pane exists");
    let tab = pane.active_tab();
    let current_col = tab.sort_column;
    let network_panel = tab.path == "__network_vincinity__";
    let current_dir = tab.sort_direction;
    if network_panel {
        return div().flex().items_center().h(crate::ui::style::COLUMN_HEADER_HEIGHT).border_b(rems(0.0625)).border_color(theme.border).text_color(theme.text_muted).text_size(rems(0.75)).font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT).flex_none()
            .child(header_cell(i18n.text(Message::ColumnName).as_ref(), name_w, current_col == SortColumn::Name, current_dir, false, theme, handle.clone(), format!("network-name-{}", pane_id), {
                let h = handle.clone(); let pid = pane_id.clone(); move |_, _, cx| { h.update(cx, |this, cx| this.toggle_sort(&pid, SortColumn::Name, cx)); }
            }))
            .children([(Message::NetworkCategory, type_w), (Message::NetworkWorkgroup, size_w), (Message::NetworkLocation, date_w)].into_iter().map(|(message, width)| div().w(px(width)).flex_none().h_full().flex().items_center().px(rems(0.5)).border_l(rems(0.0625)).border_color(theme.border).text_size(rems(0.75)).child(i18n.text(message))));
    }
    // Check filters
    let name_filtered = !tab.filter_name.is_empty();

    let type_filtered = if tab.filter_types.is_empty() {
        false
    } else {
        let mut all_types = std::collections::HashSet::new();
        for f in &tab.files {
            all_types.insert(file_type_label(f));
        }
        !all_types.iter().all(|t| tab.filter_types.contains(t))
    };

    let size_filtered = if tab.filter_sizes.is_empty() {
        false
    } else {
        use crate::ui::workspace::SizeCategory;
        let categories = [
            SizeCategory::Minuscule,
            SizeCategory::Petit,
            SizeCategory::Moyen,
            SizeCategory::Grand,
            SizeCategory::Huge,
        ];
        let mut present = std::collections::HashSet::new();
        for f in &tab.files {
            for cat in &categories {
                if cat.matches(f.size) {
                    present.insert(*cat);
                    break;
                }
            }
        }
        !present.iter().all(|c| tab.filter_sizes.contains(c))
    };

    let date_filtered = if tab.filter_dates.is_empty() {
        false
    } else {
        let counts = crate::ui::date_groups::counts_by_date(&tab.files, chrono::Local::now().date_naive());
        !counts.keys().all(|group| tab.filter_dates.contains(group))
    };

    div()
        .flex()
        .items_center()
        .h(crate::ui::style::COLUMN_HEADER_HEIGHT)
        .border_b(rems(0.0625)).border_color(theme.border).text_color(theme.text_muted)
        .text_size(rems(0.75)).font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
        .flex_none()
        .child(
            header_cell(
                i18n.text(Message::ColumnName).as_ref(),
                name_w,
                current_col == SortColumn::Name,
                current_dir,
                name_filtered,
                theme,
                handle.clone(),
                format!("filter-{}-name", pane_id),
                {
                    let h = handle.clone();
                    let pid = pane_id.clone();
                    move |_, _, cx| {
                        h.update(cx, |this, cx| this.toggle_sort(&pid, SortColumn::Name, cx));
                    }
                },
            )
            .on_mouse_down(MouseButton::Right, {
                let h = handle.clone();
                let pid = pane_id.clone();
                let anchor_key = format!("filter-{}-name", pane_id);
                move |e, window, cx| {
                    let fallback = e.position;
                    h.update(cx, |this, cx| {
                        let anchor = this
                            .context_anchor_points
                            .get(&anchor_key)
                            .copied()
                            .unwrap_or(fallback);
                        this.open_name_filter(&pid, anchor, window, cx);
                    });
                }
            }),
        )
        .when(tab.quick_search.is_some() || tab.path == "trash://", |headers| headers.child(
            header_cell(i18n.text(if tab.path == "trash://" { Message::TrashOriginalLocation } else { Message::ColumnLocation }).as_ref(), location_w, current_col == if tab.path == "trash://" { SortColumn::OriginalLocation } else { SortColumn::Location }, current_dir, !tab.filter_original_location.is_empty(), theme, handle.clone(), format!("filter-{pane_id}-location"), {
                let handle = handle.clone(); let pane_id = pane_id.clone();
                move |_, _, cx| { handle.update(cx, |this, cx| this.toggle_sort(&pane_id, if this.state.find_pane(&pane_id).is_some_and(|pane| pane.active_tab().path == "trash://") { SortColumn::OriginalLocation } else { SortColumn::Location }, cx)); }
            }).on_mouse_down(MouseButton::Right, {
                let handle = handle.clone(); let pane_id = pane_id.clone(); let trash = tab.path == "trash://";
                move |event, window, cx| {
                    if trash { handle.update(cx, |this, cx| {
                        let anchor = this.context_anchor_points.get(&format!("filter-{pane_id}-location")).copied().unwrap_or(event.position);
                        this.open_original_location_filter(&pane_id, anchor, window, cx);
                    }); }
                }
            })
        ))
        .child(
            header_cell(
                i18n.text(Message::ColumnType).as_ref(),
                type_w,
                current_col == SortColumn::Type,
                current_dir,
                type_filtered,
                theme,
                handle.clone(),
                format!("filter-{}-type", pane_id),
                {
                    let h = handle.clone();
                    let pid = pane_id.clone();
                    move |_, _, cx| {
                        h.update(cx, |this, cx| this.toggle_sort(&pid, SortColumn::Type, cx));
                    }
                },
            )
            .on_mouse_down(MouseButton::Right, {
                let h = handle.clone();
                let pid = pane_id.clone();
                let anchor_key = format!("filter-{}-type", pane_id);
                move |e, _, cx| {
                    let fallback = e.position;
                    h.update(cx, |this, cx| {
                        let anchor = this
                            .context_anchor_points
                            .get(&anchor_key)
                            .copied()
                            .unwrap_or(fallback);
                        this.open_type_filter_menu(&pid, anchor, cx);
                    });
                }
            }),
        )
        .when(!network_panel, |row| row.child(
            header_cell(
                i18n.text(Message::ColumnSize).as_ref(),
                size_w,
                current_col == SortColumn::Size,
                current_dir,
                size_filtered,
                theme,
                handle.clone(),
                format!("filter-{}-size", pane_id),
                {
                    let h = handle.clone();
                    let pid = pane_id.clone();
                    move |_, _, cx| {
                        h.update(cx, |this, cx| this.toggle_sort(&pid, SortColumn::Size, cx));
                    }
                },
            )
            .on_mouse_down(MouseButton::Right, {
                let h = handle.clone();
                let pid = pane_id.clone();
                let anchor_key = format!("filter-{}-size", pane_id);
                move |e, _, cx| {
                    let fallback = e.position;
                    h.update(cx, |this, cx| {
                        let anchor = this
                            .context_anchor_points
                            .get(&anchor_key)
                            .copied()
                            .unwrap_or(fallback);
                        this.open_size_filter_menu(&pid, anchor, cx);
                    });
                }
            }),
        ))
        .when(!network_panel, |row| row.child(
            header_cell(
                i18n.text(Message::ColumnModified).as_ref(),
                date_w,
                current_col == SortColumn::Modified,
                current_dir,
                date_filtered,
                theme,
                handle.clone(),
                format!("filter-{}-date", pane_id),
                {
                    let h = handle.clone();
                    let pid = pane_id.clone();
                    move |_, _, cx| {
                        h.update(cx, |this, cx| {
                            this.toggle_sort(&pid, SortColumn::Modified, cx)
                        });
                    }
                },
            )
            .on_mouse_down(MouseButton::Right, {
                let h = handle.clone();
                let pid = pane_id.clone();
                let anchor_key = format!("filter-{}-date", pane_id);
                move |e, _, cx| {
                    let fallback = e.position;
                    h.update(cx, |this, cx| {
                        let anchor = this
                            .context_anchor_points
                            .get(&anchor_key)
                            .copied()
                            .unwrap_or(fallback);
                        this.open_date_filter_menu(&pid, anchor, cx);
                    });
                }
            }),
        ))
        .when(deleted_w > 0.0, |row| row.child(header_cell(
            i18n.text(Message::TrashDeletedAt).as_ref(), deleted_w, current_col == SortColumn::DeletedAt,
            current_dir, false, theme, handle.clone(), format!("trash-deleted-{pane_id}"), {
                let handle = handle.clone(); let pane_id = pane_id.clone();
                move |_, _, cx| { handle.update(cx, |this, cx| this.toggle_sort(&pane_id, SortColumn::DeletedAt, cx)); }
            }
        )))
}

fn header_cell(
    label: &str,
    width: f32,
    is_active: bool,
    direction: SortDirection,
    is_filtered: bool,
    theme: &OxydeTheme,
    handle: Entity<OxydeApp>,
    anchor_key: String,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let anchor_handle = handle.clone();
    div()
        .id(SharedString::from(anchor_key.clone()))
        .relative()
        .w(px(width))
        .flex_none()
        .px(rems(0.5))
        .h_full()
        .flex()
        .items_center()
        .child(
            div()
                .flex_1()
                .overflow_hidden()
                .text_ellipsis()
                .child(label.to_string()),
        )
        .child(div().ml_1().when(is_filtered, |s| {
            s.child(
                svg()
                    .path("icons/filter.svg")
                    .size_3()
                    .text_color(theme.accent),
            )
        }))
        .child(
            div()
                .w(rems(1.0))
                .flex()
                .justify_center()
                .when(is_active, |s| {
                    s.child(
                        svg()
                            .path(if direction == SortDirection::Ascending {
                                "icons/arrow-up.svg"
                            } else {
                                "icons/arrow-down.svg"
                            })
                            .size_3()
                            .text_color(theme.accent),
                    )
                }),
        )
        .child(
            canvas(
                |bounds, _, _| bounds,
                move |_, bounds, _, cx| {
                    // The dropdown has a small internal left inset; shift the filter
                    // menu back slightly so its visible edge aligns with the header.
                    let anchor = point(bounds.left() - rem_pixels(rems(0.25)), bounds.bottom());
                    anchor_handle.update(cx, |this, _| {
                        this.context_anchor_points
                            .insert(anchor_key.clone(), anchor);
                    });
                },
            )
            .absolute()
            .size_full(),
        )
        .hover(|s| s.bg(theme.hover))
        .on_mouse_down(MouseButton::Left, on_click)
}

pub fn get_type_filter_items(
    i18n: &crate::ui::i18n::I18n,
    h: Entity<OxydeApp>,
    pid: String,
    tab_files: Vec<FileEntry>,
    current_filters: std::collections::HashSet<String>,
    pos: Point<Pixels>,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::{DropdownItem, IconName};

    let mut exts = std::collections::HashMap::<String, u64>::new();
    for f in &tab_files {
        *exts.entry(file_type_label(f)).or_default() += 1;
    }
    let mut sorted_exts: Vec<String> = exts.keys().cloned().collect();
    sorted_exts.sort();

    let mut items = Vec::new();

    for ext in sorted_exts {
        let h_ext = h.clone();
        let pid_ext = pid.clone();
        let e_cap = ext.clone();
        let is_sel = current_filters.contains(&e_cap);

        items.push(DropdownItem {
            label: match ext.as_str() {
                "Dossier" => i18n.text(Message::FileTypeFolder).to_string(),
                "Fichier" => i18n.text(Message::FileTypeFile).to_string(),
                "Symlink" => i18n.text(Message::FileTypeSymlink).to_string(),
                "Junction" => i18n.text(Message::FileTypeJunction).to_string(),
                _ => ext.clone(),
            },
            icon: Some(if is_sel {
                IconName::Check
            } else {
                IconName::Square
            }),
            badge: Some(crate::ui::format::format_number(exts[&ext], i18n.language())),
            stay_open: true,
            has_submenu: false,
            is_dimmed: false,
            on_click: std::rc::Rc::new(move |_, app| {
                let pid_cap = pid_ext.clone();
                let e_inner = e_cap.clone();
                h_ext.update(app, |this, cx| {
                    this.toggle_filter_type(&pid_cap, e_inner, cx);
                    this.open_type_filter_menu(&pid_cap, pos, cx);
                });
            }),
            on_hover: None,
        });
    }

    items
}

pub fn get_date_filter_items(
    i18n: &crate::ui::i18n::I18n,
    handle: Entity<OxydeApp>,
    pane_id: String,
    files: Vec<oxyde_lib::models::FileEntry>,
    current_filters: std::collections::HashSet<crate::ui::date_groups::DateGroup>,
    pos: Point<Pixels>,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::{DropdownItem, IconName};
    use crate::ui::date_groups::DateGroup;
    use std::rc::Rc;

    let categories = DateGroup::ALL;
    let present_categories = crate::ui::date_groups::counts_by_date(
        &files,
        chrono::Local::now().date_naive(),
    );

    let mut items = Vec::new();

    for cat in categories {
        if !present_categories.contains_key(&cat) {
            continue;
        }

        let h = handle.clone();
        let pid = pane_id.clone();
        let active = current_filters.contains(&cat);
        let c_cat = cat;

        items.push(DropdownItem {
            label: i18n.text(cat.message()).to_string(),
            icon: Some(if active {
                IconName::Check
            } else {
                IconName::Square
            }),
            badge: Some(crate::ui::format::format_number(present_categories[&cat], i18n.language())),
            stay_open: true,
            has_submenu: false,
            is_dimmed: false,
            on_click: Rc::new(move |_, cx| {
                let pid_inner = pid.clone();
                h.update(cx, |this, cx| {
                    this.update_filter_date(&pid_inner, c_cat, !active, cx);
                    this.open_date_filter_menu(&pid_inner, pos, cx);
                });
            }),
            on_hover: None,
        });
    }

    items
}

pub fn get_size_filter_items(
    i18n: &crate::ui::i18n::I18n,
    handle: Entity<OxydeApp>,
    pane_id: String,
    files: Vec<oxyde_lib::models::FileEntry>,
    current_filters: std::collections::HashSet<crate::ui::workspace::SizeCategory>,
    pos: Point<Pixels>,
) -> Vec<crate::ui::components::dropdown::DropdownItem> {
    use crate::ui::components::dropdown::{DropdownItem, IconName};
    use crate::ui::workspace::SizeCategory;
    use std::rc::Rc;

    let categories = [
        SizeCategory::Minuscule,
        SizeCategory::Petit,
        SizeCategory::Moyen,
        SizeCategory::Grand,
        SizeCategory::Huge,
    ];

    let mut present_categories = std::collections::HashMap::<_, u64>::new();
    for f in &files {
        for cat in &categories {
            if cat.matches(f.size) {
                *present_categories.entry(*cat).or_default() += 1;
                break;
            }
        }
    }

    let mut items = Vec::new();

    for cat in categories {
        if !present_categories.contains_key(&cat) {
            continue;
        }

        let h = handle.clone();
        let pid = pane_id.clone();
        let active = current_filters.contains(&cat);
        let c_cat = cat;

        items.push(DropdownItem {
            label: i18n.text(cat.message()).to_string(),
            icon: Some(if active {
                IconName::Check
            } else {
                IconName::Square
            }),
            badge: Some(crate::ui::format::format_number(present_categories[&cat], i18n.language())),
            stay_open: true,
            has_submenu: false,
            is_dimmed: false,
            on_click: Rc::new(move |_, cx| {
                let pid_inner = pid.clone();
                h.update(cx, |this, cx| {
                    this.update_filter_size(&pid_inner, c_cat, !active, cx);
                    this.open_size_filter_menu(&pid_inner, pos, cx);
                });
            }),
            on_hover: None,
        });
    }

    items
}

fn size_histogram(size: u64, total: u64, color: Hsla) -> Div {
    div().absolute().left_0().top_0().bottom_0().w(relative((size as f64 / total as f64).clamp(0.0, 1.0) as f32)).bg(color.opacity(0.12))
}

fn size_loading(path: &str, color: Hsla) -> AnyElement {
    div().flex().items_center().justify_end().gap(rems(0.1875))
        .h(rems(1.0))
        .children((0..3).map(|index| {
            div().size(rems(0.1875)).rounded_full().bg(color)
                .with_animation(
                    SharedString::from(format!("size-loading-{path}-{index}")),
                    Animation::new(std::time::Duration::from_millis(1000)).repeat(),
                    move |dot, delta| {
                        let wave = (1.0 + ((delta - index as f32 / 3.0) * std::f32::consts::TAU).cos()) * 0.5;
                        dot.opacity(0.25 + wave * 0.75)
                    },
                )
        })).into_any_element()
}
