//! The file chooser window: places beside the folder's list, the folder above it, and
//! the buttons that answer the application below.
use super::{FileChooserWindow, Listing, ICON};
use crate::ui::components::tooltip::{NativeTooltipExt, Tooltip};
use crate::ui::elements::button::{button_style, ButtonVariant};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;

const SIZE_WIDTH: Rems = rems(6.5);
const DATE_WIDTH: Rems = rems(9.5);

impl FileChooserWindow {
    fn render_places(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let mut places = div().id("file-chooser-places").size_full().overflow_y_scroll()
            .p(rems(0.5)).flex().flex_col().gap(rems(0.125));
        for (index, place) in self.places.iter().enumerate() {
            let current = place.path == self.folder;
            let path = place.path.clone();
            places = places.child(
                div().id(("file-chooser-place", index)).h(rems(1.875)).px(rems(0.625)).flex_none()
                    .flex().items_center().gap(rems(0.5)).rounded(rems(0.375))
                    .bg(if current { theme.selected } else { theme.sidebar })
                    .when(!current, |row| row.hover(|row| row.bg(theme.hover)))
                    .child(svg().path(place.icon).size(rems(1.0)).flex_none().text_color(theme.text))
                    .child(div().flex_1().min_w_0().truncate().child(place.label.clone()))
                    .on_click(cx.listener(move |this, _, _, cx| this.navigate(path.clone(), cx))),
            );
        }
        places
    }

    fn render_location(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let parent = self.parent().is_some();
        let up = button_style(div().id("file-chooser-up"), theme, ButtonVariant::Ghost, parent)
            .size(rems(1.875)).flex_none().flex().items_center().justify_center().rounded(rems(0.375))
            .native_tooltip(Tooltip::text(cx.i18n().text(Message::Up)))
            .child(svg().path("icons/arrow-up.svg").size(rems(1.0))
                .text_color(if parent { theme.text } else { theme.icon_disabled }))
            .when(parent, |button| button.on_click(cx.listener(|this, _, _, cx| this.go_up(cx))));
        div().flex_none().h(rems(2.75)).px(rems(0.75)).flex().items_center().gap(rems(0.5))
            .border_b(rems(0.0625)).border_color(theme.border).bg(theme.header)
            .child(up)
            .child(div().flex_1().min_w_0().truncate().child(oxyde_lib::utils::path_text::display(&self.folder)))
    }

    fn render_columns(&self, theme: OxydeTheme, cx: &App) -> impl IntoElement {
        let i18n = cx.i18n();
        div().flex_none().h(rems(1.75)).pl(rems(0.75)).pr(rems(1.25)).flex().items_center().gap(rems(0.75))
            .border_b(rems(0.0625)).border_color(theme.border)
            .text_size(rems(0.75)).text_color(theme.text_muted)
            .child(div().flex_1().min_w_0().pl(rems(1.75)).truncate().child(i18n.text(Message::ColumnName)))
            .child(div().w(SIZE_WIDTH).flex_none().flex().justify_end().child(i18n.text(Message::ColumnSize)))
            .child(div().w(DATE_WIDTH).flex_none().truncate().child(i18n.text(Message::ColumnModified)))
    }

    fn render_row(&self, index: usize, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let entry = &self.entries[index];
        let selected = self.selected == Some(index);
        let language = cx.i18n().language();
        let size = crate::ui::format::format_size(entry.size, entry.is_dir, !entry.is_dir, language);
        let date = crate::ui::format::format_date(entry.modified, crate::ui::format::date_format(cx));
        div().id(("file-chooser-entry", index)).relative().h(px(crate::ui::style::row_height())).w_full()
            .pl(rems(0.75)).pr(rems(1.25)).flex().items_center().gap(rems(0.75))
            .bg(if selected { theme.selected } else { theme.active_background })
            .when(!selected, |row| row.hover(|row| row.bg(theme.hover)))
            .when(selected, |row| row.child(div().absolute().left_0().top_0().bottom_0().w(rems(0.1875)).bg(theme.accent)))
            .when(entry.is_hidden || entry.is_system, |row| row.opacity(0.6))
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                this.select(index, cx);
                if event.click_count == 2 {
                    this.open_entry(index, window, cx);
                }
            }))
            .child(crate::ui::components::icons::render_file_icon(entry.path.clone(),
                crate::ui::components::icons::entry_icon(entry), rems(1.0), &theme, self.system_icons, false))
            .child(div().flex_1().min_w_0().truncate().child(entry.name.clone()))
            .child(div().w(SIZE_WIDTH).flex_none().flex().justify_end().text_color(theme.text_muted).child(size))
            .child(div().w(DATE_WIDTH).flex_none().truncate().text_color(theme.text_muted).child(date))
            .into_any_element()
    }

    fn render_list(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> AnyElement {
        let message = match self.listing {
            Listing::Loading => Some(None),
            Listing::Failed => Some(Some(Message::FileChooserUnreadable)),
            Listing::Ready if self.entries.is_empty() => Some(Some(Message::FolderEmpty)),
            Listing::Ready => None,
        };
        if let Some(message) = message {
            return div().flex_1().min_h_0().flex().items_center().justify_center().italic()
                .text_color(theme.text_muted)
                .when_some(message, |area, message| area.child(cx.i18n().text(message)))
                .into_any_element();
        }
        let source = cx.entity();
        let viewport = self.scroll.bounds().size.height.as_f32();
        let content = viewport + self.scroll.max_offset().y.as_f32();
        let entity_id = cx.entity().entity_id();
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
            .child(uniform_list("file-chooser-entries", self.entries.len(), move |range, _, cx| {
                source.update(cx, |this, cx| range.map(|index| this.render_row(index, theme, cx)).collect::<Vec<_>>())
            }).flex_1().min_h_0().track_scroll(&self.list))
            .child(scrollbar)
            .into_any_element()
    }

    fn render_footer(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.i18n();
        let accept = self.accept_label.clone().unwrap_or_else(|| i18n.text(Message::Open));
        let enabled = self.selected.is_some();
        let cancel = crate::ui::components::tool_window::action_button("file-chooser-cancel", i18n.text(Message::Cancel), false, cx)
            .on_click(cx.listener(|this, _, window, cx| this.finish(None, window, cx)));
        let open = button_style(div().id("file-chooser-open"), theme, ButtonVariant::Primary, enabled)
            .flex_none().whitespace_nowrap().px(rems(0.75)).py(rems(0.375)).text_size(rems(0.8125)).rounded(rems(0.375))
            .child(accept)
            .when(enabled, |button| button.tab_index(0).on_click(cx.listener(|this, _, window, cx| this.accept(window, cx))));
        crate::ui::components::tool_window::footer(cx).child(cancel).child(open)
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
            .child(div().flex_1().min_h_0().flex()
                .child(div().w(rems(13.0)).flex_none().min_h_0().bg(theme.sidebar)
                    .border_r(rems(0.0625)).border_color(theme.border)
                    .child(self.render_places(theme, cx)))
                .child(div().flex_1().min_w_0().flex().flex_col()
                    .child(self.render_location(theme, cx))
                    .child(self.render_columns(theme, cx))
                    .child(self.render_list(theme, cx))))
            .child(self.render_footer(theme, cx));
        crate::ui::components::client_decorations::frame(root, theme.border, window)
    }
}
