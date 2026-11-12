//! The details view, a copy of the main window's (`file_list/details.rs`,
//! `headers.rs`, `details_row.rs`, `layout/measurements.rs`): the header strip, the
//! measured metadata columns, rows and their scrollbar. Choosing needs no menus, drags,
//! renaming, column filters or horizontal scrolling: the window's minimum width keeps
//! the columns in view.
use super::{ColumnWidths, FileChooserWindow};
use crate::ui::components::icons::render_file_icon;
use crate::ui::components::scrollbar::{render_scrollbar_with_pointer, ScrollbarAxis};
use crate::ui::components::tooltip::hover::SharedTooltipExt;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::app::layout::measurements;
use crate::ui::format::{format_date, format_size};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::{logical, rem_pixels};
use crate::ui::style::DetailsColumnWidths;
use crate::ui::theme::{OxydeTheme, ThemeContext};
use crate::ui::workspace::{SortColumn, SortDirection};
use crate::ui::style::TabularFigures;
use gpui::prelude::*;
use gpui::*;

/// The widths a row lays its cells out with.
#[derive(Clone, Copy)]
struct RowWidths {
    total: f32,
    name: f32,
    file_type: f32,
    size: f32,
    permissions: f32,
    date: f32,
}

impl FileChooserWindow {
    /// Measures the metadata columns with `measure_metadata_columns`'s helpers, again
    /// only when the scale, the language, the date format or the measured values change.
    pub(super) fn measure_columns(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let i18n = cx.i18n();
        let language = i18n.language();
        let date_format = crate::ui::format::date_format(cx);
        let key = (window.rem_size().as_f32().round() as u32, language, date_format, self.column_revision);
        if self.measured == Some(key) { return; }
        // Every format has the same number of tabular digits. One sample suffices.
        let date_sample = crate::ui::format::format_date(1_760_000_000_000, date_format);
        let measure = |text: &str, bold: bool, tabular: bool| measurements::column_text_width(window, cx, text, bold, tabular);
        let (types, sizes) = &*self.column_values;
        let type_width = types.iter().map(|label| measure(&measurements::type_column_text(label, i18n), false, false)).fold(0.0_f32, f32::max);
        let size_width = sizes.iter().map(|&(size, directory, calculated)| measure(&measurements::size_column_text(size, directory, calculated, i18n), false, true)).fold(0.0_f32, f32::max);
        // The chooser filters no column: no header reserves a filter mark.
        self.column_widths = ColumnWidths {
            file_type: crate::ui::style::metadata_column_width(type_width, measure(i18n.text(Message::ColumnType).as_ref(), true, false), false),
            size: crate::ui::style::metadata_column_width(size_width, measure(i18n.text(Message::ColumnSize).as_ref(), true, false), false),
            date: crate::ui::style::metadata_column_width(measure(&date_sample, false, true), measure(i18n.text(Message::ColumnModified).as_ref(), true, false), false),
            // Fixed width: four tabular digits, the most the column ever shows.
            permissions: crate::ui::style::metadata_column_width(measure("0000", false, true), measure(i18n.text(Message::ColumnPermissions).as_ref(), true, false), false),
        };
        self.measured = Some(key);
    }

