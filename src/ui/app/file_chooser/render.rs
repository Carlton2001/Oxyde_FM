//! The file chooser window: the toolbar above, places beside the folder's entries, in
//! details or as a grid of cards, and below them what the folder holds and the
//! buttons that answer the application.
use super::arrange::SortKey;
use super::{FileChooserWindow, Listing, Place, ICON};
use crate::ui::elements::button::{button_style, ButtonVariant};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::logical;
use crate::ui::theme::{OxydeTheme, ThemeContext};
use fluent_bundle::FluentArgs;
use gpui::prelude::*;
use gpui::*;

const TYPE_WIDTH: Rems = rems(8.0);
const SIZE_WIDTH: Rems = rems(6.5);
const DATE_WIDTH: Rems = rems(9.5);
/// The list's padding beside the cards, and the scrollbar's room.
const GRID_PADDING: Rems = rems(1.25);

impl FileChooserWindow {
    fn render_place(&self, id: (&'static str, usize), place: &Place, theme: OxydeTheme, cx: &mut Context<Self>) -> Stateful<Div> {
        let current = place.path == self.folder;
        let path = place.path.clone();
        div().id(id).h(rems(1.875)).px(rems(0.625)).flex_none().relative()
            .flex().items_center().gap(rems(0.5)).rounded(rems(0.375))
            .bg(if current { theme.selected } else { theme.sidebar })
            .when(!current, |row| row.hover(|row| row.bg(theme.hover)))
            .child(svg().path(place.icon).size(rems(1.0)).flex_none().text_color(theme.text))
            .child(div().flex_1().min_w_0().truncate().child(place.label.clone()))
            .on_click(cx.listener(move |this, _, _, cx| this.show_folder(path.clone(), cx)))
    }

    fn render_places(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut places = div().id("file-chooser-places").size_full().overflow_y_scroll()
            .p(rems(0.5)).flex().flex_col().gap(rems(0.125));
        for (index, place) in self.places.iter().enumerate() {
            places = places.child(self.render_place(("file-chooser-place", index), place, theme, cx));
        }
        if !self.volumes.is_empty() {
            // The main sidebar's divider between quick access and volumes.
            places = places.child(div().flex_none().h(rems(0.75)).px(rems(0.625)).flex().items_center()
                .child(div().flex_1().h(rems(0.0625)).bg(theme.border)));
        }
        for (index, volume) in self.volumes.iter().enumerate() {
            places = places.child(self.render_place(("file-chooser-volume", index), volume, theme, cx));
        }
        places
    }

    fn render_sort_header(&self, key: SortKey, message: Message, width: Option<Rems>, theme: OxydeTheme, cx: &mut Context<Self>) -> Stateful<Div> {
        let sorted = self.options.sort == key;
        let chevron = if self.options.descending { "icons/chevron-down.svg" } else { "icons/chevron-up.svg" };
        div().id(("file-chooser-sort", key as usize)).h_full().flex().items_center().gap(rems(0.25))
            .map(|cell| match width {
                Some(width) => cell.w(width).flex_none(),
                None => cell.flex_1().min_w_0().pl(rems(1.75)),
            })
            .when(key == SortKey::Size, |cell| cell.justify_end())
            .text_color(if sorted { theme.text } else { theme.text_muted })
            .hover(|cell| cell.text_color(theme.text))
            .child(div().min_w_0().truncate().child(cx.i18n().text(message)))
            .when(sorted, |cell| cell.child(svg().path(chevron).size(rems(0.75)).flex_none().text_color(theme.text)))
            .on_click(cx.listener(move |this, _, _, cx| this.sort_by(key, cx)))
    }

    fn render_columns(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex_none().h(rems(1.75)).pl(rems(0.75)).pr(rems(1.25)).flex().items_center().gap(rems(0.75))
            .border_b(rems(0.0625)).border_color(theme.border).text_size(rems(0.75))
            .child(self.render_sort_header(SortKey::Name, Message::ColumnName, None, theme, cx))
            .child(self.render_sort_header(SortKey::Type, Message::ColumnType, Some(TYPE_WIDTH), theme, cx))
            .child(self.render_sort_header(SortKey::Size, Message::ColumnSize, Some(SIZE_WIDTH), theme, cx))
            .child(self.render_sort_header(SortKey::Modified, Message::ColumnModified, Some(DATE_WIDTH), theme, cx))
    }

    /// Selects on press, and opens on a double press.
    fn press(position: usize, cx: &mut Context<Self>) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
        let this = cx.entity().downgrade();
        move |event, window, cx| {
            let _ = this.update(cx, |this, cx| {
                window.focus(&this.focus, cx);
                this.select(position, cx);
                if event.click_count == 2 {
                    this.open_entry(position, window, cx);
                }
            });
        }
    }

