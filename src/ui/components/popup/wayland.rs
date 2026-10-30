//! Where a menu goes on a compositor that owns window placement: the same rules as the Windows
//! placement, as a positioner. X11 refuses such surfaces, and menus stay inside the window.
use super::PopupAnchor;
use gpui::popup::{PopupAnchor as Corner, PopupConstraintAdjustment as Adjust, PopupGravity as Gravity, PopupOptions};
use gpui::*;

/// Between a source element and the visible frame.
const ELEMENT_GAP: Pixels = px(4.0);

/// Only Wayland has such surfaces. GPUI chooses its backend from the same variable.
pub(crate) fn available() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty())
}

/// One axis of a positioner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Axis {
    pub(crate) start: Pixels,
    pub(crate) end: Pixels,
    pub(crate) at_end: bool,
    pub(crate) forward: bool,
    pub(crate) offset: Pixels,
}

/// An axis the compositor may flip, for a visible frame `gap` away from `start..end`, whichever
/// side it opens on. The surface includes margins of `inset`. Mutter does not mirror an offset
/// when flipping, unlike the anchor and gravity, so the two sides come from the rectangle itself.
pub(crate) fn flip_axis(start: Pixels, end: Pixels, gap: Pixels, inset: Pixels, forward: bool) -> Axis {
    let near = inset - gap;
    if end - start < near * 2.0 {
        return Axis { start: end - near, end: start + near, at_end: !forward, forward, offset: px(0.0) };
    }
    if forward {
        Axis { start: start + near * 2.0, end, at_end: true, forward, offset: -near }
    } else {
        Axis { start, end: end - near * 2.0, at_end: false, forward, offset: near }
    }
}

/// An axis the compositor only slides, whose frame starts at the element's edge.
pub(crate) fn slide_axis(start: Pixels, end: Pixels, inset: Pixels, forward: bool) -> Axis {
    Axis { start, end, at_end: !forward, forward, offset: if forward { -inset } else { inset } }
}

pub(crate) fn positioner(x: Axis, y: Axis) -> (Bounds<Pixels>, Corner, Gravity, Point<Pixels>) {
    let corner = match (x.at_end, y.at_end) {
        (false, false) => Corner::TopLeft,
        (true, false) => Corner::TopRight,
        (false, true) => Corner::BottomLeft,
        (true, true) => Corner::BottomRight,
    };
    let gravity = match (x.forward, y.forward) {
        (false, false) => Gravity::TopLeft,
        (true, false) => Gravity::TopRight,
        (false, true) => Gravity::BottomLeft,
        (true, true) => Gravity::BottomRight,
    };
    let rect = Bounds::from_corners(point(x.start, y.start), point(x.end, y.end));
    (rect, corner, gravity, point(x.offset, y.offset))
}

fn axes(anchor: &PopupAnchor, bounds: Bounds<Pixels>, inset: Pixels) -> (Axis, Axis) {
    if anchor.beside {
        (flip_axis(bounds.left(), bounds.right(), px(0.0), inset, !anchor.open_left), slide_axis(bounds.top(), bounds.bottom(), inset, true))
    } else {
        let gap = if bounds.size.height > px(0.0) { ELEMENT_GAP } else { px(0.0) };
        (slide_axis(bounds.left(), bounds.right(), inset, !anchor.open_left), flip_axis(bounds.top(), bounds.bottom(), gap, inset, !anchor.above))
    }
}

/// A surface cannot change its anchor, gravity or offset once open.
pub(super) fn same_placement(left: &PopupAnchor, right: &PopupAnchor, inset: Pixels) -> bool {
    let shape = |anchor: &PopupAnchor| {
        let (x, y) = axes(anchor, anchor.bounds, inset);
        (x.at_end, x.forward, x.offset, y.at_end, y.forward, y.offset)
    };
    shape(left) == shape(right)
}

/// The rectangle of `anchor`, whose element is at `bounds` in its parent's coordinates.
pub(super) fn rect(anchor: &PopupAnchor, bounds: Bounds<Pixels>, inset: Pixels) -> Bounds<Pixels> {
    let (x, y) = axes(anchor, bounds, inset);
    positioner(x, y).0
}

pub(super) fn options(parent: AnyWindowHandle, anchor: &PopupAnchor, bounds: Bounds<Pixels>, inset: Pixels, grab: bool) -> PopupOptions {
    let (x, y) = axes(anchor, bounds, inset);
    let (anchor_rect, corner, gravity, offset) = positioner(x, y);
    let constraint_adjustment = if anchor.beside { Adjust::FLIP_X | Adjust::SLIDE_Y } else { Adjust::SLIDE_X | Adjust::FLIP_Y | Adjust::SLIDE_Y };
    PopupOptions { parent, anchor_rect, anchor: corner, gravity, constraint_adjustment, offset, grab }
}

#[cfg(test)]
#[path = "tests/wayland.rs"]
mod tests;
