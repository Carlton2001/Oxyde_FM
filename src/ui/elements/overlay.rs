//! A child painted above the elements that follow it, inside its parent's clip.
//! GPUI's `deferred` paints over the whole window: an overlay of a list item must
//! still stop at the list's edges, under the bars around it.
use gpui::{AnyElement, App, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Window};

/// Lays `child` out where it stands and paints it once the window's tree is painted,
/// clipped as the place it stands in is. The child takes no input of its own.
pub(crate) fn clipped_overlay(child: impl IntoElement) -> ClippedOverlay {
    ClippedOverlay { child: Some(child.into_any_element()) }
}

pub(crate) struct ClippedOverlay {
    child: Option<AnyElement>,
}

impl Element for ClippedOverlay {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        (self.child.as_mut().expect("overlay child").request_layout(window, cx), ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), window: &mut Window, _: &mut App) {
        let Some(child) = self.child.take() else { return };
        let offset = window.element_offset();
        let mask = window.content_mask();
        window.defer_draw(child, offset, 0, Some(mask));
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), _: &mut (), _: &mut Window, _: &mut App) {}
}

impl IntoElement for ClippedOverlay {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
