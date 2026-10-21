//! Keep GPUI's hover detection and delay, but present the result in its owned surface.
use gpui::*;

pub(crate) trait NativeTooltipExt: StatefulInteractiveElement + Sized {
    fn native_tooltip(self, builder: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self {
        #[cfg(target_os = "windows")]
        {
            self.tooltip(move |window, cx| {
                let view = builder(window, cx);
                super::hover::native_window::proxy(view, Window::window_handle(window), cx)
            })
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.tooltip(builder)
        }
    }

    /// Place the tooltip below this element; it no longer follows the pointer.
    fn fixed_tooltip(self, builder: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self
    where
        Self: ParentElement,
    {
        let bounds = std::rc::Rc::new(std::cell::Cell::new(Bounds::<Pixels>::default()));
        let measured = bounds.clone();
        self.native_tooltip(super::Tooltip::anchored(builder, bounds))
            .child(canvas(move |rect, _, _| measured.set(rect), |_, _, _, _| {}).absolute().left_0().top_0().size_full())
    }
}
impl<T: StatefulInteractiveElement> NativeTooltipExt for T {}
