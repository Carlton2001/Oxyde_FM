//! The filter panel of a details column: the column's name and Clear, its field, its values
//! with their counts, the custom range of Size and Date, and the choice to keep the filters
//! from folder to folder. Every change applies at once.
use crate::ui::app::context_menus::context_menu_contains_point;
use crate::ui::app::filters::{ColumnFilterPanel, RangeEditor};
use crate::ui::app::{DragTarget, OxydeApp};
use crate::ui::components::dropdown::{DropdownItem, GenericDropdown, IconName};
use crate::ui::components::input::{self, FieldEdge};
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::{logical, rem_pixels};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use crate::ui::workspace::{FilterColumn, FilterValue, NameMatcher, TabState};
use gpui::prelude::*;
use gpui::*;
use std::rc::Rc;

/// Narrowest panel, room for two size fields and the calendar.
const PANEL_MIN_WIDTH: Rems = rems(16.0);
/// Narrowest date panel, room for two dates side by side with their calendar buttons.
const DATE_PANEL_MIN_WIDTH: Rems = rems(20.0);

impl OxydeApp {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_column_filter(
        &self,
        panel: &ColumnFilterPanel,
        m_idx: usize,
        h: Entity<Self>,
        scroll_handle: UniformListScrollHandle,
        is_dragging: bool,
        win_h: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> GenericDropdown {
        let theme = *cx.theme();
        let tab = self.state.find_pane(&panel.pane_id).map(|pane| pane.active_tab());
        let entries = match (tab, panel.column) {
            (_, FilterColumn::Name | FilterColumn::Location) | (None, _) => Rc::new(Vec::new()),
            (Some(tab), _) => panel.entries(tab, cx.i18n()),
        };
        let searchable = panel.column == FilterColumn::Type && entries.len() > crate::ui::app::filters::TYPE_SEARCH_THRESHOLD;
        let field = panel.field.clone().filter(|_| panel.column != FilterColumn::Type || searchable);
        // The menu window takes the keyboard once open: the field asks for it until it holds it.
        if let Some(field) = &field {
            if !panel.focused.get() {
                let handle = field.read(cx).focus_handle.clone();
                if handle.is_focused(window) {
                    panel.focused.set(true);
                } else {
                    window.focus(&handle, cx);
                }
            }
        }
        let query = field.as_ref().filter(|_| searchable).map(|field| field.read(cx).content().to_lowercase()).unwrap_or_default();
        let visible: Vec<_> = entries.iter().filter(|entry| query.is_empty() || entry.label.to_lowercase().contains(&query)).collect();

        let mut header = div().w_full().flex().flex_col().gap(rems(0.25))
            .child(self.filter_title(panel, tab, h.clone(), theme, cx));
        if let Some(field) = &field {
            header = header.child(self.filter_field(panel, field, tab, h.clone(), theme, window, cx));
        }

        let mut items = Vec::new();
        if let Some(tab) = tab.filter(|_| !matches!(panel.column, FilterColumn::Name | FilterColumn::Location)) {
            let values: Vec<FilterValue> = visible.iter().map(|entry| entry.value.clone()).collect();
            let checked = values.iter().filter(|value| tab.is_value_checked(value)).count();
            if !values.is_empty() {
                let (handle, pane_id) = (h.clone(), panel.pane_id.clone());
                let all = checked == values.len();
                items.push(DropdownItem::new(cx.i18n().text(Message::FilterAll), move |_, cx| {
                    handle.update(cx, |this, cx| this.set_filter_values(&pane_id, values.clone(), !all, cx));
                }).icon(if all { IconName::Checkbox(true) } else if checked > 0 { IconName::CheckboxMixed } else { IconName::Checkbox(false) }).stay_open(true));
                items.push(DropdownItem::separator());
            }
            let system = cfg!(target_os = "windows") && self.use_system_icons();
            for entry in visible {
                let (handle, pane_id, value) = (h.clone(), panel.pane_id.clone(), entry.value.clone());
                let checked = tab.is_value_checked(&entry.value);
                let mut item = DropdownItem::new(entry.label.clone(), move |_, cx| {
                    handle.update(cx, |this, cx| this.toggle_filter_value(&pane_id, value.clone(), cx));
                }).stay_open(true).icon(if panel.column == FilterColumn::Type {
                    let (path, icon) = entry.sample.clone().unwrap_or((String::new(), crate::ui::components::icons::IconType::File));
                    IconName::CheckboxFile { checked, path, icon, system: system && entry.sample.is_some() }
                } else {
                    IconName::Checkbox(checked)
                });
                item.badge = Some(crate::ui::format::format_number(entry.count, cx.i18n().language()));
                // A value the other filters hide stays offered, faded.
                item.is_faded = entry.count == 0;
                items.push(item);
            }
            if let Some(range) = &panel.range {
                let handle = h.clone();
                if !items.is_empty() {
                    items.push(DropdownItem::separator());
                }
                items.push(DropdownItem::new(cx.i18n().text(Message::FilterCustom), move |_, cx| {
                    handle.update(cx, |this, cx| this.toggle_range_editor(cx));
                }).icon(if range.open { IconName::ChevronUp } else { IconName::ChevronDown }).stay_open(true));
            }
        }

        let mut footer = div().w_full().flex().flex_col();
        if let Some(range) = panel.range.as_ref().filter(|range| range.open) {
            footer = footer.child(self.range_editor(panel.column, range, h.clone(), theme, window, cx));
        }
        if let Some(tab) = tab {
            footer = footer
                .child(crate::ui::elements::separator::horizontal(theme.menu_border))
                .child(keep_row(tab.keep_filters, panel.pane_id.clone(), h.clone(), theme, cx));
        }

        GenericDropdown {
            id: ("context-menu", m_idx).into(),
            h_app: h.clone(),
            items,
            reserve_icon_space: true,
            is_context_menu: true,
            scroll_handle,
            width: None,
            min_width: Some(panel_min_width(panel.column, tab, window, cx)),
            max_height: logical(320.0),
            on_close: {
                let h = h.clone();
                Rc::new(move |_, cx| {
                    h.update(cx, |this, cx| {
                        this.context_menus.clear();
                        this.submenu_timer = None;
                        this.submenu_close_timer = None;
                        cx.notify();
                    });
                })
            },
            on_outside: Some({
                let h = h.clone();
                Rc::new(move |position, _, cx| {
                    h.update(cx, |this, cx| {
                        let inside_sibling = this.context_menus.iter().enumerate().any(|(index, menu)| {
                            index != m_idx && context_menu_contains_point(menu, position, px(win_h))
                        });
                        if !inside_sibling {
                            this.context_menus.clear();
                            this.submenu_timer = None;
                            this.submenu_close_timer = None;
                            cx.notify();
                        }
                    });
                })
            }),
            on_drag: Rc::new(move |this, tid, vh, ch, _| {
                this.drag_target = Some(DragTarget::ContextMenuV { menu_idx: m_idx, viewport_h: vh, content_h: ch, track_id: tid });
            }),
            is_dragging,
            header: Some(header.into_any_element()),
            footer: Some(footer.into_any_element()),
            on_item_hover: None,
            on_width_calculated: Some(Rc::new(move |w, cx| {
                h.update(cx, |this, _| {
                    if let Some(menu) = this.context_menus.get_mut(m_idx) {
                        menu.width = w;
                    }
                });
            })),
            on_hover_change: None,
        }
    }

    /// The column's name, with Clear while it filters.
    fn filter_title(&self, panel: &ColumnFilterPanel, tab: Option<&TabState>, h: Entity<Self>, theme: OxydeTheme, cx: &Context<Self>) -> Div {
        let title = filter_title_message(panel.column, tab);
        let set = tab.is_some_and(|tab| column_set(tab, panel.column));
        let (pane_id, column) = (panel.pane_id.clone(), panel.column);
        div()
            .h(rems(1.75))
            .pl(rems(0.5))
            .flex()
            .items_center()
            .justify_between()
            .gap(rems(0.5))
            .child(div().text_size(rems(0.75)).font_weight(FontWeight::SEMIBOLD).text_color(theme.text_muted).child(cx.i18n().text(title)))
            .when(set, |row| row.child(
                div()
                    .id("filter-clear")
                    .h(rems(1.5))
                    .px(rems(0.5))
                    .flex()
                    .items_center()
                    .rounded(rems(0.3125))
                    .text_size(rems(0.75))
                    .text_color(theme.accent)
                    .hover(|style| style.bg(theme.menu_hover))
                    .child(cx.i18n().text(Message::FilterClear))
                    .on_click(move |_, _, cx| h.update(cx, |this, cx| this.clear_column_filter(&pane_id, column, cx))),
            ))
    }

    /// The name or location text, with the regex switch for names, or the search among types.
    #[allow(clippy::too_many_arguments)]
    fn filter_field(&self, panel: &ColumnFilterPanel, field: &Entity<input::TextInput>, tab: Option<&TabState>, h: Entity<Self>, theme: OxydeTheme, window: &Window, cx: &Context<Self>) -> Div {
        let frame = input::field_frame(field, window, cx).w_full();
        let regex = panel.column == FilterColumn::Name && tab.is_some_and(|tab| tab.filter_name_regex);
        // An expression reads in the monospace face, as in advanced search.
        let editor = div().flex_1().min_w_0().h_full().font_family(if regex { "JetBrains Mono" } else { "Inter" }).child(field.clone());
        match panel.column {
            FilterColumn::Name => {
                let valid = tab.is_none_or(|tab| NameMatcher::is_valid(&tab.filter_name, tab.filter_name_regex));
                let pane_id = panel.pane_id.clone();
                frame
                    .when(!valid, |frame| frame.border_color(theme.error))
                    .child(
                        input::field_option("filter-regex", FieldEdge::Leading, regex, &theme)
                            .child(input::field_button_icon("icons/regex.svg", regex, &theme))
                            .fixed_tooltip(Tooltip::text(cx.i18n().text(Message::FilterRegex)))
                            .on_click(move |_, _, cx| h.update(cx, |this, cx| this.toggle_name_regex(&pane_id, cx))),
                    )
                    .child(editor)
            }
            FilterColumn::Type => frame.child(input::field_icon("icons/search.svg", FieldEdge::Leading, &theme)).child(editor),
            _ => frame.child(editor),
        }
    }

    /// Two bounds, then the unit of sizes or the calendar of the date being picked.
    fn range_editor(&self, column: FilterColumn, range: &RangeEditor, h: Entity<Self>, theme: OxydeTheme, window: &Window, cx: &Context<Self>) -> Div {
        let format = crate::ui::format::date_format(cx);
        let date = matches!(column, FilterColumn::Date | FilterColumn::Deleted);
        let mut editor = div().w_full().flex().flex_col().gap(rems(0.25)).pt(rems(0.25));
        // Both bounds share a line, the calendar opening below it.
        let mut bounds = div().w_full().flex().gap(rems(0.25));
        for index in 0..2 {
            let valid = if date { range.date_bound(index, format).is_ok() } else { range.size_bound(index).is_ok() };
            let open = range.calendar.is_some_and(|(open, _)| open == index);
            let toggle = h.clone();
            bounds = bounds.child(
                input::field_frame(&range.inputs[index], window, cx)
                    .flex_1()
                    .min_w_0()
                    .when(!valid, |frame| frame.border_color(theme.error))
                    .child(div().flex_1().min_w_0().h_full().child(range.inputs[index].clone()))
                    .when(date, |frame| frame.child(
                        input::field_button(("filter-calendar", index), FieldEdge::Trailing, open, &theme)
                            .child(input::field_button_icon("icons/calendar.svg", open, &theme))
                            .on_click(move |_, _, cx| toggle.update(cx, |this, cx| this.toggle_range_calendar(index, cx))),
                    )),
            );
        }
        editor = editor.child(bounds);
        if let Some((_, day)) = range.calendar {
            let (pick, step) = (h.clone(), h.clone());
            editor = editor.child(div().flex().justify_center().child(crate::ui::components::calendar::month(
                day,
                theme,
                cx.i18n(),
                Rc::new(move |day, _, cx| pick.update(cx, |this, cx| this.pick_range_date(day, cx))),
                Rc::new(move |months, _, cx| step.update(cx, |this, cx| this.step_range_calendar(months, cx))),
            )));
        }
        if !date {
            use crate::ui::elements::segmented::{segment, segmented, SegmentPlace};
            let units = crate::ui::format::size_units(cx.i18n().language());
            let mut choice = segmented("filter-size-unit", None, theme);
            for unit in 1..=3u32 {
                let pick = h.clone();
                choice = choice.child(
                    segment(("filter-size-unit", unit as usize), range.unit == unit, true, theme)
                        .place(unit as usize - 1, 3)
                        .flex_1()
                        .child(units[unit as usize])
                        .on_click(move |_, _, cx| pick.update(cx, |this, cx| this.set_range_unit(unit, cx))),
                );
            }
            // The menu sets no size of its own: the units take the interface's.
            editor = editor.child(choice.w_full().text_size(rems(0.8125)));
        }
        editor
    }
}

/// Width of `text` at `size`, in the interface's face.
fn text_width(text: &str, size: Rems, weight: FontWeight, window: &Window) -> f32 {
    let font = Font { weight, ..font("Inter") };
    let run = TextRun { len: text.len(), font, color: Hsla::default(), background_color: None, underline: None, strikethrough: None };
    window.text_system().shape_line(text.to_string().into(), rem_pixels(size), &[run], None).width.as_f32()
}

/// Outer width fitting the title with Clear and the keep row, never under the floor of its column.
fn panel_min_width(column: FilterColumn, tab: Option<&TabState>, window: &Window, cx: &Context<OxydeApp>) -> f32 {
    let i18n = cx.i18n();
    let title = text_width(&i18n.text(filter_title_message(column, tab)), rems(0.75), FontWeight::SEMIBOLD, window)
        + text_width(&i18n.text(Message::FilterClear), rems(0.75), FontWeight::NORMAL, window)
        + rem_pixels(rems(2.0)).as_f32();
    let keep = text_width(&i18n.text(Message::FilterKeep), rems(0.8125), FontWeight::NORMAL, window)
        + rem_pixels(rems(2.5)).as_f32();
    // The menu's own padding, border and scrollbar gutter.
    let chrome = rem_pixels(rems(0.75)).as_f32();
    let floor = if matches!(column, FilterColumn::Date | FilterColumn::Deleted) { DATE_PANEL_MIN_WIDTH } else { PANEL_MIN_WIDTH };
    rem_pixels(floor).as_f32().max(title.max(keep) + chrome)
}

/// Whether the column holds a filter, even one keeping every entry: Clear then shows.
fn column_set(tab: &TabState, column: FilterColumn) -> bool {
    match column {
        FilterColumn::Name => !tab.filter_name.is_empty(),
        FilterColumn::Location => !tab.filter_original_location.is_empty(),
        FilterColumn::Type => !tab.filter_types.is_empty(),
        FilterColumn::Size => !tab.filter_sizes.is_empty() || tab.filter_size_range.is_some(),
        FilterColumn::Permissions => !tab.filter_permissions.is_empty(),
        FilterColumn::Date => !tab.filter_dates.is_empty() || tab.filter_date_range.is_some(),
        FilterColumn::Deleted => !tab.filter_deleted.is_empty() || tab.filter_deleted_range.is_some(),
    }
}

/// « Filter by » the column, as the panel's title reads.
fn filter_title_message(column: FilterColumn, tab: Option<&TabState>) -> Message {
    match column {
        FilterColumn::Name => Message::FilterTitleName,
        FilterColumn::Location if tab.is_some_and(|tab| tab.path == "trash://") => Message::FilterTitleOriginalLocation,
        FilterColumn::Location => Message::FilterTitleLocation,
        FilterColumn::Type => Message::FilterTitleType,
        FilterColumn::Size => Message::FilterTitleSize,
        FilterColumn::Permissions => Message::FilterTitlePermissions,
        FilterColumn::Date => Message::FilterTitleModified,
        FilterColumn::Deleted => Message::FilterTitleDeleted,
    }
}

/// « Keep when changing folders », drawn as a menu entry.
fn keep_row(keep: bool, pane_id: String, h: Entity<OxydeApp>, theme: OxydeTheme, cx: &Context<OxydeApp>) -> impl IntoElement {
    let action: crate::ui::components::popup::keyboard::Action = Rc::new(move |_, cx| {
        h.update(cx, |this, cx| this.toggle_keep_filters(&pane_id, cx));
    });
    let click = action.clone();
    let row = div()
        .id("filter-keep")
        .w_full()
        .h(px(crate::ui::style::menu_item_height()))
        .px(rems(0.5))
        .flex()
        .items_center()
        .rounded(rems(0.3125))
        .hover(|style| style.bg(theme.menu_hover))
        .child(div().flex_none().mr(rems(0.5)).child(crate::ui::elements::checkbox::checkbox(theme, keep, false)))
        .child(div().text_size(rems(0.8125)).text_color(theme.text).child(cx.i18n().text(Message::FilterKeep)))
        .on_click(move |_, window, cx| click(window, cx));
    crate::ui::components::popup::keyboard::item(row, action, None, None)
}
