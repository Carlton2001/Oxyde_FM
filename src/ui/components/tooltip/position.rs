//! Cursor-relative positioning shared by ordinary tooltips and file metadata.
use crate::ui::scale::rem_pixels;
use gpui::*;

const CURSOR_OFFSET: Rems = rems(0.75); // 12 logical pixels at normal scale.
const EDGE_PADDING: Rems = rems(3.125); // Tauri: 50 logical pixels.

pub(super) fn cursor_origin(
    cursor: Point<Pixels>,
    content: Size<Pixels>,
    viewport: Size<Pixels>,
    offset: Pixels,
    edge: Pixels,
) -> Point<Pixels> {
    let axis = |cursor: Pixels, content: Pixels, viewport: Pixels| {
        let edge = if content + edge * 2.0 <= viewport {
            edge
        } else {
            px(0.0)
        };
        let desired = cursor + offset;
        if desired + content > viewport - edge {
            (viewport - content - edge).max(edge)
        } else {
            desired
        }
    };
    point(
        axis(cursor.x, content.width, viewport.width),
        axis(cursor.y, content.height, viewport.height),
    )
}

pub(super) fn origin(window: &Window, content: Size<Pixels>) -> Point<Pixels> {
    cursor_origin(
        window.mouse_position(),
        content,
        window.viewport_size(),
        rem_pixels(CURSOR_OFFSET),
        rem_pixels(EDGE_PADDING),
    )
}

fn anchored_origin(
    bounds: Bounds<Pixels>,
    content: Size<Pixels>,
    viewport: Size<Pixels>,
    gap: Pixels,
) -> Point<Pixels> {
    cursor_origin(
        point(bounds.left(), bounds.bottom()),
        content,
        viewport,
        gap,
        px(0.0),
    )
}

pub(super) fn follow_pointer(window: &mut Window) {
    // GPUI registers event listeners only during Element::paint.
    window.on_mouse_event(|_: &MouseMoveEvent, phase, window, _| {
        if phase == DispatchPhase::Bubble {
            window.refresh();
        }
    });
}

pub(super) struct CursorTooltip {
    content: AnyElement,
    fixed_bounds: Option<std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>>,
}
impl CursorTooltip {
    pub(super) fn new(content: AnyElement) -> Self {
        Self {
            content,
            fixed_bounds: None,
        }
    }
    pub(super) fn anchored(
        content: AnyElement,
        bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
    ) -> Self {
        Self {
            content,
            fixed_bounds: Some(bounds),
        }
    }
}
impl IntoElement for CursorTooltip {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for CursorTooltip {
    type RequestLayoutState = Size<Pixels>;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Size<Pixels>) {
        let size = self
            .content
            .layout_as_root(AvailableSpace::min_size(), window, cx);
        let style = Style {
            size: gpui::size(size.width.into(), size.height.into()),
            ..Default::default()
        };
        (window.request_layout(style, [], cx), size)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        size: &mut Size<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if super::hover::native_window::is_surface(window, cx) {
            // The native frame already positions this element inside its
            // shadow inset. Preserve that origin for simple and fixed tips.
            self.content.prepaint_at(_bounds.origin, window, cx);
            return;
        }
        let origin = if let Some(bounds) = &self.fixed_bounds {
            let bounds = bounds.get();
            anchored_origin(
                bounds,
                *size,
                window.viewport_size(),
                rem_pixels(rems(0.25)),
            )
        } else {
            origin(window, *size)
        };
        self.content.prepaint_at(origin, window, cx);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Size<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.paint(window, cx);
        if super::hover::native_window::is_surface(window, cx) { return; }
        if self.fixed_bounds.is_none() {
            follow_pointer(window);
        }
    }
}

#[cfg(test)]
#[path = "tests/position_tests.rs"]
mod tests;