    fn render_row(&self, position: usize, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let Some(entry) = self.shown_entry(position) else { return div().into_any_element() };
        let selected = self.selected == Some(position);
        let i18n = cx.i18n();
        let size = if entry.is_dir { String::new() } else { crate::ui::format::format_size(entry.size, false, true, i18n.language()) };
        let kind = crate::ui::components::file_list::file_type_display(entry, i18n);
        let date = crate::ui::format::format_date(entry.modified, crate::ui::format::date_format(cx));
        div().id(("file-chooser-entry", position)).relative().h(px(crate::ui::style::row_height())).w_full()
            .pl(rems(0.75)).pr(rems(1.25)).flex().items_center().gap(rems(0.75))
            .bg(if selected { theme.selected } else { theme.active_background })
            .when(!selected, |row| row.hover(|row| row.bg(theme.hover)))
            .when(selected, |row| row.child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
            .when(entry.is_hidden || entry.is_system, |row| row.opacity(0.6))
            .on_mouse_down(MouseButton::Left, Self::press(position, cx))
            .child(crate::ui::components::icons::render_file_icon(entry.path.clone(),
                crate::ui::components::icons::entry_icon(entry), rems(1.0), &theme, self.system_icons, false))
            .child(div().flex_1().min_w_0().truncate().child(entry.name.clone()))
            .child(div().w(TYPE_WIDTH).flex_none().truncate().text_color(theme.text_muted).child(kind))
            .child(div().w(SIZE_WIDTH).flex_none().flex().justify_end().text_color(theme.text_muted).child(size))
            .child(div().w(DATE_WIDTH).flex_none().truncate().text_color(theme.text_muted).child(date))
            .into_any_element()
    }

    fn render_card(&self, position: usize, width: f32, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let Some(entry) = self.shown_entry(position) else { return div().into_any_element() };
        let selected = self.selected == Some(position);
        let icon = crate::ui::components::icons::entry_icon(entry);
        let picture = if self.thumbnails && !entry.is_dir && crate::ui::components::file_list::supports_thumbnail(entry) {
            let pane = format!("file-chooser-{}", cx.entity().entity_id());
            crate::ui::components::file_list::render_grid_thumbnail(entry, &pane, icon, theme, self.system_icons).into_any_element()
        } else {
            crate::ui::components::icons::render_file_icon(entry.path.clone(), icon, rems(4.0), &theme, self.system_icons, true)
        };
        div().id(("file-chooser-card", position)).flex_none()
            .w(px(width - logical(4.0))).h(px(crate::ui::style::grid_card_row_height() - logical(4.0))).m(px(logical(2.0)))
            .p(rems(0.25)).flex().flex_col().items_center().justify_center().gap(rems(0.125))
            .rounded_md().border(rems(0.0625))
            .border_color(if selected { theme.accent } else { theme.border.opacity(0.0) })
            .bg(if selected { theme.selected } else { theme.active_background })
            .when(!selected, |card| card.hover(|card| card.bg(theme.hover)))
            .on_mouse_down(MouseButton::Left, Self::press(position, cx))
            .child(div().w_full().h(rems(4.25)).flex_none().flex().items_center().justify_center()
                .when(entry.is_hidden || entry.is_system, |area| area.opacity(0.5))
                .child(picture))
            .child(div().w_full().px(rems(0.25)).text_center().text_size(rems(0.75)).line_clamp(2)
                .when(entry.is_hidden || entry.is_system, |name| name.opacity(0.5))
                .child(entry.name.clone()))
            .into_any_element()
    }

    fn render_grid_row(&self, row: usize, width: f32, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let columns = self.columns.max(1);
        let start = row * columns;
        let end = (start + columns).min(self.shown.len());
        let mut line = div().id(("file-chooser-row", row)).w_full().px(GRID_PADDING / 2.0).flex();
        for position in start..end {
            line = line.child(self.render_card(position, width, theme, cx));
        }
        line.into_any_element()
    }

    fn render_list(&mut self, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let message = match self.listing {
            Listing::Loading => Some(None),
            Listing::Failed => Some(Some(Message::FileChooserUnreadable)),
            Listing::Ready if self.shown.is_empty() && !self.entries.is_empty() => Some(Some(Message::OpsNoMatchingItems)),
            Listing::Ready if self.shown.is_empty() => Some(Some(Message::FolderEmpty)),
            Listing::Ready => None,
        };
        if let Some(message) = message {
            return div().flex_1().min_h_0().flex().items_center().justify_center().italic()
                .text_color(theme.text_muted)
                .when_some(message, |area, message| area.child(cx.i18n().text(message)))
                .into_any_element();
        }
        // Cards fill the width the list had when last laid out.
        let width = self.grid_width.get();
        let inner = (width - crate::ui::scale::rem_pixels(GRID_PADDING).as_f32()).max(0.0);
        self.columns = ((inner / crate::ui::style::grid_card_min_width()).floor() as usize).max(1);
        let card = inner / self.columns as f32;
        let grid = self.options.grid;
        let source = cx.entity();
        let entity_id = source.entity_id();
        let measured = self.grid_width.clone();
        let viewport = self.scroll.bounds().size.height.as_f32();
        let content = viewport + self.scroll.max_offset().y.as_f32();
        let scrollbar = crate::ui::components::scrollbar::render_scrollbar_with_pointer(
            theme, cx.entity(), crate::ui::components::scrollbar::ScrollbarAxis::Vertical,
            viewport, content, -self.scroll.offset().y.as_f32(), self.scroll_drag.is_some(),
            "file-chooser-list".into(), None,
            move |this: &mut Self, track, viewport, content, _, cx| {
                this.scroll_drag = Some((track, viewport, content));
                cx.notify(entity_id);
            },
        );
        div().relative().flex_1().min_h_0().flex()
            // A new width lays the cards out again.
            .child(canvas(move |bounds, _, cx| {
                let width = bounds.size.width.as_f32();
                if (measured.get() - width).abs() > 0.5 {
                    measured.set(width);
                    cx.notify(entity_id);
                }
            }, |_, _, _, _| {}).absolute().inset_0())
            .child(uniform_list("file-chooser-entries", self.rows(), move |range, _, cx| {
                source.update(cx, |this, cx| range.map(|row| if grid {
                    this.render_grid_row(row, card, theme, cx)
                } else {
                    this.render_row(row, theme, cx)
                }).collect::<Vec<_>>())
            }).flex_1().min_h_0().track_scroll(&self.list))
            .child(scrollbar)
            .into_any_element()
    }

    /// What the folder shows, and the size of the file selected.
    fn status(&self, cx: &App) -> SharedString {
        let language = cx.i18n().language();
        let folders = self.shown.iter().filter(|&&index| self.entries[index].is_dir).count();
        let label = |count: usize, message| {
            let mut args = FluentArgs::new();
            args.set("count", count);
            args.set("number", crate::ui::format::format_number(count as u64, language));
            cx.i18n().text_with_args(message, &args)
        };
        let mut parts = vec![label(folders, Message::StatusFolders), label(self.shown.len() - folders, Message::StatusFiles)];
        if let Some(entry) = self.selected.and_then(|position| self.shown_entry(position)).filter(|entry| !entry.is_dir) {
            parts.push(format!("{} — {}", entry.name, crate::ui::format::format_size(entry.size, false, true, language)).into());
        }
        parts.join(" · ").into()
    }

    fn render_footer(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.i18n();
        let accept = self.accept_label.clone().unwrap_or_else(|| i18n.text(Message::Open));
        let enabled = self.selected.is_some();
        let status = (self.listing == Listing::Ready).then(|| self.status(cx));
        let cancel = crate::ui::components::tool_window::action_button("file-chooser-cancel", cx.i18n().text(Message::Cancel), false, cx)
            .on_click(cx.listener(|this, _, window, cx| this.finish(None, window, cx)));
        let open = button_style(div().id("file-chooser-open"), theme, ButtonVariant::Primary, enabled)
            .flex_none().whitespace_nowrap().px(rems(0.75)).py(rems(0.375)).text_size(rems(0.8125)).rounded(rems(0.375))
            .child(accept)
            .when(enabled, |button| button.tab_index(0).on_click(cx.listener(|this, _, window, cx| this.accept(window, cx))));
        crate::ui::components::tool_window::footer(cx)
            .child(div().flex_1().min_w_0().truncate().text_size(rems(0.75)).text_color(theme.text_muted)
                .when_some(status, |line, status| line.child(status)))
            .child(cancel)
            .child(open)
    }

    pub(super) fn move_scrollbar(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some((track, viewport, content)) = self.scroll_drag.clone() else { return };
        if !event.dragging() {
            self.scroll_drag = None;
            cx.notify();
            return;
        }
        let Some(origin) = self.scrollbar_origins.get(&track) else { return };
        let thumb = (viewport * (viewport / content.max(1.0)).min(1.0))
            .max(crate::ui::scale::rem_pixels(rems(2.5)).as_f32());
        let travel = (viewport - thumb).max(1.0);
        let position = (event.position.y.as_f32() - origin.y - self.grab_offset).clamp(0.0, travel);
        self.scroll.set_offset(point(px(0.0), px(-position / travel * (content - viewport).max(0.0))));
        cx.notify();
    }
}

impl Render for FileChooserWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let title = self.title.clone().unwrap_or_else(|| cx.i18n().text(Message::FileChooserTitle));
        window.set_window_title(title.as_ref());
        let closing = cx.entity().downgrade();
        // The header's own close button: the user cancels.
        let header = crate::ui::components::tool_window::header(ICON, title, window, true, cx,
            move |window, cx| { let _ = closing.update(cx, |this, cx| this.finish(None, window, cx)); });
        let list = self.render_list(theme, cx);
        let root = div()
            .id("file-chooser")
            .child(crate::ui::components::tool_window::pointer_drag(cx.entity(),
                |this, event, cx| this.move_scrollbar(event, cx),
                |this, cx| { this.scroll_drag = None; cx.notify(); }))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| this.key_down(event, window, cx)))
            .size_full().flex().flex_col()
            .bg(theme.active_background).text_color(theme.text)
            .font_family("Inter").font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .text_size(rems(0.8125)).line_height(relative(1.25))
            .child(header)
            .child(self.render_toolbar(theme, window, cx))
            .child(div().flex_1().min_h_0().flex()
                .child(div().w(rems(13.0)).flex_none().min_h_0().bg(theme.sidebar)
                    .border_r(rems(0.0625)).border_color(theme.border)
                    .child(self.render_places(theme, cx)))
                .child(div().flex_1().min_w_0().flex().flex_col()
                    // Also the grid's sort.
                    .child(self.render_columns(theme, cx))
                    .child(list)))
            .child(self.render_footer(theme, cx));
        crate::ui::components::client_decorations::frame(root, theme.border, window)
    }
}
