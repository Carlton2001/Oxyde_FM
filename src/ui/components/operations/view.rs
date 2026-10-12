use crate::ui::app::OxydeApp;
use crate::ui::scale::logical;
use gpui::prelude::*;
use gpui::*;
use super::{panel_width, panel_max_height, render_operations_panel, OperationsPanelView};

impl OperationsPanelView {
    pub fn new(app: Entity<OxydeApp>, cx: &mut Context<Self>) -> Self {
        let position = app.read(cx).operations_panel_position;
        let subscription = cx.observe(&app, |_, _, cx| cx.notify());
        Self {
            app: app.downgrade(),
            _app_subscription: subscription,
            position,
            bounds: None,
            drag_offset: None,
            last_drag_update: None,
        }
    }

    pub(crate) fn update_drag(
        &mut self,
        pointer: Point<Pixels>,
        viewport: Size<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(grab_offset) = self.drag_offset else {
            return;
        };
        let now = std::time::Instant::now();
        if self
            .last_drag_update
            .is_some_and(|last| now.duration_since(last) < std::time::Duration::from_millis(16))
        {
            return;
        }
        let panel_size = self
            .bounds
            .map(|bounds| bounds.size)
            .unwrap_or(size(px(panel_width()), px(panel_max_height())));
        let margin = logical(12.0);
        let max_left = (viewport.width.as_f32() - panel_size.width.as_f32() - margin).max(margin);
        let max_top = (viewport.height.as_f32() - panel_size.height.as_f32() - logical(36.0)).max(margin);
        self.position = Some(point(
            px((pointer.x.as_f32() - grab_offset.x.as_f32()).clamp(margin, max_left)),
            px((pointer.y.as_f32() - grab_offset.y.as_f32()).clamp(margin, max_top)),
        ));
        self.last_drag_update = Some(now);
        cx.notify();
    }

    pub(super) fn update_scrollbar_drag(&self, pointer: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let (viewport_h, content_h, grab_offset, origin, scroll_handle) = {
            let app = app.read(cx);
            let Some(crate::ui::app::DragTarget::OperationsPanelV {
                viewport_h,
                content_h,
                track_id,
            }) = app.drag_target.as_ref()
            else {
                return;
            };
            let Some(origin) = app.scrollbar_origins.get(track_id) else {
                return;
            };
            (
                *viewport_h,
                *content_h,
                app.grab_offset,
                *origin,
                app.operations_panel_scroll.clone(),
            )
        };
        let mouse_pos_in_track = pointer.y.as_f32() - origin.y;
        let thumb_size = (viewport_h * (viewport_h / content_h.max(1.0)).min(1.0))
            .max(logical(40.0))
            .min(viewport_h);
        let travel_distance = (viewport_h - thumb_size).max(1.0);
        let content_range = (content_h - viewport_h).max(0.0);
        let target_thumb_pos = (mouse_pos_in_track - grab_offset).clamp(0.0, travel_distance);
        let target_offset = (target_thumb_pos / travel_distance) * content_range;
        scroll_handle.set_offset(point(px(0.0), px(-target_offset)));
        cx.notify();
    }

    pub(crate) fn finish_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag_offset.take().is_none() {
            return;
        }
        self.last_drag_update = None;
        if let (Some(app), Some(position)) = (self.app.upgrade(), self.position) {
            app.update(cx, |this, cx| {
                this.persist_operations_panel_position(position, cx);
                cx.notify();
            });
        }
        cx.notify();
    }
}

impl Render for OperationsPanelView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(app_handle) = self.app.upgrade() else {
            return div().into_any_element();
        };
        let panel_handle = cx.entity();
        let position = self.position;
        let bounds = self.bounds;
        let is_dragging = self.drag_offset.is_some();
        let viewport = window.viewport_size();
        app_handle.update(cx, |app, cx| {
            render_operations_panel(
                app,
                app_handle.clone(),
                panel_handle,
                position,
                bounds,
                is_dragging,
                viewport,
                cx,
            )
            .into_any_element()
        })
    }
}
