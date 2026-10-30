//! Size the surface around its real content and publish row geometry after paint.
use super::{position, MenuWindow, Surfaces};
use gpui::*;

pub(super) struct FramedContent {
    pub(super) content: AnyElement,
    pub(super) root: Entity<MenuWindow>,
    pub(super) inset: Pixels,
}
impl IntoElement for FramedContent {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for FramedContent {
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
        let content = self
            .content
            .layout_as_root(AvailableSpace::min_size(), window, cx);
        let measured = size(
            content.width + self.inset * 2.0,
            content.height + self.inset * 2.0,
        );
        let style = Style {
            size: size(measured.width.into(), measured.height.into()),
            ..Default::default()
        };
        (window.request_layout(style, [], cx), measured)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        measured: &mut Size<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content
            .prepaint_at(point(self.inset, self.inset), window, cx);
        // Before this frame is presented, which would otherwise cut the content. Only the visible
        // frame takes the pointer, not the shadow margins over the window beneath.
        #[cfg(not(target_os = "windows"))]
        if self.root.read(cx).measured != Some(*measured) {
            self.root.update(cx, |root, _| root.measured = Some(*measured));
            let inner = Bounds::new(point(self.inset, self.inset), size(measured.width - self.inset * 2.0, measured.height - self.inset * 2.0));
            window.set_input_region(Some(&[inner]));
            let root = self.root.clone();
            root.update(cx, |root, cx| position(root, window, cx));
        }
        #[cfg(target_os = "windows")]
        if self.root.read(cx).measured != Some(*measured) {
            let root = self.root.clone();
            let measured = *measured;
            let handle = Window::window_handle(window);
            cx.defer(move |cx| {
                root.update(cx, |root, _| root.measured = Some(measured));
                let _ = handle.update(cx, |_, window, cx| root.update(cx, |root, cx| position(root, window, cx)));
            });
        }
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
        let id = Window::window_handle(window).window_id();
        let state = cx.default_global::<Surfaces>();
        let rows = state.rows.get(&id).cloned().unwrap_or_default();
        if state.previous_rows.get(&id) != Some(&rows) {
            state.previous_rows.insert(id, rows);
            let parent = self.root.read(cx).spec.parent;
            cx.defer(move |cx| {
                let _ = parent.update(cx, |_, window, _| window.refresh());
            });
        }
    }
}
