use crate::ui::app::OxydeApp;
use crate::ui::i18n::Message;
use crate::ui::theme::OxydeTheme;
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::workspace::{FilterColumn, SortColumn, SortDirection};
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
    permissions_w: f32,
    date_w: f32,
    deleted_w: f32,
    theme: &OxydeTheme,
    i18n: &crate::ui::i18n::I18n,
    date_format: crate::ui::format::DateFormat,
) -> impl IntoElement {
    let pane = view.state.layout.find_pane(&pane_id).expect("pane exists");
    let tab = pane.active_tab();
    let current_col = tab.sort_column;
    let network_panel = tab.path == "__network_vincinity__";
    let current_dir = tab.sort_direction;
    if network_panel {
        return div().flex().items_center().h(crate::ui::style::COLUMN_HEADER_HEIGHT).text_color(theme.text_muted).text_size(rems(0.75)).font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT).flex_none()
            .child(header_cell(i18n.text(Message::ColumnName).as_ref(), name_w, current_col == SortColumn::Name, current_dir, None, theme, handle.clone(), format!("network-name-{}", pane_id), {
                let h = handle.clone(); let pid = pane_id.clone(); move |_, _, cx| { h.update(cx, |this, cx| this.toggle_sort(&pid, SortColumn::Name, cx)); }
            }))
            // Workgroup and network location are Windows Shell columns GVfs never fills.
            .children(network_columns(type_w, size_w, date_w).into_iter().map(|(message, width)| div().w(px(width)).flex_none().h_full().flex().items_center().px(rems(0.5)).border_l(rems(0.0625)).border_color(theme.border).text_size(rems(0.75)).child(i18n.text(message))));
    }
    let flags = tab.column_filter_flags();
    let trash = tab.path == "trash://";
    // The funnel of a filterable column, lit while its filter hides entries. A right click
    // on the header opens the filter; the tooltip says so, under what the filter keeps.
    let hint = i18n.text(Message::FilterRightClick).to_string();
    let filter = |column: FilterColumn, active: bool| Some(HeaderFilter {
        pane_id: pane_id.clone(),
        column,
        active,
        tooltip: match if active { super::filter_summary(tab, column, i18n, date_format) } else { None } {
            Some(summary) => format!("{summary}
{hint}"),
            None => hint.clone(),
        },
    });
    let sort = |column: SortColumn| {
        let handle = handle.clone();
        let pane_id = pane_id.clone();
        move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| handle.update(cx, |this, cx| this.toggle_sort(&pane_id, column, cx))
    };
    let location_sort = if trash { SortColumn::OriginalLocation } else { SortColumn::Location };

    div()
        .flex()
        .items_center()
        .h(crate::ui::style::COLUMN_HEADER_HEIGHT)
        .text_color(theme.text_muted)
        .text_size(rems(0.75)).font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
        .flex_none()
        .child(header_cell(i18n.text(Message::ColumnName).as_ref(), name_w, current_col == SortColumn::Name, current_dir,
            filter(FilterColumn::Name, flags.name), theme, handle.clone(), format!("filter-{pane_id}-name"), sort(SortColumn::Name)))
        .when(tab.quick_search.is_some() || trash, |headers| headers.child(header_cell(
            i18n.text(if trash { Message::TrashOriginalLocation } else { Message::ColumnLocation }).as_ref(), location_w,
            current_col == location_sort, current_dir,
            filter(FilterColumn::Location, flags.location),
            theme, handle.clone(), format!("filter-{pane_id}-location"), sort(location_sort),
        )))
        .child(header_cell(i18n.text(Message::ColumnType).as_ref(), type_w, current_col == SortColumn::Type, current_dir,
            filter(FilterColumn::Type, flags.types), theme, handle.clone(), format!("filter-{pane_id}-type"), sort(SortColumn::Type)))
        .when(!network_panel, |row| row.child(header_cell(i18n.text(Message::ColumnSize).as_ref(), size_w, current_col == SortColumn::Size, current_dir,
            filter(FilterColumn::Size, flags.sizes), theme, handle.clone(), format!("filter-{pane_id}-size"), sort(SortColumn::Size))))
        .when(!network_panel && permissions_w > 0.0, |row| row.child(header_cell(i18n.text(Message::ColumnPermissions).as_ref(), permissions_w,
            current_col == SortColumn::Permissions, current_dir, filter(FilterColumn::Permissions, flags.permissions), theme, handle.clone(),
            format!("filter-{pane_id}-permissions"), sort(SortColumn::Permissions))))
        .when(!network_panel, |row| row.child(header_cell(i18n.text(Message::ColumnModified).as_ref(), date_w, current_col == SortColumn::Modified, current_dir,
            filter(FilterColumn::Date, flags.dates), theme, handle.clone(), format!("filter-{pane_id}-date"), sort(SortColumn::Modified))))
        .when(deleted_w > 0.0, |row| row.child(header_cell(
            i18n.text(Message::TrashDeletedAt).as_ref(), deleted_w, current_col == SortColumn::DeletedAt,
            current_dir, filter(FilterColumn::Deleted, flags.deleted), theme, handle.clone(), format!("trash-deleted-{pane_id}"), sort(SortColumn::DeletedAt),
        )))
}

