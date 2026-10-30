//! Where a passive tooltip surface goes on a compositor that owns window placement: GPUI
//! anchors it to its parent, and the compositor keeps it on screen. X11 refuses such
//! surfaces, and the tooltip then stays inside the window.
use crate::ui::components::popup::{flip_axis, positioner, slide_axis};
use gpui::*;
use std::cell::Cell;

/// Between the pointer and the visible frame.
const POINTER_GAP: Pixels = px(12.0);
/// Between the source element and the visible frame.
const ELEMENT_GAP: Pixels = px(4.0);

pub(super) struct Placement {
    pub(super) shape: Option<bool>,
    inset: Pixels,
    pub(super) pointer: Cell<Point<Pixels>>,
    placed: Cell<Option<(Bounds<Pixels>, Size<Pixels>)>>,
}

impl Placement {
    pub(super) fn new(shape: Option<bool>, pointer: Point<Pixels>, inset: Pixels) -> Self {
        Self { shape, inset, pointer: Cell::new(pointer), placed: Cell::new(None) }
    }

    /// Moves or resizes the surface, only when something changed: each request costs a
    /// round trip to the compositor.
    pub(super) fn apply(&self, fixed: Option<Bounds<Pixels>>, measured: Size<Pixels>, window: &mut Window) {
        let measured = size(measured.width.ceil(), measured.height.ceil());
        let rect = placement(fixed, self.pointer.get(), self.inset).0;
        if self.placed.get() != Some((rect, measured)) && window.reposition_popup(rect, measured) {
            self.placed.set(Some((rect, measured)));
        }
    }
}

pub(super) fn available() -> bool {
    crate::ui::components::popup::available()
}

/// What a surface cannot change once open: following the pointer, or the form of its element.
pub(super) fn shape(fixed: Option<Bounds<Pixels>>, inset: Pixels) -> Option<bool> {
    fixed.map(|bounds| flip_axis(bounds.top(), bounds.bottom(), ELEMENT_GAP, inset, true).at_end)
}

/// The surface includes transparent shadow margins of `inset`, which the placement cancels so
/// that the visible frame keeps its gap. They also keep the surface overlapping its parent
/// at the window's edges, without which Mutter dismisses it.
fn placement(fixed: Option<Bounds<Pixels>>, pointer: Point<Pixels>, inset: Pixels) -> (Bounds<Pixels>, popup::PopupAnchor, popup::PopupGravity, Point<Pixels>) {
    match fixed {
        Some(bounds) => positioner(
            slide_axis(bounds.left(), bounds.right(), inset, true),
            flip_axis(bounds.top(), bounds.bottom(), ELEMENT_GAP, inset, true),
        ),
        None => positioner(
            flip_axis(pointer.x, pointer.x, POINTER_GAP, inset, true),
            flip_axis(pointer.y, pointer.y, POINTER_GAP, inset, true),
        ),
    }
}

pub(super) fn options(parent: AnyWindowHandle, fixed: Option<Bounds<Pixels>>, pointer: Point<Pixels>, inset: Pixels) -> popup::PopupOptions {
    use popup::PopupConstraintAdjustment as Adjust;
    let (anchor_rect, anchor, gravity, offset) = placement(fixed, pointer, inset);
    let constraint_adjustment = if fixed.is_some() {
        Adjust::SLIDE_X | Adjust::FLIP_Y | Adjust::SLIDE_Y
    } else {
        Adjust::FLIP_X | Adjust::FLIP_Y | Adjust::SLIDE_X | Adjust::SLIDE_Y
    };
    popup::PopupOptions { parent, anchor_rect, anchor, gravity, constraint_adjustment, offset, grab: false }
}

#[cfg(test)]
#[path = "tests/anchored.rs"]
mod tests;
