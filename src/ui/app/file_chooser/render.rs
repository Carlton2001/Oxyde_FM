//! The file chooser window, laid out as the main one: the header, the places beside a
//! pane holding the navigation bar above the entries, in details or as a grid, and
//! below them the name to save under, the application's type filters and choices, and
//! the buttons that answer it.
use super::{FileChooserWindow, ICON};
use crate::ui::elements::button::{button_style, ButtonVariant};
use crate::ui::i18n::{I18nContext, Message};
use crate::ui::scale::logical;
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;

impl FileChooserWindow {
    fn render_footer(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let i18n = cx.i18n();
        let default_label = if self.saving() { Message::FileChooserSave }
            else if self.directory { Message::FileChooserSelect }
            else { Message::Open };
        let accept = self.accept_label.clone().unwrap_or_else(|| i18n.text(default_label));
        let enabled = self.can_accept(cx);
        let cancel = crate::ui::components::tool_window::action_button("file-chooser-cancel", cx.i18n().text(Message::Cancel), false, cx)
            .on_click(cx.listener(|this, _, window, cx| this.finish(None, window, cx)));
        let open = button_style(div().id("file-chooser-open"), theme, ButtonVariant::Primary, enabled)
            .flex_none().whitespace_nowrap().flex().items_center().justify_center()
            .px(rems(0.75)).py(rems(0.375)).text_size(rems(0.8125)).rounded(crate::ui::elements::button::ACTION_BUTTON_RADIUS)
            .child(accept)
            .when(enabled, |button| button.tab_index(0).on_click(cx.listener(|this, _, window, cx| this.accept(window, cx))));
        let name = self.render_name_field(theme, cx);
        let named = name.is_some();
        let asked: Vec<AnyElement> = self.render_type_filter(cx).into_iter().chain(self.render_choices(theme, cx)).collect();
        // The name stretches; else a gap keeps the buttons at the end.
        let spaced = !named && !asked.is_empty();
        crate::ui::components::tool_window::footer(cx)
            .children(name)
            .children(asked)
            .when(spaced, |footer| footer.child(div().flex_1()))
            .child(cancel)
            .child(open)
    }

    /// The scroll handle a dragged thumb moves, from its track's name.
    fn dragged_scroll(&self, track: &str) -> ScrollHandle {
        if track.ends_with("file-chooser-sidebar") {
            self.sidebar_scroll.clone()
        } else if track.ends_with("file-chooser-grid") {
            self.grid_scroll.clone()
        } else {
            self.list.0.borrow().base_handle.clone()
        }
    }

    /// A thumb's drag, as the main window's `FileListV` drag moves its list.
    fn move_scrollbar(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some((track, viewport_h, content_h)) = self.scroll_drag.clone() else { return };
        if !event.dragging() {
            self.scroll_drag = None;
            cx.notify();
            return;
        }
        let Some(origin) = self.scrollbar_origins.get(&track) else { return };
        let mouse_pos_in_track = event.position.y.as_f32() - origin.y;
        let thumb_size = (viewport_h * (viewport_h / content_h.max(1.0)).min(1.0)).max(logical(40.0));
        let travel_distance = (viewport_h - thumb_size).max(1.0);
        let content_range = content_h - viewport_h;
        let target_thumb_pos = (mouse_pos_in_track - self.grab_offset).clamp(0.0, travel_distance);
        let target_px = (target_thumb_pos / travel_distance) * content_range;
        self.dragged_scroll(&track).set_offset(point(px(0.0), px(-target_px)));
        cx.notify();
    }

    fn render_pane(&self, theme: OxydeTheme, cx: &mut Context<Self>) -> impl IntoElement {
        let entity_id = cx.entity().entity_id();
        let measured = self.pane_width.clone();
        div()
            .id("file-chooser-pane")
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            // A new width lays the columns and cards out again.
            .child(canvas(move |bounds, _, cx| {
                let width = bounds.size.width.as_f32();
                if (measured.get() - width).abs() > 0.5 {
                    measured.set(width);
                    cx.notify(entity_id);
                }
            }, |_, _, _, _| {}).absolute().size_full())
            .child(self.render_navbar(theme, cx))
            .child(if self.options.grid { self.render_grid(theme, cx) } else { self.render_details(theme, cx) })
    }
}

impl Render for FileChooserWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let title = self.title.clone().unwrap_or_else(|| cx.i18n().text(super::default_title(&self.purpose, self.directory)));
        window.set_window_title(title.as_ref());
        self.measure_columns(window, cx);
        self.window_width = window.viewport_size().width.as_f32() / logical(1.0);
        // Another folder: its entries' tooltips go, as the main window's do with its path.
        if self.tooltip_folder != self.folder {
            self.tooltip_folder = self.folder.clone();
            crate::ui::components::tooltip::hover::dismiss(window, cx);
        }
        let closing = cx.entity().downgrade();
        // The header's own close button: the user cancels.
        let header = crate::ui::components::tool_window::header(ICON, title, window, true, cx,
            move |window, cx| { let _ = closing.update(cx, |this, cx| this.finish(None, window, cx)); });
        let root = div()
            .id("file-chooser")
            .child(crate::ui::components::tool_window::pointer_drag(cx.entity(),
                |this, event, cx| if this.resizing_sidebar { this.resize_sidebar(event, cx) } else { this.move_scrollbar(event, cx) },
                |this, cx| {
                    this.scroll_drag = None;
                    this.end_sidebar_resize(cx);
                    cx.notify();
                }))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| this.key_down(event, window, cx)))
            // The mouse's side buttons move through the history, as in the main window.
            .on_mouse_down(MouseButton::Navigate(NavigationDirection::Back), cx.listener(|this, _, _, cx| this.go_back(cx)))
            .on_mouse_down(MouseButton::Navigate(NavigationDirection::Forward), cx.listener(|this, _, _, cx| this.go_forward(cx)))
            .size_full()
            .bg(theme.background)
            .text_color(theme.text)
            .font_family("Inter")
            .flex()
            .flex_col()
            .text_size(rems(0.8125))
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .child(header)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .child(self.render_sidebar(theme, cx))
                    .child(self.render_sidebar_edge(theme, cx))
                    .child(self.render_pane(theme, cx)),
            )
            .child(self.render_footer(theme, cx));
        crate::ui::components::client_decorations::frame(root, theme.border, window)
    }
}
