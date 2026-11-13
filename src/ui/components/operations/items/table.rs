//! The items' details list, a copy of the main window's (`file_list/details.rs`,
//! `headers.rs`, `details_row.rs`, `layout/measurements.rs`) as the file chooser's:
//! the header strip, measured metadata columns, rows, scrollbar and status bar. A
//! Location column, as in search results, names each item's folder when they come
//! from several; State replaces the date.
use super::super::OperationsPanelView;
use super::{outcome, Column, Widths};
use crate::ui::app::layout::measurements::{column_text_width, size_column_text};
use crate::ui::components::icons::render_file_icon;
use crate::ui::components::scrollbar::{render_scrollbar_with_pointer, ScrollbarAxis};
use crate::ui::format::format_number;
use crate::ui::i18n::{FluentArgs, I18nContext, Message};
use crate::ui::scale::rem_pixels;
use crate::ui::style::{DetailsColumnWidths, TabularFigures};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use crate::ui::workspace::SortDirection;
use gpui::prelude::*;
use gpui::*;
use oxyde_lib::systems::file_ops::{FileOperation, ItemOutcome};
use std::collections::HashMap;
use std::path::PathBuf;

const OUTCOMES: [ItemOutcome; 9] = [
    ItemOutcome::InProgress,
    ItemOutcome::Failed,
    ItemOutcome::Partial,
    ItemOutcome::Interrupted,
    ItemOutcome::Pending,
    ItemOutcome::Skipped,
    ItemOutcome::NotProcessed,
    ItemOutcome::Unknown,
    ItemOutcome::Completed,
];

/// Corners of the box that frames the list, as the Duplicates window's.
const BOX_RADIUS: Rems = rems(0.375);

/// The widths a row lays its cells out with.
#[derive(Clone, Copy)]
struct RowWidths {
    total: f32,
    name: f32,
    location: f32,
    file_type: f32,
    size: f32,
    state: f32,
}

impl OperationsPanelView {
    /// Measures the metadata columns as `measure_metadata_columns` does, again only
    /// when the scale, the language or the rows change.
    pub(in super::super) fn measure_items(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let i18n = cx.i18n();
        let key = (window.rem_size().as_f32().round() as u32, i18n.language(), self.items.revision);
        if self.items.measured == Some(key) {
            return;
        }
        let measure = |text: &str, bold: bool, tabular: bool| column_text_width(window, cx, text, bold, tabular);
        let mut types = std::collections::HashSet::new();
        let mut sizes = std::collections::HashSet::new();
        for item in self.items.rows.iter() {
            if let Some(entry) = &item.entry {
                types.insert(crate::ui::components::file_list::file_type_display(entry, i18n));
                sizes.insert(size_column_text(entry.size, entry.is_dir, entry.is_calculated, i18n));
            }
        }
        let widest = |values: &std::collections::HashSet<String>, tabular: bool| {
            values.iter().map(|value| measure(value, false, tabular)).fold(0.0_f32, f32::max)
        };
        let state = OUTCOMES
            .iter()
            .map(|&state| measure(i18n.text(outcome::label(state, *cx.theme()).0).as_ref(), false, false))
            .fold(0.0_f32, f32::max)
            + 2.0 * rem_pixels(outcome::BADGE_PADDING).as_f32();
        let header = |message| measure(i18n.text(message).as_ref(), true, false);
        self.items.widths = Widths {
            file_type: crate::ui::style::metadata_column_width(widest(&types, false), header(Message::ColumnType), false),
            size: crate::ui::style::metadata_column_width(widest(&sizes, true), header(Message::ColumnSize), false),
            state: crate::ui::style::metadata_column_width(state, header(Message::ColumnState), false),
        };
        self.items.measured = Some(key);
    }