    fn header_cell(&self, id: &'static str, message: Message, column: SortColumn, width: f32, theme: OxydeTheme, cx: &mut Context<Self>) -> Stateful<Div> {
        let is_active = self.options.sort == column;
        let direction = self.options.direction;
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
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, _, cx| this.toggle_sort(column, cx)))
    }

    fn render_headers(&self, widths: RowWidths, theme: OxydeTheme, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .h(crate::ui::style::COLUMN_HEADER_HEIGHT)
            .text_color(theme.text_muted)
            .text_size(rems(0.75)).font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .flex_none()
            .child(self.header_cell("file-chooser-header-name", Message::ColumnName, SortColumn::Name, widths.name, theme, cx))
            .child(self.header_cell("file-chooser-header-type", Message::ColumnType, SortColumn::Type, widths.file_type, theme, cx))
            .child(self.header_cell("file-chooser-header-size", Message::ColumnSize, SortColumn::Size, widths.size, theme, cx))
            .when(widths.permissions > 0.0, |row| row.child(
                self.header_cell("file-chooser-header-permissions", Message::ColumnPermissions, SortColumn::Permissions, widths.permissions, theme, cx)))
            .child(self.header_cell("file-chooser-header-date", Message::ColumnModified, SortColumn::Modified, widths.date, theme, cx))
    }

    fn render_row(&self, position: usize, widths: RowWidths, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let Some(file) = self.shown_entry(position) else { return div().into_any_element() };
        let is_dir = file.is_dir;
        let is_selected = self.selection.contains(position);
        let has_subdirectories = is_dir && self.subfolders.get(&file.path).copied().unwrap_or(false);
        let is_dimmed = file.is_hidden || file.is_system;
        let press = self.press(position, cx);
        let i18n = cx.i18n();
        let language = i18n.language();
        let file_name = crate::ui::format::entry_name(file, i18n);
        let file_type = crate::ui::components::file_list::file_type_display(file, i18n);
        let size = if is_dir && file.is_calculated && file.size == 0 {
            i18n.text(Message::FolderSizeEmpty).to_string()
        } else {
            format_size(file.size, is_dir, file.is_calculated, language)
        };
        let date = format_date(file.modified, crate::ui::format::date_format(cx));
        let row = div().id(("file-chooser-row", position));
        let row = match self.tooltip_owner() {
            Some(owner) => row.shared_tooltip(owner, super::FILES_REGION, file.path.clone(), cx, Tooltip::for_item(file.clone(), false, self.media_previews, false)),
            None => row,
        };
        let name = div()
            .flex_shrink(1.0)
            .min_w_0()
            .truncate()
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.text)
            .child(file_name);
        row
            .relative()
            .when(is_selected, |d| d.child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
            .flex()
            .items_center()
            .h(px(crate::ui::style::row_height()))
            .w(px(widths.total))
            .bg(if is_selected { theme.selected } else { theme.active_background })
            .when(!is_selected, |s| s.hover(|s| s.bg(theme.hover)))
            .on_mouse_down(MouseButton::Left, press)
            .child(
                div()
                    .w(px(widths.name))
                    .flex_none()
                    .h_full()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .when(is_dimmed, |d| d.opacity(0.75))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .px(rems(0.5))
                            .flex()
                            .items_center()
                            .child(div().flex_none().mr_2().child(render_file_icon(file.path.clone(), crate::ui::components::icons::entry_icon(file), rems(1.0), &theme, self.system_icons, false)))
                            .child(if has_subdirectories {
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .items_center()
                                    .overflow_hidden()
                                    .child(name)
                                    .child(div().ml_2().flex_none().child(svg().path("icons/chevron-right.svg").size_3().text_color(theme.text).opacity(0.5)))
                                    .into_any_element()
                            } else {
                                name.into_any_element()
                            }),
                    ),
            )
            .child(div().w(px(widths.file_type)).flex_none().px(rems(0.5)).text_color(theme.text_muted).truncate().text_size(rems(0.75)).when(is_dimmed, |d| d.opacity(0.75))
                .child(file_type))
            .child(div().w(px(widths.size)).flex_none().px(rems(0.5)).text_align(TextAlign::Right).text_color(theme.text_muted).truncate().text_size(rems(0.75)).tabular_figures().when(is_dimmed, |d| d.opacity(0.75))
                .child(div().child(size)))
            .when(widths.permissions > 0.0, |row| row.child(div().w(px(widths.permissions)).flex_none().px(rems(0.5)).text_right().text_color(theme.text_muted).truncate().text_size(rems(0.75)).tabular_figures().when(is_dimmed, |d| d.opacity(0.75))
                .children(file.mode.map(crate::ui::format::permissions::octal))))
            .child(div().w(px(widths.date)).flex_none().px(rems(0.5)).text_color(theme.text_muted).truncate().text_size(rems(0.75)).tabular_figures().when(is_dimmed, |d| d.opacity(0.75))
                .child(date))
            .into_any_element()
    }

    /// An entry's press selects it, with Ctrl or Shift among others when several may
    /// be chosen, and a double press opens it.
    pub(super) fn press(&self, position: usize, cx: &mut Context<Self>) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |event, window, cx| {
            cx.stop_propagation();
            let _ = this.update(cx, |this, cx| {
                window.focus(&this.focus, cx);
                this.press_entry(position, event.modifiers, cx);
                if event.click_count == 2 && !event.modifiers.control && !event.modifiers.shift {
                    this.open_entry(position, window, cx);
                }
            });
        }
    }

    /// A press beside the entries: nothing is selected any more.
    pub(super) fn press_background(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        if self.selection.clear() {
            cx.notify();
        }
    }

    pub(super) fn render_details(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let pane_viewport_w = self.pane_width.get().max(1.0);
        let permissions_w = if self.show_permissions { logical(self.column_widths.permissions) } else { 0.0 };
        let columns = DetailsColumnWidths::for_viewport(
            pane_viewport_w - permissions_w,
            self.column_widths.file_type,
            self.column_widths.size,
            self.column_widths.date,
        );
        let widths = RowWidths {
            total: columns.total() + permissions_w,
            name: columns.name,
            file_type: columns.file_type,
            size: columns.size,
            permissions: permissions_w,
            date: columns.date,
        };
        let status_bar_height = rem_pixels(crate::ui::style::STATUS_BAR_HEIGHT).as_f32();
        let count = self.shown.len() + 1;
        let content_h = (count as f32 * crate::ui::style::row_height()).max(1.0);
        let source = cx.entity();
        let entity_id = source.entity_id();
        let mut container = div()
            .on_scroll_wheel(|_, window, cx| crate::ui::components::tooltip::hover::dismiss(window, cx))
            .when_some(self.tooltip_owner(), |container, owner| container.child(crate::ui::components::tooltip::hover::region_container(
                owner, super::FILES_REGION, px(crate::ui::style::column_header_height()), px(status_bar_height), cx)))
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(theme.active_background)
            .overflow_hidden()
            .relative()
            // The full-width strip owns the separator: the header cells only span the columns.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(crate::ui::style::COLUMN_HEADER_HEIGHT)
                    .bg(theme.header)
                    .border_b(rems(0.0625))
                    .border_color(theme.border),
            )
            .child(
                div()
                    .size_full()
                    .child(
                        div()
                            .absolute()
                            .left(rem_pixels(rems(0.5)))
                            .w(px(widths.total))
                            .top_0()
                            .bottom(px(status_bar_height))
                            .flex()
                            .flex_col()
                            .child(self.render_headers(widths, theme, cx))
                            .child(
                                uniform_list("file-chooser-list", count, move |range, _, cx| {
                                    source.update(cx, |this, cx| range.map(|position| {
                                        if position == this.shown.len() {
                                            return div().w_full().h(px(crate::ui::style::row_height())).into_any_element();
                                        }
                                        this.render_row(position, widths, *cx.theme(), cx)
                                    }).collect::<Vec<_>>())
                                })
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.press_background(window, cx)))
                                .track_scroll(&self.list)
                                .w_full()
                                .flex_1()
                                .min_h_0(),
                            ),
                    ),
            )
            // The blank area right of the last column and the gutters also deselect.
            .child(
                div()
                    .absolute()
                    .top(px(crate::ui::style::column_header_height()))
                    .bottom(px(status_bar_height))
                    .left(rem_pixels(rems(0.5)) + px(widths.total))
                    .right_0()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.press_background(window, cx))),
            )
            .child(
                div()
                    .absolute()
                    .top(px(crate::ui::style::column_header_height()))
                    .left_0()
                    .bottom_0()
                    .w(rems(0.5))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.press_background(window, cx);
                    })),
            );

        // The measured list viewport, so the thumb reaches the rail end.
        let base = self.list.0.borrow().base_handle.clone();
        let measured_v_h = base.bounds().size.height.as_f32();
        // Before the first layout, no scrollbar shows.
        let v_v_h = if measured_v_h > 0.0 { measured_v_h } else { content_h };
        let v_offset = -base.offset().y.as_f32();
        let dragging = self.scroll_drag.as_ref().is_some_and(|(track, _, _)| track.ends_with("file-chooser-details"));
        container = container.child(
            div()
                .absolute()
                .top(px(crate::ui::style::column_header_height()))
                .bottom(px(status_bar_height))
                .right_0()
                .w(rems(3.625))
                .child(render_scrollbar_with_pointer(theme, cx.entity(), ScrollbarAxis::Vertical, v_v_h, content_h, v_offset, dragging,
                    "file-chooser-details".into(), None,
                    move |this: &mut Self, track, viewport, content, _, cx| {
                        this.scroll_drag = Some((track, viewport, content));
                        cx.notify(entity_id);
                    })),
        );

        if let Some((message, protected)) = self.empty_message(cx) {
            container = container.child(
                div()
                    .absolute()
                    .when(!protected, |empty| empty.top(px(crate::ui::style::column_header_height() + logical(18.0))))
                    .when(protected, |empty| empty.top(px(crate::ui::style::column_header_height())).bottom(px(status_bar_height)).items_center())
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .text_color(theme.text_muted)
                    .text_size(rems(0.8125))
                    .child(if protected { crate::ui::components::file_list::protected_folder_message(message, &theme).into_any_element() } else { div().child(message).into_any_element() }),
            );
        }
        container.child(self.render_status_bar(theme, cx)).into_any_element()
    }
}
