use crate::ui::scale::rem_pixels;
use crate::ui::theme::ThemeContext;
use gpui::prelude::*;
use gpui::*;

#[derive(Clone)]
pub enum TabIcon {
    Svg(&'static str),
    Item {
        path: String,
        icon: crate::ui::components::icons::IconType,
        system: bool,
    },
}

impl TabIcon {
    fn render(&self, color: Hsla) -> AnyElement {
        match self {
            Self::Svg(path) => svg()
                .path(*path)
                .size(rems(0.875))
                .text_color(color)
                .into_any_element(),
            Self::Item { path, icon, system } => crate::ui::components::icons::render_file_icon(
                path.clone(),
                *icon,
                rems(0.875),
                color,
                *system,
                false,
            ),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TabPosition {
    First,
    Middle,
    Last,
}

pub struct TabDragPreview {
    pub label: String,
    pub icon: TabIcon,
}

impl Render for TabDragPreview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        div()
            .px_4()
            .h(crate::ui::style::TAB_BAR_HEIGHT)
            .bg(theme.tab_active)
            .rounded_sm()
            .border(rems(0.0625))
            .border_color(theme.menu_border)
            .text_color(theme.text)
            .text_sm()
            .opacity(0.95)
            .shadow(crate::ui::style::rem_shadow(true))
            .flex()
            .items_center()
            .gap_2()
            .child(self.icon.render(theme.accent))
            .child(self.label.clone())
    }
}

pub struct Tab {
    id: ElementId,
    selected: bool,
    position: TabPosition,
    label: SharedString,
    icon: Option<TabIcon>,
    on_click: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_middle_click: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_right_click: Option<Box<dyn Fn(&mut Window, &mut App, Point<Pixels>) + 'static>>,
    on_bounds: Option<Box<dyn Fn(Bounds<Pixels>, &mut App) + 'static>>,
    on_close: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    drag_refinement: Option<Box<dyn FnOnce(Stateful<Div>) -> Stateful<Div> + 'static>>,
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
            on_right_click: None,
            on_bounds: None,
            on_close: None,
            drag_refinement: None,
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

    pub fn icon(mut self, icon: TabIcon) -> Self {
        self.icon = Some(icon);
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

    pub fn on_right_click(
        mut self,
        f: impl Fn(&mut Window, &mut App, Point<Pixels>) + 'static,
    ) -> Self {
        self.on_right_click = Some(Box::new(f));
        self
    }

    pub fn on_bounds(mut self, f: impl Fn(Bounds<Pixels>, &mut App) + 'static) -> Self {
        self.on_bounds = Some(Box::new(f));
        self
    }

    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Box::new(f));
        self
    }

    pub fn on_drag<T: Clone + 'static>(
        mut self,
        data: T,
        f: impl Fn(&mut Window, &mut App) -> Entity<TabDragPreview> + 'static,
    ) -> Self {
        self.drag_refinement = Some(Box::new(move |el: Stateful<Div>| {
            el.on_drag(data, move |_, _, window, cx| f(window, cx))
        }));
        self
    }

    pub fn with_dragging(mut self, dragging: bool) -> Self {
        self.is_dragging = dragging;
        self
    }
}

impl IntoElement for Tab {
    type Element = ViewElement<Self>;

    fn into_element(self) -> Self::Element {
        ViewElement::new(self)
    }
}

impl RenderOnce for Tab {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let (text_color, bg_color) = if self.selected {
            (theme.text, theme.tab_active)
        } else {
            (theme.text_muted, theme.header)
        };

        let mut container = div()
            .id(self.id.clone())
            .relative()
            .h(crate::ui::style::TAB_BAR_HEIGHT)
            .flex_none()
            .bg(bg_color)
            .flex()
            .items_center()
            .text_color(text_color)
            .text_size(rems(0.8125))
            .font_weight(crate::ui::style::DEFAULT_FONT_WEIGHT)
            .group("tab");

        if let Some(on_click) = self.on_click {
            container = container
                .on_mouse_down(MouseButton::Left, move |_, window, cx| on_click(window, cx));
        }

        if let Some(on_middle_click) = self.on_middle_click {
            container = container.on_mouse_down(MouseButton::Middle, move |_, window, cx| {
                on_middle_click(window, cx)
            });
        }

        if let Some(on_right_click) = self.on_right_click {
            container = container.on_mouse_down(MouseButton::Right, move |e, window, cx| {
                on_right_click(window, cx, e.position)
            });
        }

        if let Some(refinement) = self.drag_refinement {
            container = refinement(container);
        }

        container = container.when(self.is_dragging, |s| s.opacity(0.1));

        container = match self.position {
            TabPosition::First => {
                if self.selected {
                    container.border_r(rems(0.0625)).border_color(theme.border)
                } else {
                    container
                        .border_r(rems(0.0625))
                        .border_b(rems(0.0625))
                        .border_color(theme.border)
                }
            }
            TabPosition::Last => {
                if self.selected {
                    container
                } else {
                    container.border_b(rems(0.0625)).border_color(theme.border)
                }
            }
            TabPosition::Middle => container
                .border_r(rems(0.0625))
                .when(!self.selected, |s| s.border_b(rems(0.0625)))
                .border_color(theme.border),
        };

        container = container.child(
            div()
                .flex()
                .items_center()
                .pl_3()
                .pr(if self.on_close.is_some() {
                    rem_pixels(rems(0.75))
                } else {
                    rem_pixels(rems(1.25))
                })
                .gap_2()
                .when_some(self.icon, |s, icon| s.child(icon.render(text_color)))
                .child(div().text_ellipsis().child(self.label.clone()))
                .when_some(self.on_close, |s, on_close| {
                    s.child(
                        div()
                            .p_0p5()
                            .rounded_sm()
                            .hover(|st: StyleRefinement| st.bg(theme.tab_close_hover))
                            .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                                cx.stop_propagation();
                                (on_close)(_window, cx);
                            })
                            .child(
                                svg()
                                    .path("icons/x.svg")
                                    .size_3()
                                    .text_color(theme.icon_muted)
                                    .group_hover("tab", |st| st.text_color(theme.text)),
                            ),
                    )
                }),
        );

        if let Some(on_bounds) = self.on_bounds {
            container = container.child(
                canvas(
                    |bounds, _, _| bounds,
                    move |_, bounds, _, cx| {
                        on_bounds(bounds, cx);
                    },
                )
                .absolute()
                .size_full(),
            );
        }

        container
    }
}
