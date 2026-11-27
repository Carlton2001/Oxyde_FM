use super::card_body::{self, CardBody};
use super::presentation::selection_checkbox;
use crate::ui::app::{ContextMenuContent, ContextMenuInstance, FileDragData, FileDragPreview, FileDropTargetExt, OxydeApp};
use crate::ui::components::{with_grid_sort, oxyde_context_menu_items};
use crate::ui::paths::paths_match;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::components::tooltip::hover::SharedTooltipExt;
use crate::ui::i18n::I18nContext;
use crate::ui::scale::{logical, rem_pixels};
use crate::ui::style::{CardLayout, CARD_BORDER, CARD_MARGIN};
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
    pub(super) paths: std::rc::Rc<Vec<String>>,
    pub(super) icon: crate::ui::components::icons::IconType,
    pub(super) editing_view: Option<Entity<crate::ui::components::input::TextInput>>,
    pub(super) has_rename_error: bool,
    pub(super) theme: OxydeTheme,
    pub(super) calculating_sizes: bool,
    pub(super) provisional_size: bool,
    pub(super) histogram_total: u64,
    pub(super) card_width: f32,
    pub(super) active: bool,
    pub(super) show_tooltips: bool,
    pub(super) show_media_previews: bool,
    pub(super) search_results: bool,
    /// The pane lists the trash: a drag from it moves items out instead of copying.
    pub(super) trash_panel: bool,
    pub(super) show_grid_thumbnails: bool,
    pub(super) use_system_icons: bool,
    pub(super) show_checkboxes: bool,
    pub(super) layout: CardLayout,
    /// The card is under the pointer or selected alone: a grid card shows its whole name.
    pub(super) expanded: bool,
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
        provisional_size,
        histogram_total,
        card_width,
        active,
        show_tooltips,
        show_media_previews,
        search_results,
        trash_panel,
        show_grid_thumbnails,
        use_system_icons,
        show_checkboxes,
        layout,
        expanded,
    } = params;
    let grid = matches!(layout, CardLayout::Grid(_));
    let pane_fill = if active { theme.active_background } else { theme.background };
    let border = if has_rename_error {
        theme.error
    } else if is_selected {
        theme.accent
    } else {
        theme.border.opacity(0.0)
    };
    let can_browse = is_dir || file.is_navigable_archive() || file.needs_mount_for_browsing();
    let can_drop = is_dir || file.is_writable_archive();
    let loading_size = calculating_sizes && is_dir && !file.is_calculated;
    let mut card = div()
        .id(SharedString::from(format!("file-card-{path}"))).relative().when(histogram_total > 0, |d| d.child(
            crate::ui::elements::rounded_fill::rounded_fill(
                (file.size as f64 / histogram_total as f64) as f32,
                rems(0.3125),
                theme.size_bar,
            ).absolute().left(rems(0.0625)).right(rems(0.0625)).top(rems(0.0625)).bottom(rems(0.0625))
        ))
        .w(px(card_width - logical(4.0)))
        .h(px(layout.row_height() - 2.0 * rem_pixels(CARD_MARGIN).as_f32()))
        .m(CARD_MARGIN)
        .rounded_md()
        .border(CARD_BORDER)
        .border_color(border)
        .bg(if is_selected { theme.selected } else { pane_fill })
        .group("file-grid-card")
        .relative().when(!is_selected, |d| d.hover(|d| d.bg(theme.hover)))
        .when(show_tooltips, |d| d.file_tooltip(h.clone(), pid.clone(), path.clone(), Tooltip::item_revision(&file, provisional_size), cx, Tooltip::for_item(file.clone(), search_results, show_media_previews, provisional_size)))
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
                        ),
                        scroll_handle: UniformListScrollHandle::new(
                        ),
                        width: px(0.0),
                        open_left: false,
                        hovered_item: None,
                        native_request_id: None, open_up: false,
                    });
                    this.resolve_cloud_availability_menu(&path, cx);
                    cx.notify();
                });
            }
        })
        .on_drag(
            FileDragData {
                paths: paths.clone(),
                source_pane_id: pid.clone(),
                from_trash: trash_panel,
            },
            {
                let h = h.clone();
                move |_, cursor_offset, window, cx| {
                    let app = h.clone();
                    let paths = paths.to_vec();
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
        .external_drag_payload(crate::ui::app::outgoing_drag())
        .when(grid, |card| card.child(hover_tracker(&h, &pid, &path)));
    let body = CardBody {
        file,
        path: path.clone(),
        name,
        is_dir,
        has_subdirectories,
        is_selected,
        h: h.clone(),
        pid: pid.clone(),
        icon,
        editing_view,
        theme,
        loading_size,
        provisional_size,
        show_grid_thumbnails,
        use_system_icons,
        show_checkboxes,
        expanded,
        border,
        // A hovered card's fill replaces its own; the overlay lays it over the pane's.
        fill: if is_selected { theme.selected } else if expanded { theme.hover } else { pane_fill },
        pane_fill,
    };
    card = if grid { card_body::grid(card, body, layout, cx) } else { card_body::tile(card, body, layout, cx) };
    if can_drop {
        card = card
            .external_file_drop(h.clone(), path.clone())
            .drag_over::<FileDragData>({
                let h = h.clone();
                let target = path.clone();
                move |style, data, _, cx| {
                    if h.update(cx, |this, cx| this.note_drag_target(&data.paths, &target, cx)) {
                        return style;
                    }
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
                        this.execute_file_drop(paths.to_vec(), target, cx);
                    });
                }
            });
    }
    card.when(grid && show_checkboxes, |d| d.child(
        div().absolute().top(card_body::GRID_CARD_CONTROL_INSET).left(card_body::GRID_CARD_CONTROL_INSET)
            .when(!is_selected, |d| d.opacity(0.0).group_hover("file-grid-card", |s| s.opacity(1.0)))
            .child(selection_checkbox(is_selected, path.clone(), pid.clone(), h.clone(), theme))
    ))
}

/// Notes the card under the pointer, whose whole name then shows. GPUI keeps one hover
/// listener per element and the card's own holds its tooltip.
fn hover_tracker(h: &Entity<OxydeApp>, pane_id: &str, path: &str) -> Stateful<Div> {
    let (h, pane_id, path) = (h.clone(), pane_id.to_string(), path.to_string());
    div()
        .id(SharedString::from(format!("file-card-hover-{path}")))
        .absolute()
        .inset_0()
        .on_hover(move |hovered, _, cx| {
            h.update(cx, |this, cx| this.set_hovered_card(&pane_id, &path, *hovered, cx));
        })
}
