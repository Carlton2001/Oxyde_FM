use gpui::*;
use crate::ui::style::h_flex;

pub struct TabBar {
    id: ElementId,
    start_children: Vec<AnyElement>,
    children: Vec<AnyElement>,
    end_children: Vec<AnyElement>,
    scroll_handle: ScrollHandle,
}

impl TabBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            start_children: Vec::new(),
            children: Vec::new(),
            end_children: Vec::new(),
            scroll_handle: ScrollHandle::new(),
        }
    }

    #[allow(dead_code)]
    pub fn start_child(mut self, child: impl IntoElement) -> Self {
        self.start_children.push(child.into_any_element());
        self
    }

    #[allow(dead_code)]
    pub fn end_child(mut self, child: impl IntoElement) -> Self {
        self.end_children.push(child.into_any_element());
        self
    }
}

impl ParentElement for TabBar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl IntoElement for TabBar {
    type Element = Component<Self>;

    fn into_element(self) -> Self::Element {
        Component::new(self)
    }
}

impl RenderOnce for TabBar {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .w_full()
            .h(px(32.0))
            .bg(rgb(0x1f2023))
            .border_b_1()
            .border_color(rgb(0x161719))
            .child(
                h_flex()
                    .flex_none()
                    .children(self.start_children)
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .h_full()
                    .child(
                        h_flex()
                            .id("tabs-scroll")
                            .flex_grow()
                            .overflow_x_scroll()
                            .track_scroll(&self.scroll_handle)
                            .children(self.children)
                    )
            )
            .child(
                h_flex()
                    .flex_none()
                    .children(self.end_children)
            )
    }
}
