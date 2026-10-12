//! Position only the metadata block, as for a tooltip without a preview.
//! The preview is painted above that block without participating in placement.

use super::PreviewTask;
use crate::ui::scale::rem_pixels;
use crate::ui::theme::OxydeTheme;
use gpui::*;

pub(super) struct PreviewTooltip {
    metadata: AnyElement,
    preview: Entity<PreviewTask>,
}

pub(super) struct Layout {
    metadata_size: Size<Pixels>,
    preview_size: Size<Pixels>,
    preview: AnyElement,
    frame: Option<AnyElement>,
    separator: Option<AnyElement>,
}

impl PreviewTooltip {
    pub(super) fn new(metadata: AnyElement, preview: Entity<PreviewTask>) -> Self {
        Self { metadata, preview }
    }
}

impl IntoElement for PreviewTooltip {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

fn preview_frame_origin(metadata_origin: Point<Pixels>, extra: Pixels) -> Point<Pixels> {
    metadata_origin - point(px(0.0), extra)
}

impl Element for PreviewTooltip {
    type RequestLayoutState = Layout;
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
    ) -> (LayoutId, Layout) {
        let metadata_size = self
            .metadata
            .layout_as_root(AvailableSpace::min_size(), window, cx);
        let origin = super::position::origin(window, metadata_size);
        let limit = size(
            (metadata_size.width - rem_pixels(rems(1.5))).max(px(1.0)),
            (origin.y - rem_pixels(rems(0.75)) - px(1.0))
                .max(px(0.0))
                .min(rem_pixels(rems(26.0))),
        );
        self.preview.update(cx, |preview, cx| {
            if preview.maximum_size != Some(limit) {
                preview.maximum_size = Some(limit);
                cx.notify();
            }
        });
        let mut preview = div()
            .w(limit.width)
            .max_h(limit.height)
            .overflow_hidden()
            .font_family("Inter")
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .child(self.preview.clone())
            .into_any_element();
        let preview_size = preview.layout_as_root(AvailableSpace::min_size(), window, cx);
        let mut style = Style::default();
        style.size = size(metadata_size.width.into(), metadata_size.height.into());
        (
            window.request_layout(style, [], cx),
            Layout {
                metadata_size,
                preview_size,
                preview,
                frame: None,
                separator: None,
            },
        )
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        layout: &mut Layout,
        window: &mut Window,
        cx: &mut App,
    ) {
        let theme = *OxydeTheme::global(cx);
        let has_preview = layout.preview_size.height > px(0.0);
        let extra = if has_preview {
            layout.preview_size.height + rem_pixels(rems(0.75))
        } else {
            px(0.0)
        };
        let frame_height = layout.metadata_size.height + extra;
        let origin = super::position::origin(window, layout.metadata_size);
        let mut frame = div()
            .w(layout.metadata_size.width)
            .h(frame_height)
            .bg(theme.surface)
            .border(rems(0.0625))
            .border_color(theme.border)
            .rounded_md()
            .shadow(crate::ui::style::rem_shadow(true))
            .into_any_element();
        frame.layout_as_root(AvailableSpace::min_size(), window, cx);
        let frame_origin = preview_frame_origin(origin, extra);
        frame.prepaint_at(frame_origin, window, cx);
        layout.frame = Some(frame);
        self.metadata.prepaint_at(origin, window, cx);
        if has_preview {
            let inset = rem_pixels(rems(0.75));
            let preview_y = rem_pixels(rems(0.5));
            layout
                .preview
                .prepaint_at(frame_origin + point(inset, preview_y), window, cx);
            let separator_y = extra;
            let mut separator = div()
                .w(layout.preview_size.width)
                .h(rems(0.0625))
                .bg(theme.border)
                .into_any_element();
            separator.layout_as_root(AvailableSpace::min_size(), window, cx);
            separator.prepaint_at(frame_origin + point(inset, separator_y), window, cx);
            layout.separator = Some(separator);
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        layout: &mut Layout,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(frame) = &mut layout.frame {
            frame.paint(window, cx);
        }
        self.metadata.paint(window, cx);
        if layout.preview_size.height > px(0.0) {
            layout.preview.paint(window, cx);
        }
        if let Some(separator) = &mut layout.separator {
            separator.paint(window, cx);
        }
        super::position::follow_pointer(window);
    }
}

#[cfg(test)]
#[path = "tests/placement_tests.rs"]
mod tests;
