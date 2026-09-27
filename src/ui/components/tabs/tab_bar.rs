use crate::ui::style::h_flex;
use gpui::prelude::*;
use gpui::*;

pub struct TabBar {
    id: ElementId,
    start_children: Vec<AnyElement>,
    children: Vec<AnyElement>,
    end_children: Vec<AnyElement>,
    scroll_handle: ScrollHandle,
    on_new_tab: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    drop_target_indicator: bool,
}

impl TabBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            start_children: Vec::new(),
            children: Vec::new(),
            end_children: Vec::new(),
            scroll_handle: ScrollHandle::new(),
            on_new_tab: None,
            drop_target_indicator: false,
        }
    }

    pub fn on_new_tab(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_new_tab = Some(Box::new(f));
        self
    }

    pub fn drop_target_indicator(mut self, show: bool) -> Self {
        self.drop_target_indicator = show;
        self
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
    type Element = ViewElement<Self>;

    fn into_element(self) -> Self::Element {
        ViewElement::new(self)
    }
}

impl RenderOnce for TabBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        use crate::ui::theme::ThemeContext;
        let theme = cx.theme();

        div()
            .id(self.id.clone())
            .flex()
            .flex_none()
            .w_full()
            .h(px(32.0))
            .bg(theme.header)
            .border_b_1()
            .border_color(theme.border)
            .child(h_flex().flex_none().children(self.start_children))
            .child(
                div().relative().flex_1().h_full().child(
                    h_flex()
                        .id("tabs-scroll")
                        .flex_grow(1.0)
                        .overflow_x_scroll()
                        .track_scroll(&self.scroll_handle)
                        .children(self.children)
                        // Small gap for drop target at the end of tabs list
                        .child(
                            div()
                                .w(px(4.0)) // Reduced from 8px
                                .h_full()
                                .relative()
                                .child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .w(px(2.0))
                                        .h_full()
                                        .bg(theme.accent)
                                        .when(!self.drop_target_indicator, |s| s.opacity(0.0)),
                                ),
                        )
                        // New Tab Button
                        .when_some(self.on_new_tab, |this, on_new_tab| {
                            this.child(
                                div()
                                    .id(SharedString::from(format!("new-tab-btn-{:?}", self.id)))
                                    .ml_0p5() // Minimal left margin
                                    .w(px(24.0))
                                    .h(px(24.0))
                                    .my_auto()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .hover(|s| s.bg(rgba(0xffffff10)))
                                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                        on_new_tab(window, cx);
                                    })
                                    .child(
                                        svg()
                                            .path("icons/plus.svg")
                                            .size_4()
                                            .text_color(theme.text_muted),
                                    ),
                            )
                        })
                        // Final filler gap
                        .child(div().flex_1()),
                ),
            )
            .child(h_flex().flex_none().children(self.end_children))
    }
}
