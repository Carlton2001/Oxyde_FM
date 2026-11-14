//! Sort by and Filter by in the grid view's context menus, which has no column headers.

use super::{context_menu_sections, dropdown};
use crate::ui::app::{GridMenuKind, OxydeApp};
use crate::ui::i18n::{I18n, Message};
use crate::ui::workspace::{FilterColumn, SortColumn, SortDirection, TabState, ViewMode};
use gpui::*;

/// Adds Sort by and Filter by to a context menu of the grid view, above the information section.
pub fn with_grid_sort(
    mut items: Vec<dropdown::DropdownItem>,
    app: &OxydeApp,
    handle: Entity<OxydeApp>,
    pane_id: &str,
    i18n: &I18n,
) -> Vec<dropdown::DropdownItem> {
    let Some(pane) = app.state.find_pane(pane_id) else { return items };
    let tab = pane.active_tab();
    if tab.view_mode != ViewMode::Grid { return items; }
    let mut entries = vec![submenu(Message::SortBy, dropdown::IconName::ListOrdered, GridMenuKind::Sort, handle.clone(), pane_id, i18n)];
    // The network neighbourhood filters nothing, as its headers show.
    if tab.path != "__network_vincinity__" {
        entries.push(submenu(Message::FilterBy, dropdown::IconName::Funnel, GridMenuKind::Filters, handle, pane_id, i18n));
    }
    let index = items.iter().position(|item| [Message::Properties, Message::MoreOptions].into_iter()
        .any(|message| item.label == i18n.text(message).as_ref())).unwrap_or(items.len());
    items.splice(index..index, entries);
    context_menu_sections(items, i18n)
}

/// A root menu entry opening `kind` beside it.
fn submenu(label: Message, icon: dropdown::IconName, kind: GridMenuKind, handle: Entity<OxydeApp>, pane_id: &str, i18n: &I18n) -> dropdown::DropdownItem {
    let pane_id = pane_id.to_string();
    dropdown::DropdownItem::new(i18n.text(label).to_string(), |_, _| {})
        .icon(icon)
        .stay_open(true)
        .has_submenu(true)
        .on_hover(move |position, open_left, cx| {
            handle.update(cx, |this, cx| this.open_grid_menu(kind, &pane_id, position, open_left, cx));
        })
}

/// The rows of Sort by or Filter by for the active tab of `pane_id`, as it stands now.
pub(crate) fn grid_menu_items(kind: GridMenuKind, app: &OxydeApp, handle: Entity<OxydeApp>, pane_id: &str, i18n: &I18n) -> Vec<dropdown::DropdownItem> {
    let Some(tab) = app.state.find_pane(pane_id).map(|pane| pane.active_tab()) else { return Vec::new() };
    match kind {
        GridMenuKind::Sort => sort_items(app, tab, handle, pane_id, i18n),
        GridMenuKind::Filters => filter_items(app, tab, handle, pane_id, i18n),
    }
}

/// The keys the details view's headers sort by, in their order, then both directions. A
/// choice keeps the menu open.
fn sort_items(app: &OxydeApp, tab: &TabState, handle: Entity<OxydeApp>, pane_id: &str, i18n: &I18n) -> Vec<dropdown::DropdownItem> {
    let trash = tab.path == "trash://";
    let mut options = vec![(Message::ColumnName, Some(SortColumn::Name), None)];
    // The network neighbourhood sorts by name only, as its headers.
    if tab.path != "__network_vincinity__" {
        if trash {
            options.push((Message::TrashOriginalLocation, Some(SortColumn::OriginalLocation), None));
        } else if tab.quick_search.is_some() {
            options.push((Message::ColumnLocation, Some(SortColumn::Location), None));
        }
        options.extend([
            (Message::SortExtension, Some(SortColumn::Type), None),
            (Message::ColumnSize, Some(SortColumn::Size), None),
        ]);
        if tab.permissions_column_width(app.show_permissions_column()) > 0.0 {
            options.push((Message::ColumnPermissions, Some(SortColumn::Permissions), None));
        }
        options.push((Message::ColumnModified, Some(SortColumn::Modified), None));
        if trash {
            options.push((Message::TrashDeletedAt, Some(SortColumn::DeletedAt), None));
        }
    }
    options.extend([
        (Message::SortAscending, None, Some(SortDirection::Ascending)),
        (Message::SortDescending, None, Some(SortDirection::Descending)),
    ]);
    let mut items = Vec::new();
    for (label, column, direction) in options {
        if label == Message::SortAscending {
            items.push(dropdown::DropdownItem::separator());
        }
        let (h, pid) = (handle.clone(), pane_id.to_string());
        let mut item = dropdown::DropdownItem::new(i18n.text(label).to_string(), move |_, cx| {
            h.update(cx, |this, cx| {
                if let Some(pane) = this.state.find_pane_mut(&pid) {
                    let tab = pane.active_tab_mut();
                    if let Some(column) = column { tab.sort_column = column; }
                    if let Some(direction) = direction { tab.sort_direction = direction; }
                    let id = tab.id.clone();
                    this.prepare_current_tab(id, cx);
                }
                this.refresh_grid_menus(cx);
            });
        }).stay_open(true);
        if column == Some(tab.sort_column) || direction == Some(tab.sort_direction) {
            item.icon = Some(dropdown::IconName::Check);
        }
        items.push(item);
    }
    items
}

/// The columns the details view would filter, each opening its panel, then Clear filters.
fn filter_items(app: &OxydeApp, tab: &TabState, handle: Entity<OxydeApp>, pane_id: &str, i18n: &I18n) -> Vec<dropdown::DropdownItem> {
    let trash = tab.path == "trash://";
    let flags = tab.column_filter_flags();
    let mut columns = vec![(FilterColumn::Name, Message::ColumnName, flags.name)];
    if trash || tab.quick_search.is_some() {
        columns.push((FilterColumn::Location, if trash { Message::TrashOriginalLocation } else { Message::ColumnLocation }, flags.location));
    }
    columns.extend([(FilterColumn::Type, Message::ColumnType, flags.types), (FilterColumn::Size, Message::ColumnSize, flags.sizes)]);
    if tab.permissions_column_width(app.show_permissions_column()) > 0.0 {
        columns.push((FilterColumn::Permissions, Message::ColumnPermissions, flags.permissions));
    }
    columns.push((FilterColumn::Date, Message::ColumnModified, flags.dates));
    if trash {
        columns.push((FilterColumn::Deleted, Message::TrashDeletedAt, flags.deleted));
    }
    let mut items: Vec<_> = columns.into_iter().map(|(column, label, lit)| {
        let (h, pid) = (handle.clone(), pane_id.to_string());
        let item = dropdown::DropdownItem::new(i18n.text(label).to_string(), |_, _| {})
            .stay_open(true)
            .has_submenu(true)
            .on_hover(move |position, open_left, cx| {
                h.update(cx, |this, cx| this.open_filter_submenu(&pid, column, position, open_left, cx));
            });
        // The funnel the column's header would light.
        if lit { item.icon(dropdown::IconName::Funnel) } else { item }
    }).collect();
    if tab.has_column_filters() {
        let (h, pid) = (handle, pane_id.to_string());
        items.push(dropdown::DropdownItem::separator());
        items.push(dropdown::DropdownItem::new(i18n.text(Message::FilterClearAll).to_string(), move |_, cx| {
            h.update(cx, |this, cx| this.clear_column_filters(&pid, cx));
        }).stay_open(true));
    }
    items
}
