//! Fit the video and controls using this frame's bounds, without a second render.
use super::Viewer;
use crate::ui::{i18n::{I18nContext, Message}, theme::ThemeContext};
use gpui::{prelude::*, *};
use std::sync::atomic::Ordering;

pub(super) struct VideoLayout {
    viewer: WeakEntity<Viewer>,
    bar: AnyElement,
    ratio: f32,
    placeholder: Option<AnyElement>,
    fullscreen: bool,
}

impl Viewer {
    pub(super) fn render_video(&self, window: &Window, cx: &mut Context<Self>) -> VideoLayout {
        let dimensions = self
            .media
            .as_ref()
            .map(|media| media.control.video_dimensions.load(Ordering::Acquire))
            .unwrap_or_default();
        let (width, height) = ((dimensions >> 32) as u32, dimensions as u32);
        VideoLayout {
            fullscreen: window.is_fullscreen(),
            viewer: cx.entity().downgrade(),
            bar: self.render_media_bar(cx).into_any_element(),
            ratio: if width > 0 && height > 0 {
                width as f32 / height as f32
            } else {
                16.0 / 9.0
            },
            placeholder: self.blocked.then(|| div()
                .size_full()
                .bg(rgb(0x000000))
                .border_1()
                .border_color(cx.theme().border)
                .flex().items_center().justify_center()
                .p(rems(1.0))
                .text_color(rgb(0xffffff))
                .text_center()
                .child(cx.i18n().text(Message::ViewerVisualPaused))
                .into_any_element()),
        }
    }
}

impl IntoElement for VideoLayout {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for VideoLayout {
    type RequestLayoutState = ();
    type PrepaintState = Bounds<Pixels>;

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
    ) -> (LayoutId, ()) {
        let style = Style {
            size: size(relative(1.0).into(), relative(1.0).into()),
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Bounds<Pixels> {
        let bar_height = if self.fullscreen { px(0.0) } else { crate::ui::scale::rem_pixels(rems(2.5625)) };
        let video = fitted_video(bounds, self.ratio, bar_height);
        let Bounds { origin, size: Size { width, height } } = video;
        if !self.fullscreen { self.bar.layout_as_root(
            size(
                AvailableSpace::Definite(width),
                AvailableSpace::Definite(bar_height),
            ),
            window,
            cx,
        );
        self.bar
            .prepaint_at(origin + point(px(0.0), height), window, cx); }
        if let Some(placeholder) = &mut self.placeholder {
            placeholder.layout_as_root(size(AvailableSpace::Definite(width), AvailableSpace::Definite(height)), window, cx);
            placeholder.prepaint_at(origin, window, cx);
        }
        video
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        video: &mut Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let _ = self
            .viewer
            .update(cx, |this, cx| {
                this.place_media(*video, window, cx);
                if self.fullscreen { this.place_fullscreen_controls(bounds, window, cx); }
            });
        if let Some(placeholder) = &mut self.placeholder { placeholder.paint(window, cx); }
        if !self.fullscreen { self.bar.paint(window, cx); }
    }
}

fn fitted_video(bounds: Bounds<Pixels>, ratio: f32, reserved_height: Pixels) -> Bounds<Pixels> {
    let available_height = (bounds.size.height - reserved_height).max(px(0.0));
    let width = bounds.size.width.min(available_height * ratio).max(px(0.0));
    let height = width / ratio;
    Bounds::new(bounds.origin + point(
        (bounds.size.width - width) / 2.0,
        (available_height - height) / 2.0,
    ), size(width, height))
}

#[cfg(test)]
#[path = "tests/video_layout.rs"]
mod tests;
