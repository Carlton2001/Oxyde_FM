//! The grid view, a copy of the main window's (`file_list/grid.rs`, `grid_card.rs`,
//! `grid_scroll.rs`): cards laid out by `GridGeometry`, only the visible rows built,
//! the measured scrollbar and the status bar. Choosing needs no menus, drags or
//! renaming.
use super::FileChooserWindow;
use crate::ui::components::icons::render_file_icon;
use crate::ui::components::tooltip::hover::SharedTooltipExt;
use crate::ui::components::tooltip::Tooltip;
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::{logical, rem_pixels};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;

impl FileChooserWindow {
    fn render_card(&self, position: usize, card_width: f32, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let Some(file) = self.shown_entry(position) else { return div().into_any_element() };
        let path = file.path.clone();
        let is_dir = file.is_dir;
        let is_selected = self.selection.contains(position);
        let has_subdirectories = is_dir && self.subfolders.get(&path).copied().unwrap_or(false);
        let icon = crate::ui::components::icons::entry_icon(file);
        let press = self.press(position, cx);
        let name = crate::ui::format::entry_name(file, cx.i18n());
        let kind = if file.is_dir {
            cx.i18n().text(Message::FileTypeFolder).to_uppercase()
        } else {
            crate::ui::components::file_list::file_type_display(file, cx.i18n())
        };
        let pane = format!("file-chooser-{}", cx.entity().entity_id());
        let card = div().id(("file-chooser-card", position));
        let card = match self.tooltip_owner() {
            Some(owner) => card.shared_tooltip(owner, super::FILES_REGION, path.clone(), cx, Tooltip::for_item(file.clone(), false, self.media_previews, false)),
            None => card,
        };
        card
            .relative()
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
            .border_color(if self.rename_refused(&path) { theme.error } else if is_selected { theme.accent } else { theme.border.opacity(0.0) })
            .bg(if is_selected { theme.selected } else { theme.active_background })
            .when(!is_selected, |d| d.hover(|d| d.bg(theme.hover)))
            .on_mouse_down(MouseButton::Left, press)
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(rems(4.25))
                    .when(file.is_hidden || file.is_system, |d| d.opacity(0.75))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(if self.thumbnails && !is_dir && crate::ui::components::file_list::supports_thumbnail(file) {
                                crate::ui::components::file_list::render_grid_thumbnail(file, &pane, icon, theme, self.system_icons).into_any_element()
                            } else {
                                render_file_icon(path.clone(), icon, rems(4.0), &theme, self.system_icons, true)
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
                                .child(svg().path("icons/chevron-right.svg").size_3().text_color(theme.text).opacity(0.5)),
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
                    .when(file.is_hidden || file.is_system, |d| d.opacity(0.75))
                    .child(match self.render_rename(&path, None, theme, cx) {
                        Some(field) => field,
                        None => div().font_weight(FontWeight::MEDIUM).child(name).into_any_element(),
                    }),
            )
            .child(
                div()
                    .text_size(rems(0.625))
                    .text_color(theme.text_muted)
                    .text_ellipsis()
                    .when(file.is_hidden || file.is_system, |d| d.opacity(0.75))
                    .child(kind),
            )
            .into_any_element()
    }

    pub(super) fn render_grid(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let status_bar_height = rem_pixels(crate::ui::style::STATUS_BAR_HEIGHT).as_f32();
        let grid = self.grid_geometry();
        let columns = grid.columns.max(1);
        let card_width = grid.cell_width - logical(4.0);
        let contents = if let Some((message, protected)) = self.empty_message(cx) {
            div()
                .id("file-chooser-grid-empty")
                .flex_1()
                .flex()
                .justify_center()
                .when(!protected, |empty| empty.pt(px(crate::ui::style::column_header_height() + logical(18.0))))
                .when(protected, |empty| empty.items_center())
                .text_color(theme.text_muted)
                .text_size(rems(0.8125))
                .child(if protected { crate::ui::components::file_list::protected_folder_message(message, &theme).into_any_element() } else { div().child(message).into_any_element() })
                .on_mouse_down(MouseButton::Left, cx.listener(|this, event, window, cx| this.press_background(event, window, cx)))
                .into_any_element()
        } else {
            let row_height = crate::ui::style::grid_card_row_height();
            let rows = self.shown.len().div_ceil(columns);
            let rows_height = rows as f32 * row_height;
            let scroll = self.grid_scroll.clone();
            let actual_viewport = scroll.bounds().size.height.as_f32();
            // Before the first layout, as the main grid assumes.
            let viewport = if actual_viewport > 0.0 { actual_viewport } else { logical(200.0) };
            let max_scroll = (rows_height + crate::ui::style::row_height() - viewport).max(0.0);
            let offset = (-scroll.offset().y.as_f32()).clamp(0.0, max_scroll);
            if scroll.offset().y.as_f32() != -offset {
                scroll.set_offset(point(px(0.0), px(-offset)));
            }
            // Uniform rows: the visible range follows from the offset alone.
            let start = ((offset.max(0.0) / row_height).floor() as usize).min(rows);
            let end = (((offset + viewport + row_height) / row_height).ceil() as usize).min(rows).max(start);
            let lines: Vec<AnyElement> = (start..end).map(|row| {
                let cells: Vec<AnyElement> = (row * columns..((row + 1) * columns).min(self.shown.len()))
                    .map(|position| self.render_card(position, card_width, theme, cx))
                    .collect();
                div().w_full().flex_none().h(px(row_height)).flex().items_start().children(cells).into_any_element()
            }).collect();
            let entity_id = cx.entity().entity_id();
            let list = div()
                .id("file-chooser-grid")
                .flex_1().min_h_0().flex().flex_col().pt_1().overflow_y_scroll().track_scroll(&scroll)
                .on_scroll_wheel(move |_, _, cx| cx.notify(entity_id))
                .child(div().h(px(start as f32 * row_height)).flex_none())
                .children(lines)
                .child(div().h(px((rows_height - end as f32 * row_height).max(0.0))).flex_none())
                .child(div().w_full().flex_none().h(px(crate::ui::style::row_height())));
            // Derive the content from GPUI's measured scroll range when available, so the
            // thumb reaches the rail end exactly when the list cannot scroll further.
            let measured_max = scroll.max_offset().y.as_f32();
            let content_height = if actual_viewport > 0.0 && measured_max > 0.0 {
                actual_viewport + measured_max
            } else {
                rows_height + crate::ui::style::row_height()
            };
            div()
                .id("file-chooser-grid-scroll")
                .relative()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .overflow_hidden()
                .px_2()
                .pr(px(crate::ui::components::scrollbar::track_width()))
                .child(self.measure_entries())
                .child(list)
                .children(self.render_band(theme))
                .child({
                    // The rail and the button back to the start, as the main grid's scroll zone.
                    let zone = div().absolute().top_0().bottom_0().right_0().w(rems(3.625));
                    let rail = self.render_vertical_scrollbar(theme, super::scroll::GRID, scroll.clone(), actual_viewport.max(1.0), content_height, cx);
                    let top = self.scroll_top_button(theme, scroll.clone(), actual_viewport.max(1.0), content_height, cx);
                    self.scroll_zone(zone, cx).child(rail).child(top)
                })
                .on_mouse_down(MouseButton::Left, cx.listener(|this, event, window, cx| this.press_background(event, window, cx)))
                .into_any_element()
        };
        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .relative()
            .pb(px(status_bar_height))
            .on_scroll_wheel(|_, window, cx| crate::ui::components::tooltip::hover::dismiss(window, cx))
            .when_some(self.tooltip_owner(), |grid, owner| grid.child(crate::ui::components::tooltip::hover::region_container(
                owner, super::FILES_REGION, px(0.0), px(status_bar_height), cx)))
            .overflow_hidden()
            .bg(theme.active_background)
            .child(contents)
            .child(self.render_status_bar(*cx.theme(), cx))
            .into_any_element()
    }
}
