use std::cmp::Ordering;
use gpui::*;
use gpui::prelude::*;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TabPosition {
    First,
    Middle(Ordering),
    Last,
}

pub struct TabDragPreview {
    pub label: String,
}

impl Render for TabDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_4()
            .h(px(32.0))
            .bg(rgb(0x38393d))
            .rounded_sm()
            .border_1()
            .border_color(rgb(0x4a4b4f))
            .text_color(rgb(0xcccccc))
            .text_sm()
            .opacity(0.95)
            .shadow_lg()
            .flex()
            .items_center()
            .gap_2()
            .child(svg().path("icons/folder.svg").size_3_5().text_color(rgb(0x0078d4)))
            .child(self.label.clone())
    }
}

pub struct Tab {
    id: ElementId,
    selected: bool,
    position: TabPosition,
    label: SharedString,
    icon: Option<String>,
    on_click: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_middle_click: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_close: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    drag_refinement: Option<Box<dyn FnOnce(Stateful<Div>) -> Stateful<Div> + 'static>>,
    is_drop_target: bool,
    is_dragging: bool,
}

impl Tab {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            selected: false,
            position: TabPosition::First,
            label: label.into(),
            icon: None,
            on_click: None,
            on_middle_click: None,
            on_close: None,
            drag_refinement: None,
            is_drop_target: false,
            is_dragging: false,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn position(mut self, position: TabPosition) -> Self {
        self.position = position;
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    pub fn on_middle_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_middle_click = Some(Box::new(f));
        self
    }

    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Box::new(f));
        self
    }

    pub fn on_drag<T: Clone + 'static>(
        mut self,
        data: T,
        f: impl Fn(&mut Window, &mut App) -> Entity<TabDragPreview> + 'static
    ) -> Self {
        self.drag_refinement = Some(Box::new(move |el: Stateful<Div>| {
            el.on_drag(data, move |_, _, window, cx| f(window, cx))
        }));
        self
    }

    pub fn is_dragging(mut self, dragging: bool) -> Self {
        self.is_dragging = dragging;
        self
    }
}

impl IntoElement for Tab {
    type Element = Component<Self>;

    fn into_element(self) -> Self::Element {
        Component::new(self)
    }
}

impl RenderOnce for Tab {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let (text_color, bg_color) = if self.selected {
            (rgb(0xcccccc), rgb(0x38393d))
        } else {
            (rgb(0x999999), rgb(0x1f2023))
        };

        let mut container = div()
            .id(self.id.clone())
            .relative()
            .h(px(32.0))
            .bg(bg_color)
            .flex()
            .items_center()
            .text_sm()
            .text_color(text_color)
            .group("tab");
            
        if let Some(on_click) = self.on_click {
            container = container.on_mouse_down(MouseButton::Left, move |_, window, cx| on_click(window, cx));
        }

        if let Some(on_middle_click) = self.on_middle_click {
            container = container.on_mouse_down(MouseButton::Middle, move |_, window, cx| on_middle_click(window, cx));
        }

        if let Some(refinement) = self.drag_refinement {
            container = refinement(container);
        }
        
        container = container.when(self.is_dragging, |s| s.opacity(0.1));
        
        container = match self.position {
            TabPosition::First => {
                if self.selected {
                    container.border_r_1().border_color(rgb(0x161719))
                } else {
                    container.border_r_1().border_b_1().border_color(rgb(0x161719))
                }
            }
            TabPosition::Last => {
                if self.selected {
                    container
                } else {
                    container.border_b_1().border_color(rgb(0x161719))
                }
            }
            TabPosition::Middle(Ordering::Greater) | TabPosition::Middle(Ordering::Less) | TabPosition::Middle(Ordering::Equal) => {
                container.border_r_1().border_b_1().border_color(rgb(0x161719))
            }
        };

        // Active tab top indicator (Zed-like)
        if self.selected {
             container = container.child(
                 div()
                     .absolute()
                     .top_0()
                     .w_full()
                     .h(px(2.0))
                     .bg(rgb(0x0078d4))
             );
        }

        container = container.child(
            div()
                .absolute()
                .left_0()
                .w(px(2.0))
                .h_full()
                .when(self.is_drop_target, |s| s.bg(rgb(0x0078d4)))
        );

        container
            .child(
                div()
                    .flex()
                    .items_center()
                    .pl_3()
                    .pr(if self.on_close.is_some() { px(12.0) } else { px(20.0) })
                    .gap_2()
                    .when_some(self.icon, |s, icon| {
                        s.child(svg().path(icon).size_3_5().text_color(if self.selected { rgb(0x0078d4) } else { rgb(0x666666) }))
                    })
                    .child(div().text_ellipsis().child(self.label.clone()))
                    .when_some(self.on_close, |s, on_close| {
                        s.child(
                            div()
                                .p_0p5()
                                .rounded_sm()
                                .hover(|st: StyleRefinement| st.bg(rgb(0x333333)))
                                .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                                    cx.stop_propagation();
                                    (on_close)(_window, cx);
                                })
                                .child(
                                    svg()
                                        .path("icons/x.svg")
                                        .size_3()
                                        .text_color(if self.selected { rgb(0x888888) } else { rgb(0x666666) })
                                        .group_hover("tab", |st| st.text_color(rgb(0xcccccc)))
                                )
                        )
                    })
            )
    }
}
