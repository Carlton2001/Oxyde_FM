//! Shared scrollbar skin for the Viewer's virtualized lists. Wrapped text rows
//! have one fixed height and documents/PDF pages are measured up front, so the
//! rail keeps its size.
use super::{content::Content, Viewer};
use crate::ui::components::scrollbar::{
    render_scrollbar_with_pointer, ScrollbarAxis, ScrollbarState, ZED_TRACK_WIDTH,
};
use crate::ui::theme::ThemeContext;
use gpui::{prelude::*, *};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct ListScroll {
    origins: HashMap<String, Point<f32>>,
    grab: f32,
    /// Axis, track, viewport and content sizes captured when dragging starts.
    drag: Option<(ScrollbarAxis, String, f32, f32)>,
    hovered: bool,
    top_anchor: std::cell::Cell<f32>,
}

impl ScrollbarState for Viewer {
    fn scrollbar_origins(&mut self) -> &mut HashMap<String, Point<f32>> {
        &mut self.scroll.origins
    }
    fn set_grab_offset(&mut self, offset: f32) {
        self.scroll.grab = offset;
    }
}

fn thumb_offset(viewport: f32, content: f32, position: f32, grab: f32) -> f32 {
    let thumb = (viewport * viewport / content.max(1.0))
        .max(crate::ui::scale::rem_pixels(rems(2.5)).as_f32())
        .min(viewport);
    let travel = (viewport - thumb).max(1.0);
    ((position - grab) / travel).clamp(0.0, 1.0) * (content - viewport).max(0.0)
}

impl Viewer {
    pub(super) fn list_scroll_maximum(&self) -> f32 {
        let Some(list) = self.list_state() else { return 0.0 };
        let maximum = list.max_offset_for_scrollbar().y.as_f32();
        // GPUI's scrollbar maximum excludes list padding. Every Viewer list
        // has one rem at each end, also reachable when dragging to the bottom.
        if maximum > 0.0 {
            maximum + crate::ui::scale::rem_pixels(rems(2.0)).as_f32()
        } else {
            maximum
        }
    }

    fn list_state(&self) -> Option<&ListState> {
        match &self.content {
            Content::Text(text) => Some(&text.list),
            Content::Markdown(document) => Some(&document.list),
            Content::Document(_, list) => Some(list),
            Content::Pdf(pdf) => Some(&pdf.list),
            _ => None,
        }
    }

    fn drag_to(&mut self, position: Point<Pixels>) {
        let Some((axis, track, viewport, content)) = self.scroll.drag.clone() else { return };
        let Some(origin) = self.scroll.origins.get(&track).copied() else { return };
        match axis {
            ScrollbarAxis::Vertical => {
                let offset = thumb_offset(viewport, content, position.y.as_f32() - origin.y, self.scroll.grab);
                if let Some(list) = self.list_state() {
                    list.set_offset_from_scrollbar(point(px(0.0), px(-offset)));
                }
            }
            ScrollbarAxis::Horizontal => {}
        }
    }

    fn end_drag(&mut self) -> bool {
        let ended = self.scroll.drag.take();
        if let Some((ScrollbarAxis::Vertical, ..)) = ended {
            if let Some(list) = self.list_state() {
                list.scrollbar_drag_ended();
            }
        }
        ended.is_some()
    }

    fn rail(
        &self,
        axis: ScrollbarAxis,
        viewport: f32,
        content: f32,
        scrolled: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let handle = cx.entity();
        let id = handle.entity_id();
        let dragging = self.scroll.drag.as_ref().is_some_and(|drag| drag.0 == axis);
        render_scrollbar_with_pointer(
            *cx.theme(),
            handle.clone(),
            axis,
            viewport,
            content,
            scrolled,
            dragging,
            format!("viewer-{:?}", handle.entity_id()),
            None,
            move |this, track, viewport, content, position, cx| {
                if axis == ScrollbarAxis::Vertical {
                    if let Some(list) = this.list_state() {
                        list.scrollbar_drag_started();
                    }
                }
                this.scroll.drag = Some((axis, track, viewport, content));
                this.drag_to(position);
                cx.notify(id);
            },
        )
    }

    /// Overlays the rail on the right edge.
    pub(super) fn with_scrollbar(&self, content: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let Some(list) = self.list_state() else { return content };
        let bounds = list.viewport_bounds();
        let viewport = bounds.size.height.as_f32();
        let maximum = self.list_scroll_maximum();
        let scrolled = -list.scroll_px_offset_for_scrollbar().y.as_f32();
        let vertical = self.rail(ScrollbarAxis::Vertical, viewport, viewport + maximum, scrolled, cx);
        let handle = cx.entity();
        let movement = handle.clone();
        let wheel = handle.clone();
        let return_top = handle.clone();
        let hover = handle.clone();
        let scroll_zone = crate::ui::components::scrollbar::scroll_top::hover_zone(
            div().absolute().top_0().bottom_0().right_0().w(rems(3.625)),
            "viewer-scroll-zone".into(), move |hovered, _, cx| {
                hover.update(cx, |this, cx| {
                    if this.scroll.hovered != *hovered {
                        this.scroll.hovered = *hovered;
                        cx.notify();
                    }
                });
            },
        )
            .child(vertical)
            .child(crate::ui::components::scrollbar::scroll_top::button(
                *cx.theme(), format!("viewer-{:?}", handle.entity_id()), self.scroll.hovered,
                &self.scroll.top_anchor, scrolled, viewport, viewport + maximum,
                move |_, _, cx| {
                    return_top.update(cx, |this, cx| {
                        this.end_drag();
                        if let Some(list) = this.list_state() {
                            list.set_offset_from_scrollbar(point(px(0.0), px(0.0)));
                        }
                        cx.notify();
                    });
                },
            ));
        div()
            .id("viewer-scroll")
            .size_full()
            .relative()
            // The rails are drawn by this view; repaint them as the list scrolls.
            .on_scroll_wheel(move |_, _, cx| wheel.update(cx, |_, cx| cx.notify()))
            .child(div().size_full().pr(ZED_TRACK_WIDTH).child(content))
            .child(scroll_zone)
            .child(
                canvas(|_, _, _| (), move |_, _, window, _| {
                    let moving = movement.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase != DispatchPhase::Capture || moving.read(cx).scroll.drag.is_none() {
                            return;
                        }
                        let position = crate::ui::components::popup::owner_point(window, cx, event.position);
                        moving.update(cx, |this, cx| {
                            if event.dragging() {
                                this.drag_to(position);
                            } else {
                                this.end_drag();
                            }
                            cx.notify();
                        });
                    });
                    let release = movement.clone();
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                            release.update(cx, |this, cx| {
                                if this.end_drag() {
                                    cx.notify();
                                }
                            });
                        }
                    });
                })
                .absolute()
                .size_0(),
            )
            .into_any_element()
    }
}

#[cfg(all(test, target_os = "windows"))]
#[path = "scrollbar/tests/native_scroll_top.rs"]
mod native_tests;