/// The filter a header cell opens.
struct HeaderFilter {
    pane_id: String,
    column: FilterColumn,
    /// The filter hides entries: the funnel stays lit.
    active: bool,
    tooltip: String,
}

/// Opens the filter panel under its header cell, or closes it when it is already open there.
fn open_filter(handle: &Entity<OxydeApp>, pane_id: &str, column: FilterColumn, anchor_key: &str, fallback: Point<Pixels>, window: &mut Window, cx: &mut App) {
    handle.update(cx, |this, cx| {
        if this.close_menu_owned_by(anchor_key, cx) { return; }
        let anchor = this.context_anchor_points.get(anchor_key).copied().unwrap_or(fallback);
        this.open_column_filter(pane_id, column, anchor, window, cx);
        this.mark_menu_owner(anchor_key.to_string());
    });
}

#[allow(clippy::too_many_arguments)]
fn header_cell(
    label: &str,
    width: f32,
    is_active: bool,
    direction: SortDirection,
    filter: Option<HeaderFilter>,
    theme: &OxydeTheme,
    handle: Entity<OxydeApp>,
    anchor_key: String,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let anchor_handle = handle.clone();
    let theme = *theme;
    let funnel = filter.as_ref().map(|filter| {
        div()
            .id(SharedString::from(format!("{anchor_key}-funnel")))
            .flex_none()
            .w(rems(1.0))
            .h(rems(1.25))
            .flex()
            .items_center()
            .justify_center()
            .rounded(rems(0.25))
            // At rest only a set filter shows its funnel; hovering the header shows the others.
            .when(!filter.active, |funnel| funnel.opacity(0.0).group_hover("column-header", |style| style.opacity(1.0)))
            .child(svg().path("icons/funnel.svg").size_3().text_color(if filter.active { theme.accent } else { theme.text_muted }))
            .fixed_tooltip(Tooltip::text(filter.tooltip.clone()))
    });
    let context = filter.map(|filter| (handle.clone(), filter.pane_id, filter.column, anchor_key.clone()));
    div()
        .id(SharedString::from(anchor_key.clone()))
        .group("column-header")
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
        .children(funnel)
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
        .hover(move |s| s.bg(theme.hover))
        .on_mouse_down(MouseButton::Left, on_click)
        // Only a right click opens the filter: the funnel shows it, a left click sorts.
        .when_some(context, |cell, (handle, pane_id, column, key)| cell.on_mouse_down(MouseButton::Right, move |event, window, cx| {
            open_filter(&handle, &pane_id, column, &key, event.position, window, cx);
        }))
}

/// Workgroup and network location are Windows Shell columns: GVfs never fills them, and
/// its services are told apart by protocol.
pub(super) fn network_columns<T>(category: T, workgroup: T, location: T) -> Vec<(Message, T)> {
    if cfg!(target_os = "windows") {
        vec![(Message::NetworkCategory, category), (Message::NetworkWorkgroup, workgroup), (Message::NetworkLocation, location)]
    } else {
        vec![(Message::ColumnType, category)]
    }
}
