//! The file chooser's scrollbars, as the main window drives its own (`drag.rs`,
//! `drag/popup.rs`, `file_list/scroll_top.rs`): a thumb dragged in the list, the grid,
//! the places, the footer's form or a menu, the menus' thumbs followed from their own windows, and the
//! button that brings the entries back to their start.
use super::FileChooserWindow;
use crate::ui::components::scrollbar::{drag_scrolled, render_scrollbar_with_pointer, ScrollbarAxis};
use crate::ui::theme::OxydeTheme;
use gpui::prelude::*;
use gpui::*;

/// The thumb being dragged and what it scrolls.
pub(super) struct ScrollDrag {
    track: String,
    viewport: f32,
    content: f32,
    scroll: ScrollHandle,
}

/// The tracks' names, which tell what a drag scrolls.
pub(super) const DETAILS: &str = "file-chooser-details";
pub(super) const GRID: &str = "file-chooser-grid";
pub(super) const SIDEBAR: &str = "file-chooser-sidebar";
pub(super) const FORM: &str = "file-chooser-form";

/// The thumb of a menu's list: a crumb's or the footer's, one open at a time.
fn is_menu_track(track: &str) -> bool {
    track.contains("context-menu-scrollbar")
}

impl FileChooserWindow {
    /// Whether the thumb of the track named `name` is held.
    pub(super) fn dragging_track(&self, name: &str) -> bool {
        self.scroll_drag.as_ref().is_some_and(|drag| drag.track.ends_with(name))
    }

    /// Whether a menu's thumb is held, which its own window does not see go.
    pub(super) fn dragging_menu(&self) -> bool {
        self.scroll_drag.as_ref().is_some_and(|drag| is_menu_track(&drag.track))
    }

    /// The vertical scrollbar of `scroll`, named `name`.
    pub(super) fn render_vertical_scrollbar(&self, theme: OxydeTheme, name: &'static str, scroll: ScrollHandle, viewport: f32, content: f32, cx: &mut Context<Self>) -> AnyElement {
        let entity_id = cx.entity().entity_id();
        let offset = -scroll.offset().y.as_f32();
        render_scrollbar_with_pointer(theme, cx.entity(), ScrollbarAxis::Vertical, viewport, content, offset, self.dragging_track(name),
            name.into(), None, scroll.clone(),
            move |this: &mut Self, track, viewport, content, _, cx| {
                this.scroll_drag = Some(ScrollDrag { track, viewport, content, scroll: scroll.clone() });
                cx.notify(entity_id);
            })
    }

    /// What a menu's thumb starts: the drag of `scroll`, its list.
    pub(super) fn start_menu_drag(&mut self, track: String, viewport: f32, content: f32, scroll: ScrollHandle) {
        self.scroll_drag = Some(ScrollDrag { track, viewport, content, scroll });
    }

    /// A thumb's drag, as the main window's `FileListV` drag moves its list.
    pub(super) fn move_scrollbar(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(drag) = &self.scroll_drag else { return };
        if !event.dragging() {
            self.scroll_drag = None;
            cx.notify();
            return;
        }
        let Some(origin) = self.scrollbar_origins.get(&drag.track) else { return };
        let scrolled = drag_scrolled(drag.viewport, drag.content, drag.viewport, event.position.y.as_f32() - origin.y, self.grab_offset);
        drag.scroll.set_offset(point(px(0.0), px(-scrolled)));
        cx.notify();
    }

    /// The pointer over a menu's own window while its thumb is held.
    pub(crate) fn move_popup_scroll(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.dragging_menu() {
            self.move_scrollbar(event, cx);
        }
    }

    pub(crate) fn release_popup_scroll(&mut self, event: &MouseUpEvent, cx: &mut Context<Self>) {
        if event.button == MouseButton::Left && self.dragging_menu() {
            self.scroll_drag = None;
            cx.notify();
        }
    }

    /// The zone beside the entries' scrollbar, where the button back to the start shows.
    pub(super) fn scroll_zone(&self, zone: Div, cx: &mut Context<Self>) -> Stateful<Div> {
        let this = cx.entity().downgrade();
        crate::ui::components::scrollbar::scroll_top::hover_zone(zone, format!("file-chooser-scroll-zone-{}", cx.entity().entity_id()),
            move |hovered, _, cx| {
                let _ = this.update(cx, |this, cx| {
                    if this.scroll_zone_hover != *hovered {
                        this.scroll_zone_hover = *hovered;
                        cx.notify();
                    }
                });
            })
    }

    /// The main window's button back to the entries' start, over `scroll`'s rail.
    pub(super) fn scroll_top_button(&self, theme: OxydeTheme, scroll: ScrollHandle, viewport: f32, content: f32, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let offset = -scroll.offset().y.as_f32();
        crate::ui::components::scrollbar::scroll_top::button(theme, format!("file-chooser-{}", cx.entity().entity_id()),
            self.scroll_zone_hover, &self.scroll_top_anchor, offset, viewport, content,
            move |_, _, cx| {
                scroll.set_offset(point(px(0.0), px(0.0)));
                let _ = this.update(cx, |_, cx| cx.notify());
            })
    }
}
