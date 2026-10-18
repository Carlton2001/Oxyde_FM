use crate::ui::app::OxydeApp;
use crate::ui::i18n::Message;
use crate::ui::theme::OxydeTheme;
use crate::ui::workspace::{SortColumn, SortDirection};
use gpui::prelude::*;
use gpui::*;

pub(super) fn render_headers(
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

    let (type_filtered, size_filtered, date_filtered) = tab.metadata_filter_flags();

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
                        if this.close_menu_owned_by(&anchor_key, cx) { return; }
                        this.open_name_filter(&pid, anchor, window, cx);
                        this.mark_menu_owner(anchor_key.clone());
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
                        let anchor_key = format!("filter-{pane_id}-location");
                        if this.close_menu_owned_by(&anchor_key, cx) { return; }
                        let anchor = this.context_anchor_points.get(&anchor_key).copied().unwrap_or(event.position);
                        this.open_original_location_filter(&pane_id, anchor, window, cx);
                        this.mark_menu_owner(anchor_key);
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
                        if this.close_menu_owned_by(&anchor_key, cx) { return; }
                        this.open_type_filter_menu(&pid, anchor, cx);
                        this.mark_menu_owner(anchor_key.clone());
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
                        if this.close_menu_owned_by(&anchor_key, cx) { return; }
                        this.open_size_filter_menu(&pid, anchor, cx);
                        this.mark_menu_owner(anchor_key.clone());
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
                        if this.close_menu_owned_by(&anchor_key, cx) { return; }
                        this.open_date_filter_menu(&pid, anchor, cx);
                        this.mark_menu_owner(anchor_key.clone());
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
        .overflow_hidden()
        .px(rems(0.5))
        .h_full()
        .flex()
        .items_center()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
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
                .flex_none()
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
                    let anchor = point(bounds.left(), bounds.bottom());
                    anchor_handle.update(cx, |this, _| {
                        this.context_anchor_points
                            .insert(anchor_key.clone(), anchor);
                    });
                },
            )
            .absolute()
            .left_0()
            .top_0()
            .size_full(),
        )
        .hover(|s| s.bg(theme.hover))
        .on_mouse_down(MouseButton::Left, on_click)
}