    fn header_cell(&self, id: &'static str, message: Message, column: Column, width: f32, theme: OxydeTheme, cx: &mut Context<Self>) -> Stateful<Div> {
        let is_active = self.items.sort == column;
        let direction = self.items.direction;
        div()
            .id(id)
            .relative()
            .w(px(width))
            .flex_none()
            .overflow_hidden()
            .px(rems(0.5))
            .h_full()
            .flex()
            .items_center()
            .child(div().flex_1().min_w_0().truncate().child(cx.i18n().text(message)))
            .child(div().ml_1())
            .child(
                div()
                    .w(rems(1.0))
                    .flex_none()
                    .flex()
                    .justify_center()
                    .when(is_active, |s| {
                        s.child(
                            svg()
                                .path(if direction == SortDirection::Ascending { "icons/arrow-up.svg" } else { "icons/arrow-down.svg" })
                                .size_3()
                                .text_color(theme.accent),
                        )
                    }),
            )
            .hover(|s| s.bg(theme.hover))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| this.sort_items(column, cx)))
    }

    fn render_headers(&self, widths: RowWidths, theme: OxydeTheme, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .h(crate::ui::style::COLUMN_HEADER_HEIGHT)
            .text_color(theme.text_muted)
            .text_size(rems(0.75)).font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .flex_none()
            .child(self.header_cell("operation-items-header-name", Message::ColumnName, Column::Name, widths.name, theme, cx))
            .when(widths.location > 0.0, |row| row.child(
                self.header_cell("operation-items-header-location", Message::ColumnLocation, Column::Location, widths.location, theme, cx)))
            .child(self.header_cell("operation-items-header-type", Message::ColumnType, Column::Type, widths.file_type, theme, cx))
            .child(self.header_cell("operation-items-header-size", Message::ColumnSize, Column::Size, widths.size, theme, cx))
            .child(self.header_cell("operation-items-header-state", Message::ColumnState, Column::State, widths.state, theme, cx))
    }

    fn render_item_row(&self, position: usize, widths: RowWidths, outcomes: &HashMap<PathBuf, ItemOutcome>, system: bool, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let Some(&row) = self.items.shown.get(position) else { return div().into_any_element() };
        let Some(item) = self.items.rows.get(row) else { return div().into_any_element() };
        let i18n = cx.i18n();
        let is_selected = self.items.selected == Some(row);
        let file_type = item.entry.as_ref().map(|entry| crate::ui::components::file_list::file_type_display(entry, i18n)).unwrap_or_default();
        let size = item.entry.as_ref().map(|entry| size_column_text(entry.size, entry.is_dir, entry.is_calculated, i18n)).unwrap_or_default();
        let state = outcomes.get(&item.path).map(|&state| outcome::label(state, theme));
        div()
            .id(("operation-item", position))
            .relative()
            .when(is_selected, |d| d.child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
            .flex()
            .items_center()
            .h(px(crate::ui::style::row_height()))
            .w(px(widths.total))
            .bg(if is_selected { theme.selected } else { theme.active_background })
            .when(!is_selected, |s| s.hover(|s| s.bg(theme.hover)))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                window.focus(&this.focus, cx);
                this.items.selected = Some(row);
                if event.click_count == 2 {
                    this.reveal_item(row, cx);
                }
                cx.notify();
            }))
            .child(
                div()
                    .w(px(widths.name))
                    .flex_none()
                    .h_full()
                    .px(rems(0.5))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .child(div().flex_none().mr_2().child(render_file_icon(item.icon.0.clone(), item.icon.1, rems(1.0), &theme, system, false)))
                    .child(div().flex_shrink(1.0).min_w_0().truncate().font_weight(FontWeight::MEDIUM).text_color(theme.text).child(item.name.clone())),
            )
            .when(widths.location > 0.0, |row| row.child(div().w(px(widths.location)).flex_none().px(rems(0.5)).text_color(theme.text_muted).truncate().text_size(rems(0.75))
                .child(item.parent.clone())))
            .child(div().w(px(widths.file_type)).flex_none().px(rems(0.5)).text_color(theme.text_muted).truncate().text_size(rems(0.75))
                .child(file_type))
            .child(div().w(px(widths.size)).flex_none().px(rems(0.5)).text_align(TextAlign::Right).text_color(theme.text_muted).truncate().text_size(rems(0.75)).tabular_figures()
                .child(div().child(size)))
            .child(div().w(px(widths.state)).flex_none().px(rems(0.5)).flex().items_center().overflow_hidden()
                .when_some(state, |cell, (message, fill, color)| cell.child(outcome::badge(i18n.text(message), fill, color))))
            .into_any_element()
    }

    /// The section's title, how far each state got, then the filter field.
    fn render_items_toolbar(&self, operation: &FileOperation, outcomes: &HashMap<PathBuf, ItemOutcome>, window: &Window, cx: &mut Context<Self>) -> Div {
        let theme = *cx.theme();
        let i18n = cx.i18n();
        let language = i18n.language();
        let mut args = FluentArgs::new();
        args.set("count", operation.sources.len() as i64);
        args.set("number", format_number(operation.sources.len() as u64, language));
        let mut counts = div().flex().flex_wrap().items_center().gap(rems(0.375)).tabular_figures();
        for state in OUTCOMES {
            let count = outcomes.values().filter(|&&outcome| outcome == state).count();
            if count == 0 {
                continue;
            }
            let (message, fill, color) = outcome::label(state, theme);
            let mut args = FluentArgs::new();
            args.set("value", i18n.text(message).to_string());
            args.set("number", format_number(count as u64, language));
            counts = counts.child(outcome::badge(i18n.text_with_args(Message::OpsOutcomeCount, &args), fill, color));
        }
        // A narrow window sends the field to its own line rather than over the counts.
        div()
            .flex_none()
            .px(rems(1.0))
            .pb(rems(0.75))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(rems(0.75))
            .child(div().flex_none().font_weight(FontWeight::SEMIBOLD)
                .child(i18n.text_with_args(Message::OpsAffectedItems, &args)))
            .child(counts)
            .when_some(self.items.input.clone(), |bar, input| {
                bar.child(
                    crate::ui::components::input::field_frame(&input, window, cx)
                        .w(rems(20.0))
                        .flex_none()
                        .ml_auto()
                        .child(crate::ui::components::input::field_icon(
                            "icons/search.svg",
                            crate::ui::components::input::FieldEdge::Leading,
                            &theme,
                        ))
                        .child(div().flex_1().min_w_0().h_full().child(input)),
                )
            })
    }

    /// The toolbar, then the details list in a framed box.
    pub(in super::super) fn render_items(&self, operation: &FileOperation, system: bool, window: &Window, cx: &mut Context<Self>) -> Div {
        let theme = *cx.theme();
        let outcomes = std::rc::Rc::new(operation.item_outcomes.clone());
        let viewport = self.items.pane_width.get().max(1.0);
        let columns = DetailsColumnWidths::for_viewport_with_search(
            viewport,
            self.items.widths.file_type,
            self.items.widths.size,
            self.items.widths.state,
            self.items.origin.is_none(),
        );
        let widths = RowWidths {
            total: columns.total(),
            name: columns.name,
            location: columns.location,
            file_type: columns.file_type,
            size: columns.size,
            state: columns.date,
        };
        let count = self.items.shown.len();
        let content_h = (count as f32 * crate::ui::style::row_height()).max(1.0);
        let source = cx.entity();
        let entity_id = source.entity_id();
        let measured = self.items.pane_width.clone();
        let row_outcomes = outcomes.clone();
        let mut table = div()
            .flex_1()
            .min_h_0()
            .mx(rems(1.0))
            .mb(rems(1.0))
            .relative()
            .overflow_hidden()
            .rounded(BOX_RADIUS)
            .border(rems(0.0625))
            .border_color(theme.border)
            .bg(theme.active_background)
            // A new width lays the columns out again.
            .child(canvas(move |bounds, _, cx| {
                let width = bounds.size.width.as_f32();
                if (measured.get() - width).abs() > 0.5 {
                    measured.set(width);
                    cx.notify(entity_id);
                }
            }, |_, _, _, _| {}).absolute().size_full())
            // The full-width strip owns the separator: the header cells only span the columns.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(crate::ui::style::COLUMN_HEADER_HEIGHT)
                    .rounded_t(BOX_RADIUS)
                    .bg(theme.header)
                    .border_b(rems(0.0625))
                    .border_color(theme.border),
            )
            .child(
                div()
                    .absolute()
                    .left(rem_pixels(rems(0.5)))
                    .w(px(widths.total))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .flex_col()
                    .child(self.render_headers(widths, theme, cx))
                    .child(
                        uniform_list("operation-items", count, move |range, _, cx| {
                            source.update(cx, |this, cx| range.map(|position| {
                                this.render_item_row(position, widths, &row_outcomes, system, *cx.theme(), cx)
                            }).collect::<Vec<_>>())
                        })
                        .track_scroll(&self.items.list)
                        .w_full()
                        .flex_1()
                        .min_h_0(),
                    ),
            );
        let base = self.items.list.0.borrow().base_handle.clone();
        let measured_h = base.bounds().size.height.as_f32();
        let viewport_h = if measured_h > 0.0 { measured_h } else { content_h };
        let dragging = self.scroll_drag.as_ref().is_some_and(|(track, _, _)| track.ends_with(super::super::view::ITEMS_TRACK));
        table = table.child(
            div()
                .absolute()
                .top(px(crate::ui::style::column_header_height()))
                .bottom_0()
                .right_0()
                .w(px(crate::ui::components::scrollbar::track_width()))
                .child(render_scrollbar_with_pointer(theme, cx.entity(), ScrollbarAxis::Vertical, viewport_h, content_h, -base.offset().y.as_f32(), dragging,
                    super::super::view::ITEMS_TRACK.into(), None, base.clone(),
                    move |this: &mut Self, track, viewport, content, _, cx| {
                        this.scroll_drag = Some((track, viewport, content));
                        cx.notify(entity_id);
                    })),
        );
        let empty = if self.items.loading {
            Some(Message::OpsItemsLoading)
        } else if self.items.shown.is_empty() {
            Some(Message::OpsNoMatchingItems)
        } else {
            None
        };
        if let Some(message) = empty {
            table = table.child(
                div()
                    .absolute()
                    .top(px(crate::ui::style::column_header_height() + crate::ui::scale::logical(18.0)))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .text_color(theme.text_muted)
                    .child(cx.i18n().text(message)),
            );
        }
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(self.render_items_toolbar(operation, &outcomes, window, cx))
            .child(table)
    }
}
