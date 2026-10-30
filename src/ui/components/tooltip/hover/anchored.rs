//! Where a passive tooltip surface goes on a compositor that owns window placement: GPUI
//! anchors it to its parent, and the compositor keeps it on screen. X11 refuses such
//! surfaces, and the tooltip then stays inside the window.
use gpui::*;
use std::cell::Cell;

/// Between the pointer and the visible frame.
const POINTER_GAP: Pixels = px(12.0);
/// Between the source element and the visible frame.
const ELEMENT_GAP: Pixels = px(4.0);

pub(super) struct Placement {
    pub(super) fixed: bool,
    pub(super) pointer: Cell<Point<Pixels>>,
    placed: Cell<Option<(Bounds<Pixels>, Size<Pixels>)>>,
}

impl Placement {
    pub(super) fn new(fixed: bool, pointer: Point<Pixels>) -> Self {
        Self { fixed, pointer: Cell::new(pointer), placed: Cell::new(None) }
    }

    /// Moves or resizes the surface, only when something changed: each request costs a
    /// round trip to the compositor.
    pub(super) fn apply(&self, fixed: Option<Bounds<Pixels>>, measured: Size<Pixels>, window: &mut Window) {
        let measured = size(measured.width.ceil(), measured.height.ceil());
        let rect = rect(fixed, self.pointer.get());
        if self.placed.get() != Some((rect, measured)) && window.reposition_popup(rect, measured) {
            self.placed.set(Some((rect, measured)));
        }
    }
}

/// Only Wayland has such surfaces. GPUI chooses its backend from the same variable.
pub(super) fn available() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty())
}

fn rect(fixed: Option<Bounds<Pixels>>, pointer: Point<Pixels>) -> Bounds<Pixels> {
    fixed.unwrap_or_else(|| Bounds::new(pointer, size(px(1.0), px(1.0))))
}

/// The surface includes transparent shadow margins of `inset`, which the offset cancels so
/// that the visible frame keeps its gap. They also keep the surface overlapping its parent
/// at the window's edges, without which Mutter dismisses it.
pub(super) fn options(parent: AnyWindowHandle, fixed: Option<Bounds<Pixels>>, pointer: Point<Pixels>, inset: Pixels) -> popup::PopupOptions {
    use popup::{PopupAnchor, PopupConstraintAdjustment as Adjust, PopupGravity};
    let (anchor, offset, adjustment) = if fixed.is_some() {
        (PopupAnchor::BottomLeft, point(-inset, ELEMENT_GAP - inset), Adjust::SLIDE_X | Adjust::FLIP_Y | Adjust::SLIDE_Y)
    } else {
        let offset = POINTER_GAP - inset;
        (PopupAnchor::BottomRight, point(offset, offset), Adjust::FLIP_X | Adjust::FLIP_Y | Adjust::SLIDE_X | Adjust::SLIDE_Y)
    };
    popup::PopupOptions {
        parent,
        anchor_rect: rect(fixed, pointer),
        anchor,
        gravity: PopupGravity::BottomRight,
        constraint_adjustment: adjustment,
        offset,
        grab: false,
    }
}

#[cfg(test)]
#[path = "tests/anchored.rs"]
mod tests;
