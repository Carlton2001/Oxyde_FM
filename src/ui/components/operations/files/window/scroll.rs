use super::FilesWindow;
use crate::ui::components::scrollbar::{
    render_scrollbar_with_pointer, ScrollbarAxis, ScrollbarState,
};
use crate::ui::scale::rem_pixels;
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

impl ScrollbarState for FilesWindow {
    fn scrollbar_origins(&mut self) -> &mut std::collections::HashMap<String, Point<f32>> {
        &mut self.scrollbar_origins
    }
    fn set_grab_offset(&mut self, offset: f32) {
        self.grab_offset = offset;
    }
}

impl FilesWindow {
    pub(super) fn render_scrollbar(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.filtering {
            return div().into_any_element();
        }
        let scroll = self.scroll.0.borrow().base_handle.clone();
        let viewport = scroll.bounds().size.height.as_f32();
        let content = viewport + scroll.max_offset().y.as_f32();
        let entity_id = cx.entity().entity_id();
        render_scrollbar_with_pointer(
            *cx.theme(),
            cx.entity(),
            ScrollbarAxis::Vertical,
            viewport,
            content,
            -scroll.offset().y.as_f32(),
            self.drag.is_some(),
            "operation-items".into(),
            None,
            move |this, track, viewport, content, position, cx| {
                this.drag = Some((track, viewport, content));
                this.move_scrollbar_to(position.y.as_f32());
                cx.notify(entity_id);
            },
        )
    }

    fn move_scrollbar_to(&mut self, y: f32) {
        let Some((track, viewport, content)) = &self.drag else {
            return;
        };
        let Some(origin) = self.scrollbar_origins.get(track) else {
            return;
        };
        let thumb =
            (viewport * (viewport / content.max(1.0)).min(1.0)).max(rem_pixels(rems(2.5)).as_f32());
        let travel = (viewport - thumb).max(1.0);
        let position = (y - origin.y - self.grab_offset).clamp(0.0, travel);
        self.scroll.0.borrow().base_handle.set_offset(point(
            px(0.0),
            px(-position / travel * (content - viewport).max(0.0)),
        ));
    }

    pub(super) fn move_scrollbar(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.drag.is_none() {
            return;
        }
        if event.dragging() {
            self.move_scrollbar_to(event.position.y.as_f32());
        } else {
            self.drag = None;
        }
        cx.notify();
    }
}
