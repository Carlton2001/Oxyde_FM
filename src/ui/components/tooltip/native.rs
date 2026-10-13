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
}
impl<T: StatefulInteractiveElement> NativeTooltipExt for T {}
