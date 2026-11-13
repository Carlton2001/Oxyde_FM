//! The file chooser window, laid out as the main one: the header, the places beside a
//! pane holding the navigation bar above the entries, in details or as a grid, and
//! below them the footer (`footer.rs`).
use super::{FileChooserWindow, ICON};
use crate::ui::i18n::I18nContext;
use crate::ui::scale::logical;
use crate::ui::theme::{OxydeTheme, ThemeContext};
use gpui::prelude::*;
use gpui::*;

impl FileChooserWindow {
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
            // Anything pressed among the folders and entries counts as looking around.
            .capture_any_mouse_down(cx.listener(|this, _, _, cx| this.note_interaction(cx)))
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
        self.apply_band(cx);
        self.hide_tooltip_for_band(window, cx);
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
                |this, event, cx| if this.resizing_sidebar { this.resize_sidebar(event, cx) }
                    else if !this.move_band(event, cx) { this.move_scrollbar(event, cx) },
                |this, cx| {
                    this.scroll_drag = None;
                    this.end_band(cx);
                    this.end_sidebar_resize(cx);
                    cx.notify();
                }))
            // A menu's thumb dragged over this window: nothing here reacts to the pointer.
            .when(self.dragging_menu(), |root| root.child(deferred(canvas(
                |_, window, _| window.insert_hitbox(Bounds::new(Point::default(), window.viewport_size()), HitboxBehavior::BlockMouse),
                |_, _, _, _| {},
            ).absolute().size_0()).with_priority(100)))
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
                    .child(self.render_sidebar_edge(cx))
                    .child(self.render_pane(theme, cx)),
            )
            .child(self.render_footer(theme, window, cx));
        crate::ui::components::client_decorations::frame(root, theme.border, window, cx)
    }
}
